//! duckfn 命令行工具的入口：`cargo run -p duckfn_quack --bin duckfn -- function_descriptions`
//!
//! 工具本身的实现都在 `duckfn::cli`（需要打开 duckfn 的 `cli` feature），这里只负责把插件本体
//! 挂进来、把项目根目录传下去。
//!
//! 为什么用 `#[path]` 把 `extension` 编进来，而不是 `use duckfn_quack::...`：`#[duck_*]` 的文档
//! 元数据靠 `inventory` 的静态构造器收集，只有**真正被链接进最终二进制**的目标文件才会生效。
//! 直接依赖 rlib 时，链接器可能因为没人引用那些模块而把它们整块丢掉，导出的 CSV 就会是空的
//! （而且是静默的）。让 bin 自己把同一份源码编一遍，注册项就落在本 crate 里，一定齐全。
//!
//! The entry point of the duckfn command-line tool:
//! `cargo run -p duckfn_quack --bin duckfn -- function_descriptions`. The tool itself lives in
//! `duckfn::cli` (which needs duckfn's `cli` feature); this file only pulls the extension in and
//! passes the project root down. `#[path]` is used instead of `use duckfn_quack::...` because the
//! documentation metadata behind `#[duck_*]` is collected by `inventory`'s static constructors,
//! which only fire for object files that are really linked into the final binary: when merely
//! depending on an rlib, the linker may drop those modules entirely and the exported CSV would come
//! out empty — silently.

#[path = "../extension/mod.rs"]
mod extension;

fn main() -> std::process::ExitCode {
    duckfn::cli::run(env!("CARGO_MANIFEST_DIR"))
}
