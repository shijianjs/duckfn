# duckfn-docs-kit — CONVENTIONS.md

本文件是**维护者**向的：本包怎么写、怎么测、怎么发。包使用者（包括下游项目的 AI 助手）要的是随
npm 包发布的 [`AGENTS.md`](./AGENTS.md) —— 那份讲「怎么用这个包」，这份讲「怎么改这个包」，
**不**随包发布（`files` 白名单里没有它）。

本包是 duckfn 系（DuckDB 扩展）文档站的共享积木库：TOC 折叠控件、首页
web components、品牌 CSS tokens、版本占位符 remark 插件。它被 `docs/`
（本仓库文档站）以 npm workspace 消费，也供下游扩展项目复用 ——
**发布通用逻辑，而不是到处拷代码**。

## 包结构 / 导出

### 目录按业务语义组织，不按技术类型（默认逻辑，可递归）

**一个能力一个文件夹，它的 TS、CSS（以及将来的测试、资源）都放进去。** 目录
首先回答「这东西属于哪个业务」，而不是「这是 TS 还是 CSS」。技术类型只是业务
模块内部的次级文件，不单独开 `css/`、`utils/` 这类技术分类目录。

这条不是只管 `src/` 一层的局部偏好，而是**跨尺度的默认逻辑，可以递归**：类、
模块、多级目录、包、项目、多层项目，每一层都用同一个判断 —— 先问「属于哪个
业务语义」，再把它的实现材料（代码、样式、测试、资源）收拢在同一处。

- **类**：一个类的字段、方法、事件处理聚在一个 class 里，不拆成摊在多文件的
  partial / mixin。
- **模块 / 目录**：`home/` 一个目录装齐首页三件套的 TS、CSS、样式注入。
- **包**：本包的 `src/` 按能力分目录（`home/`、`toc-toggle/`、`theme/`），
  不按 `css/`、`elements/`、`utils/` 分。
- **项目 / 多层项目**：仓库根同理 —— Rust 运行时（根 `src/` + `test/`）与
  文档站（`docs/`）各是一个业务，各自内部再按同一逻辑递归。

**万不得已才允许按技术栈分**：两种实现材料在工具链上放不到一块儿时，才拆成
并列的顶层目录 —— 例如 Rust 代码（Cargo workspace）和文档站（npm +
Docusaurus）：构建系统、依赖管理、产物形态完全不同，强行合并只会互相污染。
这是生态系统的物理限制，不是「按技术分类更好」；能收拢的必须收拢。

```
src/
├── home/            # 首页三件套：DfkHero.ts / DfkFeatures.ts / DfkNextSteps.ts
│                    #   + home.css（三组件共享的样式）+ styles.ts（?inline 注入）
├── toc-toggle/      # TocToggle.ts + TocToggle.css（light-DOM 例外，见第 9 条）
│                    #   + client.ts / plugin.ts（胶水：插件把 client 注入每个页面）
├── sql/             # 可运行 SQL：DfkSql.ts + DfkSql.css/styles.ts（shadow：编辑器
│                    #   + 悬浮图标按钮）+ sql.css（light-DOM 结果区，见第 9 条）
│                    #   + runtime.ts（DuckDB-Wasm 单例）+ editor.ts / renderers.ts
│                    #   + PreviewTabs.ts（预览页签，末尾恒定 Table）+ remark.ts
│                    #   + extensions.ts（Node：扩展预加载插件）+ runtimeConfig.ts
│                    #     （两侧共享的注入配置契约，见「扩展预加载」）
│                    #   + collect.ts / verify.ts（Node：文档站自己的 SQL 测试与
│                    #     `duckfn-sql-verify` 命令）+ browserRunner.ts / harness.ts
│                    #     （浏览器：Playwright/playwright-core 驱系统浏览器跑块；harness 是给它的页面）
│                    #   + client.ts（dfk-* 元素注册，插件注入到每个页面）
├── theme/           # tokens.css —— 全局设计基础设施，无业务归属，单独放
├── kit.css          # 全局 CSS 聚合入口（@import theme + toc-toggle + sql）
├── dom.ts           # el() / HTMLElementBase 纯工具
├── index.ts         # 浏览器桶文件
├── register.ts      # customElements 注册
├── remark.ts        # Node 构建期 remark 插件
├── types.ts         # 组件值类型
└── vite-env.d.ts    # `*.css?inline` 的模块声明
```

- `home.css` 留在 `home/` 而非拆进各组件：`:host`/box-sizing 重置、`.dfk-section`
  系列、`dfk-rise` keyframes、断点是三组件**真正共享**的规则，拆三份会引入重复
  或额外的 base 文件。共享样式属于「home 这个业务」，就放 `home/` 里。
- `tokens.css` 是唯一没有业务归属的全局基础设施，`theme/` 是它的例外单独目录。

### 构建与导出

- 构建：Vite lib 模式产出 ESM（`dist/`），`tsc -p tsconfig.build.json` 产出
  `.d.ts`。`npm run build` 一步完成。全局 CSS（`theme/tokens.css`、
  `toc-toggle/TocToggle.css`）不经构建、作为源文件直接导出；组件自己的
  `home/home.css` 由 `home/styles.ts` 以 `?inline` 内联进 bundle（见第 9 条）。
- **package.json 与业务解耦（硬性要求）**：`exports` 只有三条**永远不改**的规则
  —— `.`（主入口）、`./src/*`（源文件直出，CSS 走这里）、`./*`（通配，
  `dist/` 下任何产物自动成为可导入子路径）。新增 / 移动 / 重命名模块**一律不碰
  package.json**，改 `vite.config.ts` 的 `entry` 一行即可。评审时看到 package.json
  里出现逐个文件、逐个入口的映射，就是违反本条。
- 运行时入口文件名与其主导出的类名一致（大驼峰），ts 源文件同理；入口的
  **导入子路径 = 它在 `src/` 下的相对路径**（通配映射到 `dist/` 同路径产物）：
  - `duckfn-docs-kit`（浏览器：`index.ts` 桶文件，CE + 类型）
  - `duckfn-docs-kit/toc-toggle/TocToggle`（浏览器：TOC 折叠类）
  - `duckfn-docs-kit/remark`（**Node 构建期**：版本占位符 remark 插件）
  - `duckfn-docs-kit/sql/remark`（**Node 构建期**：可运行 SQL remark 插件）
  - `duckfn-docs-kit/sql/extensions`（**Node 构建期**：扩展预加载 Docusaurus 插件）
  - `duckfn-docs-kit/sql/verify`（**Node 运行期**：文档站 SQL 测试的入口与 CLI；
    `sql/collect`（收集）与 `sql/browserRunner`（用 playwright-core 驱浏览器跑块）是它的两半；
    `sql/harness` 是给浏览器加载的页面产物，由 `browserRunner` serve，站点/下游不手写引用）
  - `duckfn-docs-kit/toc-toggle/plugin`（**Node 构建期**：TOC 胶水插件）
  - `duckfn-docs-kit/sql/client`、`duckfn-docs-kit/toc-toggle/client`（浏览器引导，
    由上面两个插件注入，站点不要手写引用）
- CSS 子路径：消费方直接按源文件路径引 —— `@import
  'duckfn-docs-kit/src/kit.css'`（聚合入口），或单独引
  `duckfn-docs-kit/src/theme/tokens.css` 等。不再维护 `css/kit.css` 这类
  与源路径脱钩的别名。`home.css` **不作为**全局 CSS 导出 —— 它由
  `home/styles.ts` 内联进 JS bundle，注入各组件的 shadow root。

### TOC 折叠控件（`toc-toggle/`）

- Docusaurus 的桌面目录元素是 `.theme-doc-toc-desktop`，它自带
  `position: sticky; top: calc(var(--ifm-navbar-height) + 1rem)`（见主题里的
  `theme/TOC/styles.module.css`）；而按钮由本模块插在它**前面**、同一个
  `.toc-column` 里。
- 所以按钮必须自己 `position: sticky`：否则文章一长，TOC 留在视口里悬浮，按钮却随页面滚上去，
  读者把目录折叠后再往下滚就找不回来。TOC 的 `top` / `max-height` 要相应下移一个按钮高度
  （按钮 2rem + 上下留白），两者才不会重叠 —— 三条规则都在 `TocToggle.css` 的
  `@layer docusaurus.theme-classic` 段里，且带 `.toc-column` 前缀：TOC 那条来自 CSS module 的
  哈希类名，同层同特异性时胜负取决于样式表顺序，前缀是唯一稳妥的写法。
- 折叠态隐藏的是整个 `.theme-doc-toc-desktop`（按钮在它外面，因此仍在），列收成 2.5rem 宽，
  按钮此时居中而不是靠右。

### 可运行 SQL：渲染契约与 DuckDB-Wasm 事实

`sql/` 是一条单向链：`remark.ts`（构建期）→ `DfkSql.ts`（元素）→ `runtime.ts`
（DuckDB 单例）→ `renderers.ts` + `PreviewTabs.ts`（结果渲染）。改动按这个顺序读。

**渲染器（`renderers.ts`）**

- `rendererFor(config, result)` 是唯一入口：`result.error` 一律走 `errorRenderer`；
  否则按 `config.show` 查表，`show` 缺省时「单列单行 → `text`，其余 → `table`」。
- 新增一种 `show` = `registry` 加一项 + `RunnableSqlConfig.show` 联合类型加一个字面量。
  渲染器签名统一是 `(context, result) => Promise<void | (() => void)>` —— **一律
  异步**，返回的 disposer 由 `DfkSql` 在重跑 / 断开时调用，调用方只处理一种形态。
- `html` 与 `iframe` 是**同一个渲染器**（都写 `srcdoc`）；`svg` 是另一个（内联进页面）。
- iframe 默认 `sandbox="allow-scripts"` 且**不含 `allow-same-origin`**：报告里的
  JavaScript 照跑，但 frame 持有 opaque origin，与文档站主体隔离。**父文档因此读不到
  `iframe.contentDocument`（为 `null`）——这是设计，不是 bug**，验证时别拿它当失败。
  放宽只能显式写 `option.sandbox`（`sandbox` 在 DOM 上是 `DOMTokenList`，`el()` 里
  必须走 `attrs`）。
- 内联 SVG 走 `DOMParser` + `parsererror` / `namespaceURI` 检查，插入前**剥离**所有
  可执行或可导航内容（`script`、`foreignObject`、`on*`、`javascript:` 的
  `href`/`xlink:href`）；解析失败退化为 `pre` 文本，绝不把裸标记塞进 DOM。
- 预览尺寸用 CSS 自定义属性表达（`--dfk-sql-preview-width` / `-height`、
  `--dfk-sql-table-height`），靠选择器特异性覆盖，不写 `!important`。
- **结果面底色一律用 `--ifm-background-surface-color`，不要用
  `--ifm-background-color`**：后者可以被站点声明成 `transparent`（本仓库文档站
  正是如此，页面底色另有来源），全屏 overlay 会因此变成透明、内容直接透出。

**界面契约（`DfkSql.ts` + `PreviewTabs.ts`）**

- **没有静态预览、没有工具栏、没有「编辑」模式**：CodeMirror 编辑器就是代码视图，
  在 `connectedCallback()` 里挂载（`#mounting` 守卫 + await 后 `isConnected` 守卫，
  断开即 `destroy()`）。`remark.ts` 保留下来的 `code` 子节点只是预渲染文本，
  元素没有默认 slot、`sql.css` 里 `dfk-sql > :not([slot]) { display: none }` 把它压掉。
- **加载占位（三处，尺寸要一起改）**：编辑器和表格各是一个懒 `import()`，慢网下这两段窗口
  肉眼可见，所以各有一个骨架。编辑器是**每条 SQL 一行**的条：`#setEditorPending()` 按
  `#currentSql` 的行数增删条数，每条占一个 `1.6em` 的盒子（= `.cm-scroller` 的 `line-height`），
  半行距靠 flex 居中、**不能用 margin** —— 相邻 margin 会折叠，条与条会比真实代码行更密
  （实测 3 行时矮 13px）。所以换成编辑器时块的高度不变；`import()` 自己失败时也要撤掉（状态行
  才是那条消息）。行数按**源码行**算，这是刻意的近似：编辑器里折行的长行会让真块更高，要精确
  就得先把文字排出来（还得猜站点字体与列宽），对占位来说不值当。表格那两行由 `mountTable` 在
  解析前塞进 `.dfk-sql-table`，画布落地（或错误视图接手）即移除，行高由 `ROW_HEIGHT` 经自定义
  属性传下去。第三处在 **upgrade 之前**：`dfk-sql:not(:defined)`（`sql.css`）——元素还没定义时
  既没有 shadow 树、预渲染的代码节点又被上面那条隐藏，不画点什么就是一个会突然弹出来把页面顶
  下去的空洞。这一份是 `sql/remark.ts` 在构建期就写进 HTML 的真实子节点（`skeleton()`），所以
  它也按行数走、与 shadow 里那份逐像素一致；升级后它不在 slot 里，自然不渲染，不需要 JS 删。
  两侧各有一份 bar 规则与 `@keyframes`（跨 shadow 边界拿不到对方的），改尺寸/动画要一起改。
- **不要覆盖 `.cm-content` 的垂直 padding，也不要给 `.cm-gutters` 加**：CodeMirror 的
  `ViewState.measure()` 用 `parseInt` 读 `.cm-content` 的 computed `padding-top` 算出
  `paddingTop`，再把同一数值作为第一个 gutter 元素的 `marginTop` 施加下去（经 gutter 的
  `above` 偏移）。因此覆盖值不是整像素就会留下小数偏移（`0.4rem` = 6.4px 被读成 6px，
  行号偏高 0.4px），而两侧都加等于把同一段内缩算两遍、行号整体低于代码行约 6px。基类主题
  自带的 `padding: 4px 0` 既是整数又够紧凑，**保持原样**。
- **每个结果都有 tab 栏**，包括只有一个 `Table` 页签的普通表格结果 —— 因为 tab 栏是
  全屏按钮唯一的落脚点。`text` 结果是 `[Text, Table]`；`table` 结果是 `[]` + 末尾 Table。
- **全屏按钮归 `DfkSql` 所有**（状态在它手里），节点经 `RenderContext.fullscreenButton`
  交给 `PreviewTabs`，由后者摆到 tab 栏右端、`role="tablist"` 之外，所以不随页签滚动。
  全屏 overlay 不再需要 `padding-top` 给悬浮工具栏让位，退出按钮就在原来的位置。
- 代码块那五个按钮（执行 / 格式化 / 重置 / 折行 / 复制）是紧凑的图标按钮，悬浮在代码区右上角
  （`.dfk-sql-code:hover / :focus-within` 时才 `opacity: 1` + `pointer-events: auto`，
  隐藏时不可点）；提示用 `data-tip` + `::after`。这套图标按钮与 tooltip 规则**在
  `DfkSql.css`（shadow）与 `sql.css`（light）各写一份** —— 前五个按钮在 shadow 树里，
  全屏按钮在 light DOM 的结果区，一条规则够不着两处；两处都留了交叉引用注释。
- 折行默认**开启**，用 `Compartment` + `wrap.reconfigure(lineWrapping)` 切换，不重建
  编辑器（`@codemirror/state` 因此是动态 import 列表的一员，也在 vite external 里）。
  复制成功后按钮变 `lucide:check` + `Copied` 约 1.6s 再复位，定时器在
  `disconnectedCallback()` 里清掉。
- 格式化走 `sql-formatter`（`duckdb` dialect，它认 DuckDB 的 `EXCLUDE` / `PIVOT`），与
  CodeMirror 同一套懒加载动态 import，同样列在 vite 的 external 里。它只改空白、保留
  作者的关键字大小写（默认 `keywordCase: "preserve"`），所以格式化**不清结果**；写回
  走 `setValue()`，属于普通编辑，CodeMirror 的撤销历史覆盖得到。

**表格（VTable）**

- 行高紧凑靠**构造函数选参** `defaultRowHeight` / `defaultHeaderRowHeight`（不是主题
  对象，写在 `theme` 里无效），容器高度公式随之用同一个常量。
- **列宽默认 `widthMode: 'adaptive'`**（官方「自适应容器宽度」）：先按内容量出每列宽度
  （表头已含排序图标宽度），再按比例缩放到刚好铺满容器 —— 初始视图就填满、不靠默认列宽，
  内容多的列自然多分（比「各列平分」更合理），超长未起别名的表头有 `limitMaxAutoWidth`
  （默认 450）兜底。代价：总内容宽超过容器时是**缩放**而不是横向滚动条；容器尺寸变化
  （含全屏）会重算；手动拖过的列被排除在再分配之外，其余列围着它重新铺满。
- 列宽模式在右键菜单里可切（`WIDTH_MODES`：铺满 / 按内容列宽 / 内容优先）。切换时
  **`table.widthMode` 与 `table.autoFillWidth` 的 setter 只存值、不重排**，重排由
  `updateColumns(cols, {clearColWidthCache:true, clearRowHeightCache:false})` 触发
  （`createSceneGraph` → `computeColsWidth` 按新模式重测）；特意**不用 `updateOption`**，
  因为它会把 sortState 一并清掉。列宽模式也属于「重置视图」的回退范围。
- 右键子菜单：父项 `children` 即子菜单（html 模式原生支持，箭头用 `.vtable__menu-element__arrow`）。
  子菜单的当前项用文本前缀 `✓ ` 标记 —— vendor 的 `--select` 高亮只认
  `menu.dropDownMenuHighlight`，且要按当前单元格解析，不适合表达全局状态。
- **结果区/表格/面板都要 `overscroll-behavior: contain`**：它不是继承属性，必须打到
  每个真正滚动的盒子上（含 `.dfk-sql-table *`，VTable 的内部滚动容器藏在里面）。
  否则滚轮滑到表格底部会继续链式滚动整页 —— 表现是「页面刷一下飞上去、表格消失」。
- VTable 的尺寸变化交给 `ResizeObserver`，不向外传 resize 管道；但回调里**必须把
  `table.resize()` 延到 `requestAnimationFrame`**（并在 disposer 里
  `cancelAnimationFrame`）—— `resize()` 本身会改变被观察的盒子，同步调用会被浏览器
  判为 `ResizeObserver loop completed with undelivered notifications`（dev server 会
  把它弹成整屏错误浮层）。
- `PreviewTabs` 的 `button` / `panel` 全在构造函数里一次建好，切换只改 `classList` /
  `aria-selected` / `tabIndex` / `hidden`；`Table` 面板懒挂载（首次切入才 `mountTable`，
  因为 VTable 构造时要量容器，而 `hidden` 的盒子量出来是 0），若挂载还在飞行中就被
  `dispose()`，落地后立刻释放。
- 交互能力（排序、复制、行高列宽可调、表头拖拽换位、hover 十字高亮、右键菜单折行/
  冻结/重置）集中在 `renderers.ts` 的 `class ResultTable`，全部走 VTable 自己的
  option/event，`mountTable` 只是「建盒子 + 动态 import + `new ResultTable(...)`」的薄工厂。
- 表头拖拽换位：`dragHeaderMode: 'column'`（默认 `'none'`，=只开列表头）。VTable 要求
  **先选中表头单元格**才能拖动（`_canDragHeaderPosition` 里的 `isSelected` 判断）；冻结列
  相关行为 `frozenColDragHeaderMode` **保持默认**（即 `fixedFrozenCount`：冻结**数量**不变，
  冻的是哪几列随新顺序变），这样 `#frozen` 缓存不会失真。换位会重排布局与
  `options.columns`，所以**任何重建列数组的地方都必须按显示顺序重建**：`#toggleWrap` 用
  `#displayOrder()`（逐列读 `getHeaderField(col, 0)`）而不是查询结果的列序；
  `updateColumns` 会原样采用传入的数组，照查询序传就会把用户的换位撤销。
- **样式一律用官方主题**：`#theme()` 直接返回 `themes.DEFAULT` / `themes.DARK`（按
  `data-theme` 二选一），斑马底色、hover/选中配色、冻结列阴影、排序图标色都随主题而来，
  不要再按属性手写配色。两个坑：
  - `themes.of(partial)` 不以 DEFAULT 为父主题（`new TableTheme(p, p)`）：手写的部分主题
    不会与内置默认逐层合并，没写的属性会悄悄退回 `tools/global.js` 的硬编码常量
    （fontSize 16、padding [10,16,10,16]、黑边框、冻结列无阴影、排序图标近黑）。要叠加
    小改动就用官方的 `themes.DEFAULT.extends({...})`（`TableTheme.extends`）。
  - `selectionStyle.selectionFillMode` 默认 `'overlay'`，选中色是**盖在文字之上**画的 ——
    给成不透明色（如 `--ifm-color-emphasis-200`）会让选中格文字整块消失，表现为「一选中
    就变灰、内容不见」。官方主题给的是 `rgba(0,0,255,0.1)` 这类半透明色，用官方主题即可。
- 主题跟随 `data-theme`：canvas 用具体色绘制，CSS 变量改了不会进 canvas，故 `ResultTable`
  挂一个 `MutationObserver`（监听 root 的 `data-theme`/`class`）→ 变了才
  `table.updateTheme(...)`（`updateTheme` 会整表重绘，不要无条件调）。
- 排序只在**表头 sort 图标**上触发（循环 asc→desc→normal）。自定义比较器经
  `columns[].sort` 传入，VTable 会带着当前 `order` 调用它并**原样采用返回值**（不再像
  内置比较器那样自行按 order 翻转），所以方向要在比较器里处理；空**记录**由引擎兜底排后，
  但空**字段值**（NULL）得比较器自己管（本仓库选择恒排最后、desc 不翻转）。
- 复制：`keyboardOptions` **没有默认值**，Ctrl+C / Ctrl+A 必须显式写 `copySelected` /
  `selectAllOnCtrlA` 才生效；右键菜单的「复制单元格 / 复制整表」不走选区，是自己遍历
  `getCellRawValue` + `stringify` 拼 TSV 再 `navigator.clipboard.writeText`。
- 折行：右键「此列折行」把 field 记进 `#wrapped`，随即 `defaultRowHeight = 'auto'` +
  `updateColumns(cols, {clearColWidthCache:false, clearRowHeightCache:true})` —— 只清行高
  缓存让行重新长高，保留用户拖过的列宽与行高（`updateColumns` 不动 `sortState`）。
  「重置视图」改用 `updateOption(..., {clearColWidthCache:true, clearRowHeightCache:true})`，
  它连带清排序状态与拖拽尺寸。
- 右键菜单点击监听的是 **`dropdown_menu_click`，不是 `context_menu_click`**：1.26.8 里
  `context_menu_click` 只有常量、从不触发，html 菜单项的 click 处理器发的是
  `dropdown_menu_click`（`menuKey = menuItem.menuKey || menuItem.text`，故每项都显式给
  `MENU.*` key 以与本地化文案解耦）。`getCellInfo(col,row).field` 对表体单元格也返回所属
  列 field，故菜单项对表头/表体都能定位到列。
- `menu`/`tooltip` 的 `parentElement` 默认是 `table.getElement()`（在 `.dfk-sql-table` 内），
  故不与全屏 overlay 抢 z-index。它们的 `renderMode` 默认 `html`，vendor 自己会注入一份
  **写死浅色（#fff/#000、Roboto）**的文档级样式表，所以暗色模式下菜单/提示仍是浅色 ——
  这是 vendor 现状，**不要用 `sql.css` 去改它的配色**；真要隔离就改走 Shadow DOM，但要
  注意 VTable 的样式表注入在 `document.head`，跨不过 shadow 边界（需把那段 CSS 复制进
  shadow root 才能生效）。

**扩展加载（`runtime.ts`）**

- **wasm 上 `INSTALL` 是空操作**（没有可安装的持久存储），只有 `LOAD` 真的 fetch
  `.duckdb_extension.wasm`、验签、加载；`INSTALL … FROM` 只记录「这个扩展以后从哪
  拉」。所以按名加载 =（可选 `INSTALL <name> FROM <community|'url'>`）+ `LOAD <name>`；
  `{url}` 条目 = 直接 `LOAD '<绝对URL>'`（worker 基于 blob URL，解析不了相对路径，
  站内路径由主线程先拼成绝对 URL）。**不要用 `SET custom_extension_repository`**：
  那是全局状态，会污染之后所有扩展的加载。
- 扩展名 / 仓库 / URL 是**白名单校验**而非转义——它们直接进 SQL 文本；仓库关键字
  （`community` / `core`）裸拼，URL 加引号。
- 按 `${repository}\0${name}`（或 `url\0<url>`）记忆化（成功与 in-flight 都记），
  失败时从表里删掉以便重试；**已加载集合全页共享**，与「每个块状态独立」不冲突。
- **页面进入即预热**：`DfkSql.connectedCallback()` 顺手 `init()`（失败静默——
  `init()` 可重试，最终由点 Run 的块把错误显示出来），所以第一个点 Run 的人不用等
  DuckDB 下载与预加载；没有可运行块的页面（如首页）不会触发。
- `allowUnsignedExtensions` **只由第一个 `init()` 决定**：配置在 `open()` 时一次性交给
  worker，之后改不了。站点级值（注入配置）与块级值在创建实例前合并，谁先到谁定调。
- 文件名的契约：文件名**第一个 `.` 之前必须是扩展名**（wasm 用它拼 `<name>_init_c_api`
  入口符号），所以 release 资产 `duckfn-wasm_eh.duckdb_extension.wasm` 落盘时
  必须改名为 `duckfn.duckdb_extension.wasm`；`runtimeConfig` 两侧都做校验兜底。

**扩展预加载（`sql/extensions.ts` + `runtimeConfig.ts`）**

- 站点在 `docusaurus.config.ts` 用 `dfkExtensions({preload, allowUnsignedExtensions})` 配
  一条**有序**列表；插件规范化后注入每个页面的
  `<script id="dfk-sql-runtime" type="application/json">`，`runtime.ts` 首次 `init()`
  读一次，按序加载完才置 `ready`。同一次 `getClientModules()` 还把 `sql/client`
  （注册 `dfk-*` 元素）注入每个页面——docs 页面从不 import 本包的 React 树，元素
  注册必须从客户端模块来，所以站点不再需要自备 clientModules 文件。TOC 折叠的胶水
  同理，由 `dfkTocToggle()`（`toc-toggle/plugin`）注入 `toc-toggle/client`；两个插件
  都从**站点**（`createRequire(siteDir/package.json)`）解析自己 `dist/` 下的 client
  入口，所以站点打包 config 也不会断。
- 三种来源：裸扩展名（官方仓库）、`{name, repository}`（`community` / `core` / URL）、
  `{url}`（同源静态文件或绝对 URL）。
- `{url}` 带 `release` 时，插件在 dev/build 启动时从 GitHub **最新** release 拉取该资产
  到 `static/<url>`：本地缓存（默认 `<siteDir>/.cache/duckfn-docs-kit`）+ sha256
  sidecar，与 release 的 `digest` 相同就跳过下载；网络失败时有缓存则降级为警告
  （CI 每次全新环境、无缓存，会直接失败），资产名对不上时报错并列出可用名。
- 注入路径用 `context.siteConfig.baseUrl` 拼（多语言构建时它是本地化值；Docusaurus 会把
  `static/` 拷进每个 locale 的 outDir，天然自洽）；站内相对路径由 `runtime.ts` 在
  浏览器里解析成绝对 URL。
- `runtimeConfig.ts` 是两侧唯一共享契约：无任何 import 的纯类型 + 校验函数，Node 插件与
  浏览器 bundle 都不会把对方拖进来；Docusaurus 插件 API 用**结构化类型**，本包不依赖
  `@docusaurus/types`。
- **版本耦合（改动前先读回）**：wasm 扩展只能由「与 duckdb-wasm 内置 DuckDB 版本 ABI
  兼容」的构建提供，所以 kit 的 `package.json` 把 `@duckdb/duckdb-wasm` 固定成**精确
  版本**（当前 `1.33.1-dev64.0`，内置 v1.5.5，与 CI 的 `TARGET_DUCKDB_VERSION` 一致）。
  实测：`1.32.0`（内置 v1.4.3）拒绝 v1.5.5 构建的扩展（C API slot 数 459 vs 546，
  报 `C extension API layout mismatch`）；`1.33.1-dev57.0`（内置 v1.5.4）反而能加载
  ——wasm 补丁只校验 C API slot 数（1.5.4/1.5.5 的 unstable 区未变），原生则按版本
  戳严格校验（1.5.4 的原生 duckdb 会拒绝 v1.5.5 构建的扩展）。升级 duckdb-wasm 或
  改 CI 的 `duckdb_version` 时必须成对验证（跑一遍可运行 SQL 页的两个示例块即可）。

**文档站 SQL 测试（`sql/verify` + `sql/collect` / `sql/browserRunner` / `sql/harness`）**

- 入口是 `bin/sql-verify.mjs`（bin 名 `duckfn-sql-verify`），它 import `dist/sql/verify.js`。
  bin **手写**、不进构建：Vite lib 产物是 ESM、不保留 shebang，而 npm 只需要一个带 shebang
  且有执行位的文件。站点侧接成 `npm test` 即可（本仓库见 `docs/package.json`）。
- 收集与渲染共用一份 meta 解析（`sql/remark.ts` 导出的 `parseRunnableSqlMeta`）：站点上不是
  可运行块的，测试也不会跑。
- 围栏按 CommonMark 收口：闭合围栏同字符、不短于开启围栏、且无 info string —— 这样
  ````md 包着的 ```sql 示例（`runnable-sql.md` 就这么展示 meta）不会被当成块。
- 执行环境是 **真实浏览器里的 DuckDB-Wasm**（不再是旧的 Node worker）。`sql/browserRunner.ts` 用
  **`playwright-core`** 驱动浏览器 —— 协议、连接/导航/evaluate、自动等待、超时与崩溃处理都交给成熟的
  Playwright，而不是自己手写一个半吊子驱动。选 `playwright-core` 而非 `playwright`：前者**不会自动下浏览器**，
  配 `chromium.launch({executablePath})` 拉系统 Chrome/Edge，止住离线。找浏览器：`--browser <path>` /
  环境变量 `DFK_BROWSER` 覆盖，否则按已知路径探测。`playwright-core` 在 `vite.config.ts` 里列为 external。
- **全离线**：`browserRunner` 起一个 loopback 静态服务，把 `node_modules/@duckdb/duckdb-wasm/dist`
  里的引擎（`duckdb-*.wasm` + `duckdb-browser-*.worker.js`）、构建产物 `dist/sql/harness.js`、以及
  `--extension` 指定的扩展文件按同源 URL 供出去。浏览器里 `LOAD` 同源 http URL 正常，不再有旧方案的
  80 端口 / `~/.duckdb` 暂存目录约束（那是 Node worker 才有的、把端口嵌进 Windows 暂存路径的问题）。
- **harness 复用站点运行时**：`sql/harness.ts` 是一个极简单页，直接 import `sql/runtime.ts` 的
  `DuckDBRuntime`，只是经 `init({bundle})` 把引擎来源从 jsDelivr CDN 换成本地 serve（`runtime.ts`
  为此新增了 `bundle` 选项：给定时跳过 `selectBundle`、同源直接 `new Worker`）。它把 `window.__dfkReady`
  /`__dfkRun`（行数/列数，给校验器）与 `__dfkQuery`（真实行，给 vfs 探针）暴露给 Playwright 的 `page.evaluate`。harness 由独立的
  `vite.harness.config.ts` 打成自包含单文件 `dist/sql/harness.js`（这份构建里 duckdb-wasm **不打** external，
  因为它是给浏览器直接加载、下游无 bundler 的产物）；`build` 脚本在主 lib 构建后串跑它。
- 形态与页面一致：**每页新实例 + 页内共用连接**（`newPage()` 重新导航 harness → 新文档 = 新
  `AsyncDuckDB`；页内块共用其连接，所以能依赖前一个块建的宏/表，页与页隔离）。
- 故意失败的块在 meta 里声明 `"expect": "error"`（`expectsError(config)` 读它，默认 `ok`）：
  期望是数据，不能靠对注释做字符串匹配；校验**双向** —— 声明会失败却跑成功同样要报出来。

## 代码风格（硬性要求）

这些规则是本包存在的意义所在，评审时逐条对照。

### 0. 设计目标：保留模式 UI，不是 mini React / mini Lit

**明确禁止** React / Vue / Lit 风格的「状态 → render → 重建 DOM」模型。
本包的目标是：

> 用 TypeScript 对浏览器原生 DOM API 做面向对象封装。

组件应该表现得像一个**持有内部 DOM 对象的普通类**，而不是一个「根据 state
不断重新计算 UI」的函数。冲突时按下面的优先级裁决（上面的赢）：

1. 保留 DOM 节点 —— 节点长期存在，不随状态重建
2. 类字段持有 DOM 引用
3. 直接修改 DOM（property / attribute / text / class / 事件监听）
4. 局部更新
5. 必要时才替换**局部集合**
6. 禁止整组件 re-render
7. 禁止用响应式 state 驱动 render
8. 禁止引入 Lit / React / Vue 等 UI runtime 或其设计模式

### 1. 禁止拼 HTML 字符串

不许 `innerHTML = \`...\``、不许 `insertAdjacentHTML`、不许任何「模板字符串
生成标记再塞进 DOM」的写法。

```ts
// 禁止
this.innerHTML = `<button class="x">${label}</button>`;
```

### 2. DOM 用对象操作，字段持有引用

`document.createElement` 创建、**类的字段持有引用**、直接对字段调方法。

```ts
readonly #run: HTMLButtonElement;
readonly #output: HTMLDivElement;

constructor() {
  super();
  this.#run = document.createElement('button');
  this.#run.textContent = 'Run';
  this.#run.addEventListener('click', () => this.#onRun());
  this.#output = document.createElement('div');
}
```

**`querySelector` 不得当作组件内部的状态管理方式**。需要反复访问的节点一律
存成字段；`querySelector` 只允许出现在「从外部挂载点找目标」（如
`TocToggle.ts` 里找 `.theme-doc-toc-desktop`）这类不属于组件自身结构的地方。

纯结构节点用 `el()`（`src/dom.ts`）建，不必展开成 `createElement` + 逐行赋值。它的
options 按标签收窄，直接写标签自己的属性；`class` / `text` 是 `className` /
`textContent` 的简写，`aria-*` / `data-*` 以及没有同名属性的自定义元素属性走 `attrs`
兜底。属性名拼错、值类型不对、把 `style` / `dataset` / 方法名塞进去，都是编译错误：

```ts
readonly #logo = el('img', {class: 'dfk-logo', alt: '', width: 480, height: 480});
```

第三个参数是可选的 `init` 回调，用来**就地描述不会再被单独引用的结构**，省掉一个
字段；需要反复访问的节点仍然存成字段：

```ts
this.root.append(
  el('span', {class: 'dfk-next-card-body'}, (body) =>
    body.append(this.#title, this.#details),
  ),
  this.#arrow,
);
```

`init` 里不要读**声明顺序在其后**的 `#字段`（字段初始化器按声明顺序求值，会踩 TDZ）：
在构造函数里传 `init` 最稳妥，构造函数执行时所有字段都已初始化。

### 3. 更新是局部、直接、明确的；不得有通用刷新机制

状态变化 = 改**相关的那几个**节点，用命名的领域 setter 表达，而不是把整个 UI
当成 `data` 的函数重新算一遍。

```ts
setResult(value: string): void {
  this.#output.textContent = value;
}

setLoading(value: boolean): void {
  this.#run.disabled = value;
  this.#output.hidden = value;
}
```

改哪一处用**标准 DOM API** 直说，不要绕：`textContent`、`setAttribute` /
`removeAttribute`、`classList.add/remove/toggle`、`hidden`、`disabled`、`value`，
以及 `href` / `src` 这类直接赋值的属性。除此之外不要再找第三种写法。

**必须避免**的形态（贴出来是为了让评审一眼对上号）：

```ts
set data(value) { this.#data = value; this.render(); }        // 响应式 state 驱动 render
update(data) { this.replaceChildren(); this.render(data); }   // 用替换子树模拟更新
render(data) { /* 根据 data 重新构建整个 DOM */ }              // 通用刷新机制
rebuild() { this.replaceChildren(); this.build(); }           // 同上，换个名字而已
```

不允许实现 `render()` / `rebuild()` / `rerender()` 之类的**通用**刷新机制；
不允许因为一个字段变化就重建整个子树；不允许通过「替换子树」模拟响应式更新。

### 4. `replaceChildren()` 的分级用法

`replaceChildren()` 本身不是禁品，禁的是把它当**整组件的通用 render 机制**：

- 允许：**确实需要整体更新（或一次性静态组装）的局部集合**，例如卡片网格
  一次给出全新条目、按钮的「图标 + 文本」这种固定组合。
- 禁止：拿它清空组件自己的根子树再重建 —— 那是第 3 条里的 `rebuild()`。

变长列表能增量就增量：按数据长度**增删条目**、复用已有条目对象
（`DfkHero.ts` 的 `setBadges()` 就是这么做的），只有条目语义整体失效时才整批替换。

### 5. 内容入口：命名的领域 setter（本包已统一）

`dfk-*` 元素**不接受整份 data blob，也没有通用的 render/apply 入口**。每个组件
暴露一组命名的领域 setter，一个方法只碰它负责的那部分节点：

```ts
hero.setTitle(text);
hero.setTagline(text);
hero.setPrimaryAction({label, href});
hero.setBadges(badges);

features.setSectionTitle(text);
features.setFeatures(items);
```

- setter 只做 mutation；变长集合（badge 行、卡片网格）按新长度增删条目、复用
  已有条目对象，不整体重建。
- setter 只写自己持有的节点，所以**在元素尚未连接、甚至尚未插入文档时调用也安全**。
  消费方因此不必关心 `connectedCallback()` 的时序，可以乱序批量喂数据。
- **不做 attribute reflection**：组件不读属性、也不把 setter 的值镜像回 attribute
  （内容不是可序列化的标记，唯一例外是自有节点上 `iconify-icon` 的 `icon` 属性）。
  内容入口只有 setter。
- 新增字段就加一个对应的 setter，**不要**回头引入 `setData()` / `update()` /
  `apply()` 这类通用入口。
- 改 setter 签名属于公开 API 变更，需同步 `docs/src/pages/index.tsx` 的
  `mount*()` 助手、`src/index.ts` 的类型导出，以及下游消费方。
- **例外（attribute 种子）**：`<dfk-sql>` 由 `sql/remark` 插件从 Markdown 自动
  生成为 `<dfk-sql config="…" sql="…">`（外加一份预渲染的加载占位，见「加载占位」），
  没有 `mount*()` 助手、也挂不上 ref
  去调 setter —— 内容只能来自 `config` / `sql` 两个字符串属性。因此它在
  `connectedCallback()` 里 `getAttribute` **各读一次**作初始种子。这与「不做
  attribute reflection」不冲突：读一次用于初始化，不是 attribute 变化再驱动
  重渲染，仍是保留模式。新增同类「由构建期插件生成、无 React 挂载点」的元素
  才可套用此例外，手写 JSX 的元素仍走 setter。

### 6. 生命周期：构造函数建树 + 挂 shadow root

优先在 `constructor()` 里创建静态结构、`attachShadow` 并组装完毕；
`connectedCallback()` 只做**必要的一次性初始化**（启动监听、注册外部资源）。
不要把 `connectedCallback() → rebuild() → build()` 当成标准渲染生命周期。

**Custom Elements 规范限制（必须遵守）**：构造函数里**不得给 `this`
加属性或子节点**（`this.append(...)`、`this.setAttribute(...)` 都不行），
否则 `document.createElement()` / `innerHTML` 解析创建的元素会抛错。
`attachShadow()` 不在禁止之列，所以正确拆法是：构造时把结构挂进 shadow root，
light DOM 始终空着。

```ts
constructor() {
  super();
  this.#section = document.createElement('section');
  this.#section.append(this.#title, this.#body); // 组装进自有节点
  const shadow = this.attachShadow({mode: 'open'}); // 构造函数里合法
  shadow.adoptedStyleSheets = [homeStyles()];
  shadow.appendChild(this.#section);
}
```

组件因此**从诞生那一刻起结构就完整**，setter 在元素连接前调用也安全，
`connectedCallback()` 不再承担「挂载」职责。

**监听器按对象归属决定挂在哪**：

- 挂在**自有节点**上（`this.#primary.addEventListener('click', …)`）→ 写在
  `constructor()` 里，节点与元素同生命周期，不需要清理，也不需要重挂。
- 挂在**外部对象**上（`document` / `window` / `matchMedia`）→ 必须在
  `disconnectedCallback()` 里 `removeEventListener` / `removeListener`，且注册要有
  幂等标志 —— 元素被移动或 React 重挂载会再次触发 `connectedCallback()`，重复注册
  会让回调执行多次。

### 7. 尽量类化，但没有「组件基类」

一个组件一个 class：web component 直接 `extends HTMLElementBase`（`src/dom.ts`），
**`home/` 下不存在共享的组件基类**。每个类自带字段、constructor 组装并挂进自己
的 shadow root、自己的 setter，不做 `create()` / `apply()` 之类的模板
方法抽象 —— 那种抽象会把「状态驱动刷新」重新引进来，正是第 0、3 条要避免的。

非元素的小部件（`TocToggle`、卡片类 `DfkFeatureCard` / `DfkNextStepCard`）同样
是 class。函数式导出只留给真正的纯工具（`dom.ts` 的 `el()`）。

### 8. 图标一律用官方 `<iconify-icon>` web component

npm 包 `iconify-icon`，`register.ts` 里 side-effect import 注册。
`document.createElement('iconify-icon')` + `setAttribute('icon',
'lucide:sparkles')` 即可，字形按需从 Iconify 公共 API 加载。

- **不要**自己封装 SVG（不手写路径、不拼 `data:image/svg+xml`、不做 CSS mask
  助手）—— 官方的实现比自研封装好。
- 数据契约里图标就是 Iconify 名字字符串（`'lucide:arrow-right'`），不引入
  `@iconify/types`、不装 `@iconify-icons/*` 本地图标集。
- 尺寸/颜色用 CSS 作用在 `iconify-icon` 元素上（`home.css` 的
  `.dfk-button-icon` 等），字形自带 `currentColor`。
- **尺寸只认 `font-size`**：组件内部渲染的是 `<svg width="1em" height="1em">`，
  字形大小跟随宿主的 font-size；给宿主设 CSS `width`/`height` 只会撑大空盒子，
  图标本身不变。

### 9. 默认用 Shadow DOM

`dfk-*` 组件**默认渲染进 shadow root**（`mode: 'open'`），样式用
`adoptedStyleSheets` 注入（`home/styles.ts` 把 `home.css` 以 `?inline` 内联进
bundle，全局共享一个 `CSSStyleSheet`）。这样宿主页面的全局 CSS 进不来、组件的
CSS 也漏不出去，组件边界干净，不依赖「人工命名空间」去避免污染。

**主题照样跟随宿主**：CSS 自定义属性会**继承穿过 shadow 边界**，所以组件内部
用 `var(--duckfn-*)` / `var(--ifm-*)` 即可拿到消费站的 Infima 变量与品牌色，
`[data-theme]` 切换自动生效 —— 前提是这些变量声明在**文档的** `:root` /
`[data-theme]` 上（这正是 `tokens.css` 必须留在全局、不能塞进 shadow 的原因）。
组件内部**不要**写 `[data-theme]` 选择器，也不要依赖宿主的 class。

**只有「继承属性」能穿过边界**：`box-sizing` 不是继承属性，宿主 Infima 的
`* { box-sizing: border-box }` 选不进 shadow tree，组件内所有盒子会退回
`content-box`，带 padding / max-width 的盒子尺寸随之变化 —— 足以把布局阈值挪位
（实测：feature grid 在 72rem 容器上限处从 3 列变 4 列）。所以 `home.css` 顶部
必须在 shadow 作用域里**重新声明一次** box-sizing 重置，别指望宿主的通用规则。

**例外（万不得已才退回 light DOM）**：仅当组件必须直接复用消费站 light DOM 的
CSS 时 —— 例如 `TocToggle` 注入并改写 Docusaurus 自己的 TOC、其规则必须落在
`@layer docusaurus.theme-classic` 里 —— 才不用 shadow root。这种组件的类名一律
`dfk-` 前缀（或 `toc-` 这类自有前缀），避免与宿主撞名。新增例外要在评审时说清楚
「依赖了宿主的哪条规则」。

`<dfk-sql>` 是**混合**形态，且 light-DOM 部分只剩一件事：CodeMirror 编辑器与那簇悬浮
图标按钮**全在 shadow root 里**（编辑器不再 slot，因为 style-mod 会把 `.cm-*` 基础主题
以 `adoptedStyleSheets` 挂到 `getRoot()` 解析出的根上 —— 编辑器在 shadow 里，解析出的
就是同一个 shadow root，样式正好落在用它的那棵树里；反过来把编辑器放 light DOM、样式
却落进 shadow root，就是第一阶段那个「编辑器没样式」的 bug）。只有 **VTable 结果容器**
在 light DOM（`slot="dfk-result"`），因为 VTable 往**文档级**注入样式表，shadow 边界
挡得住它。其 light-DOM 样式（`.dfk-sql-result` 一族）走 `sql/sql.css` → `kit.css` 的全局
通道，同样全部 `dfk-sql-` 前缀。

这条也是第 11 条「light DOM 恒为空」的例外之所以安全的原因：编辑器在构造函数里就挂进
shadow root，light DOM 唯一的节点（结果容器）只在**用户点「执行」之后**才创建，
hydration 早已完成。

### 10. SSR 安全

Docusaurus 预渲染在 Node 里 import 本包。

- 继承 `HTMLElement` 的类必须 `extends HTMLElementBase`（`src/dom.ts`，Node 下
  回退为空基类），否则模块求值直接崩。
- **类字段初始化器不要碰 `document`**：Node 下类体只被求值、不实例化，所以
  字段初始化器安全的前提是「服务端永远不会 new 这个类」。目前正是如此，
  浏览器侧由元素 upgrade（即 `constructor()`）触达。
- 触碰 `window` / `document` / `customElements` 的入口（`registerDfkElements()`、
  `TocToggle.init()`）要么带守卫，要么由消费方在浏览器环境调用。
- `styles.ts` 的 `CSSStyleSheet` 必须**惰性创建**（`homeStyles()` 在构造函数里
  才调用）：模块级 `new CSSStyleSheet()` 会在 Node 预渲染 import 时直接崩。
  `?inline` import 进来的只是字符串，模块级安全。
- `iconify-icon` 在 Node 里 import 是安全的（官方包已处理）。
- Node 侧模块只有 `src/remark.ts`、`src/sql/remark.ts`、`src/sql/extensions.ts`（构建期）
  与 `src/sql/collect.ts`、`src/sql/browserRunner.ts`、`src/sql/verify.ts`（测试期），
  连同无依赖的共享契约 `src/sql/runtimeConfig.ts`；它们都不得 import 任何浏览器模块。

### 11. React 19 自定义元素

JSX 类型增强写在 `src/index.ts` 的
`declare module 'react' { namespace JSX { … } }`，新增元素时同步补上。
React 19 的 SSR/hydration 不会把对象 prop 设到自定义元素上（对象无法序列化进
预渲染 HTML），docs 侧用 callback ref 显式喂内容，见
`docs/src/pages/index.tsx`。

三点因此而来的约定：

- **`mount*()` 助手必须是纯 setter 调用（幂等）**。React 可能对同一节点多次调用
  ref（StrictMode 双调用、元素移动后重挂载），重复喂同样的数据必须无副作用。
- **预渲染 HTML 里 `<dfk-*>` 是空壳，这是已知取舍**：结构挂在 shadow root 里，
  light DOM 始终为空，所以外壳有、内容没有，hydration 之前那几块是空的
  （未定义的自定义元素默认 `display: inline`，高度为 0）。这个一次性高度跳变是刻意
  接受的代价。不要为此把结构改回「预渲染时也能拼出来」的写法 —— 那必然退回拼字符串
  或响应式 render。
- **light DOM 恒为空顺带消除了 hydration mismatch**：以前 `connectedCallback()`
  往 light DOM 挂子树，React hydration 比对子节点数量对不上，会报一次可恢复的
  mismatch（minified #418）。现在服务端 HTML 与客户端元素的 light DOM 都是空的，
  比对一致。若控制台再出现 #418 指向 `dfk-*`，说明有代码把节点挂回了 light DOM，
  那是 bug，要修原因而不是用 `suppressHydrationWarning` 掩盖。

## 发版流程（npm）

发布的是 `duckfn-docs-kit` 这一个包，流程比 Rust 侧短：**切版本 → 提交并打 tag →
`npm publish` → 切下一开发版本**，没有远程流水线要等（`docs-kit-v*` 不匹配任何 workflow
的 tag 过滤器，理由见第 2 步）。

命令都在根目录的 `Justfile` 里（`just --list` 可查），实现是
`scripts/release-docs-kit.sh`。它与 `scripts/release.sh`（两个 crate 的发版）分开：两条流程
各打各的 tag，也互不触发对方的 CI。

版本号形如 `X.Y.Z`（例如 `0.1.0`），tag 形如 `docs-kit-v0.1.0`。只有**正式版本**才打 tag、
才发 npm；`0.1.1-dev.0` 这类开发版本留在分支上。

| 步骤 | 命令 |
| --- | --- |
| 0. 前置检查 | `just release_kit_check` |
| 1. 提升版本号 | `just release_kit_bump 0.1.1` |
| 2. 提交并打 tag | `git commit …` 后 `just release_kit_tag 0.1.1` |
| 3. 发布 npm 包 | `just release_kit_publish`（先 `npm login`） |
| 4. 切开发版本 | `just release_kit_dev 0.1.2-dev.0` |

### 0. 前置检查

```bash
just release_kit_check   # 构建 + 类型检查 + npm pack --dry-run
npm whoami               # 没登录先 npm login
```

`npm pack --dry-run` 打印 tarball 的文件清单，是发布前最值得看的一项：预期是 `dist/**` +
`src/**` + `bin/**` + `AGENTS.md` + `README.md` + `LICENSE` + `package.json`（当前 75 个文件）。
多出别的东西时先查 `files` 白名单，不要靠 `.npmignore` 追着排除。

### 1. 提升版本号

```bash
just release_kit_bump 0.1.1
```

脚本把版本号从工作区的开发版本（如 `0.1.1-dev.0`）切成正式版本，只改两个文件：
`duckfn-docs-kit/package.json`，以及根 `package-lock.json` 里该 workspace 的条目；然后打印
残留的旧版本号（应为空）与 `git diff --stat`。

**不要用 `npm version` / `npm install` 改版本号**：它们会顺手 reify 整个 workspace、触发对
registry 的 fetch（本机被 `EALLOWREMOTE` 拦下），结果是版本号改了、lockfile 没动。脚本直接
重写这两处 JSON —— 两个文件的既有格式就是 2 空格缩进 + LF，重写是幂等的。

### 2. 提交并打 tag

```bash
git add -A
git commit -m "chore(release-kit): 发布 duckfn-docs-kit v0.1.1"
just release_kit_tag 0.1.1     # 打 docs-kit-v0.1.1，推送 main 与 tag
```

`docs-kit-v*` **故意**不匹配 `MainDistributionPipeline.yml` 的 tag 过滤器（只认 `v*.*.*`）：推这个
tag 不构建扩展，也就不会触发文档站（`DeployDocs.yml` 现在监听的是那条流水线跑完，而不是 tag 本身）。
所以 kit 发版不必等流水线，文档站也仍然只在 crate 发版时更新。

### 3. 发布到 npm

```bash
npm login                 # 只需一次；启用了 2FA 的话发布时会要 OTP
just release_kit_publish  # 先跑 release_kit_guard，再 npm publish -w duckfn-docs-kit
```

`release_kit_guard` 在上传之前拦四种情况：版本号还是开发版本（npm 会把预发布版本也挂到
`latest` 上）、工作区不干净、本地没有对应的 `docs-kit-v*` tag、该版本在 npm 上已存在
（同一版本不能覆盖，只能发新版本）。只想打包不上传，用 `just release_kit_publish_dry`。

发布后核对：

```bash
npm view duckfn-docs-kit version
```

### 4. 切到下一开发版本

```bash
just release_kit_dev 0.1.2-dev.0
```

只动同样的两个文件，不打 tag、不发布。开发版本不能被 `npm publish` 直接发出去 ——
`release_kit_guard` 会拒绝，这正是它存在的意义。

### 发布内容

包内容 = `files` 白名单 + npm 的固定规则，所以**不需要**在包的结构之外维护清单：

- `files: ["dist", "src", "bin", "AGENTS.md"]`：`dist` 是构建产物；`src` 必须带上（CSS 子路径
  `duckfn-docs-kit/src/kit.css` 直接指向源文件）；`bin` 是 `duckfn-sql-verify` 的入口（它 import
  `dist/`，所以两者都要在包里）；`AGENTS.md` 是**面向下游的用法说明**（含给 AI 的契约速查），随包
  发布 —— 本文件（开发约定）刻意不进包，下游读者要的是用法，不是内部实现约束。
- `README.md`、`LICENSE`、`package.json` 由 npm 自动收录：本包目录下有自己的 `LICENSE`
  （根目录那份不会被带进来），README 是 npm 包页的正文。
- `dist/` 在 `.gitignore` 里，但照样进包 —— `files` 白名单优先于 gitignore。`prepack` 脚本
  会在 `npm pack` / `npm publish` 之前重跑 `npm run build`，所以发布用的产物总是当前源码
  构建出来的，不依赖上一次留下的 `dist/`。

## 其它约定

- 文本文件一律 LF（仓库根 AGENTS.md 有替换命令）。
- 注释解释「为什么」，与 docs/ 现有风格一致；本包面向国际下游用户，注释用英文。
- `exports`/`files` 已按可发布形态维护，**不要改结构**：新增 / 移动模块只改
  `vite.config.ts` 的 `entry`，通配映射自己会跟上。
- `README.md` 与 `docs/docs/docs-kit/**` 是两份正文，面向的读者不同：前者给在 npm 上直接看
  这个包的人（英文），后者是本仓库文档站的用户指南。改公开 API 时两边都要看。
- 本包已发布到 npm（`duckfn-docs-kit`），`private` 与 `publishConfig` 的处理见上面的发版流程。