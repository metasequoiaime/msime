import type { CloudCandidateProviderPreferences } from "../settings/cloud-candidates-section";
import type { InputScheme, ChineseScheme } from "./input-scheme";
import type { PreeditPreferences } from "../settings/preedit-preferences";
import type {
  CandidateFontPreferences,
  CandidateSizingPreferences,
  CandidateWindowStylePreferences,
} from "../candidate/candidate-preferences";
import type { PunctuationPreferences } from "../settings/punctuation-preferences";
import type { KeybindingPreferences } from "../settings/keybinding-preferences";
import type { WubiPreferences } from "../settings/wubi-preferences";
import type { VietnamesePreferences } from "../settings/vietnamese-preferences";
import type { KoreanPreferences } from "../settings/korean-preferences";
import type { FloatingToolbarPreferences } from "../settings/floating-toolbar-preferences";
import type { TraditionalChineseVariant } from "../settings/chinese-output-preferences";
import type {
  BuiltinShuangpinProfile,
  ShuangpinCustomProfile,
} from "../settings/shuangpin-custom-profile";
import type { AppInputModeRule } from "../settings/app-input-mode-rules";
import type { CloudClipboardRelayPreferences } from "../settings/cloud-clipboard-relay-preferences";
import type { VoiceInputPreferences } from "../settings/voice-input-preferences";
import type { NetworkProxyPreferences } from "../settings/network-proxy-preferences";
import type { TouchKeyboardSchemePreferences } from "../settings/touch-keyboard-scheme-helpers";
import type { CustomTheme, GlobalTheme } from "../theme/global-theme";
import type { PluginPreferences } from "../settings/plugin-preferences";
import type { HelpcodePreferences } from "../settings/helpcode-preferences";
import type { FuzzyPinyinPreferences } from "../settings/fuzzy-pinyin-preferences";
import type { TranslationLanguage } from "../settings/translation-language-helpers";
import type {
  NavigationPreferences,
  WordCharacterPreferences,
} from "../settings/candidate-paging-preferences";
import type { SentenceAssociationPreferences } from "../settings/sentence-association-preferences";
import type { MixedInputPreferences } from "../settings/mixed-input-preferences";
import type { FrequencyPreferences } from "../settings/frequency-preferences";
import type { AiAssistantPreferences } from "../settings/ai-assistant-preferences";
import type { LocalModePreferences } from "../settings/local-mode-preferences";
import type { ThemeMode, ThemePreferences } from "../settings/theme-preferences";
import type {
  HarmonyToolbarPreferences,
  TouchToolbarPreferences,
} from "../settings/touch-keyboard-toolbar-preferences";

export type Preferences = PunctuationPreferences &
  PreeditPreferences &
  WubiPreferences &
  CandidateFontPreferences &
  CandidateSizingPreferences &
  CandidateWindowStylePreferences &
  ThemePreferences & {
    theme?: ThemeMode;
    ai_assistant?: AiAssistantPreferences;
    sentence_association?: SentenceAssociationPreferences;
    custom_translation?: { enabled: boolean; endpoint: string; api_key: string };
    tencent_tmt?: { enabled: boolean; secret_id: string; secret_key: string; region: string };
    niutrans?: { enabled: boolean; app_id: string; apikey: string };
    voice_input?: VoiceInputPreferences;
    local_modes?: LocalModePreferences;
    clipboard_history?: boolean;
    clipboard_history_limit?: number;
    /** 自建云剪贴板中继的地址和令牌；缺省或地址为空表示走水杉账号。两项要么都空，要么都合规，见 `crates/client-core/src/preferences.rs` 的 `CloudClipboardRelayPreferences`。 */
    cloud_clipboard_relay?: CloudClipboardRelayPreferences;
    cloud_candidates?: boolean;
    cloud_candidate_provider?: CloudCandidateProviderPreferences;
    /** 云候选、翻译、AI、语音、账号、更新检查和下载共用的代理设置；本机与局域网目标一律直连。 */
    network?: NetworkProxyPreferences;
    candidate_translations?: boolean;
    candidate_english_gloss?: boolean;
    candidate_pronunciation?: boolean;
    /** 候选译文中显示 n.、v. 等词性标记；缺省为开启（对齐 Windows v0.11.1 出厂配置）。 */
    candidate_translation_show_pos?: boolean;
    /** 候选译文只显示第一条释义；缺省为关闭。 */
    candidate_translation_single_sense?: boolean;
    /** 固定排位的候选词是否显示角标；缺省为开启（对齐 Windows v0.11.1 出厂配置）。 */
    candidate_fixed_badge?: boolean;
    /** 固定排位候选角的样式；缺省为 `paperclip`。 */
    candidate_fixed_badge_style?: "paperclip" | "pushpin" | "dot";
    /** AI 联想候选的来源角标样式；缺省为 `robot`（🤖），另一档是 `whale`（🐳）。 */
    candidate_ai_badge_style?: "robot" | "whale";
    english_suggestions?: boolean;
    english_online_candidates?: boolean;
    translation_target_language?: TranslationLanguage;
    /** 可选的第二种候选翻译语言；null 或缺省表示只显示一行释义。 */
    translation_secondary_language?: TranslationLanguage | null;
    /** 用户明确选择水杉账号（api.msime.app）进行候选翻译；缺省表示未选择。 */
    translation_account?: boolean;
    translation_ai?: boolean;
    /** 各宿主共用的匿名用量上报（每日活跃、会话结束与崩溃摘要）；缺省为开启。 */
    usage_reporting?: boolean;
    floating_toolbar?: FloatingToolbarPreferences;
    mixed_input?: MixedInputPreferences;
    fuzzy_pinyin?: FuzzyPinyinPreferences;
    frequency?: FrequencyPreferences;
    word_character?: WordCharacterPreferences;
    /** 组字时 `;` 选第二个候选、`'` 选第三个。关闭时不写进文档；共享运行时在 `candidate_shortcut` 里接管，本页所有承载宿主（macOS、Linux、HarmonyOS）与 Windows 原生设置页都生效。 */
    second_third_candidate?: { enabled: boolean; keys: "semicolon_quote" };
    /** 退格在剩余拼写即将删空时回退最近一次选词；缺省为开启。 */
    backspace_reverts_selection?: boolean;
    /** 造词时按 Esc 只清除还没选的拼音、保留已选的词，再按一次才整体取消；Windows 出厂缺省为关闭。 */
    escape_keeps_selected_word?: boolean;
    navigation?: NavigationPreferences;
    keybindings?: KeybindingPreferences;
    scheme: InputScheme;
    /** 触屏键盘的布局；`fourteen_key` 是全拼 14 键，`nine_key_shuangpin` 是九键双拼，都只配全拼。 */
    touch_keyboard_layout?:
      | "twenty_six_key"
      | "nine_key"
      | "handwriting"
      | "fourteen_key"
      | "nine_key_shuangpin";
    /** Android 双拼的 26 / 14 / 18 键、iOS 双拼的 26 / 14 键选择；缺省为 26 键，与全拼布局独立保存。 */
    touch_shuangpin_layout?: "twenty_six_key" | "fourteen_key" | "eighteen_key";
    touch_keyboard_schemes?: TouchKeyboardSchemePreferences;
    touch_key_spacing_tenths?: number;
    touch_row_spacing_tenths?: number;
    touch_keyboard_height_adjustment?: number;
    touch_voice_shortcut?: boolean;
    /** 九宫格数字层的排列：电话（1 2 3 在上）或计算器（7 8 9 在上）。 */
    touch_number_keypad_order?: "phone" | "calculator";
    /** Android 文本编辑面板的七个侧边键；其他宿主保留该顺序。 */
    touch_text_edit_button_order?: (
      | "copy"
      | "paste"
      | "select_all"
      | "cut"
      | "space"
      | "number_pad"
      | "delete"
    )[];
    /** 26 键按「123」时的数字层：一行 1 到 0，或九宫格那样的 3×3 数字键。 */
    touch_twenty_six_key_number_layout?: "row" | "nine_key";
    /** 触屏 26 键双拼时在字母键底部画声母/韵母提示；缺省为关。 */
    touch_shuangpin_key_hints?: boolean;
    /** 触屏键盘的拼写写进输入框还是只画在候选栏；缺省为输入框。与桌面候选窗的 `candidate_preedit_style` 无关。 */
    touch_spelling_position?: "editor" | "candidate_bar";
    touch_toolbar?: Partial<TouchToolbarPreferences>;
    /** HarmonyOS 手机键盘的工具栏工具和显示方式；等于缺省时 client-core 不写进文档。 */
    harmony_toolbar?: Partial<HarmonyToolbarPreferences>;
    default_ime_mode?: "chinese" | "english";
    ime_mode_scope?: "app" | "global";
    /** 应用例外：键是应用标识（macOS 的 bundle id、Windows 的进程基名），值是切到该应用时的起始模式。没有规则时文档里没有这个键。 */
    app_input_mode_rules?: Record<string, AppInputModeRule>;
    /** Vim 模式应用名单（macOS）：名单里的前台应用收到无修饰 Esc 时，输入法同时切到英文。标识口径同 `app_input_mode_rules`；空表不写进文档。 */
    vim_mode_apps?: string[];
    last_chinese_scheme?: ChineseScheme | null;
    /** `custom` 用下面的 `shuangpin_custom_profile`；表不能用时输入法按小鹤运行。 */
    shuangpin_profile: BuiltinShuangpinProfile | "custom";
    /** 自定义双拼的键位表。选别的方案时保留，切回 `custom` 还能用；空表不写进文档。 */
    shuangpin_custom_profile?: ShuangpinCustomProfile;
    /** 九键双拼用哪张表，只有触屏宿主用；等于默认值 `migala3` 时文档里没有这一项。 */
    nine_key_shuangpin_profile?: "migala2" | "migala3" | "gushi2_low" | "custom";
    /** 九键双拼的自定义表，形状同 `shuangpin_custom_profile`，键是数字；空表不写进文档。 */
    nine_key_shuangpin_custom_profile?: ShuangpinCustomProfile;
    /** 五笔用 86 还是 98 码表，只在方案为五笔时起作用；缺省为 86。 */
    wubi_profile?: "wubi86" | "wubi98";
    /** 「输入时显示双拼键位提示」。没选过时文档里没有这一项，按关显示；macOS 那时仍沿用本机旧的选择。 */
    shuangpin_keymap_hint?: boolean;
    vietnamese?: VietnamesePreferences;
    korean?: KoreanPreferences;
    /** 只出单字：全拼、双拼、五笔和粤拼的候选只留单个汉字；缺省为关。 */
    single_character_only?: boolean;
    candidate_page_size: number;
    number_row_selection?: boolean;
    candidate_follow_cursor?: boolean;
    /** 切换中英文后在光标旁短暂显示的徽标（macOS、Windows、Linux 和鸿蒙 2in1）。 */
    input_mode_hud?: boolean;
    /** Linux 聚焦输入框时提示当前模式，独立于切换提示，缺省关闭。 */
    input_mode_focus_hud?: boolean;
    /** Linux 允许屏幕角落的附加模式提示，缺省开启；关闭后保留光标旁提示。 */
    input_mode_corner_hud?: boolean;
    /** macOS 菜单栏里常驻显示当前输入模式（中/双/五/日……，英文时为「英」）的状态项，默认关闭。 */
    input_mode_status_item?: boolean;
    candidate_layout?: "horizontal" | "vertical";
    show_candidate_page_number?: boolean;
    /** 候选窗和悬浮工具栏左端的水杉 logo。新装默认隐藏，旧文档缺这个字段时读成显示。 */
    show_app_logo?: boolean;
    /** 「展开候选」：横排候选窗按 Tab、↓ 展开成多行候选网格，默认关闭。 */
    expand_candidates?: boolean;
    /** 展开按钮独立于页码显示，缺字段时默认开启。 */
    show_candidate_expand_button?: boolean;
    /** 候选窗、悬浮工具栏、菜单和触屏键盘共用的主题；`theme` 仍表示 `system` 和设置窗口跟随的明暗模式。 */
    global_theme?: GlobalTheme;
    /** `custom` 全局主题由外部候选皮肤包、七个候选颜色选项与键盘编辑器设计组成。 */
    custom_theme?: CustomTheme;
    learning: boolean;
    /** 回车上屏英文时把该单词写入英文词库；Windows 出厂缺省为开启。 */
    enter_learns_english_word?: boolean;
    diagnostic_log?: { server?: boolean; tsf?: boolean };
    /** 只有 Windows TSF 读取：游戏声明自己画候选却不画时由水杉的候选窗显示。由 Windows 原生设置页编辑，这个页面没有对应控件；缺省为开启、两张进程表为空。 */
    game_compatibility?: {
      candidate_overlay?: boolean;
      overlay_processes?: string[];
      excluded_processes?: string[];
    };
    quanpin?: {
      autocorrect_transposition?: boolean;
      autocorrect_neighbor?: boolean;
    };
    quanpin_helpcode?: HelpcodePreferences;
    shuangpin_helpcode?: HelpcodePreferences;
    traditional_chinese_output?: boolean;
    /** 繁体输出用的 OpenCC 配置：标准繁体、台湾正体、台湾正体加惯用词。缺省为 `s2t`，等于缺省时文档里没有这个键。 */
    traditional_chinese_variant?: TraditionalChineseVariant;
    /** 中英文之间加空格：上屏文本里汉字与 ASCII 字母、数字相邻处加一个半角空格。缺省为关，关着时文档里没有这个键。 */
    chinese_english_spacing?: boolean;
    /** 跨次上屏的中英文边界补空格，独立开关、缺省关闭；目前仅 macOS 接入。 */
    cross_commit_spacing?: boolean;
    cross_commit_spacing_excluded_apps?: string[];
    /** 提示音包、音乐和指令表；全部取默认值时文档省略这一段。 */
    plugins?: PluginPreferences;
  };
export type ExternalSkinCatalog = {
  scanned: boolean;
  directory?: string;
  revision?: number;
  packages: Array<{ id: string; title: string; description?: string; valid?: boolean }>;
  issues?: string[];
};
export type Snapshot = {
  format_version: number;
  revision: number;
  preferences: Preferences;
  candidate_skin_catalog?: ExternalSkinCatalog;
};
