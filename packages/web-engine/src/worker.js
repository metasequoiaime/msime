// 引擎所在的 Worker：编译 wasm，同时下载并解压词库和整句模型，把词库导入引擎的内存文件系统，然后按到达顺序回答主线程的按键、点选和重置请求，每个请求回一帧。辅助码表在打开辅助码时才下载，解压后交给引擎。放在 Worker 里，一次慢的重排也卡不住页面。
//
// 与 TapTapGo 的 apps/web/src/features/msime/msime.worker.ts 同源；这里不依赖任何框架，消息协议见 index.d.ts 的 ToWorker / FromWorker。
import init, { WebEngine, build_info, import_database, import_japanese_dictionary, last_panic } from "./msime_engine.js";

// 引擎读主词库的位置。wasm32 上读不到文件元数据，引擎不会退回旧名 msime.db，所以必须是这个名字；拼音方案和五笔都放这里，见 crates/engine-wasm/src/host.rs 的 WebHost::new。
const DB_PATH = "/res/msime-pinyin.db";
// 下载期间最多每隔这么久报一次进度。
const PROGRESS_MS = 100;

class LoadError extends Error {
  constructor(code, message) {
    super(message);
    this.code = code;
  }
}

const messageOf = (e) => (e instanceof Error ? e.message : String(e));

// 加载时异常对应的错误码：CSP 没放行 'wasm-unsafe-eval' 时各浏览器的报错都会提到策略，地址空间不够是 memory，请求失败是 network。
function codeOf(e, phase) {
  if (e instanceof LoadError) return e.code;
  const message = messageOf(e);
  if (/wasm-unsafe-eval|unsafe-eval|content security policy|\bCSP\b/i.test(message)) return "csp";
  if (e instanceof RangeError || /out of memory|memory/i.test(message)) return "memory";
  if (phase === "fetch" && e instanceof TypeError) return "network";
  return "engine";
}

class PhaseError extends Error {
  constructor(phase, inner) {
    super(messageOf(inner));
    this.phase = phase;
    this.inner = inner;
  }
}

const inPhase = (phase, p) =>
  p.catch((e) => {
    throw new PhaseError(phase, e);
  });

async function request(url, signal) {
  let res;
  try {
    res = await fetch(url, { signal });
  } catch (e) {
    throw new LoadError("network", `${url}: ${messageOf(e)}`);
  }
  if (!res.ok || res.body === null) {
    // 404 几乎总是资源没部署到 assetBase 指向的位置。
    const hint = res.status === 404 ? " (are the engine files deployed there? see `npx msime-web-engine copy`)" : "";
    throw new LoadError("network", `${url}: HTTP ${res.status}${hint}`);
  }
  return res;
}

function counted(body, count) {
  return body.pipeThrough(
    new TransformStream({
      transform(chunk, controller) {
        count(chunk.byteLength);
        controller.enqueue(chunk);
      },
    }),
  );
}

// 下载一个 gzip 文件并解压到正好 rawSize 大小的缓冲区；大小不符说明不是这次构建的那个文件。
async function gunzip(asset, signal, count) {
  const res = await request(asset.url, signal);
  const out = new Uint8Array(asset.rawSize);
  let at = 0;
  // 有的服务器给 .gz 文件加 Content-Encoding: gzip（vite preview、部分 CDN），浏览器会先解压，拿到的就是原始字节；没加时由这里解压。进度仍按传输字节计，从原始字节按比例折算。
  const decoded = /\bgzip\b/i.test(res.headers.get("Content-Encoding") ?? "");
  try {
    const body = decoded
      ? counted(res.body, (n) => count((n * asset.size) / asset.rawSize))
      : counted(res.body, count).pipeThrough(new DecompressionStream("gzip"));
    const reader = body.getReader();
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      if (at + value.byteLength > out.length) {
        await reader.cancel();
        throw new LoadError("engine", `${asset.url}: larger than ${asset.rawSize} bytes`);
      }
      out.set(value, at);
      at += value.byteLength;
    }
  } catch (e) {
    if (e instanceof LoadError) throw e;
    // 下载中途断开是网络问题，解压失败是文件不对。
    throw new LoadError(e instanceof TypeError ? "network" : "engine", `${asset.url}: ${messageOf(e)}`);
  }
  if (at !== out.length) throw new LoadError("engine", `${asset.url}: ${at} of ${asset.rawSize} bytes`);
  return out;
}

/** 消息处理函数。postMessage 和 close 由调用方传入，测试可以不起 Worker 直接驱动它。 */
export function createWorkerHandler(post, close) {
  let engine = null;
  let model = null;
  let pageSize = 9;
  let modelEnabled = true;
  let backspaceDeletes = true;
  // 当前辅助码表解压后的字节，关着时为 null；换方案重建引擎时再交给新引擎。下载过的表按方案名留着，来回切换不再下载。
  let helpcode = null;
  const helpcodeTables = new Map();
  let memory = null;
  let dead = false;

  const newEngine = (scheme) => {
    const e = new WebEngine(scheme, pageSize, model);
    if (!modelEnabled) e.set_model_enabled(false);
    if (!backspaceDeletes) e.set_backspace_deletes(false);
    if (helpcode) e.set_helpcode(helpcode);
    return e;
  };

  const die = (code, message, phase) => {
    dead = true;
    engine = null;
    post({ type: "error", phase, code, message });
    close();
  };

  const runtimeFailure = (e) =>
    die(e instanceof WebAssembly.RuntimeError ? "panic" : "engine", (e instanceof WebAssembly.RuntimeError && last_panic()) || messageOf(e), "runtime");

  // 引擎回答一帧的请求。wasm 是 panic=abort，trap 之后引擎状态已经没了，Worker 随之结束。
  const answer = (seq, run) => {
    if (engine === null) return;
    const t0 = performance.now();
    let frame;
    try {
      frame = run(engine);
    } catch (e) {
      runtimeFailure(e);
      return;
    }
    post({ type: "frame", seq, frame, ms: performance.now() - t0 });
  };

  const load = async (msg) => {
    const { scheme, assets } = msg;
    pageSize = msg.pageSize ?? pageSize;
    modelEnabled = msg.modelEnabled ?? modelEnabled;
    if (typeof WebAssembly === "undefined" || typeof DecompressionStream === "undefined") {
      die("unsupported", "msime: WebAssembly or DecompressionStream is missing", "fetch");
      return;
    }
    const timings = { fetch: 0, compile: 0, import: 0, session: 0 };
    const helpcodeAsset = msg.helpcode?.asset ?? null;
    const total = assets.wasm.size + (assets.db?.size ?? 0) + (assets.model?.size ?? 0) + (assets.japanese?.size ?? 0) + (helpcodeAsset?.size ?? 0);
    let loaded = 0;
    let lastPost = 0;
    const count = (n) => {
      loaded += n;
      const now = performance.now();
      if (now - lastPost < PROGRESS_MS) return;
      lastPost = now;
      post({ type: "progress", loaded, total });
    };
    const abort = new AbortController();
    const t0 = performance.now();
    // 数据比 wasm 大，两者都在关键路径上，所以边下载数据边编译 wasm。
    const compile = inPhase(
      "compile",
      (async () => {
        const res = await request(assets.wasm.url, abort.signal);
        const streamed = new Response(counted(res.body, count), { status: res.status, headers: res.headers });
        const out = await init({ module_or_path: streamed });
        memory = out.memory ?? null;
        timings.compile = performance.now() - t0;
      })(),
    );
    const data = inPhase(
      "fetch",
      (async () => {
        const [db, m, japanese, table] = await Promise.all([
          assets.db ? gunzip(assets.db, abort.signal, count) : Promise.resolve(null),
          assets.model ? gunzip(assets.model, abort.signal, count) : Promise.resolve(null),
          assets.japanese ? gunzip(assets.japanese, abort.signal, count) : Promise.resolve(null),
          helpcodeAsset ? gunzip(helpcodeAsset, abort.signal, count) : Promise.resolve(null),
        ]);
        timings.fetch = performance.now() - t0;
        return { db, model: m, japanese, helpcode: table };
      })(),
    );
    let files;
    try {
      [, files] = await Promise.all([compile, data]);
    } catch (e) {
      abort.abort();
      die(codeOf(e.inner, e.phase), e.message, e.phase);
      return;
    }
    post({ type: "progress", loaded: total, total });
    let phase = "import";
    try {
      const t1 = performance.now();
      // 日语和韩文没有 SQLite 词库要导入；日语模型直接交给引擎，要在创建日语引擎之前。
      if (files.db) import_database(DB_PATH, files.db);
      if (files.japanese) import_japanese_dictionary(files.japanese);
      timings.import = performance.now() - t1;
      phase = "session";
      const t2 = performance.now();
      model = files.model;
      if (files.helpcode) {
        helpcode = files.helpcode;
        helpcodeTables.set(msg.helpcode.schema, helpcode);
      }
      engine = newEngine(scheme);
      timings.session = performance.now() - t2;
    } catch (e) {
      die(codeOf(e, phase), messageOf(e), phase);
      return;
    }
    post({ type: "ready", build: build_info(), scheme, timings, memoryBytes: memory?.buffer.byteLength ?? 0 });
  };

  return async (msg) => {
    if (dead) return;
    switch (msg.type) {
      case "init":
        await load(msg);
        return;
      case "scheme": {
        // 全拼和两种双拼读同一个词库，词库留在内存里，只重建引擎。
        const t0 = performance.now();
        try {
          engine?.free();
          engine = newEngine(msg.scheme);
        } catch (e) {
          runtimeFailure(e);
          return;
        }
        post({
          type: "ready",
          build: build_info(),
          scheme: msg.scheme,
          timings: { fetch: 0, compile: 0, import: 0, session: performance.now() - t0 },
          memoryBytes: memory?.buffer.byteLength ?? 0,
        });
        return;
      }
      case "keys":
        answer(msg.seq, (e) => e.keys(Uint32Array.from(msg.keys)));
        return;
      case "pick":
        answer(msg.seq, (e) => e.pick(msg.slot));
        return;
      case "reset":
        answer(msg.seq, (e) => e.reset());
        return;
      case "helpcode": {
        // 关掉，或者换成另一张表：先下载（失败只让这个请求 reject，引擎照常可用），再交给引擎。之后的按键在队列里等它。
        if (engine === null) return;
        let table = null;
        if (msg.asset) {
          table = helpcodeTables.get(msg.schema) ?? null;
          if (table === null) {
            try {
              table = await gunzip(msg.asset, undefined, () => {});
            } catch (e) {
              post({ type: "rejected", seq: msg.seq, code: codeOf(e, "fetch"), message: messageOf(e) });
              return;
            }
            helpcodeTables.set(msg.schema, table);
          }
        }
        if (engine === null) return;
        try {
          engine.set_helpcode(table ?? undefined);
        } catch (e) {
          // trap 之后引擎没了；引擎拒收这张表（超过 1 MiB）时原来的设置不变，只让这个请求失败。
          if (e instanceof WebAssembly.RuntimeError) runtimeFailure(e);
          else post({ type: "rejected", seq: msg.seq, code: "engine", message: messageOf(e) });
          return;
        }
        helpcode = table;
        post({ type: "frame", seq: msg.seq, frame: null, ms: 0 });
        return;
      }
      case "model":
        modelEnabled = msg.enabled;
        engine?.set_model_enabled(msg.enabled);
        return;
      case "backspace":
        backspaceDeletes = msg.deletes;
        engine?.set_backspace_deletes(msg.deletes);
        return;
    }
  };
}

// 作为 Worker 脚本运行时接上消息循环；在 Node 或主线程里 import 时什么也不做。
if (typeof WorkerGlobalScope !== "undefined" && globalThis instanceof WorkerGlobalScope) {
  const handle = createWorkerHandler(
    (msg) => globalThis.postMessage(msg),
    () => globalThis.close(),
  );
  // 逐条处理：引擎加载期间打的键排队等它，顺序不变。
  let queue = Promise.resolve();
  globalThis.addEventListener("message", (e) => {
    queue = queue.then(() => handle(e.data));
  });
}
