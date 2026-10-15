// 示例扩展的函数模块树。**每个模块按它需要的能力挂 feature**：`quack` 只决定这棵树编不编
// （见 Cargo.toml），要哪一套能力由构建时列的 feature 决定 —— 于是同一份示例既能编出「只用稳定版
// C API」的插件（拿 1.3/1.4 的 DuckDB 就能加载），也能编出全量插件，两者对比着测。
//
// 标在**模块**上而不是函数上：关掉某个能力时整个模块消失，不存在「半开」的状态。
//
// The example extension's function module tree. **Every module carries the feature of the capability
// it needs**: `quack` only decides whether this tree is compiled at all (see Cargo.toml), while which
// capabilities exist comes from the feature list a build passes — so the same example yields both a
// "stable C API only" plugin (loadable by DuckDB 1.3/1.4) and a full one, to test side by side.
//
// Marked at the **module** level rather than per function: turning a capability off removes the whole
// module, so there is no half-enabled state.

mod aggregate_function;
mod aggregate_auto_collect;
mod cast_function;
// chrono 时间桥：需要 duckfn 的 `chrono` feature（readme 里的 DuckDate::to_naive_date 等）。
//
// The chrono bridge: needs duckfn's `chrono` feature (`DuckDate::to_naive_date` and friends).
#[cfg(feature = "chrono")]
mod chrono_bridge;
// COPY FROM：COPY 函数在 C API 的不稳定区，所以整条链路都挂在 `duckdb-1-5` 上（`tsv_format` 只被
// 这两个 COPY 模块用到，跟着一起开）。
//
// COPY FROM: COPY functions live in the unstable region of the C API, so this whole path sits behind
// `duckdb-1-5` (`tsv_format` is used only by those two COPY modules and comes along).
#[cfg(feature = "duckdb-1-5")]
mod copy_from_function;
#[cfg(feature = "duckdb-1-5")]
mod copy_function;
mod dynamic_table_function;
// 宿主文件系统：需要 `duck_vfs`（它带上注册期那条自有连接与 `duckdb-1-5`）。
//
// The host file system: needs `duck_vfs` (it brings the registration-time connection and
// `duckdb-1-5`).
#[cfg(feature = "duck_vfs")]
mod file_system;
mod replacement_scan;
#[cfg(feature = "rust_decimal")]
mod rust_decimal_bridge;
mod scalar_function;
// 在扩展里执行 SQL：只需 `sql`（只用稳定版 C API，不需要 `duckdb-1-5`）。
//
// Running SQL from an extension: needs only `sql` (it uses the stable C API, not `duckdb-1-5`).
#[cfg(feature = "sql")]
mod sql_exec;
mod sql_macro;
mod table_function;
#[cfg(feature = "duckdb-1-5")]
mod tsv_format;
#[cfg(feature = "uuid")]
mod uuid_bridge;
