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
  { value: "comma_period", label: "逗号句号" },
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

/** 设置页「按键布局」样式胶囊下方的一行说明，只写当前选中样式的副作用（符号键去了哪、怎么打开符号），三端逐字相同；表里没有的样式不画这一行。 */
export const twentySixKeyStyleNotes: Readonly<Partial<Record<TwentySixKeyStyle, string>>> = {
  long_space: "符号键移到工具栏。",
  emoji: "长按 123 打开符号。",
};
export const fourteenKeyStyleNotes: Readonly<Partial<Record<FourteenKeyStyle, string>>> = {
  emoji: "长按 123 打开符号。",
};
export const nineKeyStyleNotes: Readonly<Partial<Record<NineKeyStyle, string>>> = {
  tall_return: "回车竖占两行，符号键在左下角。",
};

/** 设置页整块键盘预览里，这个样式与「标准」不同、要用强调色描边的键，按键 id（`PreviewKey.code`）认，不比较键面文字；标准样式不高亮。三端高亮同一组键。 */
export const twentySixKeyStyleHighlights: Readonly<Record<TwentySixKeyStyle, readonly string[]>> = {
  standard: [],
  comma_period: ["Comma", "Period"],
  long_space: ["Space"],
  emoji: ["SoftEmoji"],
};
export const fourteenKeyStyleHighlights: Readonly<Record<FourteenKeyStyle, readonly string[]>> = {
  standard: [],
  long_space: ["Space"],
  emoji: ["SoftEmoji"],
};
export const nineKeyStyleHighlights: Readonly<Record<NineKeyStyle, readonly string[]>> = {
  standard: [],
  comma_period: ["Comma", "Period"],
  emoji: ["SoftEmoji"],
  tall_return: ["Enter"],
};
/** 「顶部数字行」开关的说明，三端逐字相同。 */
export const numberRowDescription = "在 26 键、14 键和 18 键上方加一行 1 到 0；横屏时不显示。";
/** 屏幕键盘页「按键布局」入口行的副标题，三端逐字相同。 */
export const keyLayoutEntryDescription = "各类键盘的底行样式、键位和顶部数字行";

/** 共享偏好 `touch_twenty_six_symbol_first` 开关的标题和说明，三端逐字相同（Android `TouchKeyStyle.SYMBOL_FIRST_TITLE`、iOS `TouchKeyLayoutOffering.symbolFirstTitle`）。 */
export const symbolFirstTitle = "26 键「符」与「123」互换";
export const symbolFirstDescription = "符 排在 123 左边；长空格和表情键样式没有 符，不受影响。";

/** 共享偏好 `touch_number_pad_space` 的取值（`client_core::preferences::TouchNumberPadSpace`）。 */
export type NumberPadSpace = NonNullable<Preferences["touch_number_pad_space"]>;
/** 设置页里能选的三种排法，`platform_default` 不单列；名字三端逐字相同。 */
export type NumberPadSpaceChoice = Exclude<NumberPadSpace, "platform_default">;
export const numberPadSpaceOptions: readonly { value: NumberPadSpaceChoice; label: string }[] = [
  { value: "right_column", label: "空格在右列" },
  { value: "after_zero", label: "空格在 0 右边" },
  { value: "before_zero", label: "空格在 0 左边" },
];
/** 「数字键盘」一组的标题和组末说明，三端逐字相同。 */
export const numberPadSpaceTitle = "数字键盘";
export const numberPadSpaceNote = "九宫格数字键盘里空格的位置；右侧大回车不受影响。";

/** 共享 React 设置页只在鸿蒙手机上给出这一组：缺键、`platform_default` 和不认识的值按鸿蒙原来的排法（空格在 0 左边），与键盘的 `TouchLayoutStylePolicy.numberPadSpace` 相同。 */
export function numberPadSpace(preferences: Preferences): NumberPadSpaceChoice {
  const value = preferences.touch_number_pad_space;
  return value === "right_column" || value === "after_zero" ? value : "before_zero";
}

/** 「符」与「123」互换打开时把底行里排在 123 后面的 符 挪到 123 左边，其余键不变，与键盘的 `TouchLayoutStylePolicy.symbolFirst` 相同；没有 符 的行原样返回。 */
export function withSymbolFirst<T extends { code?: string }>(
  row: readonly T[],
  enabled: boolean,
): T[] {
  const layer = row.findIndex((item) => item.code === "SoftLayer");
  const symbol = row.findIndex((item) => item.code === "SoftSymbol");
  if (!enabled || layer < 0 || symbol < layer) return [...row];
  const moved = [...row];
  const [panel] = moved.splice(symbol, 1);
  moved.splice(layer, 0, panel);
  return moved;
}

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
 *
 * 123 和 中/英 与标准九键底行同宽（`NineKeyLayout.tallReturnRow`）：标准底行 9.15 份铺满整块键面，外框 8.3 份，所以这两个键按 8.3 / 9.15 换算到外框的份额，空格占网格剩下的部分。
 */
const tallReturnSide = 1.15;
const tallReturnGrid = 6;
/** 九键标准底行：符 | 123 | 空格 | 中 | 回车。 */
const nineKeyStandardRow: PreviewKey[] = [
  symbol,
  layer,
  key("空格", 4.2, "Space"),
  language,
  key("↵", 1.6, "Enter"),
];
const nineKeyStandardShare = nineKeyStandardRow.reduce((sum, item) => sum + item.weight, 0);
const tallReturnScale = (tallReturnSide * 2 + tallReturnGrid) / nineKeyStandardShare;
const tallReturnLayer = key("123", layer.weight * tallReturnScale, "SoftLayer");
const tallReturnLanguage = key("中", language.weight * tallReturnScale, "SoftLanguage");
const tallReturnSpace = tallReturnGrid - tallReturnLayer.weight - tallReturnLanguage.weight;

/**
 * 九键字母层每种样式的底行（`NineKeyLayout.actionRow`）：标准 符 | 123 | 空格 | 中 | 回车；底栏带标点在空格两侧加 ， 和 。，各从空格让出一份；表情键在 123 右边加 😀，从空格让出 1.05。
 *
 * 右侧大回车的底行按外框的三列排（份额总和与网格行相同）：符 1.15 | 123 | 空格 | 中 | 回车 1.15，123 和 中 与标准底行同宽（`tallReturnLayer`、`tallReturnLanguage`），回车其实从上一行往下跨两行，整块外框见 `keyboard-heatmap.ts` 的 `nineTallReturnRows`。
 */
export const nineKeyBottomRows: Record<NineKeyStyle, PreviewKey[]> = {
  standard: nineKeyStandardRow,
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
    tallReturnLayer,
    key("空格", tallReturnSpace, "Space"),
    tallReturnLanguage,
    key("↵", tallReturnSide, "Enter"),
  ],
};

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

/** 统计热力图要照着画的那几项：两种键盘的底行样式、数字行和「符」与「123」互换（缺省为关）。 */
export type TouchKeyLayoutStyle = {
  twentySixKeyStyle: TwentySixKeyStyle;
  nineKeyStyle: NineKeyStyle;
  numberRow: boolean;
  twentySixSymbolFirst?: boolean;
};

export function touchKeyLayoutStyle(preferences: Preferences): TouchKeyLayoutStyle {
  return {
    twentySixKeyStyle: twentySixKeyStyle(preferences),
    nineKeyStyle: nineKeyStyle(preferences),
    numberRow: preferences.touch_number_row === true,
    twentySixSymbolFirst: preferences.touch_twenty_six_symbol_first === true,
  };
}
