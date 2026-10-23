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
  /** 顿号自定义：中文标点下 `\` 键输出顿号「、」；缺省开。 */
  enumeration_comma_backslash?: boolean;
  /** 顿号自定义：中文标点下 `/` 键输出顿号「、」；缺省关。词首的 `/` 仍打开命令模式。 */
  enumeration_comma_slash?: boolean;
};

export type QuoteStyle = "curly" | "corner" | "corner_when_traditional";

/** 按界面显示的归一状态一起保存两个顿号键，避免手改的双关闭文档在点击后撤掉仍显示为开的键。 */
export function enumerationCommaPatch(
  preferences: PunctuationPreferences,
  key: "enumeration_comma_backslash" | "enumeration_comma_slash",
  checked: boolean,
): Partial<PunctuationPreferences> {
  const state = punctuationControlState(preferences);
  return {
    enumeration_comma_backslash: state.enumeration_comma_backslash,
    enumeration_comma_slash: state.enumeration_comma_slash,
    [key]: checked,
  };
}
export type BracketStyle = "solid" | "hollow";

/** 标点控件显示的有效状态；只补齐缺省值，不改写用户偏好。 */
export function punctuationControlState(
  preferences: PunctuationPreferences,
): Required<PunctuationPreferences> {
  return {
    chinese_punctuation: preferences.chinese_punctuation,
    character_width: preferences.character_width ?? "halfwidth",
    // 智能标点、重复标点转中文和两种直出在所有平台上默认开，与 client-core 的 `smart_punctuation_default` 相同；空格转换默认关。
    smart_punctuation: preferences.smart_punctuation ?? true,
    smart_punctuation_repeat: preferences.smart_punctuation_repeat ?? true,
    smart_punctuation_space_convert: preferences.smart_punctuation_space_convert ?? false,
    smart_punctuation_direct_digit: preferences.smart_punctuation_direct_digit ?? true,
    smart_punctuation_direct_letter: preferences.smart_punctuation_direct_letter ?? true,
    paired_punctuation: preferences.paired_punctuation ?? true,
    punctuation_lock: preferences.punctuation_lock ?? "follow",
    caps_lock_ascii_punctuation: preferences.caps_lock_ascii_punctuation ?? false,
    quote_style: preferences.quote_style ?? "curly",
    bracket_style: preferences.bracket_style ?? "solid",
    // 「至少留一个」的显示归一，与 client-core `punctuation_marks` 的读出归一一致：两个都关按「\ 开」显示。
    enumeration_comma_slash: preferences.enumeration_comma_slash ?? false,
    enumeration_comma_backslash:
      (preferences.enumeration_comma_backslash ?? true) ||
      !(preferences.enumeration_comma_slash ?? false),
  };
}

/** 「标点」选择的四个取值，由 `chinese_punctuation` 和 `punctuation_lock` 两个键合起来表达：前两个是跟随中英文状态时中文模式的起始标点（可用快捷键或工具栏临时切换），后两个是锁定，切换不再改变标点。 */
export type PunctuationMode = "chinese" | "english" | "always_chinese" | "always_english";

/** 从两个已存的键推出「标点」选择显示的值，只读不写：锁定优先，因为锁定时宿主不看 `chinese_punctuation`（`engine_chinese_punctuation`、`InputController` 的 `punctuationLock` 分支）。 */
export function punctuationMode(preferences: PunctuationPreferences): PunctuationMode {
  const { chinese_punctuation, punctuation_lock } = punctuationControlState(preferences);
  if (punctuation_lock === "chinese") return "always_chinese";
  if (punctuation_lock === "english") return "always_english";
  return chinese_punctuation ? "chinese" : "english";
}

/** 用户在「标点」选择里选定一项时写入的补丁。前两项写起始标点并解除锁定；「始终」两项只写锁定，不碰 `chinese_punctuation`：起始标点会上云（`settings_sync` 的 `input.chinese_punctuation`），锁定不上云，只写锁定才不会顺带改掉其他设备的起始标点。锁定时宿主不看起始标点（`engine_chinese_punctuation`，macOS 的 `runtimeChinesePunctuation`），显示也是锁定优先，所以两键不一致不影响行为。 */
export function punctuationModePatch(
  mode: PunctuationMode,
): Partial<Pick<PunctuationPreferences, "chinese_punctuation" | "punctuation_lock">> {
  switch (mode) {
    case "chinese":
      return { chinese_punctuation: true, punctuation_lock: "follow" };
    case "english":
      return { chinese_punctuation: false, punctuation_lock: "follow" };
    case "always_chinese":
      return { punctuation_lock: "chinese" };
    case "always_english":
      return { punctuation_lock: "english" };
  }
}

/** 用户切换「智能标点」时写入的补丁。数字后直出和字母后直出是智能标点真正起作用的两半（`client-core` 的 `punctuation::route` 要其中之一为真才走 ASCII），所以总开关被用户切换时两半跟着写成同一个值；打开页面、切换别的开关都不碰它们，单独在「更多选项」里调过的值只有用户再切总开关时才会被覆盖。 */
export function smartPunctuationPatch(
  enabled: boolean,
): Pick<
  Required<PunctuationPreferences>,
  "smart_punctuation" | "smart_punctuation_direct_digit" | "smart_punctuation_direct_letter"
> {
  return {
    smart_punctuation: enabled,
    smart_punctuation_direct_digit: enabled,
    smart_punctuation_direct_letter: enabled,
  };
}
