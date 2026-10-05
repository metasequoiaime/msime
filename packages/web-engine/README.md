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

把引擎接到 `<textarea>` 或 `<input>` 上，返回解除绑定的函数。组字时按键交给引擎；空闲时的回车、退格、方向键和 Esc 仍由浏览器处理。单按 Shift 切换中英文。默认在文本框下方显示候选栏，可以用 CSS 类 `msime-candidates`、`msime-candidate`、`msime-highlight` 覆盖样式；也可以传 `{ candidates: false, onFrame }` 自己画。

### 自己处理按键

`keyFromEvent(e, { composing })` 把 `KeyboardEvent` 转成打包按键，引擎不该处理的键返回 `null`；`createShiftTap()` 检测单按 Shift；`osImeIntercepting(e)` 判断系统输入法是否正在处理这个键。帧的字段见 `index.d.ts`。

## 浏览器要求

需要支持 WebAssembly、模块 Worker 和 `DecompressionStream`：Chrome / Edge 80+、Firefox 114+、Safari 16.4+。不支持时 `createMsimeEngine` 会以 `code: "unsupported"` 失败。

## 许可

GPL-3.0-only。词库、模型和链接进 wasm 的第三方组件的声明见 `assets/NOTICE.md`。
