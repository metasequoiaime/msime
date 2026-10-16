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

// 触屏网格把这四个标点拆开：, . ? 放在左列，网格三行旁各一个；! 放在右列底部、删除键和分隔符下方，和 Android 的侧栏一样。
const SIDE_MARKS: string[] = [",", ".", "?"];
const CLOSING_MARK: string = "!";

// 数字页就地提供四则运算，标点和更多符号保留在扩展页。
const DIGIT_SIDEBAR: string[] = ["+", "-", "*", "/"];

/** 触屏九键外框（拼音九键和笔画键盘）底行上的一个键。 */
export enum NineKeyActionKind {
  SYMBOL_PANEL = "symbol_panel",
  LAYER_TOGGLE = "layer_toggle",
  SPACE = "space",
  ZERO = "zero",
  LANGUAGE = "language",
  RETURN = "return",
}

export interface NineKeyAction {
  readonly kind: NineKeyActionKind;
  /** 在底行里占的宽度份额。 */
  readonly weight: number;
}

function action(kind: NineKeyActionKind, weight: number): NineKeyAction {
  return { kind: kind, weight: weight };
}

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

  static readonly SYMBOL_PANEL_WEIGHT: number = 1.05;
  static readonly LAYER_WEIGHT: number = 1.25;
  static readonly SPACE_WEIGHT: number = 4.2;
  static readonly ZERO_WEIGHT: number = 1.05;
  static readonly LANGUAGE_WEIGHT: number = 1.05;
  static readonly RETURN_WEIGHT: number = 1.6;

  /**
   * 触屏九键外框的底行，与 iOS 的 `ActionRowStyle.nineKey`、Android 的 `KeyboardActionRow.designEntries` 一致：符 1.05 | 123 1.25 | space 4.2 | 中 1.05 | return 1.6，中/英紧挨回车。标点在网格两侧的列里，所以没有逗号和句号；符 是九键上打开符号面板的键。
   *
   * 数字网格只有 1-9，这里补上 0：数字层去掉 符，0 排在空格右边，占 符 让出的同一份宽度。常规中文数字入口借用同一套网格和底行。
   */
  static actionRow(digitLayer: boolean): NineKeyAction[] {
    const row: NineKeyAction[] = [];
    if (!digitLayer) {
      row.push(action(NineKeyActionKind.SYMBOL_PANEL, NineKeyLayout.SYMBOL_PANEL_WEIGHT));
    }
    row.push(action(NineKeyActionKind.LAYER_TOGGLE, NineKeyLayout.LAYER_WEIGHT));
    row.push(action(NineKeyActionKind.SPACE, NineKeyLayout.SPACE_WEIGHT));
    if (digitLayer) {
      row.push(action(NineKeyActionKind.ZERO, NineKeyLayout.ZERO_WEIGHT));
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

  /** 触屏网格左列的标点。 */
  static sideMarks(): string[] {
    return SIDE_MARKS;
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

  /** 长按选项：常用标点键提供 1，字母键提供数字和各个小写字母。 */
  static holdOptions(key: NineKey): string[] {
    if (NineKeyLayout.showsQuickMarks(key)) return ["1"];
    if (key.input < "2" || key.input > "9" || !/^[A-Z]{3,4}$/.test(key.label)) {
      return [];
    }
    return [key.input].concat(key.label.toLowerCase().split(""));
  }
}
