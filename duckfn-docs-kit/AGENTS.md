# duckfn-docs-kit — AGENTS.md

本包是 duckfn 系（DuckDB 扩展）文档站的共享积木库：TOC 折叠控件、首页
web components、品牌 CSS tokens、版本占位符 remark 插件、iconify 图标集。
它被 `docs/`（本仓库文档站）以 npm workspace 消费，也供下游扩展项目复用 ——
**发布通用逻辑，而不是到处拷代码**。

## 包结构 / 导出

- 构建：Vite lib 模式产出 ESM（`dist/`），`tsc -p tsconfig.build.json` 产出
  `.d.ts`。`npm run build` 一步完成。CSS 不经构建，作为源文件直接导出。
- 三个运行时入口，对应 `exports` 的三个子路径：
  - `duckfn-docs-kit`（浏览器：CE + 图标 + 类型）
  - `duckfn-docs-kit/toc-toggle`（浏览器：TocToggle 类）
  - `duckfn-docs-kit/remark`（**Node 构建期**：remark 插件）
- CSS 子路径：`css/kit.css`（聚合）、`css/tokens.css`、`css/toc-toggle.css`、
  `css/home.css`，下游在 Docusaurus 的 CSS 管线里 `@import`。

## 代码风格（硬性要求）

这些规则是本包存在的意义所在，评审时逐条对照：

1. **禁止拼 HTML 字符串**：不许 `innerHTML = \`...\``、不许
   `insertAdjacentHTML`、不许任何"模板字符串生成标记再塞进 DOM"的写法。
   反例：

   ```ts
   this.innerHTML = `<button class="x">${label}</button>`;
   this.querySelector('button').addEventListener('click', fn); // 禁止
   ```

2. **DOM 用对象操作**：`document.createElement` 创建、**类的字段持有引用**、
   直接对字段调方法。正例：

   ```ts
   #button = document.createElement('button');
   #button.className = 'x';
   #button.textContent = label;
   #button.addEventListener('click', fn);
   ```

3. **尽量类化**：一个组件一个 class（web component 继承
   `elements/base.ts` 的 `DfkElement`，非元素的小部件如
   `TocToggle`、卡片 builder 也用 class）。函数式导出只留给真正的纯工具
   （`icons.ts` 的 mask 助手等）。
4. **组件数据走 `data` property**：`dfk-*` 元素只从 `data` setter 接收内容
   （纯字符串，i18n 由消费方解析后传入），不解析 attribute、不消费
   light DOM 子节点。
5. **图标只用 iconify 数据 + CSS mask**：`applyIconMask(el, icon)` 把
   `url("data:image/svg+xml,…")` 写进元素的 `--dfk-icon` 自定义属性，
   `home.css` 的 `.dfk-icon` 负责 mask 与 `background-color: currentColor`。
   拼的是**图片 URL**、不是页面标记，因此不违反第 1 条。**永远不要把
   iconify 的 `body` 用 innerHTML 注入 DOM，也不要手写 SVG 路径。**
6. **不用 Shadow DOM**：组件渲染进 light DOM（全局 `dfk-` 前缀类名），这样
   才能看见消费站的 Infima 变量、`[data-theme]` 与
   `@layer docusaurus.theme-classic` 级联。新增类名一律 `dfk-` 前缀。
7. **SSR 安全**：Docusaurus 预渲染在 Node 里 import 本包。
   - 继承 `HTMLElement` 的类必须 `extends HTMLElementBase`（`src/dom.ts`，
     Node 下回退为空基类），否则模块求值直接崩。
   - 触碰 `window` / `document` / `customElements` 的入口（
     `registerDfkElements()`、`TocToggle.init()`）要么带守卫，要么由
     消费方在浏览器环境调用。
   - `src/remark/` 是唯一允许在 Node 构建期跑的目录，它不得 import 任何
     浏览器模块。
8. **React 19 自定义元素**：JSX 类型增强写在 `src/index.ts` 的
   `declare module 'react' { namespace JSX { … } }`，新增元素时同步补上。

## 其它约定

- 文本文件一律 LF（仓库根 AGENTS.md 有替换命令）。
- 注释解释"为什么"，与 docs/ 现有风格一致；本包面向国际下游用户，注释用英文。
- `private: true` 只是暂不发 npm；`exports`/`files` 已按可发布形态维护，
  解禁时去掉 `private` 补 `publishConfig` 即可，不要改结构。
