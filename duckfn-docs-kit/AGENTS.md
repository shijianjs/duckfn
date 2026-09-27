# duckfn-docs-kit — AGENTS.md

本包是 duckfn 系（DuckDB 扩展）文档站的共享积木库：TOC 折叠控件、首页
web components、品牌 CSS tokens、版本占位符 remark 插件。它被 `docs/`
（本仓库文档站）以 npm workspace 消费，也供下游扩展项目复用 ——
**发布通用逻辑，而不是到处拷代码**。

## 包结构 / 导出

- 构建：Vite lib 模式产出 ESM（`dist/`），`tsc -p tsconfig.build.json` 产出
  `.d.ts`。`npm run build` 一步完成。CSS 不经构建，作为源文件直接导出。
- `exports` 用通配模式（`./* → ./dist/*.js`），新增运行时入口只需在
  `vite.config.ts` 的 `entry` 里加一行，不必改 `package.json`。入口文件名与
  其主导出的类名一致（大驼峰），ts 源文件同理：
  - `duckfn-docs-kit`（浏览器：`index.ts` 桶文件，CE + 类型）
  - `duckfn-docs-kit/TocToggle`（浏览器：TocToggle 类）
  - `duckfn-docs-kit/remark`（**Node 构建期**：remark 插件）
- CSS 子路径：`./css/*` 直接映射到 `src/css/`，如 `css/kit.css`（聚合）、
  `css/tokens.css`、`css/toc-toggle.css`、`css/home.css`，下游在 Docusaurus
  的 CSS 管线里 `@import`。

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

### 6. 生命周期：构造函数建树优先

优先在 `constructor()` 里创建静态结构并组装；`connectedCallback()` 只做
**必要的一次性初始化**（挂载、启动监听、注册外部资源），并设明确的初始化标志。
不要把 `connectedCallback() → rebuild() → build()` 当成标准渲染生命周期。

**Custom Elements 规范限制（必须遵守）**：构造函数里**不得给 `this`
加属性或子节点**（`this.append(...)`、`this.setAttribute(...)` 都不行），
否则 `document.createElement()` / `innerHTML` 解析创建的元素会抛错。
正确拆法是：

```ts
constructor() {
  super();
  this.#section = document.createElement('section'); // 自有的中间节点
  this.#section.append(this.#title, this.#body);     // 组装进中间节点，不碰 this
}

connectedCallback() {
  if (this.#attached) return;   // 一次性：React 重挂载 / 元素移动会再次触发
  this.#attached = true;
  this.append(this.#section);   // 只在这里把结构挂到 this 上
}
```

**监听器按对象归属决定挂在哪**：

- 挂在**自有节点**上（`this.#primary.addEventListener('click', …)`）→ 写在
  `constructor()` 里，节点与元素同生命周期，不需要清理，也不需要重挂。
- 挂在**外部对象**上（`document` / `window` / `matchMedia`）→ 必须在
  `disconnectedCallback()` 里 `removeEventListener` / `removeListener`，且注册要有
  幂等标志 —— 元素被移动或 React 重挂载会再次触发 `connectedCallback()`，重复注册
  会让回调执行多次。

### 7. 尽量类化，但没有「组件基类」

一个组件一个 class：web component 直接 `extends HTMLElementBase`（`src/dom.ts`），
**`elements/` 下不存在共享的组件基类**。每个类自带字段、constructor 组装、
connectedCallback 挂载与自己的 setter，不做 `create()` / `apply()` 之类的模板
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

### 9. 不用 Shadow DOM

组件渲染进 light DOM（全局 `dfk-` 前缀类名），这样才能看见消费站的 Infima
变量、`[data-theme]` 与 `@layer docusaurus.theme-classic` 级联。
新增类名一律 `dfk-` 前缀。

### 10. SSR 安全

Docusaurus 预渲染在 Node 里 import 本包。

- 继承 `HTMLElement` 的类必须 `extends HTMLElementBase`（`src/dom.ts`，Node 下
  回退为空基类），否则模块求值直接崩。
- **类字段初始化器不要碰 `document`**：Node 下类体只被求值、不实例化，所以
  字段初始化器安全的前提是「服务端永远不会 new 这个类」。目前正是如此，
  浏览器侧由 `connectedCallback` 触达。
- 触碰 `window` / `document` / `customElements` 的入口（`registerDfkElements()`、
  `TocToggle.init()`）要么带守卫，要么由消费方在浏览器环境调用。
- `iconify-icon` 在 Node 里 import 是安全的（官方包已处理）。
- `src/remark.ts` 是唯一允许在 Node 构建期跑的模块，它不得 import 任何浏览器模块。

### 11. React 19 自定义元素

JSX 类型增强写在 `src/index.ts` 的
`declare module 'react' { namespace JSX { … } }`，新增元素时同步补上。
React 19 的 SSR/hydration 不会把对象 prop 设到自定义元素上（对象无法序列化进
预渲染 HTML），docs 侧用 callback ref 显式喂内容，见
`docs/src/pages/index.tsx`。

三点因此而来的约定：

- **`mount*()` 助手必须是纯 setter 调用（幂等）**。React 可能对同一节点多次调用
  ref（StrictMode 双调用、元素移动后重挂载），重复喂同样的数据必须无副作用。
- **预渲染 HTML 里 `<dfk-*>` 是空壳，这是已知取舍**：结构只在浏览器侧
  `connectedCallback()` 里挂上去，所以外壳有、内容没有，hydration 之前那几块是空的
  （未定义的自定义元素默认 `display: inline`，高度为 0）。这个一次性高度跳变是刻意
  接受的代价。不要为此把结构改回「预渲染时也能拼出来」的写法 —— 那必然退回拼字符串
  或响应式 render。
- **上面这条的副作用是首页会报一次 React 可恢复的 hydration mismatch**（minified
  #418，`onRecoverableError`，指向 `dfk-hero`）。服务端 HTML 的 `<dfk-*>` 是空的，
  而 hydration 时元素早已 upgrade 并挂好了子树，React 比对子节点数量发现对不上，
  于是把该子树改为客户端重建。**这是同一条取舍的必然结果，不是 bug**：控制台里
  看到它可以忽略，但不要用 `suppressHydrationWarning` 之类的补丁去「修」它 ——
  那只会掩盖原因。想彻底消除，只能放弃保留模式（回到字符串或响应式 render）。

## 其它约定

- 文本文件一律 LF（仓库根 AGENTS.md 有替换命令）。
- 注释解释「为什么」，与 docs/ 现有风格一致；本包面向国际下游用户，注释用英文。
- `private: true` 只是暂不发 npm；`exports`/`files` 已按可发布形态维护，
  解禁时去掉 `private` 补 `publishConfig` 即可，不要改结构。