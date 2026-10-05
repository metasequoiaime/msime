// 演示页。输入区的文字、光标和行内拼音都由这里自己画；按键由一个看不见的密码框接收：浏览器在密码框里强制停用系统输入法（macOS 进入安全输入，Windows 解除输入法关联），按键原样到达页面，交给水杉的引擎处理。普通的可编辑或可聚焦元素都挡不住系统输入法。右侧的选项改动立刻作用到引擎，下方的接入代码按同样的选项生成。
import { KeyKind, createMsimeEngine, createShiftTap, keyFromEvent, osImeIntercepting, packKey, version } from "./msime/index.js";

const PINYIN = new Set(["quanpin", "xiaohe", "ziranma"]);
const NAMES = { quanpin: "全拼", xiaohe: "小鹤双拼", ziranma: "自然码双拼", wubi86: "五笔 86" };
const SCHEME_NOTES = {
  quanpin: "完整拼音，例如 woshizhongguoren。",
  xiaohe: "小鹤双拼：每个字两键，例如 你好 = ni hc。",
  ziranma: "自然码双拼：每个字两键，例如 你好 = ni hk。",
  wubi86: "五笔 86：形码，候选旁显示剩余编码，例如 你好 = wq vb。词库与拼音不同，切换时会单独下载（约 3.6 MB）。",
};
// 「看它打字」的样例，每一条都在浏览器里实测过结果，且与每页候选数无关。键序列里 " " 是空格，"^" 是单按 Shift（切换中英文），"=" 和 "-" 是下一页和上一页，数字选当前页的候选，大写字母按住 Shift 打出。
const EXAMPLES = {
  quanpin: [
    { label: "整句", keys: "woshizhongguoren " },
    { label: "长句", keys: "jintiantianqizhenhao " },
    { label: "中英混输", keys: "woyong ^GitHub^xiedaima " },
    { label: "全角标点", keys: "nihao,woshixiaoming." },
    { label: "翻页选字", keys: "gongshi=-2" },
  ],
  xiaohe: [
    { label: "你好", keys: "nihc " },
    { label: "世界", keys: "uijp " },
    { label: "我们去北京", keys: "womf qu bwjk " },
  ],
  ziranma: [
    { label: "你好", keys: "nihk " },
    { label: "世界", keys: "uijx " },
    { label: "中国", keys: "vsgo " },
  ],
  wubi86: [
    { label: "你好", keys: "wqvb " },
    { label: "中国", keys: "khlg " },
    { label: "工作", keys: "aawt " },
  ],
};

// SDK 的默认值，生成代码时只写出与它们不同的选项。
const DEFAULTS = { scheme: "quanpin", pageSize: 9, model: true, modelEnabled: true };
const options = { ...DEFAULTS };

const $ = (id) => document.getElementById(id);

// 嵌入模式（?embed）：msime.app 用 iframe 嵌入这个页面时只显示演练场，加载时不抢焦点（否则打开官网时键盘焦点会被拉进 iframe），并把内容高度告诉外层页面，让它把 iframe 调到正好的高度。
const EMBED = new URLSearchParams(location.search).has("embed");
if (EMBED) {
  document.documentElement.dataset.embed = "";
  if (window.parent !== window) {
    // 报内容实际的底部，不用 scrollHeight：它不会小于 iframe 当前的高度，iframe 一旦比内容高就再也缩不回来。
    const main = document.querySelector("main");
    const report = () => window.parent.postMessage({ type: "msime-demo:height", height: Math.ceil(main.getBoundingClientRect().bottom + window.scrollY) }, "*");
    new ResizeObserver(report).observe(document.body);
  }
}
const editor = $("editor");
const before = editor.querySelector(".before");
const preeditSpan = editor.querySelector(".preedit");
const caretSpan = editor.querySelector(".caret");
const after = editor.querySelector(".after");
const wrap = editor.parentElement;
const keysink = $("keysink");
const focusEditor = () => keysink.focus({ preventScroll: true });
const bar = $("candidates");
const barPreedit = bar.querySelector(".cand-preedit");
const barList = bar.querySelector(".cand-list");

let engine = null;
// 当前引擎是按哪些创建参数建的；只有 scheme 在拼音之间变化和 modelEnabled 变化时不用重建。
let built = null;
let text = "";
let caret = 0;
let frame = null;
let pending = 0;
// 放弃组字（点击挪光标、清空、切换选项、跑对比）时加一，之前发出的请求回来的帧就丢掉。
let generation = 0;
let busyLoading = false;
const shift = createShiftTap();

const composing = () => Boolean(frame?.composing);
const ms = (value) => (value >= 100 ? `${Math.round(value)} ms` : `${value.toFixed(1)} ms`);
const mib = (bytes) => `${(bytes / 1048576).toFixed(1)} MB`;

function setStatus(state, message) {
  $("status").dataset.state = state;
  $("status-text").textContent = message;
}

// ---- 输入区 ----

function render() {
  before.textContent = text.slice(0, caret);
  after.textContent = text.slice(caret);
  preeditSpan.textContent = composing() ? frame.preedit : "";
  editor.dataset.empty = String(text === "" && !composing());
  renderCandidates();
}

function renderCandidates() {
  if (!composing() || frame.page.length === 0) {
    bar.hidden = true;
    return;
  }
  barPreedit.textContent = frame.preedit;
  barList.replaceChildren(
    ...frame.page.map((row, i) => {
      const li = document.createElement("li");
      li.dataset.highlight = String(i === frame.highlight);
      const num = document.createElement("span");
      num.className = "num";
      num.textContent = String(i + 1);
      li.append(num, row.text);
      // 编码提示只对五笔这样的形码有用；拼音方案的编码就是整串拼音，桌面版也不显示。
      if (row.code && !PINYIN.has(options.scheme)) {
        const code = document.createElement("span");
        code.className = "cand-code";
        code.textContent = row.code;
        li.append(code);
      }
      li.addEventListener("mousedown", (e) => e.preventDefault());
      li.addEventListener("click", () => send(() => engine.pick(i)));
      return li;
    }),
  );
  if (frame.hasPrev || frame.hasNext) {
    const pager = document.createElement("li");
    pager.className = "pager";
    pager.textContent = `${frame.hasPrev ? "‹" : " "} ${frame.hasNext ? "›" : " "}`;
    barList.append(pager);
  }
  bar.hidden = false;
  // 候选栏放在光标下方，不超出输入区右边。
  const wrapRect = wrap.getBoundingClientRect();
  const caretRect = caretSpan.getBoundingClientRect();
  const left = Math.max(0, Math.min(caretRect.left - wrapRect.left - 12, wrapRect.width - bar.offsetWidth));
  bar.style.left = `${left}px`;
  bar.style.top = `${caretRect.bottom - wrapRect.top + 8}px`;
}

function insert(value) {
  text = text.slice(0, caret) + value + text.slice(caret);
  caret += value.length;
}

function deleteBack(word) {
  if (caret === 0) return;
  const head = text.slice(0, caret);
  const start = word ? head.replace(/\S+\s*$|\s+$/u, "").length : caret - (Array.from(head).pop()?.length ?? 0);
  text = text.slice(0, start) + text.slice(caret);
  caret = start;
}

function apply(gen, next, elapsed) {
  if (gen !== generation) return;
  for (const item of next.out) {
    if (item.t === "commit" || item.t === "type") insert(item.text);
    else if (item.t === "back") deleteBack(item.word);
  }
  frame = next;
  render();
  showFrame(next, elapsed);
}

function send(run) {
  if (!engine || busyLoading || playing) return;
  const gen = generation;
  const t0 = performance.now();
  pending += 1;
  run()
    .then((next) => apply(gen, next, performance.now() - t0))
    .catch((error) => console.error("msime:", error))
    .finally(() => {
      pending -= 1;
    });
}

function abandon() {
  if (!composing() && pending === 0) return;
  generation += 1;
  frame = null;
  engine?.reset().catch(() => {});
  render();
}

keysink.addEventListener("keydown", (e) => {
  // 演示回放期间输入区只读，访客的按键不进来。
  if (playing) {
    e.preventDefault();
    return;
  }
  shift.down(e);
  if (osImeIntercepting(e)) {
    $("ime-notice").hidden = false;
    return;
  }
  const busy = composing() || pending > 0;
  // 空闲时的编辑键由页面自己处理：引擎为跟打页面设计，空闲回车不输出换行，也不管光标位置。
  if (!busy && !e.ctrlKey && !e.metaKey && !e.altKey) {
    if (e.key === "Enter") {
      e.preventDefault();
      insert("\n");
      render();
      return;
    }
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      e.preventDefault();
      caret = Math.max(0, Math.min(text.length, caret + (e.key === "ArrowLeft" ? -1 : 1)));
      render();
      return;
    }
    if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      caret = e.key === "Home" ? 0 : text.length;
      render();
      return;
    }
    if (e.key === "Delete") {
      e.preventDefault();
      text = text.slice(0, caret) + text.slice(caret + 1);
      render();
      return;
    }
    if (e.key === "Escape" || e.key === "ArrowUp" || e.key === "ArrowDown" || e.key === "PageUp" || e.key === "PageDown") return;
  }
  const key = keyFromEvent(e, { composing: busy });
  if (key === null || !engine) return;
  e.preventDefault();
  send(() => engine.keys(key));
});

keysink.addEventListener("keyup", (e) => {
  if (shift.up(e)) send(() => engine.keys(packKey(KeyKind.ShiftTap)));
});

// 点击文字把光标放到点击处；正在组字时先放弃。必须在 mousedown 里同步完成：推迟到下一帧的话，点击后立刻打的字会先发出去，再被这里的放弃当成旧请求丢掉。
editor.addEventListener("mousedown", (e) => {
  if (e.button !== 0) return;
  // 焦点交给密码框，输入区自己不获得焦点。
  e.preventDefault();
  focusEditor();
  const position = document.caretPositionFromPoint?.(e.clientX, e.clientY);
  const range = position ? null : document.caretRangeFromPoint?.(e.clientX, e.clientY);
  const node = position ? position.offsetNode : range?.startContainer;
  const offset = position ? position.offset : (range?.startOffset ?? 0);
  let index = text.length;
  if (node && before.contains(node)) index = offset;
  else if (node && after.contains(node)) index = caret + offset;
  const wasBusy = composing() || pending > 0;
  abandon();
  index = Math.min(index, text.length);
  // 光标没动也没有放弃组字时不重绘：重建文字节点会打断从这里开始的拖选。
  if (index === caret && !wasBusy) return;
  caret = index;
  render();
});

keysink.addEventListener("blur", abandon);
// 密码框本身不留任何内容；粘贴的文字插到输入区里。
keysink.addEventListener("input", () => (keysink.value = ""));
keysink.addEventListener("paste", (e) => {
  e.preventDefault();
  const pasted = e.clipboardData?.getData("text/plain") ?? "";
  if (!pasted || composing() || pending > 0 || playing) return;
  insert(pasted);
  render();
});

$("clear").addEventListener("click", () => {
  abandon();
  text = "";
  caret = 0;
  render();
  focusEditor();
});

function copyText(button, value) {
  navigator.clipboard?.writeText(value).then(() => {
    const label = button.textContent;
    button.textContent = "已复制";
    setTimeout(() => (button.textContent = label), 1200);
  });
}

$("copy").addEventListener("click", (e) => copyText(e.currentTarget, text));

// ---- 引擎面板 ----

function showFacts(timings, memoryBytes) {
  $("fact-version").textContent = `@msime/web-engine ${version}`;
  $("fact-scheme").textContent = NAMES[options.scheme] + (PINYIN.has(options.scheme) && built.model ? " + 整句模型" : "");
  if (timings) {
    $("fact-download").textContent = ms(timings.fetch);
    $("fact-compile").textContent = ms(timings.compile);
    $("fact-import").textContent = ms(timings.import);
    $("fact-session").textContent = ms(timings.session);
  }
  if (memoryBytes) $("fact-memory").textContent = mib(memoryBytes);
}

function showFrame(next, elapsed) {
  $("fact-latency").textContent = ms(elapsed);
  const view = {
    out: next.out,
    composing: next.composing,
    preedit: next.preedit,
    page: next.page.map((row) => (row.code && !PINYIN.has(options.scheme) ? `${row.text} ${row.code}` : row.text)),
    pageIndex: next.pageIndex,
    highlight: next.highlight,
    hasPrev: next.hasPrev,
    hasNext: next.hasNext,
    english: next.english,
    modelOn: next.modelOn,
    rerankMs: Number(next.rerankMs.toFixed(2)),
  };
  // 短数组压成一行，面板里一屏看得完。
  $("frame").textContent = JSON.stringify(view, null, 2).replace(/\[\n\s+([^\]{]*?)\n\s+\]/g, (_, inner) => `[${inner.replace(/\n\s+/g, " ")}]`);
}

// ---- 接入代码 ----

let tab = "textarea";

function engineOptions() {
  const parts = [];
  if (options.scheme !== DEFAULTS.scheme) parts.push(`scheme: "${options.scheme}"`);
  if (options.pageSize !== DEFAULTS.pageSize) parts.push(`pageSize: ${options.pageSize}`);
  if (PINYIN.has(options.scheme) && !options.model) parts.push("model: false");
  return parts;
}

const call = (extra = []) => {
  const parts = [...engineOptions(), ...extra];
  return parts.length ? `createMsimeEngine({ ${parts.join(", ")} })` : "createMsimeEngine()";
};

function modelLine() {
  return PINYIN.has(options.scheme) && options.model && !options.modelEnabled ? "\nengine.setModelEnabled(false); // 下载了模型，但先不用它排序" : "";
}

function copyFlags() {
  const flags = [];
  if (options.scheme === "wubi86") flags.push("--no-pinyin");
  else {
    flags.push("--no-wubi");
    if (!options.model) flags.push("--no-model");
  }
  return flags.join(" ");
}

const CODE = {
  textarea: () => ({
    note: "最省事的接法：组字时按键交给引擎，空闲时的回车、退格、方向键仍由浏览器处理。访客开着系统中文输入法时，文本框的按键会先被系统输入法接走，需要切换到英文输入。",
    code: `import { createMsimeEngine, attachInput } from "@msime/web-engine";

const engine = await ${call()};${modelLine()}
attachInput(document.querySelector("textarea"), engine);`,
  }),
  custom: () => ({
    note: "这个页面的做法：按键由一个看不见的密码框接收，浏览器在密码框里停用系统输入法，访客不用切换到英文；按键交给 engine.keys()，按返回的帧更新自己画的文字和候选栏。",
    code: `import { createMsimeEngine, keyFromEvent } from "@msime/web-engine";

const engine = await ${call()};${modelLine()}
// 看不见的密码框接收按键：浏览器在密码框里停用系统输入法
// <input type="password" id="keys" autocomplete="new-password" data-1p-ignore data-lpignore="true">
const keys = document.querySelector("#keys");
let composing = false;

keys.addEventListener("keydown", async (e) => {
  const key = keyFromEvent(e, { composing });
  if (key === null) return; // 快捷键等不属于输入法的键
  e.preventDefault();
  render(await engine.keys(key));
});

function render(frame) {
  for (const o of frame.out) {
    if (o.t === "commit" || o.t === "type") insertText(o.text);
    if (o.t === "back") deleteBackward(o.word);
  }
  composing = frame.composing;
  // frame.preedit 是拼音，frame.page 是当前页候选，frame.highlight 是高亮项
  drawCandidates(frame.preedit, frame.page, frame.highlight);
}

// 鼠标点选当前页第 i 个候选
const pick = async (i) => render(await engine.pick(i));`,
  }),
  deploy: () => ({
    note: "三种部署方式任选其一。copy 的参数按当前方案只复制用得到的资源。",
    code: `# 1. 静态站点（GitHub Pages、Vercel、Cloudflare Pages / Workers）
npx @msime/web-engine copy public/msime ${copyFlags()}

<script type="module">
  import { createMsimeEngine, attachInput } from "/msime/index.js";
  const engine = await ${call()};
  attachInput(document.querySelector("textarea"), engine);
</script>

# 2. 打包器（Vite、webpack）：资源仍按第 1 步复制，再告诉 SDK 位置
npm install @msime/web-engine
const engine = await ${call(['assetBase: "/msime/assets/"'])};

# 3. CDN，什么都不用部署
import { createMsimeEngine, attachInput } from "https://cdn.jsdelivr.net/npm/@msime/web-engine@${version}/index.js";`,
  }),
};

// ---- 代码高亮 ----

const escapeHtml = (text) => text.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]);

// Prism 输出的 HTML 已经转义过，可以直接放进 innerHTML；Prism 没加载成功时退回转义后的纯文本。
function highlight(code, language) {
  const grammar = window.Prism?.languages[language];
  return grammar ? window.Prism.highlight(code, grammar, language) : escapeHtml(code);
}

// 「部署方式」里混着 shell 命令、HTML 和 JS：按行分组，连续的同一种语言一起高亮。<script> 块交给 markup，Prism 会把里面的 JS 一并高亮。
function highlightCode(code) {
  const groups = [];
  let inScript = false;
  for (const line of code.split("\n")) {
    let language = "javascript";
    if (inScript || line.trimStart().startsWith("<")) language = "markup";
    else if (/^(#|npm |npx )/.test(line)) language = "bash";
    if (line.trimStart().startsWith("<script")) inScript = true;
    if (line.includes("</script>")) inScript = false;
    const last = groups.at(-1);
    if (last && last.language === language) last.lines.push(line);
    else groups.push({ language, lines: [line] });
  }
  return groups.map((group) => highlight(group.lines.join("\n"), group.language)).join("\n");
}

function renderCode() {
  const { note, code } = CODE[tab]();
  $("code-note").textContent = note;
  $("code-body").innerHTML = highlightCode(code);
  for (const button of document.querySelectorAll(".tabs button")) button.setAttribute("aria-selected", String(button.dataset.tab === tab));
}

for (const button of document.querySelectorAll(".tabs button")) {
  button.addEventListener("click", () => {
    tab = button.dataset.tab;
    renderCode();
  });
}

$("copy-code").addEventListener("click", (e) => copyText(e.currentTarget, $("code-body").textContent));

// ---- 看它打字 ----

let playing = false;

// 样例里的一个字符对应的打包按键，规则见 EXAMPLES 上面的注释。
function packChar(c) {
  if (c === " ") return packKey(KeyKind.Space);
  if (c === "^") return packKey(KeyKind.ShiftTap);
  if (c === "=") return packKey(KeyKind.PageNext, 61);
  if (c === "-") return packKey(KeyKind.PagePrev, 45);
  if (c >= "a" && c <= "z") return packKey(KeyKind.Letter, c.charCodeAt(0));
  if (c >= "A" && c <= "Z") return packKey(KeyKind.ShiftLetter, c.charCodeAt(0));
  if (c >= "0" && c <= "9") return packKey(KeyKind.Digit, c.charCodeAt(0));
  return packKey(KeyKind.Punct, c.charCodeAt(0));
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const KEY_LABELS = { " ": "空格", "^": "Shift", "=": "下一页", "-": "上一页" };

function showTyping(keys, index) {
  const hint = $("hint");
  const line = document.createElement("span");
  line.className = "typing";
  line.append("演示：");
  [...keys].forEach((c, i) => {
    const label = KEY_LABELS[c] ? ` ${KEY_LABELS[c]} ` : c;
    if (i === index) {
      const b = document.createElement("b");
      b.textContent = label;
      line.append(b);
    } else line.append(label);
  });
  hint.replaceChildren(line);
}

async function play(example, button) {
  if (!engine || busyLoading || playing) return;
  playing = true;
  const hintBackup = [...$("hint").childNodes].map((n) => n.cloneNode(true));
  button.dataset.playing = "true";
  syncControls();
  focusEditor();
  abandon();
  // 从新的一行开始，免得接在访客已经打的字后面。
  if (text && !text.endsWith("\n")) {
    caret = text.length;
    insert("\n");
    render();
  }
  const keys = example.keys;
  try {
    for (let i = 0; i < keys.length; i += 1) {
      showTyping(keys, i);
      const c = keys[i];
      await sleep(c === " " || c === "^" || c === "=" || c === "-" || /[0-9]/.test(c) ? 480 : 110);
      const gen = generation;
      const t0 = performance.now();
      apply(gen, await engine.keys(packChar(c)), performance.now() - t0);
    }
    await sleep(500);
  } catch (error) {
    console.error("msime:", error);
  } finally {
    $("hint").replaceChildren(...hintBackup);
    delete button.dataset.playing;
    playing = false;
    syncControls();
    focusEditor();
  }
}

function renderExamples() {
  const chips = $("examples");
  const list = EXAMPLES[options.scheme];
  if (chips.dataset.scheme !== options.scheme) {
    chips.dataset.scheme = options.scheme;
    chips.replaceChildren(
      ...list.map((example) => {
        const button = document.createElement("button");
        button.type = "button";
        const code = document.createElement("code");
        code.textContent = example.keys.replaceAll(" ", "␣").replaceAll("^", "⇧");
        button.append(example.label, code);
        button.addEventListener("mousedown", (e) => e.preventDefault());
        button.addEventListener("click", () => play(example, button));
        return button;
      }),
    );
  }
  for (const button of chips.querySelectorAll("button")) button.disabled = busyLoading || playing || !engine;
}

// ---- 选项 ----

function syncControls() {
  for (const group of document.querySelectorAll(".segmented")) {
    const value = String(options[group.dataset.option]);
    for (const button of group.querySelectorAll("button")) {
      button.setAttribute("aria-checked", String(button.dataset.value === value));
      button.disabled = busyLoading || playing;
    }
  }
  const pinyin = PINYIN.has(options.scheme);
  const model = document.querySelector('[data-option="model"]');
  const modelEnabled = document.querySelector('[data-option="modelEnabled"]');
  model.checked = pinyin && options.model;
  modelEnabled.checked = pinyin && options.model && options.modelEnabled;
  model.disabled = busyLoading || playing || !pinyin;
  modelEnabled.disabled = busyLoading || playing || !pinyin || !options.model;
  $("scheme-note").textContent = SCHEME_NOTES[options.scheme];
  renderExamples();
  renderCode();
}

// 按当前选项准备好引擎：能就地切换的就地切换，否则重建。
async function applyOptions() {
  const rebuild = !engine || !built || built.pageSize !== options.pageSize || built.model !== options.model || PINYIN.has(built.scheme) !== PINYIN.has(options.scheme);
  busyLoading = true;
  abandon();
  syncControls();
  try {
    if (rebuild) {
      engine?.dispose();
      engine = null;
      $("progress").dataset.done = "false";
      $("progress").firstElementChild.style.width = "0";
      setStatus("loading", `正在加载${NAMES[options.scheme]}…`);
      const t0 = performance.now();
      engine = await createMsimeEngine({
        scheme: options.scheme,
        pageSize: options.pageSize,
        model: options.model,
        modelEnabled: options.modelEnabled,
        onProgress: (loaded, total) => {
          const percent = Math.round((loaded / total) * 100);
          $("progress").firstElementChild.style.width = `${percent}%`;
          setStatus("loading", `正在加载${NAMES[options.scheme]} ${percent}%`);
        },
      });
      built = { scheme: options.scheme, pageSize: options.pageSize, model: options.model };
      $("progress").dataset.done = "true";
      engine.onError((error) => setStatus("error", `引擎出错，请刷新页面：${error.message}`));
      setStatus("ready", `就绪 · ${NAMES[options.scheme]} · 加载 ${((performance.now() - t0) / 1000).toFixed(1)} 秒`);
      showFacts(engine.timings, engine.memoryBytes);
    } else {
      if (built.scheme !== options.scheme) {
        // 拼音方案之间共用词库，只重建会话，不重新下载。
        await engine.setScheme(options.scheme);
        built.scheme = options.scheme;
      }
      engine.setModelEnabled(options.modelEnabled);
      setStatus("ready", `就绪 · ${NAMES[options.scheme]}`);
      showFacts(null, engine.memoryBytes);
    }
    $("build").textContent = `@msime/web-engine ${version}`;
  } catch (error) {
    const reason = error.code === "unsupported" ? "这个浏览器不支持 WebAssembly 或 DecompressionStream，请换用新版 Chrome、Edge、Firefox 或 Safari。" : error.message;
    setStatus("error", `加载失败：${reason}`);
  } finally {
    busyLoading = false;
    syncControls();
  }
}

for (const group of document.querySelectorAll(".segmented")) {
  for (const button of group.querySelectorAll("button")) {
    button.addEventListener("click", () => {
      const key = group.dataset.option;
      const value = key === "pageSize" ? Number(button.dataset.value) : button.dataset.value;
      if (options[key] === value) return;
      options[key] = value;
      applyOptions().then(() => focusEditor());
    });
  }
}

for (const input of document.querySelectorAll(".switch input")) {
  input.addEventListener("change", () => {
    options[input.dataset.option] = input.checked;
    if (input.dataset.option === "model" && input.checked) options.modelEnabled = true;
    applyOptions().then(() => focusEditor());
  });
}

if (matchMedia("(pointer: coarse)").matches && !matchMedia("(any-pointer: fine)").matches) {
  $("touch-notice").hidden = false;
}

render();
syncControls();
if (!EMBED) focusEditor();
applyOptions();
