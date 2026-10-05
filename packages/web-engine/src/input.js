// 把引擎接到一个 <textarea> 或 <input> 上：拦截 keydown 交给引擎，把帧里的输出写回文本框，并画一个默认的候选栏（可以关掉，自己用 onFrame 画）。
import { KeyKind, createShiftTap, keyFromEvent, osImeIntercepting, packKey } from "./keys.js";

// 空闲时交给浏览器原生处理的键：引擎为跟打页面设计，空闲回车什么也不输出，空闲的退格、方向键、Esc 也不该由引擎代劳。
const NATIVE_WHEN_IDLE = new Set([
  packKey(KeyKind.Enter),
  packKey(KeyKind.Backspace),
  packKey(KeyKind.BackspaceWord),
  packKey(KeyKind.Escape),
  packKey(KeyKind.PagePrev),
  packKey(KeyKind.PageNext),
]);

const BAR_STYLE =
  "position:fixed;z-index:2147483647;display:none;padding:4px 8px;border:1px solid #c8c8c8;border-radius:6px;background:#fff;color:#222;box-shadow:0 2px 8px rgba(0,0,0,.15);font:15px/1.6 system-ui,sans-serif;white-space:nowrap";

function createBar() {
  const bar = document.createElement("div");
  bar.className = "msime-candidates";
  bar.setAttribute("style", BAR_STYLE);
  // 点候选时不要让文本框失焦。
  bar.addEventListener("mousedown", (e) => e.preventDefault());
  document.body.append(bar);
  return bar;
}

function renderBar(bar, el, frame, pick) {
  if (!frame.composing) {
    bar.style.display = "none";
    return;
  }
  bar.replaceChildren();
  const preedit = document.createElement("div");
  preedit.className = "msime-preedit";
  preedit.style.color = "#666";
  preedit.textContent = frame.preedit;
  bar.append(preedit);
  const row = document.createElement("div");
  frame.page.forEach((c, i) => {
    const item = document.createElement("span");
    item.className = i === frame.highlight ? "msime-candidate msime-highlight" : "msime-candidate";
    item.style.cssText = `margin-right:10px;cursor:pointer;${i === frame.highlight ? "color:#1a5fd0;font-weight:600" : ""}`;
    item.textContent = `${i + 1}.${c.text}`;
    item.addEventListener("click", () => pick(i));
    row.append(item);
  });
  if (frame.hasPrev || frame.hasNext) {
    const arrows = document.createElement("span");
    arrows.style.color = "#999";
    arrows.textContent = `${frame.hasPrev ? "‹" : " "}${frame.hasNext ? "›" : " "}`;
    row.append(arrows);
  }
  bar.append(row);
  const rect = el.getBoundingClientRect();
  bar.style.left = `${Math.max(0, rect.left)}px`;
  bar.style.top = `${rect.bottom + 4}px`;
  bar.style.display = "block";
}

// 删除光标前的一个字符（按码点）或一个词（连续的非空白字符，以及它前面的空白）。
function deleteBack(el, word) {
  const end = el.selectionEnd ?? el.value.length;
  let start = el.selectionStart ?? end;
  if (start === end) {
    const before = el.value.slice(0, end);
    if (word) {
      start = before.replace(/\S+\s*$|\s+$/u, "").length;
    } else {
      start = end - (Array.from(before).pop()?.length ?? 0);
    }
  }
  if (start === end) return;
  el.setRangeText("", start, end, "end");
  el.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: word ? "deleteWordBackward" : "deleteContentBackward" }));
}

function insert(el, text) {
  el.setRangeText(text, el.selectionStart ?? el.value.length, el.selectionEnd ?? el.value.length, "end");
  el.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: text }));
}

/**
 * 把 engine 接到 el 上，返回解除绑定的函数。options.candidates 为 false 时不画默认候选栏；options.onFrame 收到每一帧。
 */
export function attachInput(el, engine, options = {}) {
  const shift = createShiftTap();
  const bar = options.candidates === false ? null : createBar();
  let composing = false;
  let pending = 0;
  let generation = 0;

  const apply = (gen, frame) => {
    if (gen !== generation) return;
    for (const item of frame.out) {
      if (item.t === "commit" || item.t === "type") insert(el, item.text);
      else if (item.t === "back") deleteBack(el, item.word);
    }
    composing = frame.composing;
    if (bar) renderBar(bar, el, frame, pick);
    options.onFrame?.(frame);
  };

  const send = (run) => {
    const gen = generation;
    pending += 1;
    run()
      .then((frame) => apply(gen, frame))
      .catch(() => {})
      .finally(() => {
        pending -= 1;
      });
  };

  const pick = (slot) => send(() => engine.pick(slot));

  // 光标被挪走或者文本框失焦时，丢掉正在组的字，避免上屏到别处。
  const abandon = () => {
    if (!composing && pending === 0) return;
    generation += 1;
    composing = false;
    if (bar) bar.style.display = "none";
    engine.reset().catch(() => {});
  };

  const onKeyDown = (e) => {
    shift.down(e);
    if (osImeIntercepting(e)) return;
    // 组字中或者还有按键没回来时，状态由引擎决定；完全空闲时才把部分键交给浏览器。
    const busy = composing || pending > 0;
    const key = keyFromEvent(e, { composing: busy });
    if (key === null) return;
    if (!busy && NATIVE_WHEN_IDLE.has(key)) return;
    e.preventDefault();
    send(() => engine.keys(key));
  };

  const onKeyUp = (e) => {
    if (!shift.up(e)) return;
    send(() => engine.keys(packKey(KeyKind.ShiftTap)));
  };

  el.addEventListener("keydown", onKeyDown);
  el.addEventListener("keyup", onKeyUp);
  el.addEventListener("blur", abandon);
  el.addEventListener("mousedown", abandon);

  return () => {
    el.removeEventListener("keydown", onKeyDown);
    el.removeEventListener("keyup", onKeyUp);
    el.removeEventListener("blur", abandon);
    el.removeEventListener("mousedown", abandon);
    bar?.remove();
  };
}
