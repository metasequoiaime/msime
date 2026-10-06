# @msime/web-engine

水杉输入法的网页引擎。全拼、小鹤双拼、自然码双拼和五笔 86 都运行在浏览器的 Web Worker 里（Rust 编译成 WebAssembly），不需要服务器，可以部署到 GitHub Pages、Vercel、Cloudflare Pages / Workers 这类任意静态托管上。

```js
import { createMsimeEngine, attachInput } from "@msime/web-engine";

const engine = await createMsimeEngine({ scheme: "quanpin" });
attachInput(document.querySelector("textarea"), engine);
```

## 包里有什么

| 文件 | 大小 | 说明 |
| --- | --- | --- |
| `assets/msime_engine_bg.wasm` | 约 4 MB | 引擎 |
| `assets/msime-pinyin.db.gz` | 约 7.5 MB | 拼音词库，全拼和双拼共用 |
| `assets/msime-wubi86.db.gz` | 约 3.6 MB | 五笔 86 词库 |
| `assets/sentence-model.safetensors.gz` | 约 4 MB | 整句模型，只用于拼音，可以不下载 |
| `assets/NOTICE.md` | | 第三方许可声明，部署时请一并发布 |

每个页面只下载用到的方案：全拼约 16 MB，不要整句模型约 12 MB，五笔约 8 MB。浏览器会按 HTTP 缓存规则缓存这些文件。

## 接入方式

### 一、静态站点，不用打包器

把运行时和资源复制到站点的发布目录里，页面直接引用：

```sh
npx @msime/web-engine copy public/msime          # 或 site/msime、dist/msime，看你的发布目录
npx @msime/web-engine copy public/msime --no-wubi --no-model   # 只要全拼和双拼，且不要整句模型
```

```html
<textarea id="text"></textarea>
<script type="module">
  import { createMsimeEngine, attachInput } from "/msime/index.js";
  const engine = await createMsimeEngine({ scheme: "quanpin" });
  attachInput(document.getElementById("text"), engine);
</script>
```

复制出来的目录自成一体：`index.js` 默认从旁边的 `assets/` 读取资源，Worker 和页面同源，不需要任何配置。

### 二、Vite、webpack 等打包器

```sh
npm install @msime/web-engine
npx msime-web-engine copy public/msime      # 资源仍要作为静态文件发布
```

```js
import { createMsimeEngine, attachInput } from "@msime/web-engine";
const engine = await createMsimeEngine({ scheme: "quanpin", assetBase: "/msime/assets/" });
```

打包器会把 `worker.js` 一起打包，但几十 MB 的词库不应该经过打包器，所以要用 `assetBase` 指向复制出来的 `assets/`。Vite 需要在 `optimizeDeps.exclude` 里加上 `@msime/web-engine`，否则预构建会改写 `worker.js` 的地址。

### 三、CDN，不部署任何文件

```html
<script type="module">
  import { createMsimeEngine, attachInput } from "https://cdn.jsdelivr.net/npm/@msime/web-engine@VERSION/index.js";
  const engine = await createMsimeEngine();
  attachInput(document.querySelector("textarea"), engine);
</script>
```

浏览器不允许直接用别的源的脚本创建 Worker，SDK 会自动改用一个同源的 `blob:` 脚本去 import 它。适合演示和原型；正式上线建议用第一种方式，把文件部署在自己的域名下。

注意：GitHub Release 的下载地址不能直接作为 `assetBase`。它会 302 跳转，而且不返回 `Access-Control-Allow-Origin`，浏览器跨源请求会失败。

## 部署到各平台

示例站点和每个平台的配置在仓库的 [`packages/web-engine/examples/static-site`](https://github.com/metasequoiaime/msime/tree/develop/packages/web-engine/examples/static-site)。

| 平台 | 做法 |
| --- | --- |
| GitHub Pages | `copy` 到 Pages 发布的目录，用 `actions/deploy-pages` 发布，示例里有现成的 workflow。Pages 不能自定义响应头，缓存时间固定为 10 分钟，过期后靠 ETag 重新验证，不会重新下载整个文件。 |
| Vercel | `copy` 到 `public/`（框架项目）或发布目录。示例的 `vercel.json` 给 `/msime/` 配了长缓存。 |
| Cloudflare Pages | `copy` 到构建输出目录。示例的 `_headers` 放在输出目录的根目录，配长缓存。单个文件不超过 25 MiB 的限制。 |
| Cloudflare Workers | 用 Workers Static Assets 托管，`wrangler.jsonc` 的 `assets.directory` 指向发布目录，同样读取 `_headers`。引擎仍然在用户浏览器里运行：wasm 运行时需要上百 MB 内存，超过 Workers 单实例 128 MB 的上限，而且每个按键都走一次网络也太慢。 |

所有平台都会给 `.wasm` 返回 `application/wasm`。`.gz` 文件无论服务器是否加了 `Content-Encoding: gzip`，SDK 都能正确处理。

资源文件名不带版本号。要配长缓存（`immutable`）的话，请把目录换成带版本号的，例如 `copy public/msime/0.2.0`，升级时换一个目录。

## 内容安全策略（CSP）

- `script-src` 需要 `'wasm-unsafe-eval'`，否则无法编译 wasm，`createMsimeEngine` 会以 `code: "csp"` 失败。
- 第三种 CDN 用法还需要 `worker-src blob:`。
- 默认候选栏不需要 `style-src 'unsafe-inline'`：它的样式是可构造样式表（`adoptedStyleSheets`），皮肤的取值经 `style.setProperty` 写入，没有 `<style>` 元素，也没有 `style=""` 属性，`style-src 'self'` 这样严格的策略下照常显示。皮肤图片要被 `img-src` 允许（`data:` 图片需要 `img-src data:`）。

## API

### `createMsimeEngine(options?)`

下载并初始化引擎，完成后返回 `MsimeEngine`。选项：

- `scheme`：`"quanpin"`（默认）、`"xiaohe"`、`"ziranma"`、`"wubi86"`。
- `assetBase`：资源目录的 URL，相对地址按页面解析。
- `model`：拼音方案是否下载整句模型，默认 `true`。
- `pageSize`：每页候选数，默认 9。
- `onProgress(loaded, total)`：下载进度。
- `worker`：自定义 Worker，例如 CSP 不允许 `blob:` 时自己托管 `worker.js`。

失败时 reject 一个 `MsimeError`，`code` 是 `unsupported`、`network`、`csp`、`memory` 或 `engine`。

### `MsimeEngine`

- `keys(key | key[])`：发送打包按键，返回处理后的帧 `MsimeFrame`。
- `pick(slot)`：点选当前页第 `slot` 个候选。
- `reset()`：取消组字、清空上下文。
- `setScheme(scheme)`：在全拼和两种双拼之间切换，不重新下载。和五笔互换需要 `dispose()` 后新建一个引擎。
- `setModelEnabled(enabled)`、`setBackspaceDeletes(deletes)`。
- `onError(fn)`：运行期错误（引擎 panic）。之后所有请求都会 reject，需要新建引擎。
- `dispose()`：结束 Worker，释放内存。wasm 内存不会自动缩小，不用的时候请调用它。

### `attachInput(el, engine, options?)`

把引擎接到 `<textarea>` 或 `<input>` 上，返回解除绑定的函数。组字时按键交给引擎；空闲时的回车、退格、方向键和 Esc 仍由浏览器处理。单按 Shift 切换中英文。默认在文本框下方显示按水杉候选框皮肤绘制的候选栏（见下文「候选框皮肤」），选项：

- `skin`：内置皮肤 ID 或皮肤对象，默认 `"shuishan"`（水杉）。
- `layout`：`"horizontal"`（默认，横排）或 `"vertical"`（竖排）。
- `dark`：`"auto"`（默认，跟随页面的 `prefers-color-scheme`，切换时自动重画）、`true` 或 `false`。
- `candidates: false`：不画候选栏，配合 `onFrame(frame)` 自己画。
- `container`：候选栏挂在哪个元素里。不传时挂在文本框所在的顶层元素（打开的 `<dialog>`、popover 或全屏元素）里，都不是时挂在 `body` 上，每次显示前按当时的状态重新选；挂到 `body` 上的候选栏会被模态对话框盖住、点不到。
- `onFrame(frame)`：每一帧都会回调。

```js
attachInput(textarea, engine, { skin: "wechat", layout: "vertical", dark: "auto" });
```

换皮肤时调用返回的函数解除绑定，再以新的选项重新 `attachInput`，引擎不用重建。解除绑定时正在组的字会被放弃（同 `engine.reset()`），还没回来的帧不再写进文本框，也不再回调 `onFrame`。

## 候选框皮肤

候选栏画的是水杉桌面端的候选框：同样的结构、配色和皮肤规则。配色表在构建时从桌面端的主题表生成，与桌面端同一个版本，SDK 里没有另抄一份。

### 内置皮肤

| ID | 名称 | 明暗 |
| --- | --- | --- |
| `system` | 跟随系统 | 跟随 `dark`。网页读不到系统配色，画的是桌面端设置页预览里的平台默认配色：浅色白底、深色 `#202020` 底，选中项是灰色底、普通文字色 |
| `shuishan` | 水杉（默认） | 深色 |
| `light` | 浅色 | 浅色 |
| `paper` | 纸白 | 浅色 |
| `night` | 夜青 | 深色 |
| `ink` | 墨 | 深色 |
| `wechat` | 微信绿 | 跟随 `dark` |
| `graphite` | 石墨 | 跟随 `dark` |
| `willow_green` | 杨柳青 | 跟随 `dark` |
| `autumn_osmanthus` | 秋桂 | 跟随 `dark` |
| `microsoft` | 微软 | 跟随 `dark` |

五个全局主题自带明暗，和桌面端一样不受 `dark` 影响；后五个是 Windows 版的内置外观，深浅两套配色都有。`SKINS` 导出全部 ID。

Windows 外观高亮候选的文字和序号色按 Windows 版的样式表画：`wechat`、`willow_green` 是绿底白字，`graphite` 没有选中底色，只靠更深（深色下更亮）的字色标出高亮。桌面端的 `theme::resolve` 没有这个槽位，`base` 是这些外观的皮肤包在桌面上画普通文字色；皮肤对象自己写了 `selected` 时也画普通文字色。

### 自定义皮肤对象

`skin` 也可以是一个对象，字段与桌面端皮肤包 `skin.toml` 解析后的 JSON（设置页的 `SkinSummary`）相同，只取候选框用得到的部分。尺寸单位 dip 在网页上就是 CSS px：

```js
attachInput(textarea, engine, {
  skin: {
    base: "light",                     // 画在哪个主题之上：全局主题、system 或 Windows 外观（wechat 等），默认 system
    layouts: ["horizontal", "vertical"], // 支持的布局，不写为全部；也可写成 supports: { layouts, themes }
    themes: ["light"],                 // 支持的明暗，不写为全部
    cornerRadiusDip: 8,                // 圆角，0–32
    minWidthDip: 240,                  // 最小宽度，0–1000
    decorationTopDip: 28,              // 候选框上方装饰带的高度，0–500
    decorationWidthDip: 96,            // 装饰图宽度，0–1000
    decorationImage: "https://example.com/cat.png",
    decorationAlign: "right",          // left、center、right
    background: { image: "data:image/png;base64,...", fit: "cover", opacity: 0.6 }, // fit：cover、contain、stretch
    candidate: {
      light: { surface: "#FFF8F0", text: "#3A2A1A", number: "#8A6A4A", accent: "#E07020", selected: "#E0702030", hover: "#E0702014", border: "#E0C0A0", translation: "#8A6A4A", showSelectedBar: true },
    },
  },
});
```

- 颜色接受 `#RGB`、`#RRGGBB`、`#RRGGBBAA`、`rgb()`、`rgba()` 和 `transparent`，读不懂的颜色当没写，由 `base` 补上；越界的尺寸当 0。
- 图片可以是 `http:`、`https:`、`blob:`、`data:image/*` 或相对地址（按页面地址解析），其他地址（如 `javascript:`）和含引号、括号、空白的地址一律丢弃；`data:image/*` 里的括号和单引号（`encodeURIComponent` 写出的 SVG 常带）会换成百分号编码，图片不变。图片加载失败时只是不画它。
- 皮肤不支持当前的布局或明暗时，和桌面端一样只画 `base`。`base` 是全局主题时，皮肤固定画在那个主题的明暗下。
- `base` 是 `system`（或不写）时，皮肤没写的颜色用上面 `system` 那一行说的平台默认配色补；高亮候选的文字和序号同桌面端画普通的 `text`、`number`。

### `resolveSkin(skin?, { dark, layout }?)`

自己画候选栏的页面（例如有自己设计语言的 TapTapGo）可以只取水杉的配色：`resolveSkin` 把皮肤 ID 或皮肤对象解析成桌面端 `theme::resolve` 的结果，并补齐它留空的槽位（`system` 留空的用平台默认配色，Windows 外观补上高亮候选的文字色，见上文「内置皮肤」），不画任何东西。

```js
import { resolveSkin } from "@msime/web-engine";
const { palette, geometry, variables, drawn } = resolveSkin("paper", { dark: false, layout: "horizontal" });
// palette：surface、border、text、number、secondary、accent、selected、selectedText、selectedNumber、hover（都是 #RRGGBB 或 #RRGGBBAA）和 showSelectedBar
// geometry：cornerRadius、minWidth（px 或 null）、decoration、background
// variables：--cand-bg、--cand-text、--cand-selected、--msime-skin-radius 等 CSS 自定义属性，值都校验过，可以直接 setProperty
for (const [name, value] of Object.entries(variables)) myBar.style.setProperty(name, value);
```

未知的皮肤 ID、`base` 或 `layout` 抛 `TypeError`。

### `createCandidateBar(options?)`

`attachInput` 用的候选栏，也可以单独使用（例如接到自己的编辑器上）：

```js
import { createCandidateBar } from "@msime/web-engine";
const bar = createCandidateBar({ skin: "night", layout: "horizontal", dark: "auto", onPick: (i) => engine.pick(i).then(update) });
bar.render(frame, caretRect);   // frame.composing 为 false 时隐藏；画在 caretRect 下方，放不下时翻到上方，并保持在视口内
bar.setSkin("ink"); bar.setLayout("vertical"); bar.setDark(true);
bar.hide(); bar.destroy();
```

`container` 是宿主元素放在哪里，默认 `document.body`；`helpcode: true` 时在候选后显示编码（帧里的 `page[i].code`），默认关闭，`attachInput` 不打开它。

点候选不会让输入框失焦。浏览器不支持可构造样式表（Safari 16.4 以前）时抛 `Error`。

引擎由自己接管、只要候选栏的页面，从 `@msime/web-engine/candidates.js` 导入（带类型）：

```js
import { createCandidateBar } from "@msime/web-engine/candidates.js";
```

包的入口为了开箱即用，写了 `new Worker(new URL("./worker.js", import.meta.url))`，Vite、webpack 5 等打包器只要从入口导入，就会把包自带的 Worker 打成一个单独的文件，即使页面从不调用 `createMsimeEngine`。这个子路径只含候选栏和皮肤，没有这一步。

### 改样式

候选栏画在 `<msime-candidates>` 元素的 Shadow DOM 里，页面的 CSS 影响不到它，它的样式也不会漏到页面上。要改样式请用 `::part()`：

```css
msime-candidates::part(candidates) { font-size: 18px; }
msime-candidates::part(highlight) { font-weight: 600; }
```

可用的 part：`candidates`（整个候选栏）、`card`（候选框）、`preedit`（拼音或编码行）、`caret`、`paging`（翻页标记）、`candidate`（每个候选）、`highlight`（高亮的候选，同时带 `candidate`）、`number`、`text`、`code`（编码提示，只在 `createCandidateBar({ helpcode: true })` 时出现）、`decoration`、`background`。Shadow DOM 里的类名不是公开接口。

从 0.1.x 升级：旧版候选栏画在页面 DOM 里，用 CSS 类 `msime-candidates`、`msime-preedit`、`msime-candidate`、`msime-highlight` 改样式；这些类现在匹配不到任何元素，页面上针对它们写的样式不再生效，请分别改成 `msime-candidates::part(candidates)`、`msime-candidates::part(preedit)`、`msime-candidates::part(candidate)`、`msime-candidates::part(highlight)`。

### 自己处理按键

`keyFromEvent(e, { composing })` 把 `KeyboardEvent` 转成打包按键，引擎不该处理的键返回 `null`；`createShiftTap()` 检测单按 Shift；`osImeIntercepting(e)` 判断系统输入法是否正在处理这个键。帧的字段见 `index.d.ts`。

## 浏览器要求

需要支持 WebAssembly、模块 Worker 和 `DecompressionStream`：Chrome / Edge 80+、Firefox 114+、Safari 16.4+。不支持时 `createMsimeEngine` 会以 `code: "unsupported"` 失败。

## 许可

GPL-3.0-only。词库、模型和链接进 wasm 的第三方组件的声明见 `assets/NOTICE.md`。
