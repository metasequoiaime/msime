// @msime/web-engine 的类型。帧的字段与 crates/engine-wasm/src/bindings.rs 的 frame_to_js 一一对应。

/** xiaohe、ziranma、shoudao、microsoft 是小鹤、自然码、手到和微软双拼，和全拼共用拼音库；微软双拼的 `;` 是韵母 ing。japanese 是日语罗马字：下载 wasm 和日语模型（约 5 MB），组字区显示假名，候选是整句转换、词和平假名、片假名，`-` 是长音 ー，标点是 、。「」。korean 是韩文两套式（두벌식）：只下载 wasm，音节在组字区拼好后自动上屏，没有候选。 */
export type MsimeScheme = "quanpin" | "xiaohe" | "ziranma" | "shoudao" | "microsoft" | "wubi86" | "japanese" | "korean";

export declare const SCHEMES: readonly MsimeScheme[];

/** 辅助码方案：lantian 蓝天小雨点、ziranma 自然码、shouyou2_0 首右 2.0、shouyouplus 首右 plus、xiaohe 小鹤形码、jiajia 加加。任何一种都能配全拼和任何一种双拼。 */
export type MsimeHelpcode = "lantian" | "ziranma" | "shouyou2_0" | "shouyouplus" | "xiaohe" | "jiajia";

/** SDK 带的辅助码方案，顺序同引擎的 `assets::HELPCODES`。 */
export declare const HELPCODES: readonly MsimeHelpcode[];
/** 这个包的版本，也是其中 wasm 和词库的 web-engine 版本。 */
export declare const version: string;

/** 引擎要页面执行的动作，按顺序执行。 */
export type MsimeOut =
  /** 上屏 text。seat 是候选在排序后列表里的 0 起位置（不是页内下标）；原文上屏、标点本身、五笔顶字和四码唯一自动上屏为 -1。 */
  | { t: "commit"; text: string; seat: number }
  /** 空闲时直接打出的文字：英文模式下的字母、数字、空格，以及引擎不翻译的 ASCII 标点。 */
  | { t: "type"; text: string }
  /** 空闲时的退格：删除已上屏的文字，word 为 true 时是删词（Ctrl/Alt+Backspace）。 */
  | { t: "back"; word: boolean }
  /** 空闲时的 Esc，含义由页面决定。 */
  | { t: "exit" };

export interface MsimeRow {
  text: string;
  /** 候选的编码。 */
  code: string;
  /** 辅助码提示，如 `(aB)`：单字是它的两码，词是首字和末字的首码；全拼全大写，双拼只大写第二码。没开辅助码或表里没有这个字时为空串。 */
  hint: string;
}

export interface MsimeFrame {
  out: MsimeOut[];
  composing: boolean;
  /** 组字串（拼音或编码）。 */
  preedit: string;
  /** 组字串里的光标位置。 */
  caret: number;
  /** 当前页的候选，最多 pageSize 行。 */
  page: MsimeRow[];
  pageIndex: number;
  hasPrev: boolean;
  /** 本页之后还有候选。 */
  hasNext: boolean;
  /** 高亮的候选在当前页里的下标。 */
  highlight: number;
  /** 五笔：四个码表字母却没有任何候选（空码）。 */
  emptyCode: boolean;
  /** 处于英文模式（单按 Shift 切换）。 */
  english: boolean;
  /** 整句模型已加载，且没有因为慢帧被自动关掉。 */
  modelOn: boolean;
  /** 这次调用花在排序上的毫秒数，没有排序时为 0。 */
  rerankMs: number;
}

export interface MsimeAssetRef {
  url: string;
  /** 传输字节数（.gz 文件是压缩后的大小），用于进度。 */
  size: number;
  /** 解压后的字节数，用于预分配和校验。 */
  rawSize: number;
}

export interface MsimeAssets {
  wasm: MsimeAssetRef;
  /** 拼音方案用拼音库，五笔用五笔 86 库；日语和韩文不用词库，为 null。 */
  db: MsimeAssetRef | null;
  /** 整句模型；五笔、日语、韩文或不需要时为 null。 */
  model: MsimeAssetRef | null;
  /** 日语模型 msime-japanese.dat，只有日语用；其他方案省略或为 null。 */
  japanese?: MsimeAssetRef | null;
  /** 各辅助码方案的表（`helpcode-<方案>.txt.gz`），只在打开辅助码时下载其中一张；缺了某个方案时，打开它会以 unsupported 失败。 */
  helpcodes?: Partial<Record<MsimeHelpcode, MsimeAssetRef>>;
}

export type MsimeLoadPhase = "fetch" | "compile" | "import" | "session";
export type MsimeErrorCode = "unsupported" | "network" | "csp" | "memory" | "panic" | "engine" | "disposed";

export declare class MsimeError extends Error {
  readonly code: MsimeErrorCode;
  readonly phase: MsimeLoadPhase | "runtime";
}

export interface MsimeEngineOptions {
  /** 默认 quanpin。 */
  scheme?: MsimeScheme;
  /** 资源文件所在目录的 URL，相对地址按页面解析。默认是本模块旁边的 assets/，SDK 作为静态文件或从 CDN 加载时不用设；经打包器（Vite、webpack）引入时，设为 `npx msime-web-engine copy` 复制到的目录。 */
  assetBase?: string | URL;
  /** 完全自定义资源地址，优先于 assetBase。 */
  assets?: MsimeAssets;
  /** 每页候选数，默认 9。 */
  pageSize?: number;
  /** 拼音方案是否下载整句模型（约 4 MB），默认 true。设为 false 时省流量，候选按词频排序。 */
  model?: boolean;
  /** 已下载模型时是否启用它，默认 true，之后可用 setModelEnabled 切换。 */
  modelEnabled?: boolean;
  /** 辅助码方案，默认 null（关闭）。只对全拼和双拼起作用，其他方案忽略它；打开时随引擎一起下载这张表（几十 KB），之后可用 setHelpcode 切换。 */
  helpcode?: MsimeHelpcode | null;
  /** 自定义 Worker（例如 CSP 不允许 blob: 时自己托管 worker.js），或者返回 Worker 的函数。 */
  worker?: Worker | (() => Worker);
  /** 下载进度，单位是传输字节。 */
  onProgress?: (loaded: number, total: number) => void;
}

export interface MsimeEngine {
  readonly scheme: MsimeScheme;
  /** "msime-engine-wasm <版本> <提交>" */
  readonly build: string;
  readonly memoryBytes: number;
  readonly timings: Record<MsimeLoadPhase, number> | undefined;
  /** 当前的辅助码方案，关闭时为 null。 */
  readonly helpcode: MsimeHelpcode | null;
  /** 发送一个或一批打包按键（keyFromEvent 的结果，null 会被忽略），返回处理完后的帧。 */
  keys(keys: number | null | ReadonlyArray<number | null>): Promise<MsimeFrame>;
  /** 鼠标点选当前页第 slot 个候选。 */
  pick(slot: number): Promise<MsimeFrame>;
  /** 取消组字、清空上下文。 */
  reset(): Promise<MsimeFrame>;
  /** 在 quanpin 和四种双拼之间切换，辅助码设置保留；和 wubi86、japanese、korean 互换需要新建引擎。 */
  setScheme(scheme: MsimeScheme): Promise<void>;
  /** 打开（传方案名）或关闭（null）辅助码，不用重建引擎；第一次用到的表这时下载。正在组的字按新设置重新查询，下一帧可见。五笔、日语和韩文引擎上什么也不做。下载失败时 reject（code 为 network），引擎照常可用、设置不变。 */
  setHelpcode(helpcode: MsimeHelpcode | null): Promise<void>;
  setModelEnabled(enabled: boolean): void;
  /** 页面空闲时的退格是否真的删字；页面拒绝删除时传 false，引擎的上下文就不会跟着弹出。 */
  setBackspaceDeletes(deletes: boolean): void;
  /** 运行期错误（引擎 panic 等）；之后所有请求都会 reject。 */
  onError(fn: (error: MsimeError) => void): () => void;
  /** 结束 Worker，释放内存。 */
  dispose(): void;
}

export declare function createMsimeEngine(options?: MsimeEngineOptions): Promise<MsimeEngine>;

export declare const KeyKind: Readonly<{
  Letter: 1;
  ShiftLetter: 2;
  Digit: 3;
  Space: 4;
  Enter: 5;
  Backspace: 6;
  BackspaceWord: 7;
  Escape: 8;
  PagePrev: 9;
  PageNext: 10;
  HighlightPrev: 11;
  HighlightNext: 12;
  Punct: 13;
  ShiftTap: 14;
}>;
export declare function packKey(kind: number, ascii?: number): number;
/** keydown 对应的打包按键，引擎不该看到这个键时为 null。composing 传最近一帧的 composing；typed 可以覆盖 e.key。 */
export declare function keyFromEvent(e: KeyboardEvent, options?: { composing?: boolean; typed?: string | null }): number | null;
/** 系统输入法正在处理这个键。 */
export declare function osImeIntercepting(e: KeyboardEvent): boolean;
/** 单按 Shift 的检测器；up 返回 true 时发送 packKey(KeyKind.ShiftTap)。 */
export declare function createShiftTap(): { down(e: KeyboardEvent): void; up(e: KeyboardEvent): boolean };

/** 内置皮肤 ID：`system`（网页上画桌面端设置页预览的平台默认配色，跟随明暗）、五个全局主题，以及 msime-windows 的五个内置外观。 */
export type MsimeSkinId =
  | "system"
  | "shuishan"
  | "light"
  | "paper"
  | "night"
  | "ink"
  | "wechat"
  | "graphite"
  | "willow_green"
  | "autumn_osmanthus"
  | "microsoft";

/** SDK 接受的内置皮肤 ID，顺序同设置页。 */
export declare const SKINS: readonly MsimeSkinId[];
/** 不传皮肤时用的皮肤，即 `"shuishan"`。 */
export declare const DEFAULT_SKIN: "shuishan";

export type MsimeSkinLayout = "horizontal" | "vertical";
export type MsimeSkinMode = "light" | "dark";

/** 皮肤包在一种明暗下的候选框配色，同桌面 `skin.toml` 的 `[candidate.light]` / `[candidate.dark]`。颜色写成 `#RGB`、`#RRGGBB`、`#RRGGBBAA`、`rgb()`、`rgba()` 或 `transparent`，读不懂的当没写。 */
export interface MsimeSkinPalette {
  accent?: string;
  /** 高亮候选的底色。 */
  selected?: string;
  /** 鼠标悬停的候选的底色。 */
  hover?: string;
  /** 候选框底色。 */
  surface?: string;
  border?: string;
  text?: string;
  /** 序号色。 */
  number?: string;
  /** 次要文字（编码提示）的颜色。 */
  translation?: string;
  /** 是否在高亮候选左边画选中条，默认画。 */
  showSelectedBar?: boolean;
}

/** 皮肤对象，形状同桌面设置页的 `SkinSummary` JSON（只取候选框用得到的字段）。尺寸单位 dip 在网页上就是 CSS px；图片写成 http(s)、blob、`data:image/*` 或相对地址（按页面地址解析），其他地址丢弃。 */
export interface MsimeSkin {
  id?: string;
  /** 画在哪个主题之上：全局主题、`system` 或 msime-windows 的内置外观（`fluent`、`wechat` 等），默认 `system`。base 为全局主题时，皮肤固定画在那个主题的明暗下。 */
  base?: string;
  /** 皮肤支持的布局，不写时全都支持。不支持当前布局或明暗时只画 base（`ResolvedSkin.drawn` 为 false）。 */
  layouts?: readonly MsimeSkinLayout[];
  /** 皮肤支持的明暗，不写时全都支持。 */
  themes?: readonly MsimeSkinMode[];
  /** 也可以写成清单的形状 `supports: { layouts, themes }`。 */
  supports?: { layouts?: readonly MsimeSkinLayout[]; themes?: readonly MsimeSkinMode[] };
  /** 候选框最小宽度，0 到 1000。 */
  minWidthDip?: number;
  /** 候选框圆角，0 到 32。 */
  cornerRadiusDip?: number;
  /** 候选框上方装饰带的高度，0 到 500，与 decorationWidthDip 都大于 0 且有图片时才画。 */
  decorationTopDip?: number;
  /** 装饰图的宽度，0 到 1000。 */
  decorationWidthDip?: number;
  /** 装饰图；不写时用 preview。 */
  decorationImage?: string | null;
  preview?: string | null;
  /** 装饰图对齐，默认 right。 */
  decorationAlign?: "left" | "center" | "right";
  /** 候选框背景图；opacity 默认 1，夹到 0 到 1。 */
  background?: { image?: string; fit?: "cover" | "contain" | "stretch"; opacity?: number } | null;
  candidate?: { dark?: MsimeSkinPalette; light?: MsimeSkinPalette };
}

/** resolveSkin 的结果，整个被冻结。颜色都是大写的 `#RRGGBB` 或 `#RRGGBBAA`。 */
export interface ResolvedSkin {
  /** 内置 ID，或皮肤对象的 id（没有时为 null）。 */
  readonly id: string | null;
  /** 实际画的明暗；全局主题和 base 为全局主题的皮肤固定在主题自己的明暗。 */
  readonly dark: boolean;
  readonly layout: MsimeSkinLayout;
  readonly palette: Readonly<{
    surface: string;
    border: string;
    text: string;
    number: string;
    /** 次要文字（编码提示）。 */
    secondary: string;
    accent: string;
    /** 高亮候选的底色。 */
    selected: string;
    /** 高亮候选的文字色。`theme::resolve` 给了就用它；没给时，Windows 外观（且皮肤对象没写自己的 `selected`）用 Windows 版样式表的颜色（`wechat`、`willow_green` 是白色，`graphite` 是更深或更亮的字色），其余用普通的 `text`。 */
    selectedText: string;
    /** 高亮候选的序号色，取法同 `selectedText`，最后退回普通的 `number`。 */
    selectedNumber: string;
    hover: string;
    showSelectedBar: boolean;
  }>;
  readonly geometry: Readonly<{
    /** 圆角（px），皮肤没给时为 null。 */
    cornerRadius: number | null;
    minWidth: number | null;
    decoration: Readonly<{ url: string; top: number; width: number; align: "left" | "center" | "right" }> | null;
    background: Readonly<{ url: string; fit: "cover" | "contain" | "stretch"; opacity: number }> | null;
  }>;
  /** 候选栏用的 CSS 自定义属性（`--cand-bg`、`--cand-text`、`--cand-selected`、`--msime-skin-radius` 等），值都已校验，可以直接 `style.setProperty`。 */
  readonly variables: Readonly<Record<string, string>>;
  /** 皮肤对象不支持当前布局或明暗、只画了 base 时为 false。 */
  readonly drawn: boolean;
}

/** 把内置皮肤 ID 或皮肤对象解析成颜色、几何和 CSS 自定义属性：桌面端 `theme::resolve` 的结果，再补齐它留空的槽位（`system` 留空的用平台默认配色，Windows 外观补上高亮候选的文字色）。自己画候选栏的页面用它取水杉的配色。未知 ID、base 或 layout 抛 TypeError。 */
export declare function resolveSkin(skin?: MsimeSkinId | MsimeSkin, options?: { dark?: boolean; layout?: MsimeSkinLayout }): ResolvedSkin;

/** 候选栏的宿主元素标签名。 */
export declare const CANDIDATE_BAR_TAG: "msime-candidates";
/** 候选栏用 `::part()` 暴露的部分，页面可以写 `msime-candidates::part(highlight) { ... }`。 */
export declare const PARTS: Readonly<{
  /** 整个候选栏（定位的那一层，含装饰带）。 */
  candidates: "candidates";
  /** 候选框：底色、边框、圆角、阴影。 */
  card: "card";
  /** 预编辑行。 */
  preedit: "preedit";
  /** 预编辑行末尾的光标。 */
  caret: "caret";
  /** 翻页标记。 */
  paging: "paging";
  /** 每个候选。 */
  candidate: "candidate";
  /** 高亮的候选，同时也带 candidate。 */
  highlight: "highlight";
  /** 候选前的序号。 */
  number: "number";
  /** 候选文字。 */
  text: "text";
  /** 候选后的辅助码提示或编码提示。 */
  code: "code";
  /** 皮肤的装饰图。 */
  decoration: "decoration";
  /** 皮肤的背景图。 */
  background: "background";
}>;

export interface CandidateBarOptions {
  /** 内置皮肤 ID 或皮肤对象，默认 `"shuishan"`。 */
  skin?: MsimeSkinId | MsimeSkin;
  /** 横排或竖排，默认 horizontal。 */
  layout?: MsimeSkinLayout;
  /** "auto"（默认）跟随 `prefers-color-scheme` 并在它变化时重画；true、false 强制深色或浅色。只对 `system`、Windows 外观和不固定明暗的皮肤对象起作用。 */
  dark?: boolean | "auto";
  /** 点击当前页第 index 个候选。 */
  onPick?: (index: number) => void;
  /** 宿主元素放在哪里，默认 document.body。 */
  container?: Element;
  /** 在候选后显示编码（`page[i].code`），默认 false。帧里有辅助码提示（`page[i].hint`）时，不论这个选项都显示提示。 */
  helpcode?: boolean;
}

export interface CandidateBar {
  /** 宿主元素 `<msime-candidates>`，候选栏画在它的 Shadow DOM 里。 */
  readonly element: HTMLElement;
  /** 画一帧；frame.composing 为 false 时隐藏。anchorRect 是光标或文本框的视口矩形，候选栏画在它下方，放不下时画在上方，并保持在视口内。 */
  render(frame: MsimeFrame, anchorRect: Pick<DOMRectReadOnly, "left" | "top" | "bottom" | "right">): void;
  hide(): void;
  /** 换皮肤；未知 ID 抛 TypeError，原来的皮肤不变。 */
  setSkin(skin: MsimeSkinId | MsimeSkin | undefined): void;
  setLayout(layout: MsimeSkinLayout): void;
  setDark(dark: boolean | "auto"): void;
  /** 移除宿主元素和所有监听。 */
  destroy(): void;
}

/** 创建按水杉候选框皮肤绘制的候选栏。样式放在可构造样式表里，皮肤取值经 `style.setProperty` 写入，页面的 CSP 不开 `style-src 'unsafe-inline'` 也能用；浏览器不支持 adoptedStyleSheets 时抛 Error。 */
export declare function createCandidateBar(options?: CandidateBarOptions): CandidateBar;

export interface AttachInputOptions {
  /** 是否画默认候选栏（createCandidateBar），默认 true。 */
  candidates?: boolean;
  /** 候选栏的皮肤，默认 `"shuishan"`。 */
  skin?: MsimeSkinId | MsimeSkin;
  /** 候选栏横排或竖排，默认 horizontal。 */
  layout?: MsimeSkinLayout;
  /** 候选栏的明暗，默认 "auto"。 */
  dark?: boolean | "auto";
  /** 候选栏挂在哪个元素里。不传时挂在文本框所在的顶层元素（打开的 `<dialog>`、popover 或全屏元素）里，都不是时挂在 body 上，每次显示前按当时的状态重新选。 */
  container?: Element;
  /** 每一帧都会回调，自己画候选栏时用。 */
  onFrame?: (frame: MsimeFrame) => void;
}
/** 把引擎接到一个 textarea 或 input 上：组字时拦截按键交给引擎，空闲时的回车、退格、方向键和 Esc 仍由浏览器处理。返回解除绑定的函数；解除绑定时正在组的字被放弃，还没回来的帧不再写进文本框，也不再回调 onFrame。 */
export declare function attachInput(el: HTMLTextAreaElement | HTMLInputElement, engine: MsimeEngine, options?: AttachInputOptions): () => void;
