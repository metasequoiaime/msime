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

/** 标点控件显示的有效状态；只补齐缺省值，不改写用户偏好。 */
export function punctuationControlState(
  preferences: PunctuationPreferences,
): Required<PunctuationPreferences> {
  return {
    chinese_punctuation: preferences.chinese_punctuation,
    character_width: preferences.character_width ?? "halfwidth",
    smart_punctuation: preferences.smart_punctuation ?? false,
    smart_punctuation_repeat: preferences.smart_punctuation_repeat ?? false,
    smart_punctuation_space_convert: preferences.smart_punctuation_space_convert ?? false,
    smart_punctuation_direct_digit: preferences.smart_punctuation_direct_digit ?? false,
    smart_punctuation_direct_letter: preferences.smart_punctuation_direct_letter ?? false,
    paired_punctuation: preferences.paired_punctuation ?? true,
    punctuation_lock: preferences.punctuation_lock ?? "follow",
    caps_lock_ascii_punctuation: preferences.caps_lock_ascii_punctuation ?? false,
    quote_style: preferences.quote_style ?? "curly",
    bracket_style: preferences.bracket_style ?? "solid",
  };
}
