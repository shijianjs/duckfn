// 示例扩展的模块树。入口符号（`duckfn_entrypoint!`）不在这里，而在同目录的 `entry.rs`：
// CLI 要把这棵树编进自己的二进制，却会链接本包的 lib，入口符号只能定义一次，详见 entry.rs。
//
// The example extension's module tree. The entry point (`duckfn_entrypoint!`) is not here but in the
// sibling `entry.rs`: the CLI compiles this tree into its own binary and links this package's lib, so
// the entry symbol may only be defined once — see entry.rs.
mod demo;
mod functions;
mod types;
