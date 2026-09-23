# duckfn 扩展：通用约定（供 AI 助手）

本文件由 duckfn 仓库维护，位于 duckfn clone 的 `templates/duckfn-conventions.md`，
项目根目录的 `AGENTS.md` 会指向这里。

**相对路径全部相对本文件所在目录**（也就是相对 duckfn clone 的 `templates/`）：
`../docs/` 是文档站正文、`../src/` 是示例扩展、`../duckfn/` 是运行时 crate。
用相对路径是为了让 clone 目录改名或移动后，这些指向依然有效。

## 1. 知识源（按顺序查，不要凭记忆写 API）

`duckfn` 是 0.0.x 的实验性框架，API 仍在变化，**凭印象写出来的属性名和返回类型基本都是错的**。

### 1.1 duckfn 仓库 clone（首选）

| 资源 | 路径 |
| --- | --- |
| 运行时源码 | `../duckfn/src/`（`register.rs` 注册流程、`value_types/` 类型适配、`functions/` 各类 builder） |
| 属性宏源码（`duckfn-macro`） | `../duckfn-macro/src/` —— **属性宏的真相来源**：某个属性接受哪些参数、允许哪些返回形状，直接读这里 |
| 用户文档（正文 / 英文） | `../docs/docs/**` |
| 用户文档（中文） | `../docs/i18n/zh-Hans/docusaurus-plugin-content-docs/current/**` |
| **示例扩展**（每个功能都有可运行实现） | `../src/extension/**` |
| **sqllogictest 范例** | `../test/sql/**/*.test` |
| 扩展骨架本体 | `..`（crate `rusty_quack`） |
| 项目侧 AGENTS.md 模板 | `./AGENTS.md` |

> 文档正文里的版本号是 `{{DUCKFN_VERSION}}` 占位符，构建时由 `../docs/duckfn-version.ts` 替换，
> 读到占位符不用惊讶。

按主题查表：

| 主题 | 文档 | 参考实现 |
| --- | --- | --- |
| 全部属性与参数 | `../docs/docs/guide/attributes.md` | — |
| 标量函数 | `../docs/docs/guide/scalar-functions.md` | `../src/extension/functions/scalar_function.rs` |
| 聚合函数 | `../docs/docs/guide/aggregate-functions.md` | `../src/extension/functions/aggregate_function.rs` |
| 表函数 | `../docs/docs/guide/table-functions.md` | `../src/extension/functions/table_function.rs`、`dynamic_table_function.rs` |
| `COPY ... TO` / `FROM` | `../docs/docs/guide/copy-functions.md` | `../src/extension/functions/copy_function.rs`、`copy_from_function.rs` |
| 类型转换 cast | `../docs/docs/guide/casts.md` | `../src/extension/functions/cast_function.rs` |
| 替换扫描 | `../docs/docs/guide/replacement-scans.md` | `../src/extension/functions/replacement_scan.rs` |
| SQL 宏 | `../docs/docs/guide/sql-macros.md` | `../src/extension/functions/sql_macro.rs`（脚本见 `../src/extension/functions/sql/*.sql`） |
| STRUCT / ENUM 等自定义类型 | `../docs/docs/guide/custom-types.md` | `../src/extension/types/duck_struct_scalar_echo.rs`、`duck_enum_echo.rs` |
| Rust ↔ DuckDB 类型映射 | `../docs/docs/guide/types.md` | `../src/extension/types/**` |
| 错误与 panic | `../docs/docs/guide/errors-and-panics.md` | — |
| 构建与发布 | `../docs/docs/build-and-release.md` | — |
| 排错 | `../docs/docs/troubleshooting.md` | — |
| 社区扩展文档页（`function_descriptions.csv`） | `../docs/docs/community-extension-docs.md` | `../src/extension/functions/*.rs`（带 `description` / `example` 的那几个） |

### 1.2 本地 cargo registry（没有 clone 时的替代品）

`cargo` 会把依赖源码解包到本地 registry，里面就是实际被编译的代码：

```powershell
# Windows；版本号从项目的 Cargo.toml / Cargo.lock 里读
dir $env:USERPROFILE\.cargo\registry\src\*\duckfn-<版本>\
dir $env:USERPROFILE\.cargo\registry\src\*\duckfn-macro-<版本>\
```

**注意这里没有什么**：published crate 只包含 `src/` 和 `README.md` —— 没有用户文档、
没有示例扩展、没有 sqllogictest。这些只有 clone 里有，所以 clone 是首选。

### 1.3 网络兜底

- 文档站：<https://shijianjs.github.io/duckfn/zh-Hans/>
- API 文档：<https://docs.rs/duckfn>
- 仓库：<https://github.com/shijianjs/duckfn>

**铁律**：任何来源都拿不到时，停下来告诉用户「我查不到 duckfn 的这部分 API」，
不要凭记忆编属性名、参数或返回类型。写错的宏会以编译错误的形式暴露，
但更常见的是一路编到底、最后没法编译。

## 2. 版本对齐

clone 所在的分支 / tag 应当与项目 `Cargo.toml` 里的 duckfn 版本对应
（`../Cargo.toml` 的 `[workspace.package] version` 可以确认 clone 自身的版本）。
两者不一致时，**以项目实际编译的那份代码为准**（本地 registry 或 clone 里的 `duckfn/src/`），
并提醒用户当前文档可能是别的版本。

## 3. 项目骨架与硬约束

入口链路：

```text
src/lib.rs           ->  mod extension;
src/wasm_lib.rs      ->  mod extension;   （同一组 mod，镜像）
src/extension/mod.rs ->  duckfn_entrypoint!("<扩展名>");
src/bin/duckfn.rs    ->  #[path] mod extension;  +  duckfn::cli::run(...)   （命令行工具，不参与插件运行）
```

- **扩展名**必须全小写、只含下划线，且与 `Makefile` 的 `EXTENSION_NAME`、
  最终 `.duckdb_extension` 文件名一致。符号名不对，`LOAD` 会直接失败。
  扩展名从项目 `src/extension/mod.rs` 的 `duckfn_entrypoint!("...")` 里读，不要猜。
- **`src/lib.rs` 与 `src/wasm_lib.rs` 必须声明同一组 `mod`**。官方模板的
  `mod lib;` 写法在嵌套模块时会报 `error[E0583]`，本仓库的写法是让两个 crate root
  都只写 `mod extension;` 再由 `extension/mod.rs` 往下挂。新增模块时两边都要挂上。
- `Cargo.toml` 里 `crate-type = ["cdylib"]`；`libduckdb-sys` 必须开 `loadable-extension`，
  不要引入会真正链接 libduckdb 的 feature。
- 不要写 `unsafe` 去解引用 DuckDB 的 C 类型。属性覆盖不到的场景用 `quack-rs` 的公开 API。
- **`src/bin/duckfn.rs` 用 `#[path = "../extension/mod.rs"] mod extension;` 自己编一遍插件本体**，
  不要改成 `use <crate>::...`：`#[duck_*]` 的文档元数据靠 `inventory` 的静态构造器收集，只有真正被
  链接进最终二进制的目标文件才会生效，只依赖库会被链接器整块丢掉、CSV 静默变空。
  它需要 `Cargo.toml` 里 duckfn 的 `cli` feature（带来 clap 与 csv）。

## 4. 开发循环

```powershell
cargo duckdb-ext build        # -> target/debug/<扩展名>.duckdb_extension
duckdb -unsigned -c "LOAD './target/debug/<扩展名>.duckdb_extension'; SELECT ..."
```

- `cargo-duckdb-ext-tools` 是全局 `cargo` 子命令，不给项目加依赖，日常迭代用它。
- 跑 sqllogictest 用 `make test`（等价 `make configure debug test`）。
  **Windows 上 `make` 必须在 Git Bash 里跑**，PowerShell 里跑不起来。
- 提交前：`cargo clippy --all-targets -- -D warnings`，有 warning 先修。
- duckfn 仓库的 `./Justfile` 是给下游项目准备的快捷入口，复制到项目根目录即可，
  只需把 `extension_name` 改成自己的扩展名。常用：`just build`、`just sql "SELECT my_fn(1)"`、
  `just repl`、`just test`（后者会先跑 `make configure` / `make debug`）。

## 5. 新增一个函数的标准流程

1. 查 `../docs/docs/guide/attributes.md`，确认用哪个属性，以及该属性允许的返回形状。
2. 从 `../src/extension/` 里找最接近的实现，**照抄骨架、只改业务逻辑**，不要自创签名。
3. 把新模块挂进 `mod` 链（`extension/mod.rs` 与各层 `mod.rs`），确认 `lib.rs` /
   `wasm_lib.rs` 仍然镜像。
4. 加测试：`test/sql/<分类>/<名字>.test`，参照 `../test/sql/**/*.test` 的写法，
   至少覆盖：正常值、`NULL`、边界值、错误路径（`statement error`）。
5. 顺手写上 `description` / `example`：DuckDB 的 C API 没有设置函数描述与示例的接口，
   社区扩展文档页全靠导出的 `function_descriptions.csv` 覆盖，不写就是一片空白。
6. `cargo duckdb-ext build` → 手动 `LOAD` 跑一遍 → `make test`。
7. 扩展要作为社区扩展发布时：`cargo run --bin duckfn -- function_descriptions`
   （或 `just docs_csv`）重新生成 `target/function_descriptions.csv`，
   再用 `--all` 看一遍还有哪些函数没写描述（输出 `target/function_descriptions_all.csv`）。

## 6. 代码约定

- 文本文件一律 **LF**（`\n`），不要 CRLF。
- 临时脚本、数据、日志、一次性验证代码放 `target/`，不要放在被跟踪的目录里。
- 错误处理用 `duck_error(...)` / `DuckOptionResult`，不要把 panic 当控制流。
- 能用属性解决的，不要退回手写注册代码。
