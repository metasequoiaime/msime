export type PunctuationPreferences = {
  chinese_punctuation: boolean;
  /** 桌面宿主提交可打印 ASCII 字符时使用的宽度。 */
  character_width?: "halfwidth" | "fullwidth";
  smart_punctuation?: boolean;
  smart_punctuation_repeat?: boolean;
  smart_punctuation_space_convert?: boolean;
  smart_punctuation_direct_digit?: boolean;
  smart_punctuation_direct_letter?: boolean;
  paired_punctuation?: boolean;
  punctuation_lock?: "follow" | "chinese" | "english";
  caps_lock_ascii_punctuation?: boolean;
  /** 中文标点下引号键的样式；缺省（文档里没有这个键）是弯引号。 */
  quote_style?: QuoteStyle;
  /** 中文标点下方头括号键的样式；缺省是实心。 */
  bracket_style?: BracketStyle;
};

export type QuoteStyle = "curly" | "corner" | "corner_when_traditional";
export type BracketStyle = "solid" | "hollow";
