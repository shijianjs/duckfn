//! duckfn 命令行工具的入口：
//! `cargo run --features quack --bin duckfn-cli -- function_descriptions`
//!
//! 工具本身的实现都在 `duckfn::cli`（需要打开 duckfn 的 `quack` feature，它带上 `cli`），这里只
//! 负责把插件本体挂进来、把项目根目录传下去。
//!
//! 为什么用 `#[path]` 把 `extension` 编进来，而不是 `use duckfn::…`：`#[duck_*]` 的文档元数据靠
//! `inventory` 的静态构造器收集，只有**真正被链接进最终二进制**的目标文件才会生效。直接依赖 rlib
//! 时，链接器可能因为没人引用那些模块而把它们整块丢掉，导出的 CSV 就会是空的（而且是静默的）。
//! 让 bin 自己把同一份源码编一遍，注册项就落在本 crate 里，一定齐全。
//!
//! 注意只编 `extension/mod.rs`：入口符号在 `extension/entry.rs`（lib 与 wasm 各自声明它），而这个
//! bin 链接了 lib，再定义一次就是重复定义（Windows 上 LNK2005）。目标名之所以是 `duckfn-cli`
//! 而不是文件名里的 `duckfn`，是为了避开本包 cdylib 的产物同名冲突，见根 Cargo.toml 的说明。
//!
//! The entry point of the duckfn command-line tool:
//! `cargo run --features quack --bin duckfn-cli -- function_descriptions`. The tool itself lives in
//! `duckfn::cli` (which needs duckfn's `quack` feature — it pulls `cli` in); this file only pulls the
//! extension in and passes the project root down. `#[path]` is used instead of `use duckfn::…`
//! because the documentation metadata behind `#[duck_*]` is collected by `inventory`'s static
//! constructors, which only fire for object files that are really linked into the final binary: when
//! merely depending on an rlib, the linker may drop those modules entirely and the exported CSV would
//! come out empty — silently.
//!
//! Only `extension/mod.rs` is included: the entry symbol lives in `extension/entry.rs` (declared by
//! the lib and by the wasm target), and this bin links the lib, so defining it again would be a
//! duplicate definition (LNK2005 on Windows). The target is called `duckfn-cli` rather than the
//! `duckfn` its file is named after to avoid colliding with this package's cdylib artifact — see the
//! note in the root Cargo.toml.

#[path = "../../test/extension/mod.rs"]
mod extension;

fn main() -> std::process::ExitCode {
    duckfn::cli::run(env!("CARGO_MANIFEST_DIR"))
}
