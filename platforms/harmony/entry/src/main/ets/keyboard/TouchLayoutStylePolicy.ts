/**
 * 手机触屏键盘按键面类型选的底行样式（共享偏好 `touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style`）和字母上方的数字行（`touch_number_row`），取值与默认值见 `crates/client-core/src/preferences.rs` 和笔记 2026-10-27-touch-key-layout-styles.md。
 *
 * 键序和份额放在这里而不是视图里，`tests/run.sh` 能在 node 下逐键核对；三端（Android `KeyboardActionRow`、iOS 与这里）画同一组排法。默认样式就是加样式之前的底行，份额一份不差。鸿蒙没有地球键，所以这里的行都不带它。九键外框的排法在 `NineKeyLayout.actionRow`。
 */
import { KeyboardScheme, SchemeDefinition } from "./KeyboardScheme";
import { SchemeTraits } from "./SchemeTraits";
import { KoreanCompositionPolicy } from "./input/KoreanCompositionPolicy";
import { LetterHintGesture } from "./input/LetterHintGesture";
import { SchemeCompositionPolicy } from "./input/SchemeCompositionPolicy";
import { SwipeHintPolicy } from "./input/SwipeHintPolicy";

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

/** 设计底行上画出来的一个键（`TouchLayoutStylePolicy.designRowItems`）：`KeyboardView.designActionRow` 用 ForEach 逐个画，画这个键时按值传下去的量都在这里，ForEach 的键（`designRowItemKey`）也由它们组成。 */
export interface DesignRowItem {
  readonly slot: DesignRowSlot;
  /** 在底行里占的宽度份额（`layoutWeight`）。 */
  readonly share: number;
  /** 中/英 紧挨在回车左边，两键的触摸分界挪进回车键帽；只有中/英 和回车两个键位看它，其余键位恒为 false。 */
  readonly besideReturn: boolean;
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

/** 视图交来的这块键盘：用户选的方案和键面，都不随中/英变。字母层的键面（`letterFace`）和这块键盘的中文键面是不是中文 26 键（`chineseTwentySixKeyboard`）都从它算。 */
export interface TouchKeyboard {
  readonly scheme: SchemeDefinition;
  /** 本地工具模式，`none` 是没有。 */
  readonly localMode: string;
  /** 选了拼音九键或九键双拼的网格。 */
  readonly nineKey: boolean;
  /** 选了 14 键或 18 键。 */
  readonly letterGrid: boolean;
  /** 日语假名网格。 */
  readonly kanaGrid: boolean;
}

/** 视图画过的键面里决定第一帧高度的两件事，存在键盘自己的状态目录里，下次冷启动建面板时用（`TouchLayoutStylePolicy.firstPanel`）。 */
export interface TouchPanelFace {
  /** 字母层是 26、14 或 18 键这一类（`lettersRowFace`），开着数字行时多一行。 */
  readonly lettersRow: boolean;
  /** 这块键盘的中文键面是中文 26 键（停在它的英文 26 键上也算），长空格样式让工具栏一直画着。 */
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

  /** 九宫格数字键盘空格位置（共享偏好 `touch_number_pad_space`）：空格在右列最下面一格，0 占底行里原来两键的份额（Android 的出厂排法）。 */
  static readonly RIGHT_COLUMN: string = "right_column";
  /** 0 和空格都在底行，空格在 0 右边、与中/英同宽（iOS 的出厂排法）。 */
  static readonly AFTER_ZERO: string = "after_zero";
  /** 0 和空格都在底行，空格在 0 左边、0 与中/英同宽：鸿蒙的出厂排法，文档里缺键或写 `platform_default` 时就是它。 */
  static readonly BEFORE_ZERO: string = "before_zero";

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

  /** `touch_number_pad_space` 的值：缺键、`platform_default`、不是字符串或本版本不认识的写法都按鸿蒙原来的 `before_zero`，与 client-core 宽松读取的规则相同。 */
  static numberPadSpace(value: unknown): string {
    const P = TouchLayoutStylePolicy;
    if (value === P.RIGHT_COLUMN) {
      return P.RIGHT_COLUMN;
    }
    return value === P.AFTER_ZERO ? P.AFTER_ZERO : P.BEFORE_ZERO;
  }

  /** 鸿蒙字母层出厂表的显示形式（键盘点按仍把 ASCII `, . ?` 交给 Engine），与共享设置页 `nineKeySidebarDefaults.letter` 相同。 */
  static readonly NINE_KEY_SIDEBAR_DEFAULTS: string[] = ["，", "。", "？"];
  /** 鸿蒙数字层出厂表：四则运算，#+= 不在表里。 */
  static readonly DIGIT_SIDEBAR_DEFAULTS: string[] = ["+", "-", "*", "/"];

  /** 九键左侧符号栏的一张表（共享偏好 `touch_nine_key_sidebar_symbols` / `touch_nine_key_digit_sidebar_symbols`，#10223）：client-core 读写时已经规范化，这里只留字符串元素；缺键、不是数组读成空表，即本端出厂表（`NineKeyLayout.sideMarks`、`digitSidebar`）。与 `defaults` 逐项、同顺序相同的非空表也读成空表，按出厂表画和上屏，与 Android `NineKeySidebarPolicy.custom` 相同。 */
  static sidebarSymbols(value: unknown, defaults: readonly string[] = []): string[] {
    if (!Array.isArray(value)) {
      return [];
    }
    const symbols: string[] = [];
    for (const item of value as Object[]) {
      if (typeof item === "string" && item.length > 0) {
        symbols.push(item as string);
      }
    }
    const factory: boolean =
      symbols.length === defaults.length &&
      symbols.every((symbol: string, index: number): boolean => symbol === defaults[index]);
    return factory ? [] : symbols;
  }

  /**
   * 「26 键「符」与「123」互换」（共享偏好 `touch_twenty_six_symbol_first`）：底行里 符 排在 123 后面时把它挪到 123 左边，其余键次序和份额不变，与 Android `KeyboardActionRow.designEntries`、iOS `TouchBottomRowLayout.symbolFirst` 相同。只有 26 键一族的底行（标准、逗号句号在空格两侧和英文底行）是 123 | 符 打头，长空格和表情键样式、14/18 键和大千注音没有 符，原样返回。
   */
  static symbolFirst(row: readonly DesignRowEntry[], enabled: boolean): DesignRowEntry[] {
    const layer: number = row.findIndex(
      (item: DesignRowEntry): boolean => item.slot === DesignRowSlot.LAYER,
    );
    const symbol: number = row.findIndex(
      (item: DesignRowEntry): boolean => item.slot === DesignRowSlot.SYMBOL_PANEL,
    );
    if (!enabled || layer < 0 || symbol < layer) {
      return row.slice();
    }
    const moved: DesignRowEntry[] = row.slice();
    const panel: DesignRowEntry[] = moved.splice(symbol, 1);
    moved.splice(layer, 0, panel[0]);
    return moved;
  }

  /** 共享偏好 `touch_language_key_side` 里表示「中/英键在空格左边」的写法（`client_core::preferences::TouchLanguageKeySide::Left`）；另一个取值 `right` 是默认，不写进文档。 */
  static readonly LANGUAGE_LEFT: string = "left";

  /** 「中/英键放在空格左边」（`touch_language_key_side`）：只有值恰好是字符串 `left` 时算左；缺键、`right`、本版本不认识的写法和不是字符串的值都按右（现在的排法），与 client-core 宽松读取的规则相同。 */
  static languageKeyLeft(value: unknown): boolean {
    return value === TouchLayoutStylePolicy.LANGUAGE_LEFT;
  }

  /** 中/英 挪到左边时跟着整串右移一格的键：空格和紧贴空格的标点（设计底行没有 0，九键外框见 `NineKeyLayout.languageBeforeSpace`）。 */
  private static readonly LANGUAGE_RUN: readonly DesignRowSlot[] = [
    DesignRowSlot.SPACE,
    DesignRowSlot.COMMA,
    DesignRowSlot.PERIOD,
  ];

  /**
   * 「中/英键放在空格左边」（共享偏好 `touch_language_key_side` 为 `left`）：在样式和「符」与「123」互换都排完之后，把 中/英 左边紧挨着的那一串空格和标点整串右移一格，中/英 挪到这一串的最左边；所有键的份额不变，只换次序。与 Android `KeyboardActionRow.languageBeforeSpace`、iOS `TouchBottomRowLayout.languageBeforeSpace` 同一条规则（笔记 2026-10-29-language-key-left-of-space.md）。
   *
   * 空格左边没有标点的行（26 键标准、表情键）中/英 紧挨空格：123 | 符 | 中 | 空格 | ， | 回车；空格左边有标点的行（逗号句号、长空格、14/18 键、大千注音、英文 26 键的句点）标点仍紧挨空格，中/英 排在标点左边：123 | 符 | 中 | ， | 空格 | 。 | 回车。行里没有 中/英，或 中/英 左边紧挨的不是空格或标点时原样返回。
   */
  static languageBeforeSpace(row: readonly DesignRowEntry[], enabled: boolean): DesignRowEntry[] {
    const moved: DesignRowEntry[] = row.slice();
    const language: number = moved.findIndex(
      (item: DesignRowEntry): boolean => item.slot === DesignRowSlot.LANGUAGE,
    );
    if (!enabled || language < 0) {
      return moved;
    }
    let start: number = language;
    while (start > 0 && TouchLayoutStylePolicy.LANGUAGE_RUN.includes(moved[start - 1].slot)) {
      start--;
    }
    if (start === language) {
      return moved;
    }
    const key: DesignRowEntry[] = moved.splice(language, 1);
    moved.splice(start, 0, key[0]);
    return moved;
  }

  /** 这一行以 符 打头（互换后的 26 键底行）：从它借来的数字九宫格按九键一族的次序排，符 | 返回 | …（`NineKeyLayout.actionRow` 的 `nineKeyDigits`），返回键仍在字母层 123 那一格。 */
  static symbolLeads(row: readonly DesignRowEntry[]): boolean {
    return row.length > 0 && row[0].slot === DesignRowSlot.SYMBOL_PANEL;
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

  /** 底行逗号上滑或下滑输入句号（`CommaSwipeGesture`）：只有长空格样式（中文 26 键与 14/18 键）有，与 Android 长空格布局的逗号滑动相同；其余样式不接滑动。 */
  static commaSwipesPeriod(styled: StyledFace, style: string): boolean {
    return styled !== StyledFace.NONE && style === TouchLayoutStylePolicy.LONG_SPACE;
  }

  /**
   * 中文 26 键是否处在长空格样式（与在不在 123 层无关）；密码框和只收数字的编辑框里不算（`fieldRowStyle`）。这时与 Android 的 `longSpaceLayoutActive` 一样：符号面板入口移到工具栏，工具栏这一行一直画着（设成隐藏时只留标志、符号和收起；这一行留不留由视图按这块键盘的中文键面算，切到英文 26 键时也留着，面板高度不随中/英变）；123 进入带空格的数字符号层而不借用数字九宫格，那一层和 #+= 层左下的 😀/符号 换成同一个逗号（上滑或下滑输入句号）。
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

  /** 这块键盘在中文（`english` 为假）或英文下字母层的键面状态（判定数字行用的 `NumberRowFace`）；九键网格、14/18 键和假名网格跟着方案走，不随中/英变，与在不在 123 层无关。 */
  static letterFace(keyboard: TouchKeyboard, english: boolean): NumberRowFace {
    const rules: number = SchemeCompositionPolicy.selectedRulesScheme(
      keyboard.scheme.engineScheme,
      english,
      keyboard.localMode,
    );
    return {
      kanaGrid: keyboard.kanaGrid,
      handwriting: KeyboardScheme.usesHandwritingFace(
        keyboard.scheme,
        english,
        false,
        keyboard.localMode,
      ),
      nineKeyGrid: KeyboardScheme.usesNineKeyFace(keyboard.nineKey, keyboard.localMode),
      zhuyin: rules === SchemeTraits.ZHUYIN,
      stroke: rules === SchemeTraits.STROKE,
    };
  }

  /**
   * 这块键盘的中文键面是中文 26 键（全拼、26 键双拼、五笔 26 键、粤拼等）：按中文算，没有 `english` 这个参数，同一块键盘停在英文 26 键上时仍为真。长空格样式留不留工具栏那一行只看它和用户的 26 键样式（`longSpaceActive(chineseTwentySixKeyboard(…), …)`），切中/英时面板高度不跳，与 Android `TouchKeyStyle.Styles.keepsToolbarRow` 相同；此刻画着的是不是中文 26 键还要再加上此刻是中文。韩文、越南语、藏文、日语、14/18 键、九键、手写、笔画和大千注音的键盘都不算；触屏之外（2in1）由视图另判。
   */
  static chineseTwentySixKeyboard(keyboard: TouchKeyboard): boolean {
    const engineScheme: string = keyboard.scheme.engineScheme;
    const rules: number = SchemeCompositionPolicy.selectedRulesScheme(
      engineScheme,
      false,
      keyboard.localMode,
    );
    return (
      TouchLayoutStylePolicy.lettersRowFace(TouchLayoutStylePolicy.letterFace(keyboard, false)) &&
      !KeyboardScheme.usesFourteenKeyFace(keyboard.letterGrid, false, keyboard.localMode) &&
      !KoreanCompositionPolicy.selected(engineScheme, false, keyboard.localMode) &&
      rules !== SchemeTraits.VIETNAMESE &&
      rules !== SchemeTraits.TIBETAN &&
      engineScheme !== "japanese"
    );
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
   * 扩展能力建面板时（视图还没出现）定第一帧的高度：工具条画不画、字母上方多不多一行。键面要等视图按方案画出来才知道，所以用视图上次记下的键面（`TouchPanelFace`，没记过时为 null）配上现在的偏好来算，与 Android 冷启动按记住的样式画第一帧同一个意思：上次那块键盘的中文键面是中文 26 键（停在它的英文 26 键上也算）、开着长空格样式时工具栏一直画着；上次停在 26/14/18 键一类、开着数字行且不是手机横屏时多一行。没记过时与加样式之前相同：工具条只看工具栏设置，不多行。视图出现后仍按同一套规则（`stripShown`、`extraRows`）核对一次，键面与记下的不同时再改高度。
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

  /**
   * 设计底行逐个要画的键，次序就是 `row` 的次序。底行表在视图里每次重建这一行只算一次，交给 ForEach；份额和让位按值传给每个键的 @Builder，所以都放进 ForEach 的键（`designRowItemKey`）：切中/英、改样式让哪个键的份额或让位变了，ForEach 就重建那一个键，其余键留着。
   */
  static designRowItems(row: readonly DesignRowEntry[]): DesignRowItem[] {
    const beside: boolean = TouchLayoutStylePolicy.languageBesideReturn(row);
    const items: DesignRowItem[] = [];
    for (const item of row) {
      const yields: boolean =
        item.slot === DesignRowSlot.LANGUAGE || item.slot === DesignRowSlot.RETURN;
      items.push({ slot: item.slot, share: item.share, besideReturn: beside && yields });
    }
    return items;
  }

  /** `designRowItems` 每一项在 ForEach 里的键：键位、份额和让位，按值传给键的 @Builder 的量一个不缺。 */
  static designRowItemKey(item: DesignRowItem): string {
    return `${item.slot}:${item.share}:${item.besideReturn}`;
  }

  private static known(value: unknown, styles: readonly string[]): string {
    return typeof value === "string" && styles.includes(value)
      ? value
      : TouchLayoutStylePolicy.STANDARD;
  }
}

/**
 * 视图请求的手机面板高度（`KeyboardSession.resizeTouchKeyboard`）交给持有面板的扩展能力。冷启动时 `setUiContent` 与视图的 `aboutToAppear` 谁先完成没有保证，视图可能在扩展能力挂上回调之前就按真实键面请求高度（记下的键面已经过时，例如键盘进程没开时在设置里换了方案）；这时请求先留着，扩展能力给第一帧定高度时优先用它（`takePending`），挂上回调时（`attach`）再补发这之间到达的请求，面板不会停在按过时键面定的高度上。
 */
export class TouchHeightRelay {
  private target: ((heightVp: number) => void) | null = null;
  private pending: number | null = null;

  /** 视图请求一个总高度：回调已挂上就立刻交给它，否则留着最近的一次。 */
  request(heightVp: number): void {
    if (this.target !== null) {
      this.target(heightVp);
      return;
    }
    this.pending = heightVp;
  }

  /** 取走还没送达的请求（取过即清）；没有时为 null。 */
  takePending(): number | null {
    const pending: number | null = this.pending;
    this.pending = null;
    return pending;
  }

  /** 扩展能力挂上改面板高度的回调；之前留着的请求立刻补发。 */
  attach(target: (heightVp: number) => void): void {
    this.target = target;
    const pending: number | null = this.takePending();
    if (pending !== null) {
      target(pending);
    }
  }

  /** 面板释放：回调不再可用，留着的请求也作废，下一个面板从自己的视图重新收请求。 */
  detach(): void {
    this.target = null;
    this.pending = null;
  }
}

/**
 * 长空格样式下逗号键的滑动，与 Android #10087 的 `ImeBottomRow.bindCommaSwipe` 相同：上滑、下滑都输入句号（只认下滑时，照字母键滑动输入符号的默认方向上滑什么也不出；逗号键没有别的纵向手势），门槛与字母键相同，纵向移动严格大于 `THRESHOLD_VP` 且大于横向才算，点按时手指带一下不到门槛不算。滑过门槛的那一刻就定下这次是滑动，抬起时输入句号，这次按键的普通点按被吞掉。ArkUI 先送触摸抬起再送点击，所以由视图在点击里问 `swallows()`；下一次按下时清掉，点击没有送来也不会吞掉之后的点按。按下后满 `HOLD_MILLIS` 还没滑出去的触摸归长按，之后挪动手指不再起滑动；长按识别时视图问 `longPress()`，已经滑出去了长按就不再做事（Android 滑出去后取消长按），否则这次触摸不再按滑动处理。时间由视图传入，单位毫秒。
 */
export class CommaSwipeGesture {
  /** 字母键滑动输入符号的门槛（`SwipeHintPolicy.THRESHOLD_VP`）。 */
  static readonly THRESHOLD_VP: number = SwipeHintPolicy.THRESHOLD_VP;
  /** 长按时长：平台 LongPressGesture 的默认值，与字母键的 `LetterHintGesture.HOLD_MILLIS` 相同。 */
  static readonly HOLD_MILLIS: number = LetterHintGesture.HOLD_MILLIS;

  private downX: number = 0;
  private downY: number = 0;
  private downTime: number = 0;
  private tracking: boolean = false;
  private swiped: boolean = false;
  private swallow: boolean = false;

  down(x: number, y: number, time: number): void {
    this.downX = x;
    this.downY = y;
    this.downTime = time;
    this.tracking = true;
    this.swiped = false;
    this.swallow = false;
  }

  /** 手指移动：还没滑出去时，满长按时长就不再按滑动处理，否则看这一刻是否滑过门槛。 */
  move(x: number, y: number, time: number): void {
    if (!this.tracking || this.swiped) {
      return;
    }
    if (time - this.downTime >= CommaSwipeGesture.HOLD_MILLIS) {
      this.tracking = false;
      return;
    }
    const dy: number = Math.abs(y - this.downY);
    this.swiped = dy > CommaSwipeGesture.THRESHOLD_VP && dy > Math.abs(x - this.downX);
  }

  /** 手指抬起；这次是滑动就返回 true（由视图输入句号），否则返回 false，由点按输入逗号。 */
  up(x: number, y: number, time: number): boolean {
    this.move(x, y, time);
    const swiped: boolean = this.tracking && this.swiped;
    this.tracking = false;
    this.swiped = false;
    this.swallow = swiped;
    return swiped;
  }

  /** 长按识别：这次触摸已经滑出去就返回 false，长按什么也不做；否则返回 true，这次触摸归长按，抬起不再按滑动处理。 */
  longPress(): boolean {
    if (this.tracking && this.swiped) {
      return false;
    }
    this.tracking = false;
    return true;
  }

  /** 触摸被取消，或这时画着的逗号不接滑动：这次触摸不再按滑动处理。 */
  cancel(): void {
    this.tracking = false;
    this.swiped = false;
  }

  /** 点按到达时问一次：刚才那次触摸已经按滑动输入了句号，这次点按不再输入逗号。问过即清。 */
  swallows(): boolean {
    const swallowed: boolean = this.swallow;
    this.swallow = false;
    return swallowed;
  }
}
