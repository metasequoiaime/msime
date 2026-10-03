// 冒烟测试：用 Node 加载 scripts/build-web-engine.sh 产出的 dist/msime_engine.js 和 msime_engine_bg.wasm，导入词库，打 `nihao` + 空格，断言上屏了非空文字。它验证的是发布出去的那两个文件本身能在 JS 里跑通（wasm-bindgen 加载代码、启动函数、内存 VFS、WebEngine 的边界），不验证候选质量，候选质量由 routing.rs 和 parity.rs 管。
//
// 用法：node crates/engine-wasm/tests/smoke.mjs <词库路径> [--dist <dist 目录>]
// 词库通常是 `cargo run -p msime-engine-wasm --example make_fixture -- target/web-engine/fixture/msime.db` 写出的最小夹具；dist 默认是 target/web-engine/dist。只用 Node 22+ 自带的模块。
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");

function parseArgs(argv) {
  let db = null;
  let dist = resolve(repoRoot, "target/web-engine/dist");
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--dist") {
      dist = resolve(argv[++i] ?? "");
    } else if (db === null && !argv[i].startsWith("--")) {
      db = resolve(argv[i]);
    } else {
      throw new Error(`unknown argument: ${argv[i]}`);
    }
  }
  if (db === null) {
    throw new Error("usage: node crates/engine-wasm/tests/smoke.mjs <msime.db> [--dist <dir>]");
  }
  return { db, dist };
}

// 与 crates/engine-wasm/src/host.rs 的 Key::pack 一致：(kind << 8) | ascii。
const LETTER = 1;
const SPACE = 4;
const pack = (kind, byte = 0) => (kind << 8) | byte;

function fail(message) {
  console.error(`smoke: FAIL: ${message}`);
  process.exit(1);
}

const { db, dist } = parseArgs(process.argv.slice(2));
const glue = await import(pathToFileURL(resolve(dist, "msime_engine.js")).href);
// 加载代码用 --omit-default-module-path 生成，没有默认的 wasm 路径，必须显式传入。
glue.initSync({ module: readFileSync(resolve(dist, "msime_engine_bg.wasm")) });

const info = glue.build_info();
if (!info.startsWith("msime-engine-wasm ")) {
  fail(`unexpected build_info: ${info}`);
}
glue.import_database("/res/msime.db", new Uint8Array(readFileSync(db)));

const engine = new glue.WebEngine("quanpin", 9);
try {
  const keys = [..."nihao"].map((c) => pack(LETTER, c.charCodeAt(0)));
  const composing = engine.keys(Uint32Array.from(keys));
  if (!composing.composing || composing.page.length === 0) {
    fail(`typing nihao gave no candidates: ${JSON.stringify(composing)}`);
  }
  const frame = engine.keys(Uint32Array.of(pack(SPACE)));
  const commits = frame.out.filter((item) => item.t === "commit");
  if (commits.length !== 1 || typeof commits[0].text !== "string" || commits[0].text === "") {
    fail(`nihao + Space did not commit text: ${JSON.stringify(frame.out)}`);
  }
  if (frame.composing) {
    fail("still composing after the commit");
  }
  const panic = glue.last_panic();
  if (panic !== "") {
    fail(`last_panic reports: ${panic}`);
  }
  console.log(`smoke: ${info}: nihao + Space -> ${commits[0].text} (seat ${commits[0].seat})`);
} finally {
  engine.free();
}
glue.delete_database("/res/msime.db");
console.log("smoke: ok");
