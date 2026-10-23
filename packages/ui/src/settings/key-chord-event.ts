/** 键盘事件到切换热键写法的映射。契约（`client-core` 的 `KeyChord`）的键名与 DOM `KeyboardEvent.code`
 * 的物理键名逐字一致（KeyA–KeyZ、Digit0–Digit9、F1–F12 和 Space、Period 等十二个具名键），所以录入
 * 不需要第二份解析器：修饰键按 `Ctrl`、`Alt`、`Shift`、`Meta` 的顺序拼，主键名直接取 code；code
 * 不在契约里（Tab、方向键、F13 等）也原样写出，由宿主校验报「无法识别」。 */

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
  "OSLeft",
  "OSRight",
]);

export type ChordKeyEvent = Pick<
  KeyboardEvent,
  "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey"
>;

/** `code` 是不是修饰键自己的按下（不产生主键）。 */
export function isModifierKeyCode(code: string): boolean {
  return MODIFIER_CODES.has(code);
}

function modifierPrefix(event: ChordKeyEvent): string {
  return `${event.ctrlKey ? "Ctrl+" : ""}${event.altKey ? "Alt+" : ""}${
    event.shiftKey ? "Shift+" : ""
  }${event.metaKey ? "Meta+" : ""}`;
}

/** 录制中的预览：只按着修饰键时是 `Ctrl+Shift+` 这样的前缀，主键已按时是完整写法。 */
export function chordEventPreview(event: ChordKeyEvent): string {
  return `${modifierPrefix(event)}${isModifierKeyCode(event.code) ? "" : event.code}`;
}

/** 按下的非修饰键组成一次提交，写法即规范形（修饰键顺序固定、键名取 code）。 */
export function chordFromKeyEvent(event: ChordKeyEvent): string {
  return `${modifierPrefix(event)}${event.code}`;
}
