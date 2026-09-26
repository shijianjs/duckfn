// 文档站展示的 duckfn 版本号 = 最近一次正式发布（git tag）的版本。
//
// 这是文档里版本号的唯一来源：正文、README 之外的 markdown 都写占位符
// {{DUCKFN_VERSION}}，由 plugins/remark-version-placeholder.ts 在构建时替换。
// 发版时运行 `just release_bump X.Y.Z` 会自动更新这里的值。
export const DUCKFN_VERSION = '0.0.13';
