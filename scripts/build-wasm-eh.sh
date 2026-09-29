#!/usr/bin/env bash
# Build the `wasm_eh` DuckDB extension locally, mirroring CI.
#
# What CI does for wasm_eh (extension-ci-tools/makefiles/c_api_extensions):
#   1. cargo build --release --target wasm32-unknown-emscripten --features quack
#        -> target/wasm32-unknown-emscripten/release/libduckfn.a   (the staticlib)
#   2. emcc that staticlib into a *side module* exporting the entry symbol:
#        emcc libduckfn.a -o duckfn.no_metadata.wasm -O3 -sSIDE_MODULE=2 \
#             -sEXPORTED_FUNCTIONS="_duckfn_init_c_api"           (base.Makefile: link_wasm_release)
#   3. append the DuckDB extension metadata (name, target DuckDB version, platform,
#      C_STRUCT_UNSTABLE ABI) -> duckfn.duckdb_extension.wasm     (append_extension_metadata.py)
#
# We reuse `make configure` for step 0 (it writes configure/platform.txt and
# extension_version.txt) but run steps 1-3 here directly rather than through
# `make release`. Why: rust.Makefile picks the cargo artifact name from the *host*
# OS — on Windows it expects `$(EXTENSION_NAME).dll` (a native build), which is
# wrong for a wasm target whose artifact is `lib$(EXTENSION_NAME).a`. CI never
# hits that because it builds wasm in a Linux docker. So the makefiles' copy step
# looks for a file that does not exist; we do the same copy/link/metadata with the
# correct wasm names.
#
# Usage (run under Git Bash / a bash that has make + cargo + emsdk):
#   bash scripts/build-wasm-eh.sh [options]
# Options:
#   --emsdk <path>    emsdk root. Default: $EMSDK, else the local install.
#   --install-docs    Copy the produced wasm into docs/static/duckdb-extensions/
#                     so `npm test -w docs` / the site verify this local build.
#   --debug           Also emit a debug-profile build (build/wasm_eh/debug/...).
#   -h | --help       Show this help.
#
# Loadability caveat: DuckDB-Wasm loads an extension as an emscripten *side
# module*, and that only works when the emsdk/emscripten that produced the wasm
# matches the one the engine (the pinned @duckdb/duckdb-wasm) was built with. CI
# builds wasm in extension-ci-tools' pinned emscripten docker; if this machine's
# `--emsdk` points at a different emscripten, the artifact builds and carries
# correct metadata but fails to load (`Could not load dynamic lib: duckfn`). Use
# --install-docs only when your emsdk version matches CI's, otherwise the docs
# verifier will reject the local build — leave docs/static on the release asset.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXTENSION_NAME="duckfn"
# Must match the root Makefile's TARGET_DUCKDB_VERSION and CI's duckdb_version,
# and the duckdb-wasm the docs site is pinned to (ABI compatibility).
TARGET_DUCKDB_VERSION="v1.5.5"
USE_UNSTABLE_C_API=1                       # as in the root Makefile
EMSDK_DIR="${EMSDK:-/s/workspace/github/emscripten-core/emsdk}"
INSTALL_DOCS=0
BUILD_DEBUG=0

die() { echo "build-wasm-eh: $*" >&2; exit 1; }

usage() { sed -n '2,40p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

# --- arg parsing ------------------------------------------------------------
ARGS_EMSDK=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --emsdk)     ARGS_EMSDK="${2:-}"; shift 2 ;;
    --emsdk=*)   ARGS_EMSDK="${1#*=}"; shift ;;
    --install-docs) INSTALL_DOCS=1; shift ;;
    --debug)     BUILD_DEBUG=1; shift ;;
    -h|--help)   usage; exit 0 ;;
    *) die "unknown option: $1 (see --help)" ;;
  esac
done
[[ -n "$ARGS_EMSDK" ]] && EMSDK_DIR="$ARGS_EMSDK"

# Rewrite a Windows path (S:\a\b or S:/a/b) to the /s/a/b form Git Bash uses;
# already-unix paths pass through.
normalize_emsdk() {
  local p="$1"
  if [[ "$p" =~ ^([A-Za-z]):[/\\](.*)$ ]]; then
    local drive="${BASH_REMATCH[1]}" pth="${BASH_REMATCH[2]}"
    pth="${pth//\\//}"
    printf '/%s/%s' "${drive,,}" "$pth"
    return
  fi
  printf '%s' "$p"
}
EMSDK_DIR="$(normalize_emsdk "$EMSDK_DIR")"

# --- toolchain preconditions ------------------------------------------------
command -v make  >/dev/null 2>&1 || die "make not on PATH (run this in Git Bash)"
command -v cargo >/dev/null 2>&1 || die "cargo not on PATH"
[[ -d "$EMSDK_DIR" ]]            || die "emsdk not found at '$EMSDK_DIR' (--emsdk <path> or \$EMSDK)"
[[ -f "$EMSDK_DIR/emsdk_env.sh" ]] || die "no emsdk_env.sh under '$EMSDK_DIR'"
rustup target add wasm32-unknown-emscripten >/dev/null 2>&1 || true

cd "$REPO_ROOT"

echo "==> emsdk:        $EMSDK_DIR"
echo "==> repo root:    $REPO_ROOT"
echo "==> extension:    $EXTENSION_NAME  (wasm_eh, target $TARGET_DUCKDB_VERSION)"

# --- activate emscripten for this shell ------------------------------------
pushd "$EMSDK_DIR" >/dev/null
set +u; source ./emsdk_env.sh; set -u          # touches unset vars, relax `set -u`
popd >/dev/null
command -v emcc >/dev/null 2>&1 || die "emcc not on PATH after sourcing emsdk_env.sh"

# rustc defaults the wasm32-unknown-emscripten linker to `emcc.bat` on Windows,
# but recent emsdk ships `emcc`/`emcc.exe`/`emcc.py` and no `emcc.bat`, so linking
# fails with "'emcc.bat' is not recognized". Point rustc at the `emcc` that does
# resolve — via the process env only, so CI's .cargo config is untouched.
if ! command -v emcc.bat >/dev/null 2>&1; then
  export CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_LINKER="${CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_LINKER:-emcc}"
fi
echo "==> emcc:         $(command -v emcc)"
echo "==> rust linker:  ${CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_LINKER:-<default emcc.bat>}"

# --- the venv python the makefiles use for the metadata step ---------------
PYTHON_VENV_BIN="./configure/venv/Scripts/python.exe"
[[ -x "$PYTHON_VENV_BIN" ]] || PYTHON_VENV_BIN="./configure/venv/bin/python3"
[[ -x "$PYTHON_VENV_BIN" ]] || PYTHON_VENV_BIN="$(command -v python3 || command -v python)"

# --- step 0: configure (writes platform.txt / extension_version.txt) --------
# `make configure` is host-safe; only the wasm *build/copy* step has the name bug.
DUCKDB_PLATFORM=wasm_eh make configure

TARGET_DIR="target/wasm32-unknown-emscripten"
BUILD_DIR="build/wasm_eh/release"
STATICLIB="$TARGET_DIR/release/lib${EXTENSION_NAME}.a"
NO_METADATA="$BUILD_DIR/${EXTENSION_NAME}.no_metadata.wasm"
EXT_WASM="$BUILD_DIR/${EXTENSION_NAME}.duckdb_extension.wasm"
ABI_ARGS=()
[[ "$USE_UNSTABLE_C_API" -eq 1 ]] && ABI_ARGS=(--abi-type C_STRUCT_UNSTABLE)

build_profile() { # $1 = release|debug
  local profile="$1" outdir="build/wasm_eh/$1"
  echo "==> cargo build --${profile} --target wasm32-unknown-emscripten"
  DUCKDB_EXTENSION_NAME="$EXTENSION_NAME" \
  DUCKDB_EXTENSION_MIN_DUCKDB_VERSION="$TARGET_DUCKDB_VERSION" \
    cargo build --$profile --target wasm32-unknown-emscripten --features quack

  [[ -f "$TARGET_DIR/$profile/lib${EXTENSION_NAME}.a" ]] \
    || die "cargo did not produce $TARGET_DIR/$profile/lib${EXTENSION_NAME}.a"

  mkdir -p "$outdir/extension/$EXTENSION_NAME"

  # 1) stage the staticlib under the CI name
  cp "$TARGET_DIR/$profile/lib${EXTENSION_NAME}.a" "$outdir/lib${EXTENSION_NAME}.a"

  # 2) emcc the staticlib into a side module exporting the entry symbol
  echo "==> emcc link (side module, export _${EXTENSION_NAME}_init_c_api)"
  emcc "$outdir/lib${EXTENSION_NAME}.a" \
       -o "$outdir/${EXTENSION_NAME}.no_metadata.wasm" \
       -O3 -sSIDE_MODULE=2 -sEXPORTED_FUNCTIONS="_${EXTENSION_NAME}_init_c_api"

  # 3) append DuckDB extension metadata -> the loadable .duckdb_extension.wasm
  echo "==> append extension metadata"
  "$PYTHON_VENV_BIN" extension-ci-tools/scripts/append_extension_metadata.py \
      -l "$outdir/${EXTENSION_NAME}.no_metadata.wasm" \
      -o "$outdir/${EXTENSION_NAME}.duckdb_extension.wasm" \
      -n "$EXTENSION_NAME" \
      -dv "$TARGET_DUCKDB_VERSION" \
      -evf configure/extension_version.txt \
      -pf configure/platform.txt \
      "${ABI_ARGS[@]}"

  cp "$outdir/${EXTENSION_NAME}.duckdb_extension.wasm" \
     "$outdir/extension/$EXTENSION_NAME/${EXTENSION_NAME}.duckdb_extension.wasm"
}

build_profile release
# Mirror base.Makefile's move_wasm_extension: the upload artifact lives at the
# top-level build/wasm_eh/extension/<name>/, copied out of the release dir.
mkdir -p "build/wasm_eh/extension/${EXTENSION_NAME}"
cp "$EXT_WASM" "build/wasm_eh/extension/${EXTENSION_NAME}/${EXTENSION_NAME}.duckdb_extension.wasm"
[[ "$BUILD_DEBUG" -eq 1 ]] && build_profile debug

[[ -f "$EXT_WASM" ]] || die "expected artifact not produced: $EXT_WASM"
echo
echo "==> built: $EXT_WASM ($(wc -c < "$EXT_WASM") bytes)"
echo "    also:  build/wasm_eh/extension/${EXTENSION_NAME}/${EXTENSION_NAME}.duckdb_extension.wasm"

# --- optional: feed it to the docs site / SQL verifier ----------------------
if [[ "$INSTALL_DOCS" -eq 1 ]]; then
  DEST="docs/static/duckdb-extensions/${EXTENSION_NAME}.duckdb_extension.wasm"
  mkdir -p "$(dirname "$DEST")"
  cp "$EXT_WASM" "$DEST"
  echo "==> installed to $DEST"
  echo "    verify with:  npm test -w docs"
fi
