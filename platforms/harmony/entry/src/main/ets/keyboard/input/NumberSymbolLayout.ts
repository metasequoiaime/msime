/**
 * 触控键盘的 123 层和 #+= 层，移植自 `platforms/android/java/app/msime/android/keyboard/KeyboardLayout.java` 中「新设计的 123 层与 #+= 层」部分。
 *
 * 每层四行：十个键，再十个键，然后是层切换键、五个标点键和删除键，最后是该层自己的底行（返回字母、表情或符号面板、空格、回车）。中文版放的是中文写作常用的全角标点，英文版放对应的 ASCII 符号。
 */
export enum LayerKeyKind {
  /** 输出键面上的文字。 */
  CHARACTER = "character",
  /** 123 层上是 `#+=`，#+= 层上是 `123`：在两层之间切换。 */
  LAYER_TOGGLE = "layer-toggle",
  DELETE = "delete",
  /** 底行第一个键，返回字母键：中文为「拼音」，英文为「ABC」。 */
  LETTERS = "letters",
  /** 123 层的表情键。 */
  EMOJI = "emoji",
  /** #+= 层的「符号」键，位于表情键的位置，打开分类符号面板。 */
  SYMBOL_PANEL = "symbol-panel",
  SPACE = "space",
  /** 回车；与字母行一样，视图按编辑框的动作给它相应的键面。 */
  RETURN = "return",
}

/** 字符键如何输出它显示的内容。 */
export enum LayerCharacterRoute {
  /** 字母键面的 123 层一直走的符号行路径：引擎的标点处理，或方案自己对数字（韩文、越南文、藏文）以及组字中途拼写符号的处理。 */
  SYMBOL_KEY = "symbol-key",
  /** 结束当前组字并直接插入键面本身，与 Android 的 `commitNineKeyLiteral` 相同。 */
  LITERAL = "literal",
}

export interface LayerKey {
  /** 键面文字；对字符键而言也是它输出的文字。 */
  readonly text: string;
  /** 读屏器朗读的内容。 */
  readonly description: string;
  readonly kind: LayerKeyKind;
  /** 该键占所在行宽度的份额。 */
  readonly weight: number;
}

const NUMBER_DIGITS: string[] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
const CHINESE_NUMBER_SYMBOLS: string[] = ["-", "/", "：", "；", "（", "）", "¥", "@", "“", "”"];
const ENGLISH_NUMBER_SYMBOLS: string[] = ["-", "/", ":", ";", "(", ")", "$", "@", '"', "'"];
const CHINESE_PUNCTUATION: string[] = ["。", "，", "、", "？", "！"];
const ENGLISH_PUNCTUATION: string[] = [".", ",", "?", "!", "…"];
const MORE_SYMBOLS_FIRST: string[] = ["[", "]", "{", "}", "#", "%", "^", "*", "+", "="];
const CHINESE_MORE_SYMBOLS_SECOND: string[] = ["_", "\\", "|", "~", "《", "》", "€", "&", "·", "…"];
const ENGLISH_MORE_SYMBOLS_SECOND: string[] = ["_", "\\", "|", "~", "<", ">", "€", "&", "·", "£"];

function layerKey(text: string, description: string, kind: LayerKeyKind, weight: number): LayerKey {
  return { text: text, description: description, kind: kind, weight: weight };
}

function character(text: string): LayerKey {
  return layerKey(text, text, LayerKeyKind.CHARACTER, 1);
}

function characters(texts: string[]): LayerKey[] {
  return texts.map((text: string): LayerKey => character(text));
}

export class NumberSymbolLayout {
  /** 第三行两端的键（`#+=` / `123` 和删除键），与字母行的 Shift 和删除键等宽。 */
  static readonly EDGE_WEIGHT: number = 1.4;
  /** 底行：返回字母 1.25，表情或「符号」1.05，空格 6，回车 1.9，合计等于字母键面的底行（10.2）。 */
  static readonly BACK_WEIGHT: number = 1.25;
  static readonly EMOJI_WEIGHT: number = 1.05;
  static readonly SPACE_WEIGHT: number = 6;
  static readonly RETURN_WEIGHT: number = 1.9;

  /** 1–0 / 十个符号 / `#+=`、五个标点和删除键 / 拼音（或 ABC）、表情、空格、回车。 */
  static numberLayer(chinese: boolean): LayerKey[][] {
    return NumberSymbolLayout.layer(
      NUMBER_DIGITS,
      chinese ? CHINESE_NUMBER_SYMBOLS : ENGLISH_NUMBER_SYMBOLS,
      layerKey("#+=", "更多符号", LayerKeyKind.LAYER_TOGGLE, NumberSymbolLayout.EDGE_WEIGHT),
      chinese,
      layerKey("😀", "表情", LayerKeyKind.EMOJI, NumberSymbolLayout.EMOJI_WEIGHT),
    );
  }

  /** [ ] { } # % ^ * + = / _ \ | ~ 《 》 € & · … / `123`、五个标点和删除键 / 拼音（或 ABC）、符号、空格、回车。 */
  static moreSymbolLayer(chinese: boolean): LayerKey[][] {
    return NumberSymbolLayout.layer(
      MORE_SYMBOLS_FIRST,
      chinese ? CHINESE_MORE_SYMBOLS_SECOND : ENGLISH_MORE_SYMBOLS_SECOND,
      layerKey(
        "123",
        "切换到数字和符号",
        LayerKeyKind.LAYER_TOGGLE,
        NumberSymbolLayout.EDGE_WEIGHT,
      ),
      chinese,
      layerKey("符号", "切换符号键盘", LayerKeyKind.SYMBOL_PANEL, NumberSymbolLayout.EMOJI_WEIGHT),
    );
  }

  /** 当前显示的层：`moreSymbols` 为真时是 #+=，否则是 123。 */
  static rows(moreSymbols: boolean, chinese: boolean): LayerKey[][] {
    return moreSymbols
      ? NumberSymbolLayout.moreSymbolLayer(chinese)
      : NumberSymbolLayout.numberLayer(chinese);
  }

  /** 底行返回字母键的键面：中文为「拼音」，英文为「ABC」。 */
  static lettersKeyTitle(chinese: boolean): string {
    return chinese ? "拼音" : "ABC";
  }

  /**
   * 字符键如何输出它的键面。
   *
   * 数字在两种版本里都走符号行路径，因为正是这条路径让韩文、越南文或藏文的组字在数字之前结束，也让正在拼写的 URL 保留其中的数字。中文层上其余的键都按所画的样子直接插入：这些键显示的就是它们输出的确切符号，而引擎的中文标点会把 `[` 变成 【、把 `\` 变成 、。英文层上 ASCII 符号走符号行路径，英文下会原样放行，对藏文和越南文组字器则可能是拼写符号；没有 ASCII 键的符号（… € · £）直接插入。
   */
  static route(text: string, chinese: boolean): LayerCharacterRoute {
    if (text.length !== 1) {
      return LayerCharacterRoute.LITERAL;
    }
    const code: number = text.charCodeAt(0);
    if (code >= 0x30 && code <= 0x39) {
      return LayerCharacterRoute.SYMBOL_KEY;
    }
    if (chinese) {
      return LayerCharacterRoute.LITERAL;
    }
    return code >= 0x21 && code <= 0x7e
      ? LayerCharacterRoute.SYMBOL_KEY
      : LayerCharacterRoute.LITERAL;
  }

  private static layer(
    first: string[],
    second: string[],
    toggle: LayerKey,
    chinese: boolean,
    panelKey: LayerKey,
  ): LayerKey[][] {
    const third: LayerKey[] = [toggle];
    for (const mark of chinese ? CHINESE_PUNCTUATION : ENGLISH_PUNCTUATION) {
      third.push(character(mark));
    }
    third.push(layerKey("⌫", "删除", LayerKeyKind.DELETE, NumberSymbolLayout.EDGE_WEIGHT));
    const bottom: LayerKey[] = [
      layerKey(
        NumberSymbolLayout.lettersKeyTitle(chinese),
        "切换到字母键盘",
        LayerKeyKind.LETTERS,
        NumberSymbolLayout.BACK_WEIGHT,
      ),
      panelKey,
      layerKey("空格", "空格", LayerKeyKind.SPACE, NumberSymbolLayout.SPACE_WEIGHT),
      layerKey("换行", "换行", LayerKeyKind.RETURN, NumberSymbolLayout.RETURN_WEIGHT),
    ];
    return [characters(first), characters(second), third, bottom];
  }
}
