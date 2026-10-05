// 把引擎接到一个 <textarea> 或 <input> 上：拦截 keydown 交给引擎，把帧里的输出写回文本框，并用 candidates.js 画一个按水杉候选框皮肤绘制的候选栏（可以关掉，自己用 onFrame 画）。
import { createCandidateBar } from "./candidates.js";
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

// 文本框在模态对话框、popover 或全屏元素里时，候选栏要挂进同一个顶层元素：顶层元素总画在页面其他内容之上，z-index 再大也盖不过它，模态对话框之外的内容还点不到。候选栏用视口坐标定位，挂在哪里位置都一样。
function topLayerContainer(el) {
  const doc = el.ownerDocument;
  const fullscreen = doc.fullscreenElement;
  return el.closest("dialog[open], [popover]") ?? (fullscreen?.contains(el) ? fullscreen : doc.body);
}

/**
 * 把 engine 接到 el 上，返回解除绑定的函数。options.candidates 为 false 时不画默认候选栏；options.skin、options.layout（"horizontal" 或 "vertical"）和 options.dark（true、false 或 "auto"）交给 createCandidateBar，皮肤 ID 未知时抛 TypeError；options.container 是候选栏挂在哪个元素里，不传时挂在 el 所在的顶层元素（模态对话框、popover 或全屏元素）里，都不是时挂在 body 上，每次显示前按当时的状态重新选；options.onFrame 收到每一帧。解除绑定后，还在路上的帧直接丢弃，正在组的字也一并放弃。
 */
export function attachInput(el, engine, options = {}) {
  const shift = createShiftTap();
  const bar =
    options.candidates === false
      ? null
      : createCandidateBar({
          skin: options.skin,
          layout: options.layout ?? "horizontal",
          dark: options.dark ?? "auto",
          onPick: (slot) => pick(slot),
          container: options.container ?? topLayerContainer(el),
        });
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
    if (bar && composing && !options.container) {
      // 对话框可能在绑定之后才打开，页面也可能后来才进全屏，所以每次显示前重新选挂载点；移动宿主元素不影响它的 Shadow DOM 和样式表。
      const target = topLayerContainer(el);
      if (bar.element.parentNode !== target) target.append(bar.element);
    }
    bar?.render(frame, el.getBoundingClientRect());
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
    bar?.hide();
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
    // 同 abandon 放弃正在组的字；换代让还在路上的帧过不了 apply 的检查，不再写进已经不归 SDK 管的文本框，也不再调 onFrame。
    if (composing || pending > 0) engine.reset().catch(() => {});
    generation += 1;
    composing = false;
    el.removeEventListener("keydown", onKeyDown);
    el.removeEventListener("keyup", onKeyUp);
    el.removeEventListener("blur", abandon);
    el.removeEventListener("mousedown", abandon);
    bar?.destroy();
  };
}
