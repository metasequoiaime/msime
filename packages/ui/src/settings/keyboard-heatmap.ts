import {
  ansiHeatmapKeyboardRows,
  nineKeyHeatmapRows,
  touchKeyboardRows,
  type PreviewKey,
} from "../keyboard/keyboard-layouts";
import { clamp } from "../core/number";

/** Per-key press counts per local day, as the statistics store keeps them: `{ "YYYY-MM-DD": { keyId: count } }`. */
export type DailyKeyCounts = Record<string, Record<string, number>>;

export type KeyboardHeatmapLayout = "ansi" | "touch";

/** A drawn key with its count: `label` is printed on the cap, `name` is what it is announced as. A spacer has no `code` and a count of zero. */
export type KeyboardHeatmapKey = {
  code?: string;
  label: string;
  name: string;
  weight: number;
  count: number;
};

export type KeyCount = { code: string; label: string; count: number };

export type KeyboardHeatmapModel = {
  layout: KeyboardHeatmapLayout;
  rows: KeyboardHeatmapKey[][];
  /** The nine-key grid, present only on the touch layout and only when a nine-key cell has a count. */
  nineRows: KeyboardHeatmapKey[][] | null;
  /** Counted keys the drawn layout has no place for, most pressed first. */
  others: KeyCount[];
  /** The five most pressed keys, drawn or not. */
  top: KeyCount[];
  /** The largest single-key count, which the shade levels are relative to; at least 1. */
  maximum: number;
  total: number;
};

/**
 * Sums the per-key counts of the days in scope; `null` is the whole retained history, matching how `scopedBreakdown` reads the page's scope.
 *
 * Only positive finite counts are kept, so a key the result holds was pressed at least once in scope.
 */
export function scopedKeyCounts(
  dailyKeys: DailyKeyCounts | undefined,
  scopeKeys: readonly string[] | null,
): Record<string, number> {
  const result: Record<string, number> = {};
  if (!dailyKeys) return result;
  const days = scopeKeys ?? Object.keys(dailyKeys);
  for (const day of days) {
    const keys = dailyKeys[day];
    if (!keys) continue;
    for (const [code, count] of Object.entries(keys)) {
      if (!Number.isFinite(count) || count <= 0) continue;
      result[code] = (result[code] ?? 0) + count;
    }
  }
  return result;
}

/** Whether `code` exists only on an on-screen keyboard. */
function softKey(code: string): boolean {
  return code.startsWith("Nine") || code.startsWith("Soft");
}

/**
 * Which keyboard to draw: the phone's 26-key layout on a mobile page or whenever a soft-keyboard-only key was counted, otherwise the physical ANSI board.
 */
export function keyboardHeatmapLayout(
  counts: Record<string, number>,
  mobile: boolean,
): KeyboardHeatmapLayout {
  return mobile || Object.keys(counts).some(softKey) ? "touch" : "ansi";
}

const mac = (platform: string | undefined) => platform === "macos";

function metaName(platform: string | undefined): string {
  if (mac(platform)) return "Command";
  if (platform === "linux") return "Super";
  return "Win";
}

const fixedLabels: Record<string, string> = {
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  IntlBackslash: "ISO \\",
  IntlRo: "ろ",
  IntlYen: "¥",
  Lang1: "Lang1",
  Lang2: "Lang2",
  Convert: "変換",
  NonConvert: "無変換",
  KanaMode: "かな",
  Space: "空格",
  Enter: "回车",
  Backspace: "退格",
  Tab: "Tab",
  Escape: "Esc",
  Delete: "Delete",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "Page Up",
  PageDown: "Page Down",
  ArrowUp: "上箭头",
  ArrowDown: "下箭头",
  ArrowLeft: "左箭头",
  ArrowRight: "右箭头",
  CapsLock: "Caps Lock",
  ShiftLeft: "左 Shift",
  ShiftRight: "右 Shift",
  Fn: "Fn",
  ContextMenu: "菜单键",
  NumpadDecimal: "小键盘 .",
  NumpadEnter: "小键盘回车",
  NumpadAdd: "小键盘 +",
  NumpadSubtract: "小键盘 -",
  NumpadMultiply: "小键盘 *",
  NumpadDivide: "小键盘 /",
  NumLock: "Num Lock",
  Nine1: "九宫格 1（标点）",
  SoftPunctuation: "九宫格侧栏标点",
  SoftSymbol: "符号",
  SoftLayer: "123",
  SoftLanguage: "中/英",
  SoftGlobe: "地球键",
  SoftEmoji: "表情",
  SoftVoice: "语音",
};

/** The name a key is announced and listed under, e.g. `A`, `空格`, `左 Shift`; an id this page does not know is shown as itself. */
export function keyLabel(code: string, platform?: string): string {
  const fixed = fixedLabels[code];
  if (fixed) return fixed;
  let match =
    /^Key([A-Z])$/.exec(code) ?? /^Digit([0-9])$/.exec(code) ?? /^(F[0-9]{1,2})$/.exec(code);
  if (match) return match[1];
  match = /^Numpad([0-9])$/.exec(code);
  if (match) return `小键盘 ${match[1]}`;
  match = /^Nine([0-9])$/.exec(code);
  if (match) return `九宫格 ${match[1]}`;
  match = /^(Control|Alt|Meta)(Left|Right)$/.exec(code);
  if (match) {
    const side = match[2] === "Left" ? "左" : "右";
    const name =
      match[1] === "Meta"
        ? metaName(platform)
        : match[1] === "Alt"
          ? mac(platform)
            ? "Option"
            : "Alt"
          : mac(platform)
            ? "Control"
            : "Ctrl";
    return `${side} ${name}`;
  }
  return code;
}

/** The text printed on a drawn key cap: the layout's own label, upper-case letters, and the platform's names for the modifier keys. */
function capLabel(key: PreviewKey, platform: string | undefined): string {
  const code = key.code;
  if (!code) return key.label;
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (code.startsWith("Meta")) return mac(platform) ? "⌘" : metaName(platform);
  if (code.startsWith("Alt")) return mac(platform) ? "⌥" : "Alt";
  if (code.startsWith("Control")) return mac(platform) ? "⌃" : "Ctrl";
  return key.label;
}

/** Most pressed first; equal counts fall back to the id so the order does not depend on how the host serialized the map. */
function byCount(left: KeyCount, right: KeyCount): number {
  return right.count - left.count || (left.code < right.code ? -1 : left.code > right.code ? 1 : 0);
}

/**
 * Places scoped counts on the chosen keyboard: the drawn rows with each key's count, the nine-key grid when one of its cells was pressed, the keys the layout cannot draw, and the top five.
 */
export function keyboardHeatmapModel(
  counts: Record<string, number>,
  mobile: boolean,
  platform?: string,
): KeyboardHeatmapModel {
  const layout = keyboardHeatmapLayout(counts, mobile);
  const drawn = new Set<string>();
  const place = (rows: PreviewKey[][]) =>
    rows.map((row) =>
      row.map((key) => {
        if (key.code) drawn.add(key.code);
        return {
          ...(key.code ? { code: key.code } : {}),
          label: capLabel(key, platform),
          name: key.code ? keyLabel(key.code, platform) : "",
          weight: key.weight,
          count: key.code ? (counts[key.code] ?? 0) : 0,
        };
      }),
    );
  const rows = place(layout === "touch" ? touchKeyboardRows : ansiHeatmapKeyboardRows);
  const nineRows =
    layout === "touch" && Object.keys(counts).some((code) => code.startsWith("Nine"))
      ? place(nineKeyHeatmapRows)
      : null;
  const listed = Object.entries(counts)
    .filter(([, count]) => count > 0)
    .map(([code, count]) => ({ code, label: keyLabel(code, platform), count }))
    .sort(byCount);
  const total = listed.reduce((sum, key) => sum + key.count, 0);
  return {
    layout,
    rows,
    nineRows,
    others: listed.filter((key) => !drawn.has(key.code)),
    top: listed.slice(0, 5),
    maximum: Math.max(1, ...listed.map((key) => key.count)),
    total,
  };
}

/** The shade a count takes, 0 (never pressed) to 4, on the same scale as the calendar heatmap. */
export function keyHeatLevel(count: number, maximum: number): number {
  return count <= 0 ? 0 : clamp(Math.ceil((count / maximum) * 4), 1, 4);
}

/** 统计概览的按键热力图画哪种屏幕键盘。 */
export type PhoneHeatmapLayout = "full" | "nine";

const phoneKey = (label: string, code: string, weight = 1): PreviewKey => ({ label, weight, code });
const phoneLetters = (text: string) => [...text].map((letter) => phoneKey(letter, `Key${letter}`));

/**
 * 统计概览画的手机键盘，按 Android 的 `KeyHeatmapView` 布局：26 键键盘每行十个单位、按键居中，以及带两侧列的九宫格。每个键带着键盘记录的 id，计数因此落在按下的那个键帽上；`Nine1` 按鸿蒙九宫格的印字读作「分词」。
 */
export const phoneHeatmapRows: Record<PhoneHeatmapLayout, PreviewKey[][]> = {
  full: [
    phoneLetters("QWERTYUIOP"),
    phoneLetters("ASDFGHJKL"),
    [phoneKey("⇧", "ShiftLeft", 1.5), ...phoneLetters("ZXCVBNM"), phoneKey("⌫", "Backspace", 1.5)],
    [
      phoneKey("123", "SoftLayer", 1.25),
      phoneKey("中", "SoftLanguage"),
      phoneKey("，", "Comma"),
      phoneKey("空格", "Space", 4.25),
      phoneKey("。", "Period"),
      phoneKey("↵", "Enter", 1.5),
    ],
  ],
  nine: [
    [
      phoneKey("，", "Comma"),
      phoneKey("分词", "Nine1", 1.4),
      phoneKey("ABC", "Nine2", 1.4),
      phoneKey("DEF", "Nine3", 1.4),
      phoneKey("⌫", "Backspace"),
    ],
    [
      phoneKey("。", "Period"),
      phoneKey("GHI", "Nine4", 1.4),
      phoneKey("JKL", "Nine5", 1.4),
      phoneKey("MNO", "Nine6", 1.4),
      phoneKey("符", "SoftSymbol"),
    ],
    [
      phoneKey("标点", "SoftPunctuation"),
      phoneKey("PQRS", "Nine7", 1.4),
      phoneKey("TUV", "Nine8", 1.4),
      phoneKey("WXYZ", "Nine9", 1.4),
      phoneKey("0", "Nine0"),
    ],
    [
      phoneKey("123", "SoftLayer"),
      phoneKey("中", "SoftLanguage"),
      phoneKey("空格", "Space", 2.8),
      phoneKey("↵", "Enter", 1.4),
    ],
  ],
};

/** 九宫格按键是否比字母键按得更多，据此选择热力图打开时的布局。 */
export function prefersNineKey(counts: Record<string, number>): boolean {
  let letters = 0;
  let cells = 0;
  for (const [code, count] of Object.entries(counts)) {
    if (code.startsWith("Key")) letters += count;
    if (code.startsWith("Nine")) cells += count;
  }
  return cells > letters;
}

/**
 * 统计概览热力图上一个计数的深浅档（0–4），相对于所画的最大计数：没有为 0 档，其余按峰值的四等分落档。即 Android 的 `HeatmapView.level`，打字少的人也能看出哪些天或哪些键突出。
 */
export function summaryHeatLevel(count: number, peak: number): number {
  if (count <= 0 || peak <= 0) return 0;
  const ratio = count / peak;
  if (ratio > 0.75) return 4;
  if (ratio > 0.5) return 3;
  if (ratio > 0.25) return 2;
  return 1;
}

/** `layout` 所画按键中的最大计数，即深浅档参照的峰值。 */
export function phoneHeatmapPeak(
  counts: Record<string, number>,
  layout: PhoneHeatmapLayout,
): number {
  let peak = 0;
  for (const row of phoneHeatmapRows[layout])
    for (const key of row) if (key.code) peak = Math.max(peak, counts[key.code] ?? 0);
  return peak;
}

/**
 * `layout` 中按得最多的拼写键：26 键键盘上是一个字母，九宫格上是一个带字母的格（2–9）。一个都没按过时为 null。
 */
export function phoneHeatmapLeader(
  counts: Record<string, number>,
  layout: PhoneHeatmapLayout,
): { label: string; count: number } | null {
  let best: { label: string; count: number } | null = null;
  for (const row of phoneHeatmapRows[layout])
    for (const key of row) {
      const code = key.code ?? "";
      const spelling = layout === "nine" ? /^Nine[2-9]$/.test(code) : /^Key[A-Z]$/.test(code);
      const count = counts[code] ?? 0;
      if (spelling && count > (best?.count ?? 0)) best = { label: key.label, count };
    }
  return best;
}
