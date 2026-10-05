// 把浏览器的 KeyboardEvent 翻译成引擎的打包按键：(kind << 8) | ascii，与 crates/engine-wasm/src/host.rs 的 Key::pack 是同一张表。

/** 按键种类，取值与 host.rs 的 Key::unpack 一一对应。 */
export const KeyKind = Object.freeze({
  Letter: 1,
  ShiftLetter: 2,
  Digit: 3,
  Space: 4,
  Enter: 5,
  Backspace: 6,
  BackspaceWord: 7,
  Escape: 8,
  PagePrev: 9,
  PageNext: 10,
  HighlightPrev: 11,
  HighlightNext: 12,
  Punct: 13,
  ShiftTap: 14,
});

/** 打包一个按键；ascii 只有字母、数字、标点和借用标点的翻页键才需要。 */
export const packKey = (kind, ascii = 0) => (kind << 8) | ascii;

// 按住不放时允许连发的键：删除和在候选里移动。其他键连发只会重复打同一个字母或标点。
const REPEATABLE = new Set([
  "Backspace",
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "PageUp",
  "PageDown",
]);

// 不论引擎处于什么状态，都只对应一个引擎按键的命名键。
const NAMED = {
  " ": KeyKind.Space,
  Enter: KeyKind.Enter,
  Escape: KeyKind.Escape,
  PageUp: KeyKind.PagePrev,
  PageDown: KeyKind.PageNext,
  ArrowUp: KeyKind.PagePrev,
  ArrowDown: KeyKind.PageNext,
};

// 快捷键组合：Cmd/Win 一律算；Ctrl、Alt 加一个非单字符的键算；Alt/AltGr 打出的单个可打印字符（很多布局靠它打符号）不算，Alt 加字母数字仍算快捷键。
function isShortcutChord(e) {
  if (e.metaKey) return true;
  if (!e.ctrlKey && !e.altKey) return false;
  if (e.key.length !== 1) return true;
  if (e.getModifierState?.("AltGraph")) return false;
  return !e.altKey || /[a-z0-9]/i.test(e.key);
}

/**
 * keydown 对应的打包按键；引擎不该看到这个键时返回 null（快捷键、Tab、不允许连发的连发）。
 *
 * `options.composing` 是最近一帧的 `composing`：左右方向键只在组字时移动高亮，空闲时交还给页面。`options.typed` 可以覆盖 `e.key` 作为这次打出的字符，页面自己做键盘布局映射时用。
 */
export function keyFromEvent(e, options = {}) {
  if (e.repeat && !REPEATABLE.has(e.key)) return null;
  // 退格在快捷键判断之前处理：Ctrl/Alt+Backspace 是删词，而 isShortcutChord 会因为它不是单字符键把它当成快捷键。
  if (e.key === "Backspace") {
    if (e.metaKey) return null;
    return packKey(e.ctrlKey || e.altKey ? KeyKind.BackspaceWord : KeyKind.Backspace);
  }
  if (e.key === "Tab" || isShortcutChord(e)) return null;
  const named = NAMED[e.key];
  if (named !== undefined) return packKey(named);
  if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
    if (!options.composing) return null;
    return packKey(e.key === "ArrowLeft" ? KeyKind.HighlightPrev : KeyKind.HighlightNext);
  }
  const ch = options.typed ?? e.key;
  if (typeof ch !== "string" || ch.length !== 1) return null;
  const code = ch.charCodeAt(0);
  if (ch >= "a" && ch <= "z") return packKey(KeyKind.Letter, code);
  if (ch >= "A" && ch <= "Z") return packKey(KeyKind.ShiftLetter, code);
  if (ch >= "0" && ch <= "9") return packKey(KeyKind.Digit, code);
  // - 和 = 在组字时翻页、空闲时打出自己，由引擎判断，所以结束组字的那一帧回来之前发出的键也不会读错。
  if (ch === "-") return packKey(KeyKind.PagePrev, code);
  if (ch === "=") return packKey(KeyKind.PageNext, code);
  if (code > 0x20 && code < 0x7f) return packKey(KeyKind.Punct, code);
  return null;
}

/** 系统输入法正在处理这个键（keyCode 229、"Process" 或 isComposing），这时不要再交给引擎。 */
export function osImeIntercepting(e) {
  return e.isComposing || e.keyCode === 229 || e.key === "Process";
}

/** 跟踪单独按下又松开的 Shift，引擎把它当作中英文切换（ShiftTap）。keydown 和 keyup 都要调用；up 返回 true 时发送 packKey(KeyKind.ShiftTap)。 */
export function createShiftTap() {
  let armed = false;
  return {
    down(e) {
      if (e.key === "Shift") {
        if (!e.repeat) armed = true;
      } else {
        armed = false;
      }
    },
    up(e) {
      if (e.key !== "Shift" || !armed) return false;
      armed = false;
      return true;
    },
  };
}
