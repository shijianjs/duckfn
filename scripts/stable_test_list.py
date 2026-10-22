#!/usr/bin/env python3
"""Write the stable-region sqllogictest file list.

The default build is a stable C API artifact (`USE_UNSTABLE_C_API=0`), so the cases that exercise
capabilities from the unstable region must not run against it: their functions simply do not exist.
This mirrors the exclusion list in `scripts/test-stable.sh`, with one difference -- the bridges
(`chrono_bridge`, `rust_decimal_bridge`, `uuid_bridge`) stay in, because the default feature list
enables those features and they are stable-region code.

Usage: stable_test_list.py <output-file>
"""

from __future__ import annotations

import sys
from pathlib import Path

# File stems (without .test) that need capabilities outside the stable C API.
NON_STABLE = {
    # COPY functions: unstable region.
    "copy_function",
    "copy_from_function",
    # TIME_NS wrapper types: added in DuckDB 1.5, behind the `duckdb-1-5` feature.
    "wrapper_types_scalar_echo",
    "wrapper_types_table_echo",
    # Host file system: `duck_vfs`.
    "duck_vfs",
    "file_system",
    "file_util",
}


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {Path(sys.argv[0]).name} <output-file>", file=sys.stderr)
        return 2

    root = Path(__file__).resolve().parent.parent
    tests = sorted(
        path.relative_to(root).as_posix()
        for path in (root / "test" / "sql").rglob("*.test")
        if path.stem not in NON_STABLE
    )
    if not tests:
        print("error: no stable-region test case matched", file=sys.stderr)
        return 1

    out = Path(sys.argv[1])
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("\n".join(tests) + "\n", encoding="utf-8")
    print(f"{len(tests)} stable-region cases -> {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
