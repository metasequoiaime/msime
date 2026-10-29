import { NumberSymbolLayout } from "./NumberSymbolLayout";

/**
 * Display labels and Engine inputs for the quanpin nine-key grid, ported from
 * platforms/android/java/app/msime/android/NineKeyLayout.java.
 *
 * The grid is data, not behaviour: which character each key sends is the Engine's contract, and the
 * labels are what the Apple hosts print for the same keys.
 */
export interface NineKey {
  readonly label: string;
  /** The ASCII character handed to the Engine, not the label. */
  readonly input: string;
  readonly description: string;
}

function key(label: string, input: string, description: string): NineKey {
  return { label: label, input: input, description: description };
}

const ROWS: NineKey[][] = [
  [key("分词", "'", "拼音分词"), key("ABC", "2", "2 ABC"), key("DEF", "3", "3 DEF")],
  [key("GHI", "4", "4 GHI"), key("JKL", "5", "5 JKL"), key("MNO", "6", "6 MNO")],
  [key("PQRS", "7", "7 PQRS"), key("TUV", "8", "8 TUV"), key("WXYZ", "9", "9 WXYZ")],
];

// 触屏键盘的网格，与设计稿和 Android 一致：第一格是 @#，点击把当前模式的常用标点放进候选条而不发送任何内容（符号面板从底行最左边的 符 打开），被它取代的音节分隔符移到网格右侧那一列（`separator`）。2in1 屏幕键盘仍使用 `ROWS`。
const QUICK_MARKS_KEY: NineKey = key("@#", "@", "常用标点");
const TOUCH_ROWS: NineKey[][] = [[QUICK_MARKS_KEY, ROWS[0][1], ROWS[0][2]], ROWS[1], ROWS[2]];

// The same three columns carrying digits instead of letter groups. A typist who chose a grid chose
// three columns, so the digit layer re-labels the grid rather than handing over to the twenty-six
// key face's ten-across symbol rows. The first cell sends 1 rather than the apostrophe it sends
// while spelling: on this layer it is a digit key like the other eight.
const DIGIT_ROWS: NineKey[][] = [
  [key("1", "1", "1"), key("2", "2", "2"), key("3", "3", "3")],
  [key("4", "4", "4"), key("5", "5", "5"), key("6", "6", "6")],
  [key("7", "7", "7"), key("8", "8", "8"), key("9", "9", "9")],
];

// ASCII, as the twenty-six key face sends: the Engine decides whether a comma arrives as , or as ，,
// and it is the only thing that knows, since the answer depends on the composing language and on the
// punctuation lock. iOS hard-codes the Chinese forms here because its grid only ever spells Chinese.
const PUNCTUATION: string[] = [",", ".", "?", "!"];

// 触屏网格把这四个标点拆开：, . ? 放在左列，网格三行旁各一个；! 放在右列底部、删除键和分隔符下方，和 Android 的侧栏一样。九键样式「右侧大回车」的右列下两行是回车，! 接在左列末尾（`TALL_RETURN_SIDE_MARKS`）。
const SIDE_MARKS: string[] = [",", ".", "?"];
const CLOSING_MARK: string = "!";
const TALL_RETURN_SIDE_MARKS: string[] = [",", ".", "?", "!"];

// 数字页就地提供四则运算，标点和更多符号保留在扩展页。
const DIGIT_SIDEBAR: string[] = ["+", "-", "*", "/"];

/** 触屏九键外框（拼音九键和笔画键盘）底行上的一个键。 */
export enum NineKeyActionKind {
  SYMBOL_PANEL = "symbol_panel",
  LAYER_TOGGLE = "layer_toggle",
  /** 英文九键字母层空格左边的句点；九键样式「底栏带标点」时在空格右边。 */
  PERIOD = "period",
  SPACE = "space",
  /** 英文九键字母层空格右边的逗号；九键样式「底栏带标点」时在空格左边。 */
  COMMA = "comma",
  /** 九键样式「表情键」时紧挨 123 右边、打开表情面板的 😀。 */
  EMOJI = "emoji",
  ZERO = "zero",
  LANGUAGE = "language",
  RETURN = "return",
}

export interface NineKeyAction {
  readonly kind: NineKeyActionKind;
  /** 在底行里占的宽度份额。 */
  readonly weight: number;
}

/** 九键外框底行上画出来的一个键（`NineKeyLayout.actionItems`）：`KeyboardView.nineKeyActionRow` 用 ForEach 逐个画，画这个键时按值传下去的量都在这里，ForEach 的键（`actionItemKey`）也由它们组成。 */
export interface NineKeyActionItem {
  readonly kind: NineKeyActionKind;
  /** `layoutWeight`；定了固定宽度（`width` 大于 0）的键为 0。 */
  readonly share: number;
  /** 固定宽度，单位与 `tallReturnRow` 的 `faceWidth` 相同；为 0 时按 `share` 排。 */
  readonly width: number;
  /** 中/英 紧挨在回车左边，两键的触摸分界挪进回车键帽；只有中/英 和回车两个键位看它，其余键位恒为 false。 */
  readonly besideReturn: boolean;
}

/** 自定义左列（`NineKeyLayout.sidebarItems`）画出来的一项：符号、这一项的高度和它在 ForEach 里的键，按值传给键的 @Builder 的量都在这里。 */
export interface NineKeySidebarItem {
  readonly symbol: string;
  readonly height: number;
  readonly key: string;
}

function action(kind: NineKeyActionKind, weight: number): NineKeyAction {
  return { kind: kind, weight: weight };
}

/** 中/英 挪到空格左边时跟着整串右移一格的键（`NineKeyLayout.languageBeforeSpace`）。 */
const LANGUAGE_RUN: readonly NineKeyActionKind[] = [
  NineKeyActionKind.SPACE,
  NineKeyActionKind.ZERO,
  NineKeyActionKind.COMMA,
  NineKeyActionKind.PERIOD,
];

export class NineKeyLayout {
  /** 只换数字页显示，不修改中文方案；独立语言、本地工具和英文保留原符号契约。 */
  static opensNumberPad(
    symbols: boolean,
    more: boolean,
    english: boolean,
    localMode: string,
    independentLanguage: boolean,
  ): boolean {
    return symbols && !more && !english && localMode === "none" && !independentLanguage;
  }

  /**
   * 从其他中文键面借用数字页时，底行返回键的键面，与 Android `KeyboardActionRow.layerTitle(layout, true, true)` 和 iOS `numberPadReturnTitle` 一致：14 键和笔画印各自的短名，26 键一族和手写印中文字母页的「拼音」（`NumberSymbolLayout.lettersKeyTitle`）。原拼音九键不走这里，它的数字层仍印「九键」。
   *
   * 两个参数与 `KeyboardView.typingFace` 的分支同序：笔画键盘先于 14 键判断。
   *
   * @param stroke 借出数字页的是笔画键盘
   * @param fourteenKey 借出数字页的是 14 键键面
   */
  static numberPadReturnTitle(stroke: boolean, fourteenKey: boolean): string {
    if (stroke) {
      return "笔";
    }
    if (fourteenKey) {
      return "14键";
    }
    return NumberSymbolLayout.lettersKeyTitle(true);
  }

  /** 触屏网格两侧的列（左侧标点栏、右侧功能列）和中间 3×3 网格占键盘宽度的份额：每列字母格 2 份，两侧各 1.15 份。「右侧大回车」的三列按同样的份额排，它的底行与这三列对齐。 */
  static readonly SIDE_SHARE: number = 1.15;
  static readonly GRID_SHARE: number = 6;

  static readonly SYMBOL_PANEL_WEIGHT: number = 1.05;
  static readonly LAYER_WEIGHT: number = 1.25;
  static readonly SPACE_WEIGHT: number = 4.2;
  static readonly ZERO_WEIGHT: number = 1.05;
  static readonly LANGUAGE_WEIGHT: number = 1.05;
  static readonly RETURN_WEIGHT: number = 1.6;
  /** 英文九键空格两侧的句点和逗号，与英文 26 键底行的句点、逗号同一份额。 */
  static readonly PUNCTUATION_WEIGHT: number = 1;
  /** 英文九键底行多出的句点和逗号从空格里让出（4.2 减去两份 1），其余键保持中文九键的宽度。 */
  static readonly ENGLISH_SPACE_WEIGHT: number = 2.2;
  /** 九键样式「表情键」的 😀，与 符、中/英 同宽，从空格里让出。 */
  static readonly EMOJI_WEIGHT: number = 1.05;

  /**
   * 九键样式「右侧大回车」（`touch_nine_key_style` 为 `tall_return`）是否作用于这块键面：中文九键（拼音九键、九键双拼、笔画）的字母层和它们的数字层都画这个外框——左列是跨网格三行的标点栏，栏下方是与它同宽的 符；中间网格下方的底行恰好与网格同宽（`actionRow`）；右列自上而下是 ⌫、第二格，回车跨第三行和底行（笔画网格只有两行，⌫ 占第一行，回车跨第二行和底行，见 `tallReturnGridRowHeight`）。英文九键与其他样式一样不受影响。
   *
   * @param english 英文模式下的九键
   * @param style 九键样式（`TouchLayoutStylePolicy.fieldRowStyle` 之后的值）
   */
  static tallReturn(english: boolean, style: string): boolean {
    return !english && style === "tall_return";
  }

  /** 「右侧大回车」数字层的空格按标准九键底行的份额算的宽度：与 中/英 相同，0 占剩下的部分。 */
  static readonly TALL_RETURN_DIGIT_SPACE_WEIGHT: number = 1.05;

  /**
   * 「右侧大回车」网格下方那一段底行里每个键的宽度，与 Android、iOS 相同：123（数字层的切换键）和 中/英 保持标准九键底行里的绝对宽度，字母层空格占网格宽度剩下的部分；数字层是 切换键 | 0 | 空格 | 中/英，空格是标准底行的 1.05 份（`TALL_RETURN_DIGIT_SPACE_WEIGHT`），0 占剩下的部分（`tallReturnFill`）。鸿蒙没有地球键。
   *
   * 「绝对宽度」按同一块键面上的标准底行算：标准底行五个键、四道键距铺满整块宽度（`actionRow(digitLayer)`，两层都是 9.15 份），外框三列、两道键距，中列按 `GRID_SHARE` 分到的宽度里再有 `length - 1` 道键距。返回的 `weight` 是宽度，单位与 `faceWidth`、`keySpacing` 相同；键距为 0、`faceWidth` 取三列份额之和时就是 `actionRow` 里的份额。
   *
   * 「中/英键放在空格左边」时 中/英 排到 123（数字层的切换键）后面：字母层 123 | 中 | 空格，数字层 切换键 | 中 | 0 | 空格（`languageBeforeSpace`），每个键的宽度与右边时相同，只换次序。
   *
   * @param digitLayer 画的是数字层
   * @param faceWidth 整块键面（外框）的宽度
   * @param keySpacing 横向键距
   * @param languageLeft 共享偏好 `touch_language_key_side` 为 `left`（`TouchLayoutStylePolicy.languageKeyLeft`），缺省为右
   */
  static tallReturnRow(
    digitLayer: boolean,
    faceWidth: number,
    keySpacing: number,
    languageLeft: boolean = false,
  ): NineKeyAction[] {
    const standard: NineKeyAction[] = NineKeyLayout.actionRow(digitLayer);
    const standardShare: number = standard.reduce(
      (sum: number, item: NineKeyAction): number => sum + item.weight,
      0,
    );
    const unit: number = (faceWidth - (standard.length - 1) * keySpacing) / standardShare;
    const frameShare: number = NineKeyLayout.SIDE_SHARE * 2 + NineKeyLayout.GRID_SHARE;
    const grid: number = ((faceWidth - 2 * keySpacing) * NineKeyLayout.GRID_SHARE) / frameShare;
    const fixed: NineKeyAction[] = digitLayer
      ? [
          action(NineKeyActionKind.LAYER_TOGGLE, NineKeyLayout.LAYER_WEIGHT * unit),
          action(NineKeyActionKind.SPACE, NineKeyLayout.TALL_RETURN_DIGIT_SPACE_WEIGHT * unit),
          action(NineKeyActionKind.LANGUAGE, NineKeyLayout.LANGUAGE_WEIGHT * unit),
        ]
      : [
          action(NineKeyActionKind.LAYER_TOGGLE, NineKeyLayout.LAYER_WEIGHT * unit),
          action(NineKeyActionKind.LANGUAGE, NineKeyLayout.LANGUAGE_WEIGHT * unit),
        ];
    const taken: number = fixed.reduce(
      (sum: number, item: NineKeyAction): number => sum + item.weight,
      0,
    );
    const fill: NineKeyAction = action(
      NineKeyLayout.tallReturnFill(digitLayer),
      grid - fixed.length * keySpacing - taken,
    );
    return NineKeyLayout.languageBeforeSpace([fixed[0], fill].concat(fixed.slice(1)), languageLeft);
  }

  /**
   * 「中/英键放在空格左边」（共享偏好 `touch_language_key_side` 为 `left`）：把 中/英 左边紧挨着的那一串空格、0、逗号和句号整串右移一格，中/英 挪到这一串的最左边，份额不变，只换次序；与设计底行的 `TouchLayoutStylePolicy.languageBeforeSpace`、Android `KeyboardActionRow.languageBeforeSpace`、iOS `TouchBottomRowLayout.languageBeforeSpace` 同一条规则。标准九键 符 | 123 | 中 | 空格 | 回车；底栏带标点 符 | 123 | 中 | ， | 空格 | 。 | 回车；数字层 符 | 九键 | 中 | 空格 | 0 | 回车。行里没有 中/英，或它左边紧挨的不是这几种键时原样返回。
   */
  static languageBeforeSpace(row: readonly NineKeyAction[], enabled: boolean): NineKeyAction[] {
    const moved: NineKeyAction[] = row.slice();
    const language: number = moved.findIndex(
      (item: NineKeyAction): boolean => item.kind === NineKeyActionKind.LANGUAGE,
    );
    if (!enabled || language < 0) {
      return moved;
    }
    let start: number = language;
    while (start > 0 && LANGUAGE_RUN.includes(moved[start - 1].kind)) {
      start--;
    }
    if (start === language) {
      return moved;
    }
    const key: NineKeyAction[] = moved.splice(language, 1);
    moved.splice(start, 0, key[0]);
    return moved;
  }

  /**
   * 九键外框底行逐个要画的键，次序就是 `row`（`actionRow`）的次序。`fixed` 是「右侧大回车」量到外框宽度后的 `tallReturnRow`，没量到或不是这个样式时为空：除了占剩余宽度的 `fill`（`tallReturnFill`），`fixed` 里有的键取它的固定宽度、份额为 0，其余键按 `row` 的份额排。
   *
   * 底行表和 `tallReturnRow` 在视图里每次重建这一行只算一次，交给 ForEach；宽度、份额和让位按值传给每个键的 @Builder，所以都放进 ForEach 的键（`actionItemKey`）：切层、切中/英、量到外框宽度让哪个键变了，ForEach 就重建那一个键。
   */
  static actionItems(
    row: readonly NineKeyAction[],
    fixed: readonly NineKeyAction[],
    fill: NineKeyActionKind,
  ): NineKeyActionItem[] {
    const beside: boolean = NineKeyLayout.languageBesideReturn(row);
    const items: NineKeyActionItem[] = [];
    for (const item of row) {
      let width: number = 0;
      if (item.kind !== fill) {
        for (const pinned of fixed) {
          if (pinned.kind === item.kind) {
            width = pinned.weight;
          }
        }
      }
      const yields: boolean =
        item.kind === NineKeyActionKind.LANGUAGE || item.kind === NineKeyActionKind.RETURN;
      items.push({
        kind: item.kind,
        share: width > 0 ? 0 : item.weight,
        width: width,
        besideReturn: beside && yields,
      });
    }
    return items;
  }

  /** `actionItems` 每一项在 ForEach 里的键：键位、份额、固定宽度和让位，按值传给键的 @Builder 的量一个不缺。 */
  static actionItemKey(item: NineKeyActionItem): string {
    return `${item.kind}:${item.share}:${item.width}:${item.besideReturn}`;
  }

  /** 「右侧大回车」网格下方那一段里占剩余宽度的键：字母层是空格，数字层是 0。 */
  static tallReturnFill(digitLayer: boolean): NineKeyActionKind {
    return digitLayer ? NineKeyActionKind.ZERO : NineKeyActionKind.SPACE;
  }

  /**
   * 「右侧大回车」网格一行的高度，右列按它与中间网格逐行对齐：回车上方每个网格行一个键（拼音九键、九键双拼和数字层是 ⌫ 和第二格，笔画只有 ⌫），回车占网格最后一行、一道行距和底行。网格各行平分网格高度减去行距，所以要按这块网格自己的行数算：笔画网格只有两行（`StrokeLayout.ROWS`），按三行切会让回车从笔画第二行中间开始。
   *
   * @param gridHeight 网格的高度
   * @param rowSpacing 纵向行距
   * @param gridRows 网格的行数：拼音九键、九键双拼和数字层是 3，笔画是 `StrokeLayout.ROWS.length`
   */
  static tallReturnGridRowHeight(gridHeight: number, rowSpacing: number, gridRows: number): number {
    return (gridHeight - (gridRows - 1) * rowSpacing) / gridRows;
  }

  /**
   * 中/英 是否紧挨在回车左边（同一行）：这时两键的触摸分界挪进回车键帽（`KeyboardGeometry.RETURN_YIELD_VP`）。「右侧大回车」的底行里没有回车，回车在右列，不让出键帽。
   */
  static languageBesideReturn(row: readonly NineKeyAction[]): boolean {
    for (let index = 1; index < row.length; index++) {
      if (row[index].kind === NineKeyActionKind.RETURN) {
        return row[index - 1].kind === NineKeyActionKind.LANGUAGE;
      }
    }
    return false;
  }

  /** 九键底行的逗号长按打开常用标点列表：只有中文九键「底栏带标点」的逗号，与 Android、iOS 九键外框上的逗号相同（九键上逗号没有句号可输入，句号就在空格另一边）；英文九键的逗号不接长按。 */
  static commaOpensQuickPunctuation(digitLayer: boolean, english: boolean, style: string): boolean {
    return !digitLayer && !english && style === "comma_period";
  }

  /**
   * 触屏九键外框的底行，与 iOS 的 `ActionRowStyle.nineKey`、Android 的 `KeyboardActionRow.designEntries` 一致：符 1.05 | 123 1.25 | space 4.2 | 中 1.05 | return 1.6，中/英紧挨回车。标点在网格两侧的列里，所以没有逗号和句号；符 是九键上打开符号面板的键。
   *
   * 数字网格只有 1-9，这里补上 0，排在空格右边。从拼音九键、九键双拼和笔画进的数字层（`nineKeyDigits`）与 Android #9113、iOS 相同，符 和切换键留在字母层那一格、宽度不变：符 1.05 | 九键 1.25 | 空格 3.15 | 0 1.05 | 中/英 1.05 | 回车 1.6，0 的一份从空格里让出（#10061）。从 26 键、14 键、18 键、手写借来的数字九宫格底行是 返回 1.25 | 空格 4.2 | 0 1.05 | 中/英 | 回车，返回键本来就在它们 123 那一格，0 占 符 让出的那一份。
   *
   * 英文九键的字母层在空格左边放句点、右边放逗号（符 | 123 | . | space | , | 中 | return），与三端英文 26 键底行的句点、逗号位置一致；两键各占 1 份，从空格里让出，左列的 , . ? 不变。
   *
   * 中文九键字母层按共享偏好 `touch_nine_key_style` 排：`comma_period` 是 符 | 123 | ， | 空格 | 。 | 中 | 回车，逗号句号各占从空格让出的 1 份；`emoji` 是 符 | 123 | 😀 | 空格 | 中 | 回车，😀 占从空格让出的 1.05 份。数字层和英文九键不受这两种样式影响，左侧标点栏和右列不变。
   *
   * `tall_return`（`tallReturn`）的底行只是网格下方那一段，加起来正好是网格的份额 `GRID_SHARE`，各键的份额见 `tallReturnRow`（这里按没有键距算）：字母层 123 | 空格 | 中，数字层 切换键 | 0 | 空格 | 中。符 和回车不在这一段里，由外框画在左列和右列。
   *
   * @param digitLayer 画的是数字层
   * @param english 英文模式下的九键（数字层不受影响）
   * @param style 九键样式（`TouchLayoutStylePolicy.nineKeyStyle` 读出的值），缺省为 `standard`
   * @param nineKeyDigits 数字层是从拼音九键、九键双拼或笔画进的（字母层底行以 符 打头），只在 `digitLayer` 时有意义；缺省为借来的数字九宫格
   * @param space 数字层空格的位置（`TouchLayoutStylePolicy.numberPadSpace` 读出的值，共享偏好 `touch_number_pad_space`），只在 `digitLayer` 且不是右侧大回车时有意义：`before_zero`（缺省，鸿蒙原来的排法）空格在 0 左边占大份、0 与中/英同宽；`after_zero` 两键对调，0 占大份、空格与中/英同宽；`right_column` 底行没有空格，0 占两键合起来的份额，空格画在网格右列最下面一格（`KeyboardView.touchNineKeyGrid`）
   * @param languageLeft 共享偏好 `touch_language_key_side` 为 `left`（`TouchLayoutStylePolicy.languageKeyLeft`）：上面每一种排法排完之后再把 中/英 挪到空格左边（`languageBeforeSpace`），字母层、数字层和右侧大回车都算；缺省为右，就是上面的排法
   */
  static actionRow(
    digitLayer: boolean,
    english: boolean = false,
    style: string = "standard",
    nineKeyDigits: boolean = false,
    space: string = "before_zero",
    languageLeft: boolean = false,
  ): NineKeyAction[] {
    return NineKeyLayout.languageBeforeSpace(
      NineKeyLayout.styledActionRow(digitLayer, english, style, nineKeyDigits, space),
      languageLeft,
    );
  }

  /** `actionRow` 在 中/英 换边之前的排法，参数同 `actionRow`。 */
  private static styledActionRow(
    digitLayer: boolean,
    english: boolean,
    style: string,
    nineKeyDigits: boolean,
    space: string,
  ): NineKeyAction[] {
    const row: NineKeyAction[] = [];
    const englishMarks: boolean = english && !digitLayer;
    const styled: boolean = !english && !digitLayer;
    if (NineKeyLayout.tallReturn(english, style)) {
      // 份额是没有键距时的宽度：整块外框宽 SIDE_SHARE * 2 + GRID_SHARE，键距为 0。
      return NineKeyLayout.tallReturnRow(
        digitLayer,
        NineKeyLayout.SIDE_SHARE * 2 + NineKeyLayout.GRID_SHARE,
        0,
      );
    }
    if (styled && style === "comma_period") {
      row.push(action(NineKeyActionKind.SYMBOL_PANEL, NineKeyLayout.SYMBOL_PANEL_WEIGHT));
      row.push(action(NineKeyActionKind.LAYER_TOGGLE, NineKeyLayout.LAYER_WEIGHT));
      row.push(action(NineKeyActionKind.COMMA, NineKeyLayout.PUNCTUATION_WEIGHT));
      row.push(action(NineKeyActionKind.SPACE, NineKeyLayout.ENGLISH_SPACE_WEIGHT));
      row.push(action(NineKeyActionKind.PERIOD, NineKeyLayout.PUNCTUATION_WEIGHT));
      row.push(action(NineKeyActionKind.LANGUAGE, NineKeyLayout.LANGUAGE_WEIGHT));
      row.push(action(NineKeyActionKind.RETURN, NineKeyLayout.RETURN_WEIGHT));
      return row;
    }
    if (styled && style === "emoji") {
      row.push(action(NineKeyActionKind.SYMBOL_PANEL, NineKeyLayout.SYMBOL_PANEL_WEIGHT));
      row.push(action(NineKeyActionKind.LAYER_TOGGLE, NineKeyLayout.LAYER_WEIGHT));
      row.push(action(NineKeyActionKind.EMOJI, NineKeyLayout.EMOJI_WEIGHT));
      row.push(
        action(NineKeyActionKind.SPACE, NineKeyLayout.SPACE_WEIGHT - NineKeyLayout.EMOJI_WEIGHT),
      );
      row.push(action(NineKeyActionKind.LANGUAGE, NineKeyLayout.LANGUAGE_WEIGHT));
      row.push(action(NineKeyActionKind.RETURN, NineKeyLayout.RETURN_WEIGHT));
      return row;
    }
    // 符 在字母层和九键一族的数字层都排第一，切层时与切换键一起不挪位置。
    const symbolPanel: boolean = !digitLayer || nineKeyDigits;
    if (symbolPanel) {
      row.push(action(NineKeyActionKind.SYMBOL_PANEL, NineKeyLayout.SYMBOL_PANEL_WEIGHT));
    }
    row.push(action(NineKeyActionKind.LAYER_TOGGLE, NineKeyLayout.LAYER_WEIGHT));
    if (englishMarks) {
      row.push(action(NineKeyActionKind.PERIOD, NineKeyLayout.PUNCTUATION_WEIGHT));
    }
    let spaceWeight: number = englishMarks
      ? NineKeyLayout.ENGLISH_SPACE_WEIGHT
      : NineKeyLayout.SPACE_WEIGHT;
    if (digitLayer && symbolPanel) {
      // 九键一族的数字层：0 的一份从空格里让出，整行份额与字母层相同。
      spaceWeight -= NineKeyLayout.ZERO_WEIGHT;
    }
    if (digitLayer && space === "right_column") {
      // 空格在右列：0 占空格和 0 合起来的份额。
      row.push(action(NineKeyActionKind.ZERO, spaceWeight + NineKeyLayout.ZERO_WEIGHT));
    } else if (digitLayer && space === "after_zero") {
      // 两键对调：0 占大份，空格与中/英同宽。
      row.push(action(NineKeyActionKind.ZERO, spaceWeight));
      row.push(action(NineKeyActionKind.SPACE, NineKeyLayout.ZERO_WEIGHT));
    } else {
      row.push(action(NineKeyActionKind.SPACE, spaceWeight));
      if (englishMarks) {
        row.push(action(NineKeyActionKind.COMMA, NineKeyLayout.PUNCTUATION_WEIGHT));
      }
      if (digitLayer) {
        row.push(action(NineKeyActionKind.ZERO, NineKeyLayout.ZERO_WEIGHT));
      }
    }
    row.push(action(NineKeyActionKind.LANGUAGE, NineKeyLayout.LANGUAGE_WEIGHT));
    row.push(action(NineKeyActionKind.RETURN, NineKeyLayout.RETURN_WEIGHT));
    return row;
  }

  static rows(): NineKey[][] {
    return ROWS;
  }

  /** 网格的数字层，按电话拨号盘排列，1 2 3 在上。单独叫 `digits` 而不是给 `rows` 加一个布尔参数，调用处读起来就是一个键面。 */
  static digits(): NineKey[][] {
    return DIGIT_ROWS;
  }

  static punctuation(): string[] {
    return PUNCTUATION;
  }

  /** 触屏键盘的网格；见 `TOUCH_ROWS`。 */
  static touchRows(): NineKey[][] {
    return TOUCH_ROWS;
  }

  /** 音节分隔符，触屏网格把它画在右侧那一列，而不是第一格。 */
  static separator(): NineKey {
    return ROWS[0][0];
  }

  /** 点击字母层的这一格是否把常用标点放进候选条而不是交给 Engine：即触屏网格的 @#。 */
  static showsQuickMarks(key: NineKey): boolean {
    return key.input === QUICK_MARKS_KEY.input;
  }

  /**
   * 触屏网格左列的标点；「右侧大回车」时末尾多一个右列让出来的 !。
   *
   * `custom` 是共享偏好 `touch_nine_key_sidebar_symbols`（#10223）：空表用出厂的 ASCII `, . ?`，点按交给 Engine 的标点处理；用户的表原样返回，点按按字面上屏（`customSidebar`），右侧大回车也不再接 !。
   */
  static sideMarks(tallReturn: boolean = false, custom: readonly string[] = []): string[] {
    if (NineKeyLayout.customSidebar(custom)) {
      return custom.slice();
    }
    return tallReturn ? TALL_RETURN_SIDE_MARKS : SIDE_MARKS;
  }

  /**
   * 九宫格数字键盘左列 #+= 上面的符号：`custom` 是共享偏好 `touch_nine_key_digit_sidebar_symbols`，空表用出厂的四则运算 `DIGIT_SIDEBAR`（与 `digitPunctuation` 相同），用户的表原样返回，点按按字面上屏。#+= 不在表里，固定在左列最后一格。
   */
  static digitSidebar(custom: readonly string[] = []): string[] {
    return NineKeyLayout.customSidebar(custom) ? custom.slice() : DIGIT_SIDEBAR;
  }

  /** 这张左列符号表是用户自定义的（非空）：左列改画可滚动的等高栈，点按先按首选结束组字再字面上屏；空表是本端出厂表，照原来的画法和输入路径。 */
  static customSidebar(custom: readonly string[]): boolean {
    return custom.length > 0;
  }

  /** 左列符号每项至少多高（vp），与 Android `NineKeySidebarPolicy.MIN_ROW_HEIGHT_DP` 相同：键盘调到最矮时一屏少放几项，不把键压扁。 */
  static readonly SIDEBAR_MIN_ROW_VP: number = 28;
  /** 字母层自定义表一屏最多几项：与九宫格三行对齐。 */
  static readonly LETTER_SIDEBAR_VISIBLE_ROWS: number = 3;
  /** 数字层自定义表一屏最多几项：#+= 下面那一格之外的滚动区有四格高。 */
  static readonly DIGIT_SIDEBAR_VISIBLE_ROWS: number = 4;

  /**
   * 左列一屏画几项：不超过本端上限 `limit`，也不让每项矮于 `SIDEBAR_MIN_ROW_VP`；项数更少时按实际项数均分。至少一项。
   *
   * @param count 表里的项数
   * @param limit 本端一屏最多几项（`LETTER_SIDEBAR_VISIBLE_ROWS` 或 `DIGIT_SIDEBAR_VISIBLE_ROWS`）
   * @param height 左列（数字层是 #+= 上面的滚动区）的可见高度
   */
  static sidebarVisibleRows(count: number, limit: number, height: number): number {
    const fits: number = Math.floor(height / NineKeyLayout.SIDEBAR_MIN_ROW_VP);
    return Math.max(1, Math.min(count, limit, fits));
  }

  /**
   * 自定义左列里每一项的高度：可见高度减去项与项之间的行距，再按 `sidebarVisibleRows` 均分。多于一屏的项接在后面，上下滚动。
   *
   * @param height 左列（数字层是滚动区）的可见高度
   * @param count 表里的项数
   * @param limit 本端一屏最多几项
   * @param spacing 项与项之间的行距，与出厂表各格之间的行距相同
   */
  static sidebarRowHeight(height: number, count: number, limit: number, spacing: number): number {
    const rows: number = NineKeyLayout.sidebarVisibleRows(count, limit, height);
    return Math.max(0, (height - (rows - 1) * spacing) / rows);
  }

  /**
   * 数字层自定义表时左列的切法，与出厂表五格等高的画法对齐：#+= 仍是最后一格、与出厂时同高，上面四格加三道行距是滚动区。返回滚动区的高度。
   *
   * @param height 整个左列的高度
   * @param spacing 格与格之间的行距
   */
  static digitSidebarScrollHeight(height: number, spacing: number): number {
    const cell: number = NineKeyLayout.digitSidebarCellHeight(height, spacing);
    return (
      NineKeyLayout.DIGIT_SIDEBAR_VISIBLE_ROWS * cell +
      (NineKeyLayout.DIGIT_SIDEBAR_VISIBLE_ROWS - 1) * spacing
    );
  }

  /** 出厂数字层左列一格的高度：四个运算符加 #+= 五格、四道行距均分整列，自定义表时 #+= 也是这么高。 */
  static digitSidebarCellHeight(height: number, spacing: number): number {
    const cells: number = NineKeyLayout.DIGIT_SIDEBAR_VISIBLE_ROWS + 1;
    return Math.max(0, (height - (cells - 1) * spacing) / cells);
  }

  /**
   * 自定义左列逐项要画的东西：每项等高（`sidebarRowHeight`），键带上符号、下标、高度和 `face`（`sidebarItemKey`）。视图每次重建左列只算一次，交给 ForEach。
   *
   * @param symbols 用户的表
   * @param height 左列（数字层是滚动区）的可见高度
   * @param limit 本端一屏最多几项
   * @param spacing 项与项之间的行距；笔画的标点栏格与格之间没有空隙，传 0
   * @param face 视图的 `faceKey`
   */
  static sidebarItems(
    symbols: readonly string[],
    height: number,
    limit: number,
    spacing: number,
    face: string,
  ): NineKeySidebarItem[] {
    const rowHeight: number = NineKeyLayout.sidebarRowHeight(
      height,
      symbols.length,
      limit,
      spacing,
    );
    return symbols.map((symbol: string, index: number): NineKeySidebarItem => ({
      symbol: symbol,
      height: rowHeight,
      key: NineKeyLayout.sidebarItemKey(symbol, index, rowHeight, face),
    }));
  }

  /** 自定义左列每一项在 ForEach 里的键：符号、下标、高度和键面的 `faceKey`。只用符号会把重复的项并掉，按值传给 @Builder 的高度变了也不会刷新。 */
  static sidebarItemKey(symbol: string, index: number, height: number, face: string): string {
    return `${symbol}:${index}:${height}:${face}`;
  }

  /** 触屏网格右列底部的标点。 */
  static closingMark(): string {
    return CLOSING_MARK;
  }

  /** 字母层印出长按可输入的数字；常用标点键也保留 1，分词和数字层不重复印。 */
  static digitHint(key: NineKey): string {
    if (NineKeyLayout.showsQuickMarks(key)) return "1";
    return NineKeyLayout.holdOptions(key).length > 0 ? key.input : "";
  }

  static digitPunctuation(): string[] {
    return DIGIT_SIDEBAR;
  }

  /** 笔画键盘左侧标点栏：空表是出厂的 `, . ? !` 四格（`punctuation`），用户的字母层表（`touch_nine_key_sidebar_symbols`）原样返回，与拼音九键共用一张表。 */
  static strokeSidebar(custom: readonly string[] = []): string[] {
    return NineKeyLayout.customSidebar(custom) ? custom.slice() : PUNCTUATION;
  }

  /** 长按选项：常用标点键提供 1，字母键提供数字和各个小写字母。 */
  static holdOptions(key: NineKey): string[] {
    if (NineKeyLayout.showsQuickMarks(key)) return ["1"];
    if (key.input < "2" || key.input > "9" || !/^[A-Z]{3,4}$/.test(key.label)) {
      return [];
    }
    return [key.input].concat(key.label.toLowerCase().split(""));
  }
}
