// 用可控 Worker 驱动真实 SDK，逐条交付回复，不依赖计时或真实词库下载。
import assert from "node:assert/strict";
import { cpSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { pathToFileURL } from "node:url";
import { writeThemeCatalog } from "../tools/theme-catalog.mjs";

const fixture = mkdtempSync(join(tmpdir(), "msime-sdk-schemes-"));
after(() => rmSync(fixture, { recursive: true, force: true }));
cpSync(new URL("../src/", import.meta.url), fixture, { recursive: true });
writeFileSync(join(fixture, "package.json"), '{"type":"module"}');
writeFileSync(join(fixture, "assets.js"), 'export const files = {}, helpcodes = {}, cantonese = null, version = "synthetic";');
writeThemeCatalog(join(fixture, "theme-catalog.js"));
const { createMsimeEngine } = await import(pathToFileURL(join(fixture, "index.js")).href);
// 此用例只核对真实 Worker 的消息编号；引擎替身不执行词库、排序或按键算法。
writeFileSync(join(fixture, "msime_engine.js"), `
  export default async function init({ module_or_path }) { await module_or_path.arrayBuffer(); return {}; }
  export const selectionCalls = [];
  export class WebEngine {
    free() {}
    keys() { return { out: [] }; }
    pick(slot) { selectionCalls.push(["pick", slot]); return { out: [] }; }
    pick_at(...args) { selectionCalls.push(["pick_at", ...args]); return { out: [] }; }
    set_candidate_expansion(...args) { selectionCalls.push(["expansion", ...args]); return { out: [] }; }
  }
  export const build_info = () => "synthetic";
  export const import_database = () => {};
  export const import_japanese_dictionary = () => {};
  export const last_panic = () => "";
`);
const { createWorkerHandler } = await import(pathToFileURL(join(fixture, "worker.js")).href);
const { selectionCalls } = await import(pathToFileURL(join(fixture, "msime_engine.js")).href);

class ControlledWorker extends EventTarget {
  messages = [];
  terminated = false;
  postMessage(message) {
    this.messages.push(message);
    if (message.type === "init") {
      queueMicrotask(() => this.reply({ type: "ready", scheme: message.scheme }));
    }
  }
  reply(message) {
    this.dispatchEvent(new MessageEvent("message", { data: message }));
  }
  ready(index) {
    const message = this.messages.filter((item) => item.type === "scheme")[index];
    assert.ok(message, `missing scheme request ${index}`);
    this.reply({ type: "ready", seq: message.seq, scheme: message.scheme });
  }
  terminate() { this.terminated = true; }
}

const flush = () => new Promise((resolve) => setImmediate(resolve));
async function setup(t) {
  const worker = new ControlledWorker();
  const engine = await createMsimeEngine({ worker, assets: {} });
  t.after(() => engine.dispose());
  return { worker, engine };
}

test("the SDK carries candidate identity and displayed text across every selection path", async (t) => {
  const { worker, engine } = await setup(t);
  const calls = [engine.pick(2, "synthetic:7", "示例甲", 9), engine.pickAny(19, "synthetic:7", "示例乙"), engine.setCandidateExpansion(true, "synthetic:7")];
  const messages = worker.messages.filter((message) => message.type !== "init");
  assert.deepEqual(messages.map(({ seq, ...message }) => message), [
    { type: "pick", slot: 2, revision: "synthetic:7", text: "示例甲", pageStart: 9 },
    { type: "pickAny", index: 19, revision: "synthetic:7", text: "示例乙" },
    { type: "expansion", open: true, revision: "synthetic:7" },
  ]);
  for (const message of messages) worker.reply({ type: "frame", seq: message.seq, frame: { out: [] } });
  await Promise.all(calls);
});

test("the production Worker guards global and current-page selections before WASM conversion", async () => {
  const replies = [];
  const handle = createWorkerHandler((message) => replies.push(message), () => assert.fail("unexpected worker failure"));
  await handle({ type: "init", scheme: "quanpin", assets: { wasm: { url: "data:application/octet-stream,synthetic", size: 9 } } });
  selectionCalls.length = 0;
  await handle({ type: "expansion", seq: 1, open: true, revision: "synthetic:4" });
  await handle({ type: "pickAny", seq: 2, index: 24, revision: "synthetic:5", text: "示例甲" });
  await handle({ type: "pick", seq: 3, slot: 2, revision: "synthetic:5", text: "示例乙", pageStart: 9 });
  await handle({ type: "pick", seq: 4, slot: 1 });
  for (const index of [-1, 1.5, NaN, 0x100000000]) {
    await handle({ type: "pickAny", seq: 5, index, revision: "synthetic:5", text: "示例甲" });
    await handle({ type: "pick", seq: 6, slot: index, revision: "synthetic:5", text: "示例甲", pageStart: 0 });
    await handle({ type: "pick", seq: 7, slot: index });
  }
  for (const revision of [null, 4, {}]) {
    await handle({ type: "pick", seq: 8, slot: 0, revision, text: "示例甲", pageStart: 0 });
  }
  assert.deepEqual(selectionCalls, [
    ["expansion", true, "synthetic:4"],
    ["pick_at", 24, "synthetic:5", "示例甲", true],
    ["pick_at", 2, "synthetic:5", "示例乙", false, 9],
    ["pick", 1],
  ]);
  assert.equal(replies.filter((reply) => reply.type === "frame").length, 19);
});

test("concurrent scheme switches each wait for their own ready reply", async (t) => {
  const { worker, engine } = await setup(t);
  let first = false, second = false;
  const one = engine.setScheme("xiaohe").then(() => { first = true; });
  const two = engine.setScheme("ziranma").then(() => { second = true; });
  void one.catch(() => {});
  void two.catch(() => {});
  worker.ready(0);
  await flush();
  assert.equal(first, true);
  assert.equal(second, false);
  assert.equal(engine.scheme, "xiaohe");
  worker.ready(1);
  await Promise.all([one, two]);
  assert.equal(second, true);
  assert.equal(engine.scheme, "ziranma");
});

test("switching back to the current scheme still queues behind an unfinished switch", async (t) => {
  const { worker, engine } = await setup(t);
  const one = engine.setScheme("xiaohe");
  let returned = false;
  const two = engine.setScheme("quanpin").then(() => { returned = true; });
  void one.catch(() => {});
  void two.catch(() => {});
  await flush();
  assert.equal(returned, false);
  assert.equal(worker.messages.filter((item) => item.type === "scheme").length, 2);
  worker.ready(0);
  await one;
  assert.equal(engine.scheme, "xiaohe");
  worker.ready(1);
  await two;
  assert.equal(engine.scheme, "quanpin");
});

for (const failure of ["dispose", "runtime"]) {
  test(`${failure} rejects every unfinished scheme switch`, async (t) => {
    const { worker, engine } = await setup(t);
    const codes = [];
    void engine.setScheme("xiaohe").catch((error) => codes.push(error.code));
    void engine.setScheme("ziranma").catch((error) => codes.push(error.code));
    if (failure === "dispose") engine.dispose();
    else worker.reply({ type: "error", code: "engine", message: "synthetic failure", phase: "runtime" });
    await flush();
    assert.deepEqual(codes, [failure === "dispose" ? "disposed" : "engine", failure === "dispose" ? "disposed" : "engine"]);
    assert.equal(worker.terminated, true);
  });
}

test("an idle redundant scheme switch sends no request", async (t) => {
  const { worker, engine } = await setup(t);
  await engine.setScheme("quanpin");
  assert.equal(worker.messages.filter((item) => item.type === "scheme").length, 0);
});


test("the production Worker keeps initial ready unnumbered and echoes each switch sequence", async () => {
  const replies = [];
  const handle = createWorkerHandler((message) => replies.push(message), () => assert.fail("unexpected worker failure"));
  await handle({ type: "init", scheme: "quanpin", assets: {
    wasm: { url: "data:application/octet-stream,synthetic", size: 9 },
  } });
  const initial = replies.find((message) => message.type === "ready");
  assert.ok(initial);
  assert.equal(initial.seq, undefined);
  await handle({ type: "scheme", seq: 41, scheme: "xiaohe" });
  await handle({ type: "scheme", seq: 42, scheme: "quanpin" });
  assert.deepEqual(replies.filter((message) => message.type === "ready").map(({ seq, scheme }) => [seq, scheme]), [
    [undefined, "quanpin"], [41, "xiaohe"], [42, "quanpin"],
  ]);
});


test("scheme switches preserve the order of interleaved key requests", async (t) => {
  const { worker, engine } = await setup(t);
  const first = engine.setScheme("xiaohe");
  const keys = engine.keys(0x16e);
  const second = engine.setScheme("ziranma");
  void first.catch(() => {});
  void keys.catch(() => {});
  void second.catch(() => {});
  const messages = worker.messages.filter((message) => message.type !== "init");
  assert.deepEqual(messages.map((message) => message.type), ["scheme", "keys", "scheme"]);
  worker.ready(0);
  await first;
  const frame = { composing: true, preedit: "n", out: [] };
  worker.reply({ type: "frame", seq: messages[1].seq, frame });
  assert.deepEqual(await keys, frame);
  worker.ready(1);
  await second;
  assert.equal(engine.scheme, "ziranma");
});
