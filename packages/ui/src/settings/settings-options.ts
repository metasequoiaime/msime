import type {
  TouchKeyboardScheme,
  Preferences,
  AiAssistantPreferences,
  VoiceInputPreferences,
  LocalDictionaryKind,
  NavigationPreferences,
  HostCapabilities,
} from "../index";
import type { ThemeCatalogEntry } from "../theme/global-theme";
export { logo } from "./app-resources";

// Options and defaults that the settings model in index.tsx shares with the settings pages, or that several pages share with each other.

export const touchKeyboardSchemeOptions: [TouchKeyboardScheme, string][] = [
  ["quanpin", "全拼 26 键"],
  ["nine_key", "全拼 9 键"],
  ["xiaohe", "小鹤双拼"],
  ["ziranma", "自然码双拼"],
  ["microsoft", "微软双拼"],
  ["shoudao", "首道双拼"],
  ["wubi", "86 五笔"],
  ["japanese_nine_key", "日语 9 键"],
  ["japanese", "日语 26 键"],
  ["handwriting", "手写"],
  ["thoughtful_reply", "高情商回复"],
];

export const allTouchKeyboardSchemes = touchKeyboardSchemeOptions.map(([scheme]) => scheme);

export function selectTouchKeyboardScheme(
  preferences: Preferences,
  selected: TouchKeyboardScheme,
): Preferences {
  const touch_keyboard_schemes = {
    enabled: preferences.touch_keyboard_schemes?.enabled ?? allTouchKeyboardSchemes,
    selected,
  };
  if (["xiaohe", "ziranma", "microsoft", "shoudao"].includes(selected))
    return {
      ...preferences,
      scheme: "shuangpin",
      last_chinese_scheme: "shuangpin",
      shuangpin_profile: selected as Preferences["shuangpin_profile"],
      touch_keyboard_layout: "twenty_six_key",
      touch_keyboard_schemes,
    };
  if (selected === "japanese" || selected === "japanese_nine_key")
    return {
      ...preferences,
      scheme: "japanese",
      last_chinese_scheme:
        preferences.scheme === "japanese" ? preferences.last_chinese_scheme : preferences.scheme,
      touch_keyboard_layout: selected === "japanese_nine_key" ? "nine_key" : "twenty_six_key",
      touch_keyboard_schemes,
    };
  if (selected === "wubi")
    return {
      ...preferences,
      scheme: "wubi",
      last_chinese_scheme: "wubi",
      touch_keyboard_layout: "twenty_six_key",
      touch_keyboard_schemes,
    };
  return {
    ...preferences,
    scheme: "quanpin",
    last_chinese_scheme: "quanpin",
    touch_keyboard_layout:
      selected === "nine_key"
        ? "nine_key"
        : selected === "handwriting"
          ? "handwriting"
          : "twenty_six_key",
    touch_keyboard_schemes,
  };
}

// The prompt slots start empty as in AiAssistantPreferences::default(). The associative-candidate paths (client-core's chat_completion_http_request and the Linux online provider) treat a blank slot as the built-in DEFAULT_CANDIDATE_PROMPT, the text Windows compiles into ai_assistant.prompt. Android and the iOS keyboard mirror instead read ai_assistant.prompt as a polish instruction and use their own polish text when it is blank, which is why this default stays empty rather than holding the associative prompt.
export const defaultAiAssistant: AiAssistantPreferences = {
  enabled: false,
  provider: "deepseek",
  model: "deepseek-v4-flash",
  endpoint: "https://api.deepseek.com/chat/completions",
  candidate_limit: 3,
  token: "",
  tokens: {},
  prompt_id: "custom_1",
  prompt: "",
  prompt_custom_1: "",
  prompt_custom_2: "",
  prompt_custom_3: "",
};

// asr_provider mirrors client-core's default; the two disagreeing meant a host wrote a provider no backend implements.
export const defaultVoiceInput: VoiceInputPreferences = {
  enabled: true,
  language: "zh-CN",
  asr_provider: "doubao",
  doubao_auth_mode: "api_key",
  asr_resource_id: "volc.seedasr.sauc.duration",
};

export const localDictionaryKinds: [LocalDictionaryKind, string][] = [
  ["pinyin", "全拼"],
  ["wubi", "五笔"],
  ["english", "英文"],
  ["quick_phrase", "快捷短语"],
];

export const defaultNavigation: NavigationPreferences = {
  minus_equal: true,
  comma_period: true,
  brackets: false,
  tab: true,
  page_up_down: true,
  arrows: true,
};

// The Linux hosts do not draw the candidate list themselves; when the desktop panel that does ignores these settings, the host says why (HostCapabilities.candidate_panel_limit) and the appearance and skin pages say so once.
export const candidatePanelLimitNotes: Record<
  NonNullable<HostCapabilities["candidate_panel_limit"]>,
  string
> = {
  gnome_shell:
    "GNOME Shell 自己绘制 IBus 候选窗并跟随 Shell 主题，这里的候选字体、颜色和皮肤在当前桌面不会生效。",
  fcitx_theme:
    "Fcitx5 正在使用你在 Fcitx5 配置中选择的经典界面主题，这里的候选颜色和皮肤不会覆盖它；字体仍然生效。改回 Fcitx5 默认主题后即可使用这里的设置。",
  kimpanel:
    "Fcitx5 的候选窗由桌面的 Kimpanel 绘制，使用桌面自己的字体和主题，这里的候选字体、颜色和皮肤不会生效。",
};

// The last column is the description on a host whose skin reaches only the candidate window (Linux presents the toolbar as an input method menu).
/** A theme card's one-line description. `candidateOnly` is for hosts without a floating toolbar. */
export function globalThemeDescription(entry: ThemeCatalogEntry, candidateOnly = false): string {
  if (entry.id === "system")
    return candidateOnly
      ? "候选窗与键盘使用平台自带配色"
      : "候选窗、悬浮工具栏与键盘使用平台自带配色";
  if (entry.id === "custom") return "外部皮肤、候选颜色与自定义键盘";
  const tone = entry.appearance === "dark" ? "深色" : "浅色";
  return candidateOnly ? `${tone}候选窗与键盘` : `${tone}候选窗、悬浮工具栏与键盘`;
}
