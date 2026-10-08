#!/usr/bin/env node
// 把 SDK 的运行时和资源复制到静态站点目录（GitHub Pages、Vercel、Cloudflare Pages / Workers 的 public 或输出目录）。复制出来的目录自成一体：页面直接 import 其中的 index.js，资源就在它旁边的 assets/，Worker 与页面同源。
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const RUNTIME = ["index.js", "index.d.ts", "keys.js", "input.js", "skin.js", "candidates.js", "theme-catalog.js", "worker.js", "assets.js", "msime_engine.js"];

function usage(code) {
  const text = `msime-web-engine ${pkg.version}

usage:
  msime-web-engine copy <dir> [--no-wubi] [--no-pinyin] [--no-japanese] [--no-model] [--no-helpcode] [--clean]
  msime-web-engine info

copy   复制运行时和资源到 <dir>。页面里 import "<dir 对应的 URL>/index.js" 即可，无需设置 assetBase。
       --no-wubi / --no-pinyin / --no-japanese 不复制对应词库或日语模型，--no-model 不复制整句模型（约 4 MB），
       --no-helpcode 不复制辅助码表（共约 290 KB，--no-pinyin 时也不复制），--clean 先清空 <dir>。
info   列出这个版本包含的文件和大小。`;
  (code === 0 ? console.log : console.error)(text);
  process.exit(code);
}

async function info() {
  const { files } = readManifest();
  console.log(`@msime/web-engine ${pkg.version}`);
  for (const a of files) console.log(`  assets/${a.name.padEnd(32)} ${String(a.size).padStart(10)} B  (raw ${a.raw_size} B)`);
  for (const a of await helpcodeFiles()) console.log(`  assets/${a.name.padEnd(32)} ${String(a.size).padStart(10)} B  (raw ${a.rawSize} B)`);
}

// 辅助码表不在 web-engine-manifest.json 里（那份清单只列 release 的文件），名字和大小记在构建时生成的 assets.js。
async function helpcodeFiles() {
  const { helpcodes = {} } = await import(pathToFileURL(join(root, "assets.js")).href);
  return Object.values(helpcodes);
}

function readManifest() {
  const path = join(root, "assets", "web-engine-manifest.json");
  if (!existsSync(path)) throw new Error(`${path} is missing; this package was not assembled by scripts/build-web-engine.sh`);
  const manifest = JSON.parse(readFileSync(path, "utf8"));
  return { manifest, files: manifest.artifacts.filter((a) => a.role !== "glue") };
}

async function copy(dest, flags) {
  const { files } = readManifest();
  // 辅助码只给全拼和双拼用，不要拼音时也就不要它们。
  const tables = flags.has("--no-helpcode") || flags.has("--no-pinyin") ? [] : await helpcodeFiles();
  const skip = new Set();
  if (flags.has("--no-wubi")) skip.add("wubi86");
  if (flags.has("--no-pinyin")) skip.add("pinyin");
  if (flags.has("--no-japanese")) skip.add("japanese");
  if (flags.has("--no-model") || flags.has("--no-pinyin")) skip.add("model");
  if (flags.has("--clean")) rmSync(dest, { recursive: true, force: true });
  mkdirSync(join(dest, "assets"), { recursive: true });
  for (const name of RUNTIME) copyFileSync(join(root, name), join(dest, name));
  let bytes = 0;
  for (const a of files) {
    if (skip.has(a.role)) continue;
    copyFileSync(join(root, "assets", a.name), join(dest, "assets", a.name));
    bytes += a.size;
  }
  for (const a of tables) {
    copyFileSync(join(root, "assets", a.name), join(dest, "assets", a.name));
    bytes += a.size;
  }
  copyFileSync(join(root, "assets", "web-engine-manifest.json"), join(dest, "assets", "web-engine-manifest.json"));
  const shown = relative(process.cwd(), dest) || ".";
  console.log(`@msime/web-engine ${pkg.version} -> ${shown} (${(bytes / 1048576).toFixed(1)} MiB of assets)`);
  console.log(`
在页面里：
  import { createMsimeEngine, keyFromEvent } from "/<${shown} 部署后的路径>/index.js";
  const engine = await createMsimeEngine({ scheme: "quanpin" });

经打包器引入 "@msime/web-engine" 时，改为传 assetBase：
  createMsimeEngine({ assetBase: "/<${shown} 部署后的路径>/assets/" })

资源文件名不带版本号，升级时建议把 <dir> 换成带版本的目录（如 msime/${pkg.version}），再给它配长缓存。`);
}

const [command, ...rest] = process.argv.slice(2);
try {
  if (command === "info") await info();
  else if (command === "copy") {
    const positional = rest.filter((a) => !a.startsWith("--"));
    const flags = new Set(rest.filter((a) => a.startsWith("--")));
    const known = new Set(["--no-wubi", "--no-pinyin", "--no-japanese", "--no-model", "--no-helpcode", "--clean"]);
    for (const f of flags) if (!known.has(f)) usage(2);
    if (positional.length !== 1) usage(2);
    await copy(resolve(positional[0]), flags);
  } else if (command === "-h" || command === "--help") usage(0);
  else usage(2);
} catch (e) {
  console.error(`msime-web-engine: ${e.message}`);
  process.exit(1);
}
