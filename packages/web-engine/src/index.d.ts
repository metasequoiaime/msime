// @msime/web-engine 的类型。帧的字段与 crates/engine-wasm/src/bindings.rs 的 frame_to_js 一一对应。

export type MsimeScheme = "quanpin" | "xiaohe" | "ziranma" | "wubi86";

export declare const SCHEMES: readonly MsimeScheme[];
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
  /** 拼音方案用拼音库，五笔用五笔 86 库。 */
  db: MsimeAssetRef;
  /** 整句模型；五笔或不需要时为 null。 */
  model: MsimeAssetRef | null;
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
  /** 发送一个或一批打包按键（keyFromEvent 的结果，null 会被忽略），返回处理完后的帧。 */
  keys(keys: number | null | ReadonlyArray<number | null>): Promise<MsimeFrame>;
  /** 鼠标点选当前页第 slot 个候选。 */
  pick(slot: number): Promise<MsimeFrame>;
  /** 取消组字、清空上下文。 */
  reset(): Promise<MsimeFrame>;
  /** 在 quanpin、xiaohe、ziranma 之间切换；和 wubi86 互换需要新建引擎。 */
  setScheme(scheme: MsimeScheme): Promise<void>;
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

export interface AttachInputOptions {
  /** 是否画默认候选栏（类名 msime-candidates、msime-preedit、msime-candidate、msime-highlight），默认 true。 */
  candidates?: boolean;
  /** 每一帧都会回调，自己画候选栏时用。 */
  onFrame?: (frame: MsimeFrame) => void;
}
/** 把引擎接到一个 textarea 或 input 上：组字时拦截按键交给引擎，空闲时的回车、退格、方向键和 Esc 仍由浏览器处理。返回解除绑定的函数。 */
export declare function attachInput(el: HTMLTextAreaElement | HTMLInputElement, engine: MsimeEngine, options?: AttachInputOptions): () => void;
