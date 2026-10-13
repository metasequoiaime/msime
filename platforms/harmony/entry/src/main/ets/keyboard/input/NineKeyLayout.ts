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

// 触屏键盘的网格，与设计稿和 Android 一致：第一格是 @#，点击打开符号面板而不发送任何内容，被它取代的音节分隔符移到网格右侧那一列（`separator`）。2in1 屏幕键盘仍使用 `ROWS`。
const SYMBOLS_KEY: NineKey = key("@#", "@", "符号");
const TOUCH_ROWS: NineKey[][] = [[SYMBOLS_KEY, ROWS[0][1], ROWS[0][2]], ROWS[1], ROWS[2]];

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

// What the sidebar offers on the digit layer, where there are no readings to show and the four
// sentence marks are already on the letter layer's sidebar.
const DIGIT_SIDEBAR: string[] = ["@", "#", "/", "-"];

export class NineKeyLayout {
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

  /** 点击字母层的这一格是否打开符号面板而不是交给 Engine：即触屏网格的 @#。 */
  static opensSymbols(key: NineKey): boolean {
    return key.input === SYMBOLS_KEY.input;
  }

  /** 触屏网格左列的标点。 */
  static sideMarks(): string[] {
    return SIDE_MARKS;
  }

  /** 触屏网格右列底部的标点。 */
  static closingMark(): string {
    return CLOSING_MARK;
  }

  /** 字母层印出长按可输入的数字；符号入口也保留 1，分词和数字层不重复印。 */
  static digitHint(key: NineKey): string {
    if (NineKeyLayout.opensSymbols(key)) return "1";
    return NineKeyLayout.holdOptions(key).length > 0 ? key.input : "";
  }

  static digitPunctuation(): string[] {
    return DIGIT_SIDEBAR;
  }

  /** 长按选项：符号入口提供 1，字母键提供数字和各个小写字母。 */
  static holdOptions(key: NineKey): string[] {
    if (NineKeyLayout.opensSymbols(key)) return ["1"];
    if (key.input < "2" || key.input > "9" || !/^[A-Z]{3,4}$/.test(key.label)) {
      return [];
    }
    return [key.input].concat(key.label.toLowerCase().split(""));
  }
}
