/**
 * 手机触屏键盘按键面类型选的底行样式（共享偏好 `touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style`）和字母上方的数字行（`touch_number_row`），取值与默认值见 `crates/client-core/src/preferences.rs` 和笔记 2026-10-27-touch-key-layout-styles.md。
 *
 * 键序和份额放在这里而不是视图里，`tests/run.sh` 能在 node 下逐键核对；三端（Android `KeyboardActionRow`、iOS 与这里）画同一组排法。默认样式就是加样式之前的底行，份额一份不差。鸿蒙没有地球键，所以这里的行都不带它。九键外框的排法在 `NineKeyLayout.actionRow`。
 */

/** 设计底行（`KeyboardView.designActionRow`）上的一个键位。 */
export enum DesignRowSlot {
  LAYER = "layer",
  SYMBOL_PANEL = "symbol_panel",
  EMOJI = "emoji",
  COMMA = "comma",
  PERIOD = "period",
  SPACE = "space",
  LANGUAGE = "language",
  RETURN = "return",
}

export interface DesignRowEntry {
  readonly slot: DesignRowSlot;
  /** 在底行里占的宽度份额（`layoutWeight`）。 */
  readonly share: number;
}

/** 这一行按哪种键面类型的样式排：中文 26 键、14 键与 18 键，或者不受样式影响（英文 26 键、韩文等、大千注音）。 */
export enum StyledFace {
  NONE = "none",
  TWENTY_SIX = "twenty_six",
  FOURTEEN = "fourteen",
}

/** 底行逗号长按做什么：输入句号，或者打开常用标点列表（与 14 键逗号相同）。 */
export enum CommaHold {
  PERIOD = "period",
  MENU = "menu",
}

/** 决定数字行的键面状态，由视图按当前方案和中英文填写。 */
export interface NumberRowFace {
  /** 日语假名网格。 */
  readonly kanaGrid: boolean;
  /** 方案本身是手写（与在不在 123 层无关，手写的 123 层不画补充行）。 */
  readonly handwriting: boolean;
  /** 拼音九键、九键双拼和英文九键的网格。 */
  readonly nineKeyGrid: boolean;
  /** 大千注音。 */
  readonly zhuyin: boolean;
  /** 笔画键盘。 */
  readonly stroke: boolean;
}

/** 视图画过的键面里决定第一帧高度的两件事，存在键盘自己的状态目录里，下次冷启动建面板时用（`TouchLayoutStylePolicy.firstPanel`）。 */
export interface TouchPanelFace {
  /** 字母层是 26、14 或 18 键这一类（`lettersRowFace`），开着数字行时多一行。 */
  readonly lettersRow: boolean;
  /** 字母层是中文 26 键，长空格样式让工具栏一直画着。 */
  readonly chineseTwentySix: boolean;
}

/** 面板的形状：工具条那一行画不画，按键区多出几行（`KeyboardMetrics.totalHeightVp` 的 `withStrip` 和 `extraRows`）。 */
export interface TouchPanelShape {
  readonly strip: boolean;
  readonly extraRows: number;
}

function entry(slot: DesignRowSlot, share: number): DesignRowEntry {
  return { slot: slot, share: share };
}

export class TouchLayoutStylePolicy {
  static readonly STANDARD: string = "standard";
  static readonly COMMA_PERIOD: string = "comma_period";
  static readonly LONG_SPACE: string = "long_space";
  static readonly EMOJI: string = "emoji";
  /** 九键「右侧大回车」：回车跨右列下两行，符 在符号栏下方（`NineKeyLayout.tallReturn`）。 */
  static readonly TALL_RETURN: string = "tall_return";

  /** 设计底行的份额：123 1.25 | 符 1.05 | ， 1 | space 4 | 。 1 | 中 1.05 | return 1.9，`KeyboardView.designActionRow` 按它们定宽。 */
  static readonly LAYER_SHARE: number = 1.25;
  static readonly SYMBOL_PANEL_SHARE: number = 1.05;
  static readonly PUNCTUATION_SHARE: number = 1;
  static readonly SPACE_SHARE: number = 4;
  static readonly LANGUAGE_SHARE: number = 1.05;
  static readonly RETURN_SHARE: number = 1.9;
  /** 英文 26 键底行在空格左边多一个句点（份额 1），这一份从空格里让出，与 iOS `englishPhoneSpaceWeight`、Android `DESIGN_ENGLISH_SPACE_WEIGHT` 相同。 */
  static readonly ENGLISH_SPACE_SHARE: number =
    TouchLayoutStylePolicy.SPACE_SHARE - TouchLayoutStylePolicy.PUNCTUATION_SHARE;

  /** 字母上方数字行的十个键，从 1 到 0。 */
  static readonly NUMBER_ROW: string[] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];

  private static readonly TWENTY_SIX_STYLES: readonly string[] = [
    TouchLayoutStylePolicy.STANDARD,
    TouchLayoutStylePolicy.COMMA_PERIOD,
    TouchLayoutStylePolicy.LONG_SPACE,
    TouchLayoutStylePolicy.EMOJI,
  ];
  private static readonly FOURTEEN_STYLES: readonly string[] = [
    TouchLayoutStylePolicy.STANDARD,
    TouchLayoutStylePolicy.LONG_SPACE,
    TouchLayoutStylePolicy.EMOJI,
  ];
  private static readonly NINE_STYLES: readonly string[] = [
    TouchLayoutStylePolicy.STANDARD,
    TouchLayoutStylePolicy.COMMA_PERIOD,
    TouchLayoutStylePolicy.EMOJI,
    TouchLayoutStylePolicy.TALL_RETURN,
  ];

  /** `touch_twenty_six_key_style` 的值；缺键、不是字符串或本版本不认识的样式（较新版本加的）都按 `standard`，与 client-core 宽松读取的规则相同。 */
  static twentySixKeyStyle(value: unknown): string {
    return TouchLayoutStylePolicy.known(value, TouchLayoutStylePolicy.TWENTY_SIX_STYLES);
  }

  /** `touch_fourteen_key_style` 的值，14 键与 18 键共用；读法同上。 */
  static fourteenKeyStyle(value: unknown): string {
    return TouchLayoutStylePolicy.known(value, TouchLayoutStylePolicy.FOURTEEN_STYLES);
  }

  /** `touch_nine_key_style` 的值；读法同上。 */
  static nineKeyStyle(value: unknown): string {
    return TouchLayoutStylePolicy.known(value, TouchLayoutStylePolicy.NINE_STYLES);
  }

  /**
   * 设计底行的键序与份额。
   *
   * 不受样式影响时（`StyledFace.NONE`）就是原来的两种底行：26 键一族 123 | 符 | 空格 | ， | 中 | 回车，英文时空格左边多一个句点、空格让到 3；其余（大千注音）123 | ， | 空格 | 。 | 中 | 回车。
   *
   * 中文 26 键：`comma_period` 是 123 | 符 | ， | 空格 3 | 。 | 中 | 回车（句号占从空格让出的一份）；`long_space` 是 123 | ， | 空格 5.05 | 中 | 回车（符 的宽度交给空格，符号入口移到工具栏），即 Android 的 `KeyboardActionRow.DESIGN_LONG_SPACE_WEIGHT`；`emoji` 是 123 | 😀 | 空格 | ， | 中 | 回车，😀 占 符 的位置和宽度。
   *
   * 14 键与 18 键：`long_space` 是 123 | ， | 空格 5 | 中 | 回车（。 的一份给空格）；`emoji` 是 123 | 😀 | 空格 | ， | 中 | 回车，😀 占原来 ， 的一份，， 占原来 。 的一份。
   *
   * 每种样式的整行份额都与默认行相同。
   *
   * @param commaSecondaryPeriod 26 键一族的底行（`KeyboardScheme.designRowUsesCommaSecondaryPeriod`）
   * @param english 英文 26 键在空格左边加句点（`KeyboardScheme.designRowHasEnglishPeriod`）；只有不受样式影响的 26 键底行看它
   * @param styled 按哪种键面类型的样式排
   * @param style 那种键面类型选中的样式（已经过 `twentySixKeyStyle` 或 `fourteenKeyStyle`）
   */
  static designRow(
    commaSecondaryPeriod: boolean,
    english: boolean,
    styled: StyledFace,
    style: string,
  ): DesignRowEntry[] {
    const P = TouchLayoutStylePolicy;
    if (styled === StyledFace.TWENTY_SIX) {
      if (style === P.COMMA_PERIOD) {
        return [
          entry(DesignRowSlot.LAYER, P.LAYER_SHARE),
          entry(DesignRowSlot.SYMBOL_PANEL, P.SYMBOL_PANEL_SHARE),
          entry(DesignRowSlot.COMMA, P.PUNCTUATION_SHARE),
          entry(DesignRowSlot.SPACE, P.SPACE_SHARE - P.PUNCTUATION_SHARE),
          entry(DesignRowSlot.PERIOD, P.PUNCTUATION_SHARE),
          entry(DesignRowSlot.LANGUAGE, P.LANGUAGE_SHARE),
          entry(DesignRowSlot.RETURN, P.RETURN_SHARE),
        ];
      }
      if (style === P.LONG_SPACE) {
        return [
          entry(DesignRowSlot.LAYER, P.LAYER_SHARE),
          entry(DesignRowSlot.COMMA, P.PUNCTUATION_SHARE),
          entry(DesignRowSlot.SPACE, P.SPACE_SHARE + P.SYMBOL_PANEL_SHARE),
          entry(DesignRowSlot.LANGUAGE, P.LANGUAGE_SHARE),
          entry(DesignRowSlot.RETURN, P.RETURN_SHARE),
        ];
      }
      if (style === P.EMOJI) {
        return [
          entry(DesignRowSlot.LAYER, P.LAYER_SHARE),
          entry(DesignRowSlot.EMOJI, P.SYMBOL_PANEL_SHARE),
          entry(DesignRowSlot.SPACE, P.SPACE_SHARE),
          entry(DesignRowSlot.COMMA, P.PUNCTUATION_SHARE),
          entry(DesignRowSlot.LANGUAGE, P.LANGUAGE_SHARE),
          entry(DesignRowSlot.RETURN, P.RETURN_SHARE),
        ];
      }
      return P.designRow(true, false, StyledFace.NONE, P.STANDARD);
    }
    if (styled === StyledFace.FOURTEEN) {
      if (style === P.LONG_SPACE) {
        return [
          entry(DesignRowSlot.LAYER, P.LAYER_SHARE),
          entry(DesignRowSlot.COMMA, P.PUNCTUATION_SHARE),
          entry(DesignRowSlot.SPACE, P.SPACE_SHARE + P.PUNCTUATION_SHARE),
          entry(DesignRowSlot.LANGUAGE, P.LANGUAGE_SHARE),
          entry(DesignRowSlot.RETURN, P.RETURN_SHARE),
        ];
      }
      if (style === P.EMOJI) {
        return [
          entry(DesignRowSlot.LAYER, P.LAYER_SHARE),
          entry(DesignRowSlot.EMOJI, P.PUNCTUATION_SHARE),
          entry(DesignRowSlot.SPACE, P.SPACE_SHARE),
          entry(DesignRowSlot.COMMA, P.PUNCTUATION_SHARE),
          entry(DesignRowSlot.LANGUAGE, P.LANGUAGE_SHARE),
          entry(DesignRowSlot.RETURN, P.RETURN_SHARE),
        ];
      }
      return P.designRow(false, false, StyledFace.NONE, P.STANDARD);
    }
    if (commaSecondaryPeriod) {
      const row: DesignRowEntry[] = [
        entry(DesignRowSlot.LAYER, P.LAYER_SHARE),
        entry(DesignRowSlot.SYMBOL_PANEL, P.SYMBOL_PANEL_SHARE),
      ];
      if (english) {
        // 英文 26 键：句点紧挨空格左边，这一份从空格里让出（`ENGLISH_SPACE_SHARE`）。
        row.push(entry(DesignRowSlot.PERIOD, P.PUNCTUATION_SHARE));
      }
      row.push(entry(DesignRowSlot.SPACE, english ? P.ENGLISH_SPACE_SHARE : P.SPACE_SHARE));
      row.push(entry(DesignRowSlot.COMMA, P.PUNCTUATION_SHARE));
      row.push(entry(DesignRowSlot.LANGUAGE, P.LANGUAGE_SHARE));
      row.push(entry(DesignRowSlot.RETURN, P.RETURN_SHARE));
      return row;
    }
    return [
      entry(DesignRowSlot.LAYER, P.LAYER_SHARE),
      entry(DesignRowSlot.COMMA, P.PUNCTUATION_SHARE),
      entry(DesignRowSlot.SPACE, P.SPACE_SHARE),
      entry(DesignRowSlot.PERIOD, P.PUNCTUATION_SHARE),
      entry(DesignRowSlot.LANGUAGE, P.LANGUAGE_SHARE),
      entry(DesignRowSlot.RETURN, P.RETURN_SHARE),
    ];
  }

  /**
   * 底行逗号的长按：26 键一族（含中文 26 键的标准、长空格和表情键样式）长按输入句号，与加样式之前相同；中文 26 键的「逗号句号在空格两侧」有独立句号键，逗号长按改为与 14 键逗号一样打开常用标点列表；14 键、18 键和大千注音一律打开常用标点列表。
   */
  static commaHold(commaSecondaryPeriod: boolean, styled: StyledFace, style: string): CommaHold {
    if (styled === StyledFace.TWENTY_SIX) {
      return style === TouchLayoutStylePolicy.COMMA_PERIOD ? CommaHold.MENU : CommaHold.PERIOD;
    }
    if (styled === StyledFace.FOURTEEN) {
      return CommaHold.MENU;
    }
    return commaSecondaryPeriod ? CommaHold.PERIOD : CommaHold.MENU;
  }

  /** 底行逗号下滑输入句号：只有长空格样式（中文 26 键与 14/18 键）有，与 Android 长空格布局的逗号下滑相同；其余样式不接下滑。 */
  static commaSwipesPeriod(styled: StyledFace, style: string): boolean {
    return styled !== StyledFace.NONE && style === TouchLayoutStylePolicy.LONG_SPACE;
  }

  /**
   * 中文 26 键是否处在长空格样式（与在不在 123 层无关）；密码框和只收数字的编辑框里不算（`fieldRowStyle`）。这时与 Android 的 `longSpaceLayoutActive` 一样：符号面板入口移到工具栏，工具栏这一行一直画着（设成隐藏时只留标志、符号和收起）；123 进入带空格的数字符号层而不借用数字九宫格，那一层和 #+= 层左下的 😀/符号 换成同一个逗号（下滑输入句号）。
   */
  static longSpaceActive(
    chineseTwentySix: boolean,
    twentySixStyle: string,
    standardField: boolean,
  ): boolean {
    return (
      chineseTwentySix &&
      TouchLayoutStylePolicy.fieldRowStyle(twentySixStyle, standardField) ===
        TouchLayoutStylePolicy.LONG_SPACE
    );
  }

  /** 一种键面类型（中文 26 键、14/18 键、九键）的底行实际按哪种样式排：密码框和只收数字的编辑框（`standardField`）里每种样式都退回标准底行，与 Android `bottomRowStyle` 遇到 `directNumberPad` 或密码框时一律用标准底行相同（用户在密码框里切到中文时也是这样）；其他编辑框照选中的样式。 */
  static fieldRowStyle(style: string, standardField: boolean): string {
    return standardField ? TouchLayoutStylePolicy.STANDARD : style;
  }

  /** 字母层是 26、14 或 18 键这一类（数字行和 123 层补充行只出现在这一类上）：九键网格、假名网格、手写、笔画和大千注音都不算。 */
  static lettersRowFace(face: NumberRowFace): boolean {
    return !face.kanaGrid && !face.handwriting && !face.nineKeyGrid && !face.zhuyin && !face.stroke;
  }

  /**
   * 数字行（字母层）或它在 123、#+= 和借用数字九宫格上的补充行要不要多画这一行：开关打开、字母层是 26/14/18 键一类，并且不是手机横屏。手机横屏窗口太矮，与 Android 的 480 dp 门槛和 iOS 一样不画，存着的值不变，转回竖屏自动回来。返回多出的行数（0 或 1），键盘高度正好多一行，切到 123 层时不变。
   */
  static numberRowExtraRows(
    enabled: boolean,
    face: NumberRowFace,
    phoneLandscape: boolean,
  ): number {
    return enabled && !phoneLandscape && TouchLayoutStylePolicy.lettersRowFace(face) ? 1 : 0;
  }

  /**
   * 数字行开着时 123、#+= 层和借用数字九宫格顶部那一排补充键，与 Android `KeyboardLayout.symbolExtraRow` 同一张表：#+= 层补 1 到 0，借用的数字九宫格补括号、百分号、货币与上标（括号随中文标点用全角），123 层补该层原来没有的 [ ] { } # % ^ * + =。
   *
   * @param more 在 #+= 层
   * @param numberPad 画的是借用的数字九宫格
   * @param chinesePunctuation 中文标点（全角括号）
   */
  static symbolExtraRow(more: boolean, numberPad: boolean, chinesePunctuation: boolean): string[] {
    if (more) {
      return TouchLayoutStylePolicy.NUMBER_ROW.slice();
    }
    if (!numberPad) {
      return ["[", "]", "{", "}", "#", "%", "^", "*", "+", "="];
    }
    return [
      chinesePunctuation ? "（" : "(",
      chinesePunctuation ? "）" : ")",
      "%",
      "¥",
      "$",
      "€",
      "£",
      "‰",
      "²",
      "³",
    ];
  }

  /**
   * 扩展能力建面板时（视图还没出现）定第一帧的高度：工具条画不画、字母上方多不多一行。键面要等视图按方案画出来才知道，所以用视图上次记下的键面（`TouchPanelFace`，没记过时为 null）配上现在的偏好来算，与 Android 冷启动按记住的样式画第一帧同一个意思：上次停在中文 26 键、开着长空格样式时工具栏一直画着；上次停在 26/14/18 键一类、开着数字行且不是手机横屏时多一行。没记过时与加样式之前相同：工具条只看工具栏设置，不多行。视图出现后仍按同一套规则（`stripShown`、`extraRows`）核对一次，键面与记下的不同时再改高度。
   *
   * @param face 视图上次记下的键面
   * @param numberRow 共享偏好 `touch_number_row`
   * @param twentySixStyle 共享偏好 `touch_twenty_six_key_style`（已经过 `twentySixKeyStyle`）
   * @param toolbarRowHidden 工具栏设置空闲时不画这一行（`TouchToolbarPolicy.rowHidden`）
   * @param phoneLandscape 手机横屏
   */
  static firstPanel(
    face: TouchPanelFace | null,
    numberRow: boolean,
    twentySixStyle: string,
    toolbarRowHidden: boolean,
    phoneLandscape: boolean,
  ): TouchPanelShape {
    const P = TouchLayoutStylePolicy;
    if (face === null) {
      return { strip: !toolbarRowHidden, extraRows: 0 };
    }
    // 编辑框的属性这时还没到，按普通编辑框算；第一个编辑框是密码框时由视图改回。
    const longSpace: boolean = P.longSpaceActive(face.chineseTwentySix, twentySixStyle, false);
    const extraRows: number = numberRow && face.lettersRow && !phoneLandscape ? 1 : 0;
    return { strip: !toolbarRowHidden || longSpace, extraRows: extraRows };
  }

  /** 读视图记下的键面（`touchPanelFaceText` 写的那几个字节）；读不懂时为 null，按没记过算。 */
  static parseTouchPanelFace(text: string): TouchPanelFace | null {
    let decoded: Object;
    try {
      decoded = JSON.parse(text) as Object;
    } catch {
      return null;
    }
    if (decoded === null || typeof decoded !== "object" || Array.isArray(decoded)) {
      return null;
    }
    const record: Record<string, Object> = decoded as Record<string, Object>;
    if (typeof record.letters_row !== "boolean" || typeof record.chinese_twenty_six !== "boolean") {
      return null;
    }
    return {
      lettersRow: record.letters_row === true,
      chineseTwentySix: record.chinese_twenty_six === true,
    };
  }

  static touchPanelFaceText(face: TouchPanelFace): string {
    return `{"letters_row":${face.lettersRow},"chinese_twenty_six":${face.chineseTwentySix}}`;
  }

  /** 中/英键是否紧挨在回车左边：这时两键的触摸分界挪进回车键帽（`KeyboardGeometry.RETURN_YIELD_VP`）。 */
  static languageBesideReturn(row: readonly DesignRowEntry[]): boolean {
    for (let index = 1; index < row.length; index++) {
      if (row[index].slot === DesignRowSlot.RETURN) {
        return row[index - 1].slot === DesignRowSlot.LANGUAGE;
      }
    }
    return false;
  }

  private static known(value: unknown, styles: readonly string[]): string {
    return typeof value === "string" && styles.includes(value)
      ? value
      : TouchLayoutStylePolicy.STANDARD;
  }
}

/**
 * 长空格样式下底行逗号的下滑：手指按下后向下移过 `THRESHOLD_VP`、且竖向移动大于横向时，抬起输入句号，这次按键的普通点按被吞掉。ArkUI 先送触摸抬起再送点击，所以由视图在点击里问 `swallows()`；下一次按下时清掉，点击没有送来也不会吞掉之后的点按。长按已经识别时（输入句号或打开列表）调 `cancel()`，抬起不再按下滑处理。
 */
export class CommaSwipeGesture {
  /** 与字母键滑动输入符号的门槛（`SwipeHintPolicy.THRESHOLD_VP`）相同。 */
  static readonly THRESHOLD_VP: number = 14;

  private downX: number = 0;
  private downY: number = 0;
  private tracking: boolean = false;
  private swallow: boolean = false;

  down(x: number, y: number): void {
    this.downX = x;
    this.downY = y;
    this.tracking = true;
    this.swallow = false;
  }

  /** 手指抬起；是一次下滑就返回 true（由视图输入句号），否则返回 false，由点按输入逗号。 */
  up(x: number, y: number): boolean {
    if (!this.tracking) {
      return false;
    }
    this.tracking = false;
    const dy: number = y - this.downY;
    const swiped: boolean = dy > CommaSwipeGesture.THRESHOLD_VP && dy > Math.abs(x - this.downX);
    this.swallow = swiped;
    return swiped;
  }

  /** 长按已经识别或触摸被取消：这次触摸不再按下滑处理。 */
  cancel(): void {
    this.tracking = false;
  }

  /** 点按到达时问一次：刚才那次触摸已经按下滑输入了句号，这次点按不再输入逗号。问过即清。 */
  swallows(): boolean {
    const swallowed: boolean = this.swallow;
    this.swallow = false;
    return swallowed;
  }
}
