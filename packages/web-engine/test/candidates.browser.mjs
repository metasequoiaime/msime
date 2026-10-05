// 候选栏（src/candidates.js）和 attachInput 接线（src/input.js）的浏览器检查：用 Node 的 HTTP 服务器以 packages/web-engine 为根提供源码，在无头 Chrome 里打开 test/candidates-check.html，经 DevTools 协议读回页面写在 body 上的结果。检查本身在页面里跑（显示、隐藏、点选、换皮肤、销毁、模态对话框里的挂载点、解除绑定后丢弃迟到的帧），见那个文件。
//
// 不在 `*.test.mjs` 的 glob 里，本地 `node --test` 不需要浏览器。CI 的 ubuntu-24.04 runner 自带 google-chrome；本地用 `CHROME_BIN` 指定浏览器，例如 Playwright 下载的 Chromium。
//
// 用法：node packages/web-engine/test/candidates.browser.mjs
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { dirname, extname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { writeThemeCatalog } from "../tools/theme-catalog.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const TIMEOUT_MS = 60_000;
const TYPES = { ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8" };

/** 浏览器：`CHROME_BIN`，否则在 PATH 里找常见的 Chrome/Chromium 名字。 */
function findChrome() {
  if (process.env.CHROME_BIN) return process.env.CHROME_BIN;
  for (const name of ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"]) {
    const found = spawnSync("which", [name], { encoding: "utf8" });
    if (found.status === 0 && found.stdout.trim()) return found.stdout.trim();
  }
  throw new Error("candidates.browser: no Chrome found; set CHROME_BIN to a Chrome or Chromium executable");
}

/** 只读的静态服务器：只提供 packages/web-engine 下的 .html 和 .js。 */
function serve() {
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, "http://localhost").pathname);
    const file = resolve(root, `.${path}`);
    if (!file.startsWith(root + sep) || !TYPES[extname(file)] || !existsSync(file)) {
      res.writeHead(404);
      res.end();
      return;
    }
    res.writeHead(200, { "Content-Type": TYPES[extname(file)], "Cache-Control": "no-store" });
    res.end(readFileSync(file));
  });
  return new Promise((ready) => server.listen(0, "127.0.0.1", () => ready(server)));
}

/** 起一个无头 Chrome，经 DevTools 协议（Node 自带的 WebSocket）打开 url，等页面在 body 上写出 data-result，返回结果和 #log 的内容。浏览器在自己的临时 profile 里运行，结束、失败或超时都会被关掉，profile 随后删除。 */
async function runPage(chrome, url) {
  const profile = mkdtempSync(join(tmpdir(), "msime-candidates-browser-"));
  const args = ["--headless=new", "--disable-gpu", "--no-first-run", "--no-default-browser-check", "--remote-debugging-port=0", `--user-data-dir=${profile}`, "about:blank"];
  if (process.getuid?.() === 0) args.unshift("--no-sandbox");
  const child = spawn(chrome, args, { stdio: ["ignore", "ignore", "pipe"], detached: process.platform !== "win32" });
  const exited = new Promise((done) => child.once("exit", done));
  let socket = null;
  const deadline = Date.now() + TIMEOUT_MS;
  try {
    const endpoint = await new Promise((done, fail) => {
      let stderr = "";
      child.once("error", fail);
      setTimeout(() => fail(new Error("timed out waiting for DevTools")), TIMEOUT_MS).unref();
      child.once("exit", (code) => fail(new Error(`chrome exited with ${code} before DevTools was ready\n${stderr}`)));
      child.stderr.on("data", (chunk) => {
        stderr += chunk;
        const match = /DevTools listening on (ws:\/\/\S+)/.exec(stderr);
        if (match) done(match[1]);
      });
    });
    socket = new WebSocket(endpoint);
    await new Promise((done, fail) => {
      socket.addEventListener("open", done, { once: true });
      socket.addEventListener("error", () => fail(new Error("cannot connect to DevTools")), { once: true });
    });
    let nextId = 0;
    const waiting = new Map();
    socket.addEventListener("message", (event) => {
      const message = JSON.parse(event.data);
      const pending = waiting.get(message.id);
      if (!pending) return;
      waiting.delete(message.id);
      if (message.error) pending.fail(new Error(`${message.error.message}`));
      else pending.done(message.result);
    });
    const send = (method, params = {}, sessionId) =>
      new Promise((done, fail) => {
        const id = ++nextId;
        waiting.set(id, { done, fail });
        socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
      });
    const { targetId } = await send("Target.createTarget", { url });
    const { sessionId } = await send("Target.attachToTarget", { targetId, flatten: true });
    const read = `({ result: document.body?.dataset.result ?? null, log: document.getElementById("log")?.textContent ?? "" })`;
    for (;;) {
      const { result } = await send("Runtime.evaluate", { expression: read, returnByValue: true }, sessionId);
      if (result.value?.result) return result.value;
      if (Date.now() > deadline) return { result: null, log: `timed out after ${TIMEOUT_MS} ms; log so far: ${result.value?.log ?? ""}` };
      await new Promise((done) => setTimeout(done, 100));
    }
  } finally {
    socket?.close();
    // 连同 Chrome 的子进程一起结束：先礼后兵，等它真的退出再删 profile。
    if (child.exitCode === null && child.signalCode === null) {
      const group = process.platform !== "win32" ? -child.pid : child.pid;
      process.kill(group, "SIGTERM");
      const forced = setTimeout(() => process.kill(group, "SIGKILL"), 5000);
      await exited;
      clearTimeout(forced);
    }
    rmSync(profile, { recursive: true, force: true });
  }
}

writeThemeCatalog();
const chrome = findChrome();
const server = await serve();
try {
  const url = `http://127.0.0.1:${server.address().port}/test/candidates-check.html`;
  const { result, log } = await runPage(chrome, url);
  assert.equal(result, "ok", `candidates-check.html did not pass:\n${log}`);
  console.log("candidates.browser: ok");
} finally {
  server.close();
}
