import type { Preferences } from "../core/preferences-model";
import type { PreviewKey } from "./keyboard-layouts";

/** 共享偏好 `touch_twenty_six_key_style` 的取值（`client_core::preferences::TouchTwentySixKeyStyle`）。 */
export type TwentySixKeyStyle = NonNullable<Preferences["touch_twenty_six_key_style"]>;
/** 共享偏好 `touch_fourteen_key_style` 的取值，14 键与 18 键共用。 */
export type FourteenKeyStyle = NonNullable<Preferences["touch_fourteen_key_style"]>;
/** 共享偏好 `touch_nine_key_style` 的取值。 */
export type NineKeyStyle = NonNullable<Preferences["touch_nine_key_style"]>;

/** 设置页「按键布局」里每种样式的名字，三端相同。 */
export const twentySixKeyStyleOptions: readonly { value: TwentySixKeyStyle; label: string }[] = [
  { value: "standard", label: "标准" },
  { value: "comma_period", label: "逗号句号在空格两侧" },
  { value: "long_space", label: "长空格" },
  { value: "emoji", label: "表情键" },
];

export const fourteenKeyStyleOptions: readonly { value: FourteenKeyStyle; label: string }[] = [
  { value: "standard", label: "标准" },
  { value: "long_space", label: "长空格" },
  { value: "emoji", label: "表情键" },
];

export const nineKeyStyleOptions: readonly { value: NineKeyStyle; label: string }[] = [
  { value: "standard", label: "标准" },
  { value: "comma_period", label: "底栏带标点" },
  { value: "emoji", label: "表情键" },
  { value: "tall_return", label: "右侧大回车" },
];

const key = (label: string, weight: number, code: string): PreviewKey => ({ label, weight, code });

const layer = key("123", 1.25, "SoftLayer");
const symbol = key("符", 1.05, "SoftSymbol");
const language = key("中", 1.05, "SoftLanguage");
const comma = key("，", 1, "Comma");
const period = key("。", 1, "Period");

/**
 * HarmonyOS 手机中文 26 键每种样式的底行，键序、份额和键 id 与键盘的 `TouchLayoutStylePolicy.designRow` 相同（鸿蒙没有地球键）：标准 123 | 符 | 空格 | ， | 中 | 回车；逗号句号在空格两侧 123 | 符 | ， | 空格 | 。 | 中 | 回车，句号占从空格让出的一份；长空格 123 | ， | 空格 | 中 | 回车，符 的宽度给空格；表情键 123 | 😀 | 空格 | ， | 中 | 回车，😀 占 符 的位置和宽度。
 */
export const twentySixKeyBottomRows: Record<TwentySixKeyStyle, PreviewKey[]> = {
  standard: [layer, symbol, key("空格", 4, "Space"), comma, language, key("↵", 1.9, "Enter")],
  comma_period: [
    layer,
    symbol,
    comma,
    key("空格", 3, "Space"),
    period,
    language,
    key("↵", 1.9, "Enter"),
  ],
  long_space: [layer, comma, key("空格", 5.05, "Space"), language, key("↵", 1.9, "Enter")],
  emoji: [
    layer,
    key("😀", 1.05, "SoftEmoji"),
    key("空格", 4, "Space"),
    comma,
    language,
    key("↵", 1.9, "Enter"),
  ],
};

/** 14 键与 18 键每种样式的底行：标准 123 | ， | 空格 | 。 | 中 | 回车；长空格把 。 的一份给空格；表情键 123 | 😀 | 空格 | ，，😀 是原 ， 的一份、， 是原 。 的一份。 */
export const fourteenKeyBottomRows: Record<FourteenKeyStyle, PreviewKey[]> = {
  standard: [layer, comma, key("空格", 4, "Space"), period, language, key("↵", 1.9, "Enter")],
  long_space: [layer, comma, key("空格", 5, "Space"), language, key("↵", 1.9, "Enter")],
  emoji: [
    layer,
    key("😀", 1, "SoftEmoji"),
    key("空格", 4, "Space"),
    comma,
    language,
    key("↵", 1.9, "Enter"),
  ],
};

/**
 * 九键「右侧大回车」外框的份额，与 HarmonyOS `NineKeyLayout.SIDE_SHARE`、`GRID_SHARE` 相同：左列和右列各 1.15，中间网格 6（三列字母格各 2）。底行与这三列对齐：符 在左列下方，123 | 空格 | 中/英 恰好占网格的宽度，回车在右列。
 */
const tallReturnSide = 1.15;
const tallReturnGrid = 6;
const tallReturnSpace = tallReturnGrid - layer.weight - language.weight;

/** 可能跨行的示意键：`rowSpan` 为 2 时这个键从所在行往下多占一行（「右侧大回车」的回车），下一行同一位置是没有 `code` 的占位。 */
export type FramePreviewKey = PreviewKey & { rowSpan?: number };

/**
 * 九键字母层每种样式的底行（`NineKeyLayout.actionRow`）：标准 符 | 123 | 空格 | 中 | 回车；底栏带标点在空格两侧加 ， 和 。，各从空格让出一份；表情键在 123 右边加 😀，从空格让出 1.05。
 *
 * 右侧大回车的底行按外框的三列排（份额总和与网格行相同）：符 1.15 | 123 1.25 | 空格 3.7 | 中 1.05 | 回车 1.15，回车其实从上一行往下跨两行，示意图见 `nineKeyTallReturnPreview`。
 */
export const nineKeyBottomRows: Record<NineKeyStyle, PreviewKey[]> = {
  standard: [symbol, layer, key("空格", 4.2, "Space"), language, key("↵", 1.6, "Enter")],
  comma_period: [
    symbol,
    layer,
    comma,
    key("空格", 2.2, "Space"),
    period,
    language,
    key("↵", 1.6, "Enter"),
  ],
  emoji: [
    symbol,
    layer,
    key("😀", 1.05, "SoftEmoji"),
    key("空格", 3.15, "Space"),
    language,
    key("↵", 1.6, "Enter"),
  ],
  tall_return: [
    key("符", tallReturnSide, "SoftSymbol"),
    layer,
    key("空格", tallReturnSpace, "Space"),
    language,
    key("↵", tallReturnSide, "Enter"),
  ],
};

/**
 * 设置页「右侧大回车」选项下的示意：九宫格最后一行和它下面的底行。左列是符号栏（末尾多了从右列让过来的 ！），栏下方是 符；回车在右列、跨这两行，底行在回车下方的位置是占位。
 */
export const nineKeyTallReturnPreview: FramePreviewKey[][] = [
  [
    key("？！", tallReturnSide, "SoftPunctuation"),
    key("PQRS", 2, "Nine7"),
    key("TUV", 2, "Nine8"),
    key("WXYZ", 2, "Nine9"),
    { ...key("↵", tallReturnSide, "Enter"), rowSpan: 2 },
  ],
  [...nineKeyBottomRows.tall_return.slice(0, -1), { label: "", weight: tallReturnSide }],
];

/** 字母上方数字行的十个键，按下记 `Digit1`…`Digit0`。 */
export const numberRowKeys: PreviewKey[] = [..."1234567890"].map((digit) =>
  key(digit, 1, `Digit${digit}`),
);

/** 只认表里自己的键：`in` 会把 `toString` 这类原型上的名字也当成样式。 */
function known(rows: Record<string, PreviewKey[]>, value: string): boolean {
  return Object.prototype.hasOwnProperty.call(rows, value);
}

/** 偏好里存的样式；缺键或本页不认识的值（较新版本加的样式）按标准，与键盘的宽松读法相同。 */
export function twentySixKeyStyle(preferences: Preferences): TwentySixKeyStyle {
  const value = preferences.touch_twenty_six_key_style;
  return value && known(twentySixKeyBottomRows, value) ? value : "standard";
}

export function fourteenKeyStyle(preferences: Preferences): FourteenKeyStyle {
  const value = preferences.touch_fourteen_key_style;
  return value && known(fourteenKeyBottomRows, value) ? value : "standard";
}

export function nineKeyStyle(preferences: Preferences): NineKeyStyle {
  const value = preferences.touch_nine_key_style;
  return value && known(nineKeyBottomRows, value) ? value : "standard";
}

/** 统计热力图要照着画的那几项：两种键盘的底行样式和数字行。 */
export type TouchKeyLayoutStyle = {
  twentySixKeyStyle: TwentySixKeyStyle;
  nineKeyStyle: NineKeyStyle;
  numberRow: boolean;
};

export function touchKeyLayoutStyle(preferences: Preferences): TouchKeyLayoutStyle {
  return {
    twentySixKeyStyle: twentySixKeyStyle(preferences),
    nineKeyStyle: nineKeyStyle(preferences),
    numberRow: preferences.touch_number_row === true,
  };
}
