# 新建 duckfn-docs-kit（npm + TS + Vite）并让 docs 依赖它

## Context（为什么做）

仓库根是 Rust Cargo workspace（`duckfn`），`docs/` 是它的 Docusaurus 3.10 文档站。目前文档站的通用逻辑（TOC 折叠、首页版式、品牌 CSS tokens、版本占位符 remark 插件、iconify 图标集）都散在 `docs/` 里，其它 DuckDB 扩展项目要复用只能整段拷代码。

目标：在仓库根新建独立模块 `duckfn-docs-kit`，把这些通用逻辑封装进去，`docs` 通过依赖它来使用；将来该 kit 可发布给其它扩展项目。

已确认的关键决策：

* **依赖方式：npm workspaces**（仓库根新增 `package.json`，`workspaces: ["docs", "duckfn-docs-kit"]`，lockfile 上移到根）。

* **抽取深度：深度抽取** —— 首页 Hero / Features / NextSteps 做成 kit 提供的 **Web Component**，`docs` 首页变薄壳只传数据；`CodeShowcase` 因依赖主题 `CodeBlock` 保留在 docs。已知代价：这些 CE 区块在 Docusaurus 预渲染 HTML 里为空、hydration 后才出现（与已接受的 `<Icon>` 同类），用户已确认接受。

* **构建工具：Vite lib 模式（多入口）产出 ESM，`tsc --emitDeclarationOnly`** **产出** **`.d.ts`**，CSS 作为原文件直接导出。遵循用户 "npm+ts+vite 项目" 的明确要求。

* **CE 用原生 Custom Elements（不引 Lit）**：用户的风格要求（禁拼 HTML 字符串、类字段持有 DOM）正是 Lit 模板标签的反面；原生 CE + `document.createElement` 最贴合，且不给 docs 增加运行时依赖。

## 风格硬性要求（写进 kit/AGENTS.md）

* 禁止 `innerHTML = \`...\``/`insertAdjacentHTML\` / 模板字符串拼 HTML。

* DOM 一律 `document.createElement` 创建、**类的字段持有引用**、通过方法操作；尽量一个组件一个 class。

* 反例：`this.innerHTML = \`...\`; this.querySelector('button').xxx\`。

* 正例：`#button = document.createElement('button'); ... this.#button.addEventListener(...)`。

* 图标渲染用 CSS `mask` + iconify 图标数据转成的 data URL（拼的是图片 URL、不是页面 DOM；与仓库现有 `.toc-toggle::after` 同款手法），仍属"用 iconify、不手写 SVG"。

* 文本文件 LF。

## kit 目录结构

```
duckfn-docs-kit/
  package.json          # type:module, exports 映射, scripts: build=vite build && tsc --emitDeclarationOnly
  tsconfig.json         # declaration + emitDeclarationOnly, outDir dist, rootDir src
  vite.config.ts        # build.lib 多入口, formats:['es'], outDir dist, emptyOutDir
  .gitignore            # /node_modules /dist
  AGENTS.md             # 上面的风格要求 + 包约定
  src/
    index.ts            # 浏览器入口：re-export CE/register/icons/types + React.JSX 增强
    register.ts         # registerDfkElements()（SSR 守卫 + 幂等 define）
    icons.ts            # re-export 8 个 iconify 图标 + iconToDataUrl()/applyIconMask()
    toc-toggle.ts       # TocToggle 类 + createTocToggle()（供 docs client module 适配）
    remark/
      version-placeholder.ts   # 参数化 remark 插件（纯 Node，不碰 window）
    elements/
      base.ts           # abstract DfkElement<T>：data setter + #built + el() 工具
      hero.ts           # class DfkHero extends DfkElement<HeroData>
      features.ts       # class DfkFeatures + class DfkFeatureCard
      next-steps.ts     # class DfkNextSteps + class DfkNextStepCard
    types.ts            # HeroData / FeaturesData / NextStepsData / ...
    css/
      tokens.css        # --duckfn-* 两套（:root / [data-theme=dark]）
      toc-toggle.css    # .toc-toggle + @layer docusaurus.theme-classic 块（保留原注释）
      home.css          # dfk- 前缀的 Hero/Features/NextSteps 样式 + keyframes + 断点 + reduced-motion
      kit.css           # @import 上述三者（下游一行接入）
```

### package.json exports（关键）

```json
"exports": {
  ".":            { "types": "./dist/index.d.ts", "default": "./dist/index.js" },
  "./toc-toggle": { "types": "./dist/toc-toggle.d.ts", "default": "./dist/toc-toggle.js" },
  "./remark":     { "types": "./dist/remark/version-placeholder.d.ts", "default": "./dist/remark/version-placeholder.js" },
  "./css/kit.css":        "./src/css/kit.css",
  "./css/tokens.css":     "./src/css/tokens.css",
  "./css/toc-toggle.css": "./src/css/toc-toggle.css",
  "./css/home.css":       "./src/css/home.css"
}
```

* `private: true`（暂不发 npm，workspace 内 link；exports 已是可发布形态，将来去 private + 加 publishConfig 即可）。

* dependencies：`@iconify-icons/lucide`、`@iconify-icons/simple-icons`、`@iconify/types`（Vite 打进 dist，docs 不再需要这两个 @iconify-icons 包）。

* devDependencies：`typescript`、`vite`、`unified`(仅类型)。

### vite.config.ts 要点

`build.lib.entry = { index, toc: 'src/toc-toggle', remark: 'src/remark/version-placeholder' }`，`formats:['es']`，`outDir:'dist'`，`emptyOutDir:true`；不 external 化 @iconify（打进去）。CSS 不经 Vite，原样从 `src/css` 导出。

## API 表面

* `registerDfkElements()`：定义 `<dfk-hero>` `<dfk-features>` `<dfk-next-steps>`（幂等、SSR 守卫）。

* 组件数据接口（`types.ts`）：`HeroData`（logoSrc/title/tagline/primary/secondary{icon}/badges）、`FeaturesData`（sectionTitle/items\[{icon,title,details}]）、`NextStepsData`（sectionTitle/items\[{href,title,details}]）——全是当前 locale 的**纯字符串**，i18n 由 docs 侧命令式 `translate()` 提供。

* `icons.ts`：`iconHash/iconLifeBuoy/iconBraces/iconSparkles/iconPackage/iconShieldCheck/iconArrowRight/iconGithub` + `applyIconMask(el, icon)`。

* `createTocToggle(labels?)`：返回 `{ init(), onRouteDidUpdate() }`，把原 `tocToggle.ts` 的模块级状态收进 `TocToggle` 类（`#button`/`#toc` 字段持有，去掉 `document.querySelector('.toc-toggle')`）。

* `remarkVersionPlaceholder({version, placeholder?})`：占位符默认 `{{DUCKFN_VERSION}}`，值由参数传入。

## 迁移与改动清单

### 新建 kit（上述文件）

* `toc-toggle.ts` 逻辑源自 `docs/src/clientModules/tocToggle.ts`：保留 STORAGE\_KEY、`<body>` 状态类、`matchMedia(997px)` 监听、SSR 守卫；类化。

* `home.css` 源自 `docs/src/pages/index.module.css` 中 Hero/Features/NextSteps 段，类名 CSS Modules hash → 全局 `dfk-` 前缀（`.hero→.dfk-hero` 等），保留 reduced-motion 动画、keyframes、断点。CodeShowcase 段（`.codeGrid/.codeColumn/.codeCaption/.showcaseLink*`）留在 docs。

* `tokens.css`/`toc-toggle.css` 源自 `docs/src/css/custom.css`：迁 `--duckfn-*` 两套 + `.toc-toggle`/`@layer docusaurus.theme-classic` 块（连同"为什么必须进 layer"的注释）。`--ifm-color-primary*` 七级、`.code-compare` **留在 docs**（本站与 Infima 的接线/正文用类）。

* `remark/version-placeholder.ts` 源自 `docs/plugins/remark-version-placeholder.ts`，去掉对 `../duckfn-version` 的内置 import，改参数。

### docs 侧

* `docs/package.json`：加 `duckfn-docs-kit`（workspace 用版本号 `"0.1.0"` 自动 link）；删 `@iconify-icons/lucide`、`@iconify-icons/simple-icons`（保留 `@iconify/react`、`@iconify/types` 供 CodeShowcase 的 `<Icon>`）。

* `docs/docusaurus.config.ts`：

  * `clientModules: ['./src/clientModules/tocToggle.ts']` 保留该路径，但文件内容改为薄适配：`import {createTocToggle} from 'duckfn-docs-kit/toc-toggle'; const t=createTocToggle(); t.init(); export const onRouteDidUpdate=()=>t.onRouteDidUpdate();`

  * `remarkPlugins: [[remarkVersionPlaceholder, {version: DUCKFN_VERSION}]]`，import 自 `duckfn-docs-kit/remark` + `./duckfn-version`。

* `docs/src/pages/index.tsx`：顶部 `registerDfkElements()`；`FEATURES/NEXT_STEPS` 改为返回字符串数组（`translate({id,message})` 命令式，仍用同一 `code.json` key）；`<Hero/>`→`<dfk-hero ref={assign(heroData(logoUrl))}/>`、`<Features/>`→`<dfk-features .../>`、`<NextSteps/>`→`<dfk-next-steps .../>`（`assign` 是 ref 回调赋 `data` property，绕开 React 把对象 String() 成 attribute）。`CodeShowcase` 原样保留（`<Icon icon={iconArrowRight}>` 从 kit 取图标）。

* `docs/src/css/custom.css`：顶部 `@import 'duckfn-docs-kit/css/kit.css';`，删除已迁走的 `--duckfn-*` 与 toc-toggle 段，保留 `--ifm-*`、`.code-compare`。

* 删除 `docs/plugins/remark-version-placeholder.ts`；`docs/duckfn-version.ts` **原样保留**（`scripts/release.sh` 按此路径替换版本号）。

### 根 workspace + CI

* 新建根 `package.json`：`{ "name":"duckfn-docs-workspace", "private":true, "workspaces":["docs","duckfn-docs-kit"], "scripts":{ "build":"npm run build -w duckfn-docs-kit && npm run build -w docs", "typecheck":"... 同理", "start":"npm run build -w duckfn-docs-kit && npm start -w docs" } }`。

* 根 `.gitignore` 追加 `/node_modules/`（kit 的 dist 由 kit 自身 .gitignore 管；根 `build` 规则不匹配 `dist`）。

* 删 `docs/package-lock.json`，根执行 `npm install` 生成根 lockfile。

* `.github/workflows/DeployDocs.yml`：`cache-dependency-path` → `package-lock.json`；`Install dependencies` 去 `working-directory: docs` 在根 `npm ci`；`Build site` 改到根执行 `npm run build`（env `DOCS_URL/DOCS_BASE_URL` 在同一 shell 导出会传给子进程）；artifact `docs/build` 不变。

## 实施顺序

1. 建 kit 骨架（package.json/tsconfig/vite.config/.gitignore/AGENTS.md + 空 src）。
2. 根 package.json + 根 .gitignore 追加 + docs 加 kit 依赖 → 删 docs lockfile → 根 `npm install`。验证：`npm ls duckfn-docs-kit` 显示 LINKED。
3. 写 icons.ts + toc-toggle.ts + CSS 四文件 + remark 插件 + elements/base+hero+features+next-steps + types + register + index。验证：`npm run build -w duckfn-docs-kit` 产出 dist（js+d.ts）。
4. docs 侧全部改动（薄壳 index.tsx、custom.css、docusaurus.config、tocToggle 适配、删旧 remark）。验证：`npm run typecheck -w docs`。
5. 全量：根 `npm run build`。验证：`npm run serve -w docs` 浏览器看三个 dfk 区块 hydration 后渲染、图标、暗色、动画；文档页 TOC 折叠+持久化；`npm start -w docs -- --locale zh-Hans` 中文回归。
6. 改 DeployDocs.yml。
7. `cargo package -p duckfn --list` 核对无 JS 混入；对新增文本文件跑 LF 替换。

## 风险与注意

* **React 19 自定义元素**：`index.ts` 里 `declare global { namespace React { namespace JSX { interface IntrinsicElements {...} } } }` 增强，`data` 走 ref property 赋值。

* **CE 内链接是整页跳转**（非 SPA 客户端路由）：v1 接受，后续可加 `dfk-navigate` 事件由 docs 接管。

* **预渲染空壳**：`<dfk-*>` 静态 HTML 为空（已接受）；`<title>/meta` 由 Layout 提供不受影响。

* **Vite 必须交付构建产物**：webpack/`@docusaurus/faster` 不转译 node\_modules，docs 只吃 kit 的 dist（ES2022 ESM），不能直接吃 kit 的 TS 源码。

* **CSS 裸包名 @import**：靠 postcss-import 解析；失败则回退 preset `customCss: ['duckfn-docs-kit/css/kit.css','./src/css/custom.css']`（数组）。

* **kit SSR 安全**：`registerDfkElements`、toc-toggle init 都要 `typeof window/customElements` 守卫（预渲染在 Node import）。

* **lockfile 迁移与 CI cache 路径必须同批改**。

## 验证总览

* `npm run build -w duckfn-docs-kit`：产出 `dist/*.js` + `dist/*.d.ts`。

* `npm run typecheck`（根，串 kit + docs）。

* `npm run build`（根）：Docusaurus en + zh-Hans 均 SUCCESS。

* 浏览器（`npm run serve -w docs`）：首页三个 `dfk-*` 区块 hydration 后渲染、图标可见、暗色模式、入场动画；文档页 TOC 折叠按钮 + 刷新持久化 + 窄窗切换；中文站文案回归。

* `cargo package -p duckfn --list`：无 JS/kit 文件混入发布包。

