import type { Preferences } from "../core/preferences-model";
import {
  inferredTouchKeyboardScheme,
  type TouchKeyboardScheme,
} from "../settings/touch-keyboard-scheme-helpers";

/**
 * A key drawn in a keyboard preview. `code` is the W3C `KeyboardEvent.code` (or soft-keyboard id) the statistics store counts the key under; it is absent on spacers and on keys the store has no id for.
 */
export type PreviewKey = { label: string; weight: number; code?: string };

const key = (label: string, weight = 1, code?: string): PreviewKey =>
  code === undefined ? { label, weight } : { label, weight, code };

const punctuationCodes: Record<string, string> = {
  "`": "Backquote",
  "-": "Minus",
  "=": "Equal",
  "[": "BracketLeft",
  "]": "BracketRight",
  "\\": "Backslash",
  ";": "Semicolon",
  "'": "Quote",
  ",": "Comma",
  ".": "Period",
  "/": "Slash",
};

/** The `KeyboardEvent.code` of the key that types `character` unshifted on an ANSI layout. */
function characterCode(character: string): string | undefined {
  if (/^[a-z]$/.test(character)) return `Key${character.toUpperCase()}`;
  if (/^[0-9]$/.test(character)) return `Digit${character}`;
  return punctuationCodes[character];
}

const letters = (text: string) => [...text].map((label) => key(label, 1, characterCode(label)));

export const desktopKeyboardRows: PreviewKey[][] = [
  [...letters("`1234567890-="), key("Backspace", 1.9, "Backspace")],
  [key("Tab", 1.5, "Tab"), ...letters("qwertyuiop[]"), key("\\", 1.4, "Backslash")],
  [key("Caps Lock", 1.85, "CapsLock"), ...letters("asdfghjkl;'"), key("Enter", 2, "Enter")],
  [key("Shift", 2.35, "ShiftLeft"), ...letters("zxcvbnm,./"), key("Shift", 2.15, "ShiftRight")],
  [
    key("Ctrl", 1.25, "ControlLeft"),
    key("Win", 1.25, "MetaLeft"),
    key("Alt", 1.25, "AltLeft"),
    key("Space", 6.7, "Space"),
    key("Alt", 1.25, "AltRight"),
    key("Win", 1.25, "MetaRight"),
    key("Del", 1.25, "Delete"),
    key("Ctrl", 1.25, "ControlRight"),
  ],
];

const functionKeys = (from: number, to: number) =>
  Array.from({ length: to - from + 1 }, (_, index) =>
    key(`F${from + index}`, 1, `F${from + index}`),
  );

/**
 * The physical ANSI keyboard the typing statistics heatmap draws: the escape and function row over the main block of `desktopKeyboardRows`. The bottom row differs from the preview's, which puts Del there for the on-screen keyboard; a real ANSI board has the context-menu key in that place, and Delete is drawn nowhere rather than somewhere it is not.
 */
export const ansiHeatmapKeyboardRows: PreviewKey[][] = [
  [
    key("Esc", 1, "Escape"),
    key("", 0.9),
    ...functionKeys(1, 4),
    key("", 0.5),
    ...functionKeys(5, 8),
    key("", 0.5),
    ...functionKeys(9, 12),
  ],
  ...desktopKeyboardRows.slice(0, 4),
  [
    key("Ctrl", 1.25, "ControlLeft"),
    key("Win", 1.25, "MetaLeft"),
    key("Alt", 1.25, "AltLeft"),
    key("Space", 6.7, "Space"),
    key("Alt", 1.25, "AltRight"),
    key("Win", 1.25, "MetaRight"),
    key("Menu", 1.25, "ContextMenu"),
    key("Ctrl", 1.25, "ControlRight"),
  ],
];

export const touchKeyboardRows: PreviewKey[][] = [
  letters("qwertyuiop"),
  [key("", 0.5), ...letters("asdfghjkl"), key("", 0.5)],
  [key("⇧", 1.5, "ShiftLeft"), ...letters("zxcvbnm"), key("⌫", 1.5, "Backspace")],
  [
    key("符号", 1.5, "SoftSymbol"),
    key("中/英", 1.5, "SoftLanguage"),
    key("空格", 4.5, "Space"),
    key("，", 1, "Comma"),
    key("↵", 1.5, "Enter"),
  ],
];

/** HarmonyOS 手机键盘上画出的几种中文键盘，键名取 `HarmonyKeyboardLayout`。 */
export type HarmonyTouchLayout =
  | "twenty_six_key"
  | "fourteen_key"
  | "eighteen_key"
  | "fourteen_key_shuangpin"
  | "eighteen_key_shuangpin"
  | "nine_key"
  | "japanese_nine_key";

// HarmonyOS 26 键和 14 键共用的底排（`KeyboardView.ets` 的 `designActionRow`）：123、逗号、空格、句号、中/英、回车。
const harmonyActionRow = [
  key("123", 1.25),
  key("，", 1),
  key("空格", 4),
  key("。", 1),
  key("中/英", 1.05),
  key("↵", 1.9),
];

/**
 * HarmonyOS 手机上「尺寸」预览按当前的中文键盘画，键位和列宽照原生键盘：26 键见 `KeyboardView.ets` 的 `typingFace`，14 键是 `FourteenKeyLayout.ts`，9 键是 `NineKeyLayout.ts` 的触屏网格（左列标点、3 × 3 字母格、右列删除 / 分词 / 感叹号，`NINE_KEY_SIDE_SHARE` 1.15、每格 2），日语 9 键是 `JapaneseNineKeyLayout.ts` 的 4 × 3 假名格。日语 9 键的底排是按 vp 定宽的经典底排，这里按宽度折成比例。
 */
export const harmonyTouchKeyboardRows: Record<HarmonyTouchLayout, PreviewKey[][]> = {
  twenty_six_key: [
    letters("qwertyuiop"),
    [key("", 0.5), ...letters("asdfghjkl"), key("", 0.5)],
    [key("⇧", 1.4, "ShiftLeft"), ...letters("zxcvbnm"), key("⌫", 1.4, "Backspace")],
    harmonyActionRow,
  ],
  fourteen_key: [
    ["QW", "ER", "TY", "UI", "OP"].map((label) => key(label)),
    ["AS", "DF", "GH", "JK", "L"].map((label) => key(label)),
    [key("符", 1.4), ...["ZX", "CV", "BN", "M"].map((label) => key(label)), key("⌫", 1.4)],
    harmonyActionRow,
  ],
  eighteen_key: [
    ["Q", "WE", "RT", "Y", "U", "IO", "P"].map((label) => key(label)),
    ["A", "SD", "FG", "H", "JK", "L"].map((label) => key(label)),
    [key("符", 1.4), ...["Z", "XC", "V", "BN", "M"].map((label) => key(label)), key("⌫", 1.4)],
    harmonyActionRow,
  ],
  fourteen_key_shuangpin: [
    ["QW", "ER", "TY", "UI", "OP"].map((label) => key(label)),
    ["AS", "DF", "GH", "JK", "L;"].map((label) => key(label)),
    [key("符", 1.4), ...["ZX", "CV", "BN", "M"].map((label) => key(label)), key("⌫", 1.4)],
    harmonyActionRow,
  ],
  eighteen_key_shuangpin: [
    ["Q", "WE", "RT", "Y", "U", "IO", "P"].map((label) => key(label)),
    ["A", "SD", "FG", "H", "JK", "L;"].map((label) => key(label)),
    [key("符", 1.4), ...["Z", "XC", "V", "BN", "M"].map((label) => key(label)), key("⌫", 1.4)],
    harmonyActionRow,
  ],
  nine_key: [
    [key("，", 1.15), key("@#", 2), key("ABC", 2), key("DEF", 2), key("⌫", 1.15)],
    [key("。", 1.15), key("GHI", 2), key("JKL", 2), key("MNO", 2), key("分词", 1.15)],
    [key("？", 1.15), key("PQRS", 2), key("TUV", 2), key("WXYZ", 2), key("！", 1.15)],
    [key("123", 1.25), key("空格", 4.2), key("0", 1.05), key("中/英", 1.05), key("↵", 1.6)],
  ],
  japanese_nine_key: [
    [key("あ"), key("か"), key("さ")],
    [key("た"), key("な"), key("は")],
    [key("ま"), key("や"), key("ら")],
    [key("小゛゜"), key("わ"), key("、")],
    [key("123", 1.25), key("符", 1.15), key("空白", 4.5), key("中", 0.9), key("↵", 1.55)],
  ],
};

/** 把触屏方案和鸿蒙自己的 26/14/18 键选择映射到实际绘制的键面。详情页和「屏幕键盘」页共用这条规则，避免预览各画各的键位。 */
export function harmonyTouchLayout(
  selected: TouchKeyboardScheme,
  shuangpinLayout: "twenty_six_key" | "fourteen_key" | "eighteen_key" = "twenty_six_key",
): HarmonyTouchLayout {
  if (
    selected === "xiaohe" ||
    selected === "ziranma" ||
    selected === "microsoft" ||
    selected === "shoudao"
  ) {
    return shuangpinLayout === "fourteen_key"
      ? "fourteen_key_shuangpin"
      : shuangpinLayout === "eighteen_key"
        ? "eighteen_key_shuangpin"
        : "twenty_six_key";
  }
  if (
    selected === "fourteen_key" ||
    selected === "eighteen_key" ||
    selected === "nine_key" ||
    selected === "japanese_nine_key"
  )
    return selected;
  return "twenty_six_key";
}

/** 鸿蒙手机当前方案对应的预览键面；没有方案字段时沿用默认的 26 键。 */
export function harmonyTouchRowsForPreferences(preferences: Preferences): PreviewKey[][] {
  const selected = inferredTouchKeyboardScheme(preferences);
  const layout = harmonyTouchLayout(selected, preferences.touch_shuangpin_layout);
  return harmonyTouchKeyboardRows[layout];
}

/** The nine-key pinyin grid, each cell named by the digit printed on it; `Nine1` is the punctuation and separator cell. */
export const nineKeyHeatmapRows: PreviewKey[][] = [
  [key("1 ，。", 1, "Nine1"), key("2 ABC", 1, "Nine2"), key("3 DEF", 1, "Nine3")],
  [key("4 GHI", 1, "Nine4"), key("5 JKL", 1, "Nine5"), key("6 MNO", 1, "Nine6")],
  [key("7 PQRS", 1, "Nine7"), key("8 TUV", 1, "Nine8"), key("9 WXYZ", 1, "Nine9")],
  [key("", 1), key("0", 1, "Nine0"), key("", 1)],
];

export const actionKeyboardLabels = new Set(["Backspace", "Enter", "Shift", "Del", "⇧", "⌫", "↵"]);
