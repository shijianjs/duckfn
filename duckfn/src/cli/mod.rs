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

/// 顶层命令行定义。
///
/// The top-level command-line definition.
#[derive(Debug, Parser)]
#[command(
    name = "duckfn",
    about = "duckfn 的辅助命令（不参与扩展运行时）",
    version
)]
struct Cli {
    /// 要执行的子命令。
    ///
    /// The subcommand to run.
    #[command(subcommand)]
    command: Command,
}

/// 目前只有一个子命令；以后加工具命令就在这里加一个变体。
///
/// Only one subcommand for now; future tools are added as further variants here.
#[derive(Debug, Subcommand)]
enum Command {
    /// 把 `#[duck_*]` 上声明的函数文档导出成 CSV
    ///
    /// Export the function documentation declared on `#[duck_*]` as a CSV
    #[command(name = "function_descriptions")]
    FunctionDescriptions {
        /// 连没写 description / comment / example 的函数一起导出（文件名带 `_all` 后缀）
        ///
        /// Also export functions without any description / comment / example (the file name gets
        /// an `_all` suffix)
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
