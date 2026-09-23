//! duckfn 的命令行工具：不参与插件运行时的那部分逻辑。
//!
//! 由扩展项目里的 `src/bin/duckfn.rs` 调用，例如
//! `cargo run --bin duckfn -- function_descriptions`。整个模块挂在 `cli` feature 后面，
//! 不打开时 duckfn 不依赖 clap / csv，插件本体（cdylib）也完全不受影响。
//!
//! The duckfn command-line tool: the part of the logic that is not part of the extension runtime.
//! It is called by the extension project's `src/bin/duckfn.rs`, e.g.
//! `cargo run --bin duckfn -- function_descriptions`. The whole module sits behind the `cli`
//! feature, so with it off duckfn depends on neither clap nor csv and the extension itself is
//! unaffected.

/// `function_descriptions` 子命令：导出社区扩展文档页用的 CSV。
///
/// The `function_descriptions` subcommand: exports the CSV the community-extension doc pages use.
pub mod function_descriptions;

use clap::{Parser, Subcommand};
use std::path::Path;
use std::process::ExitCode;

/// Helpers for duckfn extensions (not part of the extension runtime).
//
// clap 没有 i18n：`about` / `help` 都是编译期静态字符串，没有按 locale 取词的地方。而且 derive
// 会把**文档注释本身当成帮助文本**（第一段是 `about`，整段是 `long_about`），所以这些位置上的
// `///` 只能用一种语言 —— 本仓库其它注释照旧中英双语，这里凡是用户会看到的文本一律用英文，
// 免得 `--help` 里中英两段并排出现（改之前就是这样）。中文说明放在 `//` 里。
//
// clap has no i18n: `about` and `help` are compile-time constants with nowhere to look a locale up,
// and the derive turns **doc comments into the help text itself** (first paragraph is `about`, the
// whole comment is `long_about`). A `///` in these positions can therefore only ever be one
// language — everything user-visible here is English so `--help` does not print two paragraphs side
// by side (which is what it did before); Chinese notes live in `//` comments.
//
// 中文标题：duckfn 的辅助命令（不参与扩展运行时）。
#[derive(Debug, Parser)]
#[command(name = "duckfn", version)]
struct Cli {
    /// The subcommand to run.
    //
    // 中文：要执行的子命令。
    #[command(subcommand)]
    command: Command,
}

// 目前只有一个子命令；以后加工具命令就在这里加一个变体。
//
// Only one subcommand for now; future tools are added as further variants here.
#[derive(Debug, Subcommand)]
enum Command {
    /// Export the function documentation declared on `#[duck_*]` attributes as a CSV.
    //
    // 中文：把 `#[duck_*]` 上声明的函数文档导出成 CSV。
    #[command(name = "function_descriptions")]
    FunctionDescriptions {
        /// Also export functions without any description, comment or example (the file name gets
        /// an `_all` suffix).
        //
        // 中文：连没写 description / comment / example 的函数一起导出（文件名带 `_all` 后缀）。
        #[arg(long)]
        all: bool,
    },
}

/// CLI 入口，由扩展项目的 `src/bin/duckfn.rs` 调用。
///
/// `project_dir` 是扩展项目的根目录（bin 里传 `env!("CARGO_MANIFEST_DIR")`）；输出固定在它下面的
/// `target/`，与当前工作目录无关。
///
/// The CLI entry point, called by the extension project's `src/bin/duckfn.rs`. `project_dir` is
/// the extension project's root (the bin passes `env!("CARGO_MANIFEST_DIR")`); the output lands
/// under its `target/` regardless of the current working directory.
///
/// 失败时打一行 stderr 并返回 [`ExitCode::FAILURE`]，所以这里不返回 `Result`；参数错误由 clap
/// 自行处理（它会打印用法并以非零码退出）。
///
/// Failures print one line to stderr and return [`ExitCode::FAILURE`], which is why this returns no
/// `Result`; argument errors are handled by clap itself (it prints usage and exits non-zero).
pub fn run(project_dir: impl AsRef<Path>) -> ExitCode {
    let cli = Cli::parse();
    let project_dir = project_dir.as_ref();

    match cli.command {
        Command::FunctionDescriptions { all } => {
            match function_descriptions::export(project_dir, all) {
                Ok(summary) => {
                    let path = function_descriptions::output_path(project_dir, all);
                    println!(
                        "wrote {} function descriptions to {} ({} without a description)",
                        summary.written,
                        path.display(),
                        summary.without_description
                    );
                    if summary.skipped > 0 {
                        println!(
                            "skipped {} function(s) with no documentation — \
                             pass --all to include them",
                            summary.skipped
                        );
                    }
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("duckfn: cannot write the function descriptions: {error}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}
