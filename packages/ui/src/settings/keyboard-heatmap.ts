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

/** 只在屏幕键盘上才有的键：九宫格、字母组网格和其他触屏键。 */
function softKey(code: string): boolean {
  return (
    code.startsWith("Nine") ||
    code.startsWith("Fourteen") ||
    code.startsWith("Eighteen") ||
    code.startsWith("Soft")
  );
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
  // 字母组网格按键面字母与键数命名，和各原生宿主的统计标签一致。
  match = /^Eighteen([A-Z]{1,2})$/.exec(code);
  if (match) return `18 键 ${match[1]}`;
  match = /^Fourteen([A-Z]{1,2})$/.exec(code);
  if (match) return `14 键 ${match[1]}`;
  match = /^Eighteen([A-Z]{1,2})$/.exec(code);
  if (match) return `18 键 ${match[1]}`;
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

/**
 * 统计概览热力图上的一个键：`label` 印在键帽上，`name` 是读屏念的名字，省略时取 `keyLabel(code)`。`also` 是同一个键位在别的模式下记的 id，与 `code` 的次数相加。没有 `code` 的是占位，只占宽度、不画键帽（26 键第二行两端的半键缩进）。
 */
export type PhoneHeatmapKey = PreviewKey & { name?: string; also?: readonly string[] };

const phoneKey = (label: string, code: string, weight = 1, name?: string): PhoneHeatmapKey =>
  name === undefined ? { label, weight, code } : { label, weight, code, name };
const phoneLetters = (text: string) => [...text].map((letter) => phoneKey(letter, `Key${letter}`));
const phoneIndent: PhoneHeatmapKey = { label: "", weight: 0.5 };
// 九宫格两侧的 ，。？！ 四个键按下都记作同一个 `SoftPunctuation`，所以每个键帽上画的是四键合计。
const sidePunctuation = (label: string) =>
  phoneKey(label, "SoftPunctuation", 1.15, "侧栏标点（四键合计）");

/**
 * 统计概览画的手机键盘，照 HarmonyOS 手机竖屏、默认设置下的触屏键盘（`KeyboardView.ets`），每行按份额铺满整行：
 *
 * - 26 键（全拼）：中文模式的字母键面画大写（`LetterKeyFacePolicy`）；第二行两端各缩进半键；第三行左端在全拼中文模式下是「分词」（`usesQuanpinSeparator`，按下记 `'` 的键 `Quote`，其他模式下是记 `ShiftLeft` 的 ⇧），左右两键各 1.4 份（`LETTER_EDGE_SHARE`）；底行是 `designActionRow` 的 123 1.25 | 符 1.05 | 空格 4 | ， 1 | 中 1.05 | 回车 1.9，逗号长按输入句号。
 * - 9 键（拼音）：`touchNineKeyGrid` 的左列 ，。？（1.15 份）、3 × 3 字母格（每格 2 份，首格 @# 记 `Nine1`）、右列 删除 / 分词 / ！（1.15 份，分词记 `Quote`）；底行是 `NineKeyLayout.actionRow(false)` 的 符 1.05 | 123 1.25 | 空格 4.2 | 中 1.05 | 回车 1.6。
 *
 * 每个键带着键盘按下时记录的 id（`KeyIdPolicy`），计数因此落在按下的那个键帽上。空格键在键盘上印的是方案简称（如「全拼」），这里统一印「空格」，与 Android、iOS 的热力图一致。
 */
export const phoneHeatmapRows: Record<PhoneHeatmapLayout, PhoneHeatmapKey[][]> = {
  full: [
    phoneLetters("QWERTYUIOP"),
    [phoneIndent, ...phoneLetters("ASDFGHJKL"), phoneIndent],
    [
      // 热力图画的是中文全拼键盘，这一格只算分词的 `Quote`；英文模式下同一键位的 ⇧ 记 `ShiftLeft`，是另一个键，列进「其他键」，与 Android、iOS 一致。
      phoneKey("分词", "Quote", 1.4, "分词"),
      ...phoneLetters("ZXCVBNM"),
      phoneKey("⌫", "Backspace", 1.4),
    ],
    [
      phoneKey("123", "SoftLayer", 1.25),
      phoneKey("符", "SoftSymbol", 1.05),
      phoneKey("空格", "Space", 4),
      phoneKey("，", "Comma", 1),
      phoneKey("中", "SoftLanguage", 1.05),
      phoneKey("↵", "Enter", 1.9),
    ],
  ],
  nine: [
    [
      sidePunctuation("，"),
      phoneKey("@#", "Nine1", 2, "常用标点"),
      phoneKey("ABC", "Nine2", 2),
      phoneKey("DEF", "Nine3", 2),
      phoneKey("⌫", "Backspace", 1.15),
    ],
    [
      sidePunctuation("。"),
      phoneKey("GHI", "Nine4", 2),
      phoneKey("JKL", "Nine5", 2),
      phoneKey("MNO", "Nine6", 2),
      phoneKey("分词", "Quote", 1.15, "分词"),
    ],
    [
      sidePunctuation("？"),
      phoneKey("PQRS", "Nine7", 2),
      phoneKey("TUV", "Nine8", 2),
      phoneKey("WXYZ", "Nine9", 2),
      sidePunctuation("！"),
    ],
    [
      phoneKey("符", "SoftSymbol", 1.05),
      phoneKey("123", "SoftLayer", 1.25),
      phoneKey("空格", "Space", 4.2),
      phoneKey("中", "SoftLanguage", 1.05),
      phoneKey("↵", "Enter", 1.6),
    ],
  ],
};

/** 一个画出的键的次数：`code` 加上 `also` 里各 id 的次数；占位为 0。 */
export function phoneHeatmapCount(counts: Record<string, number>, key: PhoneHeatmapKey): number {
  if (!key.code) return 0;
  let count = counts[key.code] ?? 0;
  for (const code of key.also ?? []) count += counts[code] ?? 0;
  return count;
}

/** `layout` 画出的键所用的 id（含 `also`），每个只出现一次。 */
function phoneHeatmapCodes(layout: PhoneHeatmapLayout): Set<string> {
  const codes = new Set<string>();
  for (const row of phoneHeatmapRows[layout])
    for (const key of row) {
      if (key.code) codes.add(key.code);
      for (const code of key.also ?? []) codes.add(code);
    }
  return codes;
}

/** `layout` 中由不止一个键帽共用的 id：这些键帽画的是合计，所以不单独标占比。 */
export function phoneHeatmapSharedCodes(layout: PhoneHeatmapLayout): Set<string> {
  const seen = new Set<string>();
  const shared = new Set<string>();
  for (const row of phoneHeatmapRows[layout])
    for (const key of row) {
      if (!key.code) continue;
      if (seen.has(key.code)) shared.add(key.code);
      seen.add(key.code);
    }
  return shared;
}

/** 按过但 `layout` 没有画出来的键，按得多的在前，次数相同按 id 排序；与画出的键合起来正好是全部按键。 */
export function phoneHeatmapOthers(
  counts: Record<string, number>,
  layout: PhoneHeatmapLayout,
): KeyCount[] {
  const drawn = phoneHeatmapCodes(layout);
  return Object.entries(counts)
    .filter(([code, count]) => count > 0 && !drawn.has(code))
    .map(([code, count]) => ({ code, label: keyLabel(code), count }))
    .sort(byCount);
}

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
    for (const key of row) peak = Math.max(peak, phoneHeatmapCount(counts, key));
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
