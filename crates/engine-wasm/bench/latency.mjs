// 网页引擎的按键延迟基准：在 Node 里加载发布用的 wasm、拼音库和整句模型，把 resources/eval/sentences-v1.tsv 的每条输入逐键送进 `WebEngine.keys()`，统计每次调用的耗时。
//
// 它量的是 Worker 里一次 keys() 的成本（按键路由、引擎查询、模型重排、把帧转成 JS 对象），不含 postMessage 往返和页面绘制。网页端每帧只处理一批按键，这里一键一批，是最坏的情况。release-web-engine.yml 用 `--max-p95 60` 卡门槛。
//
// 模型开着测：每条输入开始前 `set_model_enabled(true)`，它同时清掉慢帧熔断（连续三次重排超过 120 ms 就关模型），否则机器一慢模型被熔断，量出来的反而是不带模型的数字。熔断被触发的条数会单独报告。
//
// 用法：node crates/engine-wasm/bench/latency.mjs [--dist <dist 目录>] [--set <tsv>] [--scheme quanpin] [--no-model] [--max-p95 <ms>]
// dist 默认 target/web-engine/dist（scripts/build-web-engine.sh 带 --pinyin --wubi --model 的产物），set 默认 resources/eval/sentences-v1.tsv。只用 Node 22+ 自带的模块。
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { gunzipSync } from "node:zlib";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");

function parseArgs(argv) {
  const options = {
    dist: resolve(repoRoot, "target/web-engine/dist"),
    set: resolve(repoRoot, "resources/eval/sentences-v1.tsv"),
    scheme: "quanpin",
    model: true,
    maxP95: null,
  };
  for (let i = 0; i < argv.length; i++) {
    const value = () => {
      const next = argv[++i];
      if (next === undefined) throw new Error(`${argv[i - 1]} needs a value`);
      return next;
    };
    switch (argv[i]) {
      case "--dist":
        options.dist = resolve(value());
        break;
      case "--set":
        options.set = resolve(value());
        break;
      case "--scheme":
        options.scheme = value();
        break;
      case "--no-model":
        options.model = false;
        break;
      case "--max-p95": {
        options.maxP95 = Number(value());
        if (!Number.isFinite(options.maxP95)) throw new Error("--max-p95 must be a number");
        break;
      }
      default:
        throw new Error(`unknown argument: ${argv[i]}`);
    }
  }
  return options;
}

// 评测集的格式：`#` 开头是注释，第一列 id，第二列按键序列。
function loadCases(path) {
  const cases = [];
  for (const line of readFileSync(path, "utf8").split("\n")) {
    if (line === "" || line.startsWith("#") || line.startsWith("id\t")) continue;
    const fields = line.split("\t");
    if (fields.length < 2) throw new Error(`${path}: malformed row: ${line}`);
    cases.push({ id: fields[0], input: fields[1] });
  }
  return cases;
}

// 与 crates/engine-wasm/src/host.rs 的 Key::pack 一致：(kind << 8) | ascii。
const LETTER = 1;
const DIGIT = 3;
const PUNCT = 13;
function packChar(c) {
  const code = c.charCodeAt(0);
  if (c >= "a" && c <= "z") return (LETTER << 8) | code;
  if (c >= "0" && c <= "9") return (DIGIT << 8) | code;
  if (code > 0x20 && code < 0x7f) return (PUNCT << 8) | code;
  throw new Error(`cannot pack key ${JSON.stringify(c)}`);
}

// 最近秩百分位，和 crates/input-runtime/examples/rerank_latency.rs 的算法相同。
function percentile(sorted, p) {
  if (sorted.length === 0) return 0;
  const rank = Math.max(1, Math.ceil((p / 100) * sorted.length));
  return sorted[Math.min(rank, sorted.length) - 1];
}

const options = parseArgs(process.argv.slice(2));
const isWubi = options.scheme === "wubi86";
const glue = await import(pathToFileURL(resolve(options.dist, "msime_engine.js")).href);
glue.initSync({ module: readFileSync(resolve(options.dist, "msime_engine_bg.wasm")) });

const dbFile = isWubi ? "msime-wubi86.db.gz" : "msime-pinyin.db.gz";
glue.import_database("/res/msime-pinyin.db", new Uint8Array(gunzipSync(readFileSync(resolve(options.dist, dbFile)))));
const model =
  options.model && !isWubi ? new Uint8Array(gunzipSync(readFileSync(resolve(options.dist, "sentence-model.safetensors.gz")))) : undefined;

const cases = loadCases(options.set);
const engine = new glue.WebEngine(options.scheme, 9, model);
const samples = [];
const rerank = [];
let breakerCases = 0;
const started = performance.now();
try {
  for (const testCase of cases) {
    engine.reset();
    if (model) engine.set_model_enabled(true);
    let tripped = false;
    for (const c of testCase.input) {
      const keys = Uint32Array.of(packChar(c));
      const before = performance.now();
      const frame = engine.keys(keys);
      samples.push(performance.now() - before);
      if (frame.rerankMs > 0) rerank.push(frame.rerankMs);
      if (model && !frame.modelOn) tripped = true;
    }
    if (tripped) breakerCases++;
  }
} finally {
  engine.free();
}
const panic = glue.last_panic();
if (panic !== "") {
  console.error(`latency: engine panicked: ${panic}`);
  process.exit(1);
}

const sorted = [...samples].sort((a, b) => a - b);
const mean = samples.reduce((sum, value) => sum + value, 0) / Math.max(1, samples.length);
const p50 = percentile(sorted, 50);
const p95 = percentile(sorted, 95);
const max = percentile(sorted, 100);
const rerankSorted = [...rerank].sort((a, b) => a - b);
console.log(`latency: ${glue.build_info()}, scheme ${options.scheme}, model ${model ? "on" : "off"}`);
console.log(`latency: ${cases.length} inputs, ${samples.length} keys() calls in ${((performance.now() - started) / 1000).toFixed(1)} s`);
console.log(
  `latency: keys() mean ${mean.toFixed(2)} ms  p50 ${p50.toFixed(2)} ms  p95 ${p95.toFixed(2)} ms  max ${max.toFixed(2)} ms`,
);
if (model) {
  console.log(
    `latency: rerank p50 ${percentile(rerankSorted, 50).toFixed(2)} ms  p95 ${percentile(rerankSorted, 95).toFixed(2)} ms over ${rerank.length} reranks; slow-frame breaker tripped in ${breakerCases} of ${cases.length} inputs`,
  );
}
if (options.maxP95 !== null) {
  if (p95 > options.maxP95) {
    console.error(`latency: FAIL: p95 ${p95.toFixed(2)} ms is over the ${options.maxP95} ms gate`);
    process.exit(1);
  }
  console.log(`latency: p95 within the ${options.maxP95} ms gate`);
}
