// 主线程入口：起一个模块 Worker 运行引擎（worker.js），把按键、点选、重置变成返回 Promise 的方法。类型见 index.d.ts，用法见 README.md。
//
// assets.js 由 scripts/build-web-engine.sh 在打包时生成，记着同一次构建产出的每个资源文件（含辅助码表）的名字和大小，所以 SDK、wasm 和词库的版本永远一致。
import { files, helpcodes, version } from "./assets.js";

export { KeyKind, packKey, keyFromEvent, osImeIntercepting, createShiftTap } from "./keys.js";
export { attachInput } from "./input.js";
export { DEFAULT_SKIN, SKINS, resolveSkin } from "./skin.js";
export { CANDIDATE_BAR_TAG, PARTS, createCandidateBar } from "./candidates.js";
export { version };

export const SCHEMES = Object.freeze(["quanpin", "xiaohe", "ziranma", "shoudao", "microsoft", "wubi86", "japanese", "korean"]);
// 全拼和四种双拼共用拼音库，可以就地切换，也只有它们用辅助码。
const PINYIN = new Set(["quanpin", "xiaohe", "ziranma", "shoudao", "microsoft"]);
// 辅助码方案，顺序和名字同引擎的 assets::HELPCODES（crates/engine-wasm/tests/helpcodes.rs 核对）。
export const HELPCODES = Object.freeze(["lantian", "ziranma", "shouyou2_0", "shouyouplus", "xiaohe", "jiajia", "wubi86"]);

export class MsimeError extends Error {
  constructor(code, message, phase = "runtime") {
    super(message);
    this.name = "MsimeError";
    this.code = code;
    this.phase = phase;
  }
}

// 相对的 assetBase 按页面地址解析（和 <img src> 一样）；没有页面时（Node 测试）按本模块解析。
function baseUrl(assetBase) {
  const page = globalThis.location?.href;
  const url = assetBase === undefined ? new URL("./assets/", import.meta.url) : new URL(String(assetBase), page ?? import.meta.url);
  if (!url.pathname.endsWith("/")) url.pathname += "/";
  return url;
}

function resolveAssets(scheme, options) {
  if (options.assets) return options.assets;
  const ref = (file) => ({ url: new URL(file.name, base).href, size: file.size, rawSize: file.rawSize });
  const base = baseUrl(options.assetBase);
  // 辅助码表很小，只记地址，等打开辅助码时才下载用到的那一张。
  const tables = Object.fromEntries(Object.entries(helpcodes ?? {}).map(([schema, file]) => [schema, ref(file)]));
  if (!files.wasm) throw new MsimeError("unsupported", "this build of @msime/web-engine ships no wasm; pass options.assets", "fetch");
  // 韩文只用 wasm：音节由引擎自己拼，不查词库，也没有候选要重排。
  if (scheme === "korean") return { wasm: ref(files.wasm), db: null, model: null, japanese: null };
  // 日语用 wasm 和日语模型，不用拼音库和整句模型。
  if (scheme === "japanese") {
    if (!files.japanese) throw new MsimeError("unsupported", "this build of @msime/web-engine ships no japanese model; pass options.assets", "fetch");
    return { wasm: ref(files.wasm), db: null, model: null, japanese: ref(files.japanese) };
  }
  const db = scheme === "wubi86" ? files.wubi86 : files.pinyin;
  if (!db) {
    throw new MsimeError("unsupported", `this build of @msime/web-engine ships no ${scheme === "wubi86" ? "wubi86" : "pinyin"} dictionary; pass options.assets`, "fetch");
  }
  const wantModel = scheme !== "wubi86" && options.model !== false && files.model;
  return { wasm: ref(files.wasm), db: ref(db), model: wantModel ? ref(files.model) : null, japanese: null, helpcodes: scheme === "wubi86" ? {} : tables };
}

// 辅助码表的地址；这次构建或自定义的 assets 里没有它时报 unsupported。
function helpcodeAsset(assets, schema, phase) {
  const asset = assets.helpcodes?.[schema];
  if (!asset) throw new MsimeError("unsupported", `this build of @msime/web-engine ships no ${schema} helpcode table; pass options.assets.helpcodes`, phase);
  return asset;
}

// 默认的 Worker。同源时写成打包器认得的 new Worker(new URL(..., import.meta.url))，Vite、webpack 5 会把 worker.js 一起打包；SDK 从 CDN 等其他源加载时，浏览器不允许直接用跨源脚本起 Worker，就用同源的 blob 脚本去 import 它。
function defaultWorker() {
  const url = new URL("./worker.js", import.meta.url);
  if (globalThis.location && url.origin !== globalThis.location.origin) {
    const blob = new Blob([`import ${JSON.stringify(url.href)};`], { type: "text/javascript" });
    const blobUrl = URL.createObjectURL(blob);
    return { worker: new Worker(blobUrl, { type: "module" }), release: () => URL.revokeObjectURL(blobUrl) };
  }
  return { worker: new Worker(new URL("./worker.js", import.meta.url), { type: "module" }), release: () => {} };
}

/**
 * 创建一个引擎，下载并初始化完成后 resolve。失败时 reject 一个 MsimeError，code 见 index.d.ts。
 */
export function createMsimeEngine(options = {}) {
  let scheme = options.scheme ?? "quanpin";
  if (!SCHEMES.includes(scheme)) return Promise.reject(new MsimeError("engine", `unknown scheme: ${scheme}`, "fetch"));
  const wanted = options.helpcode ?? null;
  if (wanted !== null && !HELPCODES.includes(wanted)) return Promise.reject(new MsimeError("engine", `unknown helpcode: ${wanted}`, "fetch"));
  let assets;
  // 辅助码只对全拼和双拼起作用，其他方案忽略这个选项，也不下载表。
  let helpcode = PINYIN.has(scheme) ? wanted : null;
  let initialHelpcode = null;
  try {
    assets = resolveAssets(scheme, options);
    if (helpcode !== null) initialHelpcode = { schema: helpcode, asset: helpcodeAsset(assets, helpcode, "fetch") };
  } catch (e) {
    return Promise.reject(e);
  }
  const { worker, release } = options.worker
    ? { worker: typeof options.worker === "function" ? options.worker() : options.worker, release: () => {} }
    : defaultWorker();

  let seq = 0;
  const pending = new Map();
  const errorListeners = new Set();
  let failure = null;
  let waitingReady = null;
  let info = null;

  const fail = (error) => {
    if (failure) return;
    failure = error;
    for (const { reject } of pending.values()) reject(error);
    pending.clear();
    waitingReady?.reject(error);
    waitingReady = null;
    worker.terminate();
    release();
    if (error.code === "disposed") return;
    for (const fn of errorListeners) fn(error);
  };

  worker.addEventListener("message", (e) => {
    const msg = e.data;
    switch (msg.type) {
      case "progress":
        options.onProgress?.(msg.loaded, msg.total);
        return;
      case "ready":
        info = msg;
        waitingReady?.resolve(msg);
        waitingReady = null;
        return;
      case "frame": {
        const entry = pending.get(msg.seq);
        pending.delete(msg.seq);
        entry?.resolve(msg.frame);
        return;
      }
      // 只让这一个请求失败、引擎照常可用的错误（辅助码表没下载下来）。
      case "rejected": {
        const entry = pending.get(msg.seq);
        pending.delete(msg.seq);
        entry?.reject(new MsimeError(msg.code, msg.message, "fetch"));
        return;
      }
      case "error":
        fail(new MsimeError(msg.code, msg.message, msg.phase));
        return;
    }
  });
  // Worker 脚本本身没加载起来（404、CSP 的 worker-src、语法错误）只会触发 error 事件，没有消息。
  worker.addEventListener("error", (e) => {
    e.preventDefault?.();
    fail(new MsimeError("engine", `msime worker failed to start: ${e.message || "see the browser console"}`, "fetch"));
  });

  const waitReady = () =>
    new Promise((resolve, reject) => {
      waitingReady = { resolve, reject };
    });

  const request = (msg) => {
    if (failure) return Promise.reject(failure);
    const id = ++seq;
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      worker.postMessage({ ...msg, seq: id });
    });
  };

  const engine = {
    get scheme() {
      return scheme;
    },
    get build() {
      return info?.build ?? "";
    },
    get memoryBytes() {
      return info?.memoryBytes ?? 0;
    },
    get timings() {
      return info?.timings;
    },
    get helpcode() {
      return helpcode;
    },
    keys(keys) {
      const list = (Array.isArray(keys) ? keys : [keys]).filter((k) => typeof k === "number");
      return request({ type: "keys", keys: list });
    },
    pick(slot) {
      return request({ type: "pick", slot });
    },
    reset() {
      return request({ type: "reset" });
    },
    async setScheme(next) {
      if (failure) throw failure;
      if (!SCHEMES.includes(next)) throw new MsimeError("engine", `unknown scheme: ${next}`);
      if (next === scheme) return;
      // 五笔和拼音用不同的词库，换词库要重新下载，建一个新引擎更简单也更省内存（wasm 内存不会缩小）。
      // 只有全拼和两种双拼共用一个词库；五笔、日语和韩文各要自己的资源，换过去得新建引擎。
      if (!PINYIN.has(next) || !PINYIN.has(scheme)) {
        throw new MsimeError("unsupported", `switching between ${scheme} and ${next} needs a new engine: dispose() this one and call createMsimeEngine({ scheme: "${next}" })`);
      }
      const ready = waitReady();
      worker.postMessage({ type: "scheme", seq: ++seq, scheme: next });
      await ready;
      scheme = next;
    },
    async setHelpcode(next) {
      if (failure) throw failure;
      const name = next ?? null;
      if (name !== null && !HELPCODES.includes(name)) throw new MsimeError("engine", `unknown helpcode: ${name}`);
      // 五笔、日语和韩文没有辅助码，记不记都一样，什么也不做。
      if (!PINYIN.has(scheme) || name === helpcode) return;
      const asset = name === null ? null : helpcodeAsset(assets, name, "runtime");
      await request({ type: "helpcode", schema: name, asset });
      helpcode = name;
    },
    setModelEnabled(enabled) {
      if (!failure) worker.postMessage({ type: "model", enabled: Boolean(enabled) });
    },
    setBackspaceDeletes(deletes) {
      if (!failure) worker.postMessage({ type: "backspace", deletes: Boolean(deletes) });
    },
    onError(fn) {
      errorListeners.add(fn);
      return () => errorListeners.delete(fn);
    },
    dispose() {
      fail(new MsimeError("disposed", "the engine was disposed"));
    },
  };

  const ready = waitReady();
  worker.postMessage({
    type: "init",
    scheme,
    assets,
    pageSize: options.pageSize ?? 9,
    modelEnabled: options.modelEnabled ?? true,
    helpcode: initialHelpcode,
  });
  return ready.then(() => engine);
}
