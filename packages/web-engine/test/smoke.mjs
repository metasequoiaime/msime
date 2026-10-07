// 对组装好的 npm 包（scripts/build-web-engine.sh 写出的 target/web-engine/npm/package）做端到端冒烟：用 Node 的 HTTP 服务器提供 wasm 和 gzip 过的词库，经 createMsimeEngine、worker.js 的消息处理、加载代码和 wasm 打 nihao + 空格，断言上屏；再验证不带词库的韩文、带最小日语模型的日语、CLI 的 copy、方案切换、点选、404 和缺词库时的错误。
//
// Node 没有浏览器的 Worker，这里用一个同进程的替身：把消息结构化克隆后交给 worker.js 导出的 createWorkerHandler，回复同样克隆后作为 message 事件派发。真正的 Worker 加载路径由浏览器测试覆盖。
//
// 用法：node packages/web-engine/test/smoke.mjs <msime.db> [--package <dir>]
// 词库通常是 `cargo run -p msime-engine-wasm --example make_fixture -- target/web-engine/fixture/msime.db` 写出的最小夹具。
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { gzipSync } from "node:zlib";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const args = process.argv.slice(2);
let db = null;
let pkgDir = resolve(repoRoot, "target/web-engine/npm/package");
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--package") pkgDir = resolve(args[++i] ?? "");
  else if (db === null && !args[i].startsWith("--")) db = resolve(args[i]);
  else throw new Error(`unknown argument: ${args[i]}`);
}
if (db === null) throw new Error("usage: node packages/web-engine/test/smoke.mjs <msime.db> [--package <dir>]");

const sdk = await import(pathToFileURL(join(pkgDir, "index.js")).href);
const { createWorkerHandler } = await import(pathToFileURL(join(pkgDir, "worker.js")).href);
const { KeyKind, packKey } = sdk;

// 资源服务器：/wasm 是包里的 wasm，/db.gz 是 gzip 后的夹具，其余 404。
const wasmBytes = readFileSync(join(pkgDir, "assets/msime_engine_bg.wasm"));
const rawDb = readFileSync(db);
const gzDb = gzipSync(rawDb);
// 最小的日语模型（MSJPDT1，布局见 crates/dict-builder/src/japanese.rs 的 pack）：にほん→日本、にほんご→日本語，1×1 连接矩阵。
function japaneseModel(entries) {
  const enc = new TextEncoder();
  const strings = [];
  let stringBytes = 0;
  const tokens = Buffer.alloc(entries.length * 20);
  entries.forEach(([reading, surface, cost], i) => {
    const at = i * 20;
    for (const [k, text] of [reading, surface].entries()) {
      const bytes = enc.encode(text);
      tokens.writeUInt32LE(stringBytes, at + k * 6);
      tokens.writeUInt16LE(bytes.length, at + 4 + k * 6);
      strings.push(bytes);
      stringBytes += bytes.length;
    }
    tokens.writeInt32LE(cost, at + 16);
  });
  const header = Buffer.alloc(56);
  header.write("MSJPDT1\0", 0, "latin1");
  header.writeUInt32LE(1, 8);
  header.writeUInt32LE(entries.length, 12);
  header.writeUInt32LE(1, 16);
  header.writeBigUInt64LE(56n, 24);
  header.writeBigUInt64LE(BigInt(56 + tokens.length), 32);
  header.writeBigUInt64LE(BigInt(56 + tokens.length + 2), 40);
  header.writeBigUInt64LE(BigInt(stringBytes), 48);
  return Buffer.concat([header, tokens, Buffer.alloc(2), ...strings]);
}
const rawJapanese = japaneseModel([
  ["にほん", "日本", 1000],
  ["にほんご", "日本語", 500],
]);
const gzJapanese = gzipSync(rawJapanese);

const server = createServer((req, res) => {
  if (req.url === "/msime-japanese.dat.gz") {
    res.writeHead(200, { "Content-Type": "application/gzip" });
    res.end(gzJapanese);
  } else if (req.url === "/msime_engine_bg.wasm") {
    res.writeHead(200, { "Content-Type": "application/wasm" });
    res.end(wasmBytes);
  } else if (req.url === "/msime.db.gz") {
    res.writeHead(200, { "Content-Type": "application/gzip" });
    res.end(gzDb);
  } else {
    res.writeHead(404);
    res.end();
  }
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const origin = `http://127.0.0.1:${server.address().port}`;

class InProcessWorker extends EventTarget {
  constructor() {
    super();
    this.terminated = false;
    this.queue = Promise.resolve();
    this.handle = createWorkerHandler(
      (msg) => {
        const data = structuredClone(msg);
        queueMicrotask(() => {
          if (!this.terminated) this.dispatchEvent(new MessageEvent("message", { data }));
        });
      },
      () => {},
    );
  }
  postMessage(msg) {
    const data = structuredClone(msg);
    this.queue = this.queue.then(() => this.handle(data));
  }
  terminate() {
    this.terminated = true;
  }
}

const assets = (dbPath = "/msime.db.gz") => ({
  wasm: { url: `${origin}/msime_engine_bg.wasm`, size: wasmBytes.length, rawSize: wasmBytes.length },
  db: { url: `${origin}${dbPath}`, size: gzDb.length, rawSize: rawDb.length },
  model: null,
});

const letters = (s) => [...s].map((c) => packKey(KeyKind.Letter, c.charCodeAt(0)));
let tmp = null;

try {
  // 1. 正常加载、组字、上屏，进度报到 100%。
  let progress = null;
  const engine = await sdk.createMsimeEngine({
    worker: () => new InProcessWorker(),
    assets: assets(),
    onProgress: (loaded, total) => (progress = { loaded, total }),
  });
  assert.match(engine.build, /^msime-engine-wasm /);
  assert.equal(engine.scheme, "quanpin");
  assert.ok(progress && progress.loaded === progress.total, `progress ended at ${JSON.stringify(progress)}`);
  const composing = await engine.keys(letters("nihao"));
  assert.ok(composing.composing && composing.page.length > 0, `nihao gave no candidates: ${JSON.stringify(composing)}`);
  const committed = await engine.keys(packKey(KeyKind.Space));
  const commits = committed.out.filter((o) => o.t === "commit");
  assert.equal(commits.length, 1, JSON.stringify(committed.out));
  assert.ok(commits[0].text.length > 0);
  assert.equal(committed.composing, false);
  console.log(`smoke: ${engine.build}: nihao + Space -> ${commits[0].text}`);

  // 2. 点选、同一词库内切换方案、重置。null 按键被忽略。
  await engine.keys([...letters("nihao"), null]);
  const picked = await engine.pick(0);
  assert.equal(picked.out.filter((o) => o.t === "commit").length, 1, `pick did not commit: ${JSON.stringify(picked.out)}`);
  await engine.setScheme("xiaohe");
  assert.equal(engine.scheme, "xiaohe");
  await assert.rejects(engine.setScheme("wubi86"), (e) => e.code === "unsupported");
  await assert.rejects(engine.setScheme("korean"), (e) => e.code === "unsupported");
  const reset = await engine.reset();
  assert.equal(reset.composing, false);

  // 3. dispose 之后的请求 reject，且不触发 onError。
  let errors = 0;
  engine.onError(() => errors++);
  engine.dispose();
  await assert.rejects(engine.keys(letters("a")), (e) => e.code === "disposed");
  assert.equal(errors, 0);

  // 3b. 韩文只下载 wasm：不导入词库也能拼音节，Shift 打双辅音，空格先上屏音节再打出自己。
  const korean = await sdk.createMsimeEngine({
    worker: () => new InProcessWorker(),
    scheme: "korean",
    assets: { wasm: assets().wasm, db: null, model: null },
  });
  assert.equal(korean.scheme, "korean");
  const hangul = await korean.keys([...letters("dkssu"), packKey(KeyKind.ShiftLetter, "T".charCodeAt(0))]);
  assert.deepEqual(hangul.out, [{ t: "commit", text: "안", seat: -1 }]);
  assert.equal(hangul.preedit, "녔");
  const spaced = await korean.keys(packKey(KeyKind.Space));
  assert.deepEqual(spaced.out, [{ t: "commit", text: "녔", seat: -1 }, { t: "type", text: " " }]);
  assert.equal(spaced.composing, false);
  await assert.rejects(korean.setScheme("quanpin"), (e) => e.code === "unsupported");
  korean.dispose();
  console.log("smoke: korean dkssuT + Space -> 안녔");

  // 3c. 日语下载并解压模型、交给引擎：组字区是假名，空格上屏整句转换，`,` 打出 、。
  const japanese = await sdk.createMsimeEngine({
    worker: () => new InProcessWorker(),
    scheme: "japanese",
    assets: {
      wasm: assets().wasm,
      db: null,
      model: null,
      japanese: { url: `${origin}/msime-japanese.dat.gz`, size: gzJapanese.length, rawSize: rawJapanese.length },
    },
  });
  assert.equal(japanese.scheme, "japanese");
  const kana = await japanese.keys(letters("nihongo"));
  assert.equal(kana.preedit, "にほんご");
  assert.equal(kana.page[0]?.text, "日本語", JSON.stringify(kana.page));
  const converted = await japanese.keys([packKey(KeyKind.Space), packKey(KeyKind.Punct, ",".charCodeAt(0))]);
  assert.deepEqual(converted.out, [{ t: "commit", text: "日本語", seat: 0 }, { t: "commit", text: "、", seat: -1 }]);
  await assert.rejects(japanese.setScheme("quanpin"), (e) => e.code === "unsupported");
  japanese.dispose();
  console.log("smoke: japanese nihongo + Space -> 日本語");

  // 4. 词库 404：network，消息里提示部署位置。
  await assert.rejects(
    sdk.createMsimeEngine({ worker: () => new InProcessWorker(), assets: assets("/missing.db.gz") }),
    (e) => e.code === "network" && /HTTP 404/.test(e.message) && /msime-web-engine copy/.test(e.message),
  );

  // 5. 不传 assets 时按包里的 assets.js 找词库；--no-data 构建没有词库，应当明确报 unsupported，而有词库的构建应当解析出 assetBase 下的地址。
  if (sdk.version && !existsSync(join(pkgDir, "assets/msime-pinyin.db.gz"))) {
    await assert.rejects(sdk.createMsimeEngine({ worker: () => new InProcessWorker() }), (e) => e.code === "unsupported");
  }
  await assert.rejects(sdk.createMsimeEngine({ scheme: "shuangpin", worker: () => new InProcessWorker() }), /unknown scheme/);

  // 6. CLI：copy 出的目录自成一体，含运行时、wasm、NOTICE 和清单。
  tmp = mkdtempSync(join(tmpdir(), "msime-web-engine-"));
  const out = join(tmp, "site/msime");
  execFileSync(process.execPath, [join(pkgDir, "bin/msime-web-engine.mjs"), "copy", out, "--no-model"], { stdio: "pipe" });
  for (const f of ["index.js", "input.js", "keys.js", "skin.js", "candidates.js", "theme-catalog.js", "worker.js", "assets.js", "msime_engine.js", "assets/msime_engine_bg.wasm", "assets/NOTICE.md", "assets/web-engine-manifest.json"]) {
    assert.ok(existsSync(join(out, f)), `copy did not write ${f}`);
  }
  assert.ok(!existsSync(join(out, "assets/sentence-model.safetensors.gz")), "--no-model still copied the model");
  const copied = await import(pathToFileURL(join(out, "index.js")).href);
  assert.equal(copied.version, sdk.version);
  // 皮肤表是构建时生成进包里的：复制出的目录也能解析每个内置皮肤。
  assert.ok(copied.SKINS.includes("wechat") && copied.SKINS.includes(copied.DEFAULT_SKIN));
  for (const id of copied.SKINS) assert.match(copied.resolveSkin(id).variables["--cand-bg"], /^#[0-9A-F]{6}([0-9A-F]{2})?$/);
  assert.equal(typeof copied.createCandidateBar, "function");

  // 7. 只要候选栏的页面从 `@msime/web-engine/candidates.js` 导入：包的 exports 列出这个子路径和它的类型，两个文件都在包里，导出的就是入口里的那个候选栏。
  const manifest = JSON.parse(readFileSync(join(pkgDir, "package.json"), "utf8"));
  assert.deepEqual(manifest.exports["./candidates.js"], { types: "./candidates.d.ts", default: "./candidates.js" });
  for (const f of ["candidates.js", "candidates.d.ts"]) {
    assert.ok(manifest.files.includes(f) && existsSync(join(pkgDir, f)), `package is missing ${f}`);
  }
  const candidates = await import(pathToFileURL(join(pkgDir, "candidates.js")).href);
  assert.equal(candidates.createCandidateBar, sdk.createCandidateBar);
  assert.equal(candidates.CANDIDATE_BAR_TAG, sdk.CANDIDATE_BAR_TAG);
  console.log("smoke: ok");
} finally {
  server.close();
  if (tmp) rmSync(tmp, { recursive: true, force: true });
}
