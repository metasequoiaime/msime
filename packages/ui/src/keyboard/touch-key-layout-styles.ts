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

/** 九键左侧符号栏的两张表（#10223）：`letter` 是拼音九键、九键双拼和笔画字母层共用的那张（共享偏好 `touch_nine_key_sidebar_symbols`），`digit` 是所有九宫格数字键盘 #+= 上面那张（`touch_nine_key_digit_sidebar_symbols`）。 */
export type NineKeySidebarKind = "letter" | "digit";

/** 一张表最多几项、一项最多几个 Unicode 码位、用一个空格连起来最多几个 UTF-16 单元，与 client-core `NINE_KEY_SIDEBAR_MAX_*`、Android `NineKeySidebarPolicy.MAX_*` 相同。 */
export const nineKeySidebarMaxSymbols = 30;
export const nineKeySidebarMaxSymbolCodePoints = 4;
export const nineKeySidebarMaxTextUtf16 = 200;

/** 编辑页里可以点一下加入的常用符号，照 Android `NineKeySidebarPolicy.ADDABLE_SYMBOLS` 逐项抄，顺序固定。 */
export const nineKeySidebarAddableSymbols: readonly string[] = [
  "，",
  "。",
  "？",
  "！",
  "、",
  "；",
  "：",
  "…",
  "……",
  "—",
  "·",
  "“",
  "”",
  "‘",
  "’",
  "（",
  "）",
  "（）",
  "《",
  "》",
  "【",
  "】",
  "～",
  "+",
  "-",
  "*",
  "/",
  "=",
  "%",
  "(",
  ")",
  "()",
  ":",
  "@",
  "#",
  "&",
  "_",
  "|",
  "<",
  ">",
  "^",
  "￥",
  "★",
  "☆",
  "♡",
  "♥",
  "→",
  "←",
  "↑",
  "↓",
  ":-)",
];

/** 两张表的导航行标题和编辑页标题，三端逐字相同（Android `KeyLayoutStylesPolicy.NINE_KEY_SIDEBAR_TITLE` / `DIGIT_SIDEBAR_TITLE`）。 */
export const nineKeySidebarTitle = "九键左侧符号";
export const numberPadSidebarTitle = "数字键盘左侧符号";
/** 编辑页顶部的一行说明，三端逐字相同。 */
export const nineKeySidebarNote = "拼音九键、九键双拼和笔画共用，多于一屏时上下滑动。";
export const numberPadSidebarNote = "所有九宫格数字键盘共用，#+= 固定在最后。";

/** 鸿蒙的出厂表，写成键盘和编辑器里显示的样子：字母层 ，。？（键盘把 ASCII 的 , . ? 交给 Engine，按当前标点模式出字），数字层四则运算。编辑器「等于出厂表就写空数组」按它逐项比较。 */
export const nineKeySidebarDefaults: Readonly<Record<NineKeySidebarKind, readonly string[]>> = {
  letter: ["，", "。", "？"],
  digit: ["+", "-", "*", "/"],
};

/** 两张表各存在哪个共享偏好里。 */
export const nineKeySidebarPreference: Readonly<
  Record<
    NineKeySidebarKind,
    "touch_nine_key_sidebar_symbols" | "touch_nine_key_digit_sidebar_symbols"
  >
> = {
  letter: "touch_nine_key_sidebar_symbols",
  digit: "touch_nine_key_digit_sidebar_symbols",
};

/** 共享偏好名是哪张表的；不是这两个名字时为 undefined。键盘长按符号栏打开「按键布局」时带的就是偏好名（鸿蒙链接的 `target`，Android 的 `edit_sidebar_key`），设置页按它决定打开哪张表的编辑页。 */
export function nineKeySidebarKindForPreference(name: string): NineKeySidebarKind | undefined {
  return (Object.keys(nineKeySidebarPreference) as NineKeySidebarKind[]).find(
    (kind) => nineKeySidebarPreference[kind] === name,
  );
}

/** 设置页 9 键预览的左列一屏画几项，与鸿蒙键盘 `NineKeyLayout.LETTER_SIDEBAR_VISIBLE_ROWS` 相同。 */
export const nineKeySidebarVisibleRows = 3;

/** 偏好里存着的表，只留非空字符串（client-core 读写时已经规范化）；缺键、不是数组读成空表，即出厂表。与出厂表逐项、同顺序相同的非空表也读成空表（不描边、摘要照出厂表），与 Android `NineKeySidebarPolicy.custom` 相同。 */
export function storedNineKeySidebar(preferences: Preferences, kind: NineKeySidebarKind): string[] {
  const value: unknown = preferences[nineKeySidebarPreference[kind]];
  if (!Array.isArray(value)) return [];
  const symbols = value.filter(
    (item): item is string => typeof item === "string" && item.length > 0,
  );
  return nineKeySidebarStoredValue(kind, symbols);
}

/** 字母层左列实际画的符号：用户的表，空表时是鸿蒙出厂表。 */
export function nineKeySidebarSymbols(preferences: Preferences): readonly string[] {
  const stored = storedNineKeySidebar(preferences, "letter");
  return stored.length > 0 ? stored : nineKeySidebarDefaults.letter;
}

/** 九宫格数字键盘左列 #+= 上面实际画的符号：用户的表，空表时是鸿蒙出厂表。 */
export function numberPadSidebarSymbols(preferences: Preferences): readonly string[] {
  const stored = storedNineKeySidebar(preferences, "digit");
  return stored.length > 0 ? stored : nineKeySidebarDefaults.digit;
}

/** 导航行右侧的摘要，照 Android `NineKeySidebarPolicy.summary(symbols, 4)`：不超过 4 项时用空格连起来，多于 4 项时前 4 项加「 等 N 个」。 */
export function nineKeySidebarSummary(symbols: readonly string[]): string {
  const limit = 4;
  if (symbols.length <= limit) return symbols.join(" ");
  return `${symbols.slice(0, limit).join(" ")} 等 ${symbols.length} 个`;
}

// 空白按 Unicode White_Space、控制字符按 Cc 认，与 Rust 的 `char::is_whitespace`、`char::is_control` 相同（JS 的 `\s` 多认一个 U+FEFF）。
const sidebarForbidden = /[\p{White_Space}\p{Cc}]/u;

/** 一项本身合不合规：非空，不含空白（含全角空格、NBSP）和控制字符，最多 `nineKeySidebarMaxSymbolCodePoints` 个码位（不是字素：`👍🏽` 两个码位可以，`👨‍👩‍👧` 五个码位不行）。 */
export function validNineKeySidebarSymbol(symbol: string): boolean {
  return (
    symbol.length > 0 &&
    !sidebarForbidden.test(symbol) &&
    [...symbol].length <= nineKeySidebarMaxSymbolCodePoints
  );
}

/** 整张表能不能确认，照 Android `NineKeySidebarPolicy.parse`：不空、不超过 30 项、每项合规、没有重复，用一个空格连起来不超过 200 个 UTF-16 单元。 */
export function validNineKeySidebar(symbols: readonly string[]): boolean {
  return (
    symbols.length > 0 &&
    symbols.length <= nineKeySidebarMaxSymbols &&
    symbols.every(validNineKeySidebarSymbol) &&
    new Set(symbols).size === symbols.length &&
    symbols.join(" ").length <= nineKeySidebarMaxTextUtf16
  );
}

/** 这一项还能不能接到表末尾：本身合规、不重复，加上以后整张表仍合规。重复项在加入时就挡掉。 */
export function canAddNineKeySidebarSymbol(symbols: readonly string[], symbol: string): boolean {
  return !symbols.includes(symbol) && validNineKeySidebar([...symbols, symbol]);
}

/** 编辑页「当前顺序」每行符号前的序号：第 `position` 位（从 0 起）写成「1.」「2.」…，与 Android `NineKeySidebarPolicy.ordinal`、iOS `NineKeySidebar.ordinal` 同一写法。 */
export function nineKeySidebarOrdinal(position: number): string {
  return `${position + 1}.`;
}

/** 确认时写进偏好的值：与这张表的出厂表逐项、同顺序相同时写空数组（「恢复默认」也是它），否则写用户的表。 */
export function nineKeySidebarStoredValue(
  kind: NineKeySidebarKind,
  symbols: readonly string[],
): string[] {
  const defaults = nineKeySidebarDefaults[kind];
  const same =
    symbols.length === defaults.length &&
    symbols.every((symbol, index) => symbol === defaults[index]);
  return same ? [] : [...symbols];
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

/** 共享偏好 `touch_language_key_side` 开关的标题和说明，三端逐字相同（Android `TouchKeyStyle.LANGUAGE_LEFT_TITLE`、iOS `TouchKeyLayoutOffering.languageLeftTitle`）。 */
export const languageKeyLeftTitle = "中/英键放在空格左边";
export const languageKeyLeftDescription = "中/英 排到空格左边，逗号、句号仍紧挨空格。";

/** 「中/英键放在空格左边」是否打开：只有值恰好是 `left` 时算左，缺键、`right` 和不认识的值都按右，与键盘的 `TouchLayoutStylePolicy.languageKeyLeft` 相同。 */
export function languageKeyLeft(preferences: Preferences): boolean {
  return preferences.touch_language_key_side === "left";
}

/** 中/英 挪到左边时跟着整串右移一格的键：空格、0 和紧贴空格的逗号、句号。 */
const languageRunCodes: readonly string[] = ["Space", "Digit0", "Comma", "Period"];

/** 「中/英键放在空格左边」打开时把 中/英 左边紧挨着的那一串空格、0、逗号和句号整串右移一格，中/英 挪到这一串的最左边，份额不变，只换次序，与键盘的 `TouchLayoutStylePolicy.languageBeforeSpace`、`NineKeyLayout.languageBeforeSpace` 相同；在样式和「符」与「123」互换之后套。行里没有 中/英，或它左边紧挨的不是这几种键时原样返回。 */
export function withLanguageKeyLeft<T extends { code?: string }>(
  row: readonly T[],
  enabled: boolean,
): T[] {
  const moved = [...row];
  const language = moved.findIndex((item) => item.code === "SoftLanguage");
  if (!enabled || language < 0) return moved;
  let start = language;
  while (start > 0 && languageRunCodes.includes(moved[start - 1].code ?? "")) start--;
  if (start === language) return moved;
  const [languageKey] = moved.splice(language, 1);
  moved.splice(start, 0, languageKey);
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

/** 统计热力图要照着画的那几项：两种键盘的底行样式、数字行、「符」与「123」互换和「中/英键放在空格左边」（后两项缺省为关）。 */
export type TouchKeyLayoutStyle = {
  twentySixKeyStyle: TwentySixKeyStyle;
  nineKeyStyle: NineKeyStyle;
  numberRow: boolean;
  twentySixSymbolFirst?: boolean;
  languageKeyLeft?: boolean;
};

export function touchKeyLayoutStyle(preferences: Preferences): TouchKeyLayoutStyle {
  return {
    twentySixKeyStyle: twentySixKeyStyle(preferences),
    nineKeyStyle: nineKeyStyle(preferences),
    numberRow: preferences.touch_number_row === true,
    twentySixSymbolFirst: preferences.touch_twenty_six_symbol_first === true,
    languageKeyLeft: languageKeyLeft(preferences),
  };
}
