//! The preferences an agent may see and change.
//!
//! An allowlist rather than the whole document: the document also holds API keys, account tokens and endpoints, none of which an agent should read, and most of its fields are choices only the settings page can present properly. The enums are mirrored here so the tool schema names exactly the values the store accepts; `the_mirrors_serialize_as_the_store_does` keeps the two in step.

use msime_client_core::edition::Edition;
use msime_client_core::preferences::{
    AppInputModeRule, CandidateLayout, CharacterWidthPreference, ChineseScheme, DefaultImeMode,
    InputScheme, NineKeyShuangpinProfile, Preferences, PreferencesSnapshot, PreferencesStore,
    ShuangpinCustomProfile, ShuangpinProfile, WubiProfile,
};
use msime_client_core::theme::{GlobalTheme, ThemeAppearance};
use msime_client_services::skin::{catalog, theme::skin_appearance};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum Scheme {
    Quanpin,
    Shuangpin,
    Wubi,
    Japanese,
    Korean,
    Cantonese,
    Zhuyin,
    Vietnamese,
    Tibetan,
    Stroke,
}

/// 代理可以切换到的方案。日文、韩文、粤拼、注音、越南文、藏文和笔画留给用户自己切换：日文、粤拼、注音和笔画需要各自的词库，宿主不一定装了；而且这几个方案都需要一条代理看不到的退路。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum ChineseSchemeChoice {
    Quanpin,
    Shuangpin,
    Wubi,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    Xiaohe,
    Ziranma,
    Shoudao,
    Microsoft,
    SmartAbc,
    /// 用户自己的键位表，即 `shuangpin_custom_profile`。没有可用的表时不能选它。
    Custom,
}

/// 自定义双拼方案的键位表。`initials` 只写 zh ch sh 三个多字母声母（单字母声母仍在自己的字母键上），`finals` 写全 33 个韵母（a o e i u v ai ei ui ao ou iu ie ve ue an en in un ang eng ing ong ia ua uo uai ian uan iao iang uang iong），`zero_initials` 给 12 个零声母音节（a ai an ang ao e ei en eng er o ou）各一个两键编码。键是小写字母或 `;`，`;` 只能做第二个键；ü 写作 v。两个音节不能落在同一个两键编码上。
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct CustomShuangpinTable {
    #[serde(default)]
    pub initials: BTreeMap<String, String>,
    #[serde(default)]
    pub finals: BTreeMap<String, String>,
    #[serde(default)]
    pub zero_initials: BTreeMap<String, String>,
}

impl From<&ShuangpinCustomProfile> for CustomShuangpinTable {
    fn from(value: &ShuangpinCustomProfile) -> Self {
        Self {
            initials: value.initials.clone(),
            finals: value.finals.clone(),
            zero_initials: value.zero_initials.clone(),
        }
    }
}

impl From<&CustomShuangpinTable> for ShuangpinCustomProfile {
    fn from(value: &CustomShuangpinTable) -> Self {
        Self {
            initials: value.initials.clone(),
            finals: value.finals.clone(),
            zero_initials: value.zero_initials.clone(),
        }
    }
}

/// 触屏九键双拼（手机键盘的「九键双拼」布局）用哪张表。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum NineKeyProfile {
    /// 米旮旯 2。
    Migala2,
    /// 米旮旯 3。
    Migala3,
    /// 辜氏 2 低重码。
    Gushi2Low,
    /// 用户自己的表，即 `nine_key_shuangpin_custom_profile`。没有可用的表时不能选它。
    Custom,
}

impl From<NineKeyShuangpinProfile> for NineKeyProfile {
    fn from(value: NineKeyShuangpinProfile) -> Self {
        match value {
            NineKeyShuangpinProfile::Migala2 => Self::Migala2,
            NineKeyShuangpinProfile::Migala3 => Self::Migala3,
            NineKeyShuangpinProfile::Gushi2Low => Self::Gushi2Low,
            NineKeyShuangpinProfile::Custom => Self::Custom,
        }
    }
}

impl From<NineKeyProfile> for NineKeyShuangpinProfile {
    fn from(value: NineKeyProfile) -> Self {
        match value {
            NineKeyProfile::Migala2 => Self::Migala2,
            NineKeyProfile::Migala3 => Self::Migala3,
            NineKeyProfile::Gushi2Low => Self::Gushi2Low,
            NineKeyProfile::Custom => Self::Custom,
        }
    }
}

/// 九键双拼的自定义表：每个音节两位数字，第一位是声母所在的键，第二位是韵母所在的键。`initials` 写全 23 个声母（b p m f d t n l g k h j q x r z c s y w zh ch sh），`finals` 写全 33 个韵母（同 26 键自定义表），各一个数字；`zero_initials` 给 12 个零声母音节（a ai an ang ao e ei en eng er o ou）各两位数字。数字是 0 到 9；ü 写作 v。一个两位编码可以对应好几个音节。
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct NineKeyShuangpinTable {
    #[serde(default)]
    pub initials: BTreeMap<String, String>,
    #[serde(default)]
    pub finals: BTreeMap<String, String>,
    #[serde(default)]
    pub zero_initials: BTreeMap<String, String>,
}

impl From<&ShuangpinCustomProfile> for NineKeyShuangpinTable {
    fn from(value: &ShuangpinCustomProfile) -> Self {
        Self {
            initials: value.initials.clone(),
            finals: value.finals.clone(),
            zero_initials: value.zero_initials.clone(),
        }
    }
}

impl From<&NineKeyShuangpinTable> for ShuangpinCustomProfile {
    fn from(value: &NineKeyShuangpinTable) -> Self {
        Self {
            initials: value.initials.clone(),
            finals: value.finals.clone(),
            zero_initials: value.zero_initials.clone(),
        }
    }
}

/// 五笔码表版本。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub enum WubiVersion {
    #[serde(rename = "wubi86")]
    Wubi86,
    #[serde(rename = "wubi98")]
    Wubi98,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum StartMode {
    Chinese,
    English,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum Width {
    Halfwidth,
    Fullwidth,
}

/// 全局主题，与 `GlobalTheme` 同名同值。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// 跟随系统的明暗，画平台自己的配色。
    System,
    /// 只有 iOS 提供；其他宿主按 system 画。
    Native,
    Shuishan,
    Light,
    Paper,
    Night,
    Ink,
    /// 用户自己的主题：候选窗画 candidate_skin / candidate_skin_dark 选中的皮肤。
    Custom,
}

impl From<GlobalTheme> for Theme {
    fn from(value: GlobalTheme) -> Self {
        match value {
            GlobalTheme::System => Self::System,
            GlobalTheme::Native => Self::Native,
            GlobalTheme::Shuishan => Self::Shuishan,
            GlobalTheme::Light => Self::Light,
            GlobalTheme::Paper => Self::Paper,
            GlobalTheme::Night => Self::Night,
            GlobalTheme::Ink => Self::Ink,
            GlobalTheme::Custom => Self::Custom,
        }
    }
}

impl From<Theme> for GlobalTheme {
    fn from(value: Theme) -> Self {
        match value {
            Theme::System => Self::System,
            Theme::Native => Self::Native,
            Theme::Shuishan => Self::Shuishan,
            Theme::Light => Self::Light,
            Theme::Paper => Self::Paper,
            Theme::Night => Self::Night,
            Theme::Ink => Self::Ink,
            Theme::Custom => Self::Custom,
        }
    }
}

impl From<InputScheme> for Scheme {
    fn from(value: InputScheme) -> Self {
        match value {
            InputScheme::Quanpin => Self::Quanpin,
            InputScheme::Shuangpin => Self::Shuangpin,
            InputScheme::Wubi => Self::Wubi,
            InputScheme::Japanese => Self::Japanese,
            InputScheme::Korean => Self::Korean,
            InputScheme::Cantonese => Self::Cantonese,
            InputScheme::Zhuyin => Self::Zhuyin,
            InputScheme::Vietnamese => Self::Vietnamese,
            InputScheme::Tibetan => Self::Tibetan,
            InputScheme::Stroke => Self::Stroke,
        }
    }
}

impl From<ChineseSchemeChoice> for (InputScheme, ChineseScheme) {
    fn from(value: ChineseSchemeChoice) -> Self {
        match value {
            ChineseSchemeChoice::Quanpin => (InputScheme::Quanpin, ChineseScheme::Quanpin),
            ChineseSchemeChoice::Shuangpin => (InputScheme::Shuangpin, ChineseScheme::Shuangpin),
            ChineseSchemeChoice::Wubi => (InputScheme::Wubi, ChineseScheme::Wubi),
        }
    }
}

impl From<ShuangpinProfile> for Profile {
    fn from(value: ShuangpinProfile) -> Self {
        match value {
            ShuangpinProfile::Xiaohe => Self::Xiaohe,
            ShuangpinProfile::Ziranma => Self::Ziranma,
            ShuangpinProfile::Shoudao => Self::Shoudao,
            ShuangpinProfile::Microsoft => Self::Microsoft,
            ShuangpinProfile::SmartAbc => Self::SmartAbc,
            ShuangpinProfile::Custom => Self::Custom,
        }
    }
}

impl From<Profile> for ShuangpinProfile {
    fn from(value: Profile) -> Self {
        match value {
            Profile::Xiaohe => Self::Xiaohe,
            Profile::Ziranma => Self::Ziranma,
            Profile::Shoudao => Self::Shoudao,
            Profile::Microsoft => Self::Microsoft,
            Profile::SmartAbc => Self::SmartAbc,
            Profile::Custom => Self::Custom,
        }
    }
}

impl From<WubiProfile> for WubiVersion {
    fn from(value: WubiProfile) -> Self {
        match value {
            WubiProfile::Wubi86 => Self::Wubi86,
            WubiProfile::Wubi98 => Self::Wubi98,
        }
    }
}

impl From<WubiVersion> for WubiProfile {
    fn from(value: WubiVersion) -> Self {
        match value {
            WubiVersion::Wubi86 => Self::Wubi86,
            WubiVersion::Wubi98 => Self::Wubi98,
        }
    }
}

impl From<DefaultImeMode> for StartMode {
    fn from(value: DefaultImeMode) -> Self {
        match value {
            DefaultImeMode::Chinese => Self::Chinese,
            DefaultImeMode::English => Self::English,
        }
    }
}

impl From<StartMode> for DefaultImeMode {
    fn from(value: StartMode) -> Self {
        match value {
            StartMode::Chinese => Self::Chinese,
            StartMode::English => Self::English,
        }
    }
}

impl From<AppInputModeRule> for StartMode {
    fn from(value: AppInputModeRule) -> Self {
        match value {
            AppInputModeRule::Chinese => Self::Chinese,
            AppInputModeRule::English => Self::English,
        }
    }
}

impl From<StartMode> for AppInputModeRule {
    fn from(value: StartMode) -> Self {
        match value {
            StartMode::Chinese => Self::Chinese,
            StartMode::English => Self::English,
        }
    }
}

impl From<CharacterWidthPreference> for Width {
    fn from(value: CharacterWidthPreference) -> Self {
        match value {
            CharacterWidthPreference::Halfwidth => Self::Halfwidth,
            CharacterWidthPreference::Fullwidth => Self::Fullwidth,
        }
    }
}

impl From<Width> for CharacterWidthPreference {
    fn from(value: Width) -> Self {
        match value {
            Width::Halfwidth => Self::Halfwidth,
            Width::Fullwidth => Self::Fullwidth,
        }
    }
}

impl From<CandidateLayout> for Layout {
    fn from(value: CandidateLayout) -> Self {
        match value {
            CandidateLayout::Horizontal => Self::Horizontal,
            CandidateLayout::Vertical => Self::Vertical,
        }
    }
}

impl From<Layout> for CandidateLayout {
    fn from(value: Layout) -> Self {
        match value {
            Layout::Horizontal => Self::Horizontal,
            Layout::Vertical => Self::Vertical,
        }
    }
}

/// What `get_preferences` shows.
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct PreferencesView {
    /// Pass back as `expected_revision` so a change made meanwhile is not overwritten.
    pub revision: u64,
    pub scheme: Scheme,
    pub shuangpin_profile: Profile,
    /// 保存着的自定义双拼键位表，没有时为 null。
    pub shuangpin_custom_profile: Option<CustomShuangpinTable>,
    /// 触屏九键双拼用哪张表（Android 的九键双拼布局）。
    pub nine_key_shuangpin_profile: NineKeyProfile,
    /// 保存着的九键双拼自定义表，没有时为 null。
    pub nine_key_shuangpin_custom_profile: Option<NineKeyShuangpinTable>,
    /// 每页候选数，1 到 10，第十个用数字键 0 选；Windows、iOS 和鸿蒙最多显示 9 个。
    pub candidate_page_size: u8,
    /// Candidate font size in points, 12 to 32.
    pub candidate_font_size: u8,
    /// Overall size of the floating candidate window in percent, 50 to 200; the font size and the window's geometry are both multiplied by it (Windows, macOS and the Linux Fcitx5 candidate window; the Linux IBus panel scales only the font).
    pub candidate_scale_percent: u16,
    /// Opacity of the candidate window's background, border and skin image in percent, 50 to 100; the text stays opaque. The Linux Fcitx5 candidate window honours it, the Linux IBus panel does not.
    pub candidate_opacity_percent: u8,
    /// Corner radius of the candidate window in points, 0 to 32. Null means the skin's or the platform's own radius.
    pub candidate_corner_radius: Option<u8>,
    /// Space between the candidate window's edge and its candidates in points, 0 to 16, before the overall size scales it (macOS and the Linux Fcitx5 candidate window; the Linux IBus panel ignores it). Null means the skin's or the platform's own padding.
    pub candidate_padding: Option<u8>,
    pub candidate_layout: Layout,
    pub candidate_follow_cursor: bool,
    /// Select candidates with the number row.
    pub number_row_selection: bool,
    /// 切换中英文时显示「中」或「英」的提示（macOS、Windows、Linux 和 HarmonyOS）。
    pub input_mode_hud: bool,
    /// 在候选窗和悬浮工具栏左端显示水杉 logo（macOS、Windows 和 Linux 的 Fcitx5 候选窗；Linux 的 IBus 面板不画 logo，其他宿主始终显示，没有这个开关）。
    pub show_app_logo: bool,
    /// 候选窗显示页码（Linux 是辅助文本里的当前页/总页数，Fcitx5 候选窗连同 ‹ › 翻页按钮一起隐藏，IBus 面板自带的翻页箭头不受影响；macOS 是首行的「1 / 3」和 ‹ › 翻页按钮）。这个开关不改变键盘和滚轮翻页；滚轮翻页由 `navigation.mouse_wheel` 另外控制，默认关闭，所以 macOS 上关掉它又没开滚轮时，鼠标就不能翻页了。
    pub show_candidate_page_number: bool,
    pub fuzzy_pinyin: bool,
    /// The mode a newly focused text field starts in.
    pub default_ime_mode: StartMode,
    /// 应用例外（macOS 和 Windows）：切到这些应用时固定从这里给的模式开始，优先于按应用记忆和 `default_ime_mode`；用户在应用里手动切换后，本次停留期间让位。键在 macOS 上是 bundle id，在 Windows 上是程序文件名（如 `code.exe`，不分大小写）。
    pub app_input_mode_rules: BTreeMap<String, StartMode>,
    /// 中英切换（英文候选模式）热键，写法同设置页录入（如 `Ctrl+Shift+KeyE`）；空串表示不绑定。
    pub toggle_english_candidates: String,
    /// 全角 / 半角切换热键（如 `Ctrl+Shift+Space`）；空串表示不绑定。
    pub toggle_full_width: String,
    /// 中英文标点切换热键（如 `Ctrl+Period`）；空串表示不绑定。
    pub toggle_punctuation: String,
    /// Vim 模式应用名单（macOS）：名单里的前台应用收到无修饰 Esc 时，输入法同时切到英文。标识口径同 `app_input_mode_rules`。
    pub vim_mode_apps: Vec<String>,
    /// The width of the ASCII letters, digits and punctuation the input method outputs.
    pub character_width: Width,
    /// Type Chinese punctuation in Chinese mode.
    pub chinese_punctuation: bool,
    /// With Chinese punctuation on, type , . : after a letter or digit as English punctuation; pressing the same key again within two seconds turns it back into the Chinese mark. On by default on every platform.
    pub smart_punctuation: bool,
    /// Output Traditional Chinese.
    pub traditional_chinese_output: bool,
    /// 五笔用 86 还是 98 码表。
    pub wubi_profile: WubiVersion,
    /// 五笔拼音混输：五笔候选之后接着列出同一串字母的全拼候选，五笔编码打不出时直接出拼音候选。
    pub wubi_mixed_pinyin: bool,
    /// In Wubi, show the rest of each candidate's code after the typed keys.
    pub wubi_code_hint: bool,
    /// 双拼组字时在候选窗旁显示当前方案的键位图（macOS 和 Windows）。null 表示从没设置过：macOS 这时沿用本机旧的设置，其他宿主为关。
    pub shuangpin_keymap_hint: Option<bool>,
    /// 五笔里，完整四码且只有一个候选时在第四键提交。
    pub wubi_auto_commit_unique: bool,
    /// 五笔里 `z` 匹配码表该位置的任意一个字母（含首位）。
    pub wubi_z_wildcard: bool,
    /// 五笔里完整四码后再敲字母时上屏首选，并把该字母留作下一组开头。
    pub wubi_fifth_code_top_commit: bool,
    /// Whether the input method writes its diagnostic log: focus changes, slow requests, candidate window and dictionary events and failures, never what is typed.
    pub diagnostic_log_server: bool,
    /// Windows only: whether the TIP adds its composition and key-latency records to the diagnostic log.
    pub diagnostic_log_tsf: bool,
    /// Windows only: in a game that says it draws its own candidates but shows none, show the input method's candidate window instead. Takes effect in a game the next time the input method is switched to there.
    pub game_candidate_overlay: bool,
    /// Whether the dictionary learns from typing. Read-only here: turning it off is the user's decision.
    pub learning: bool,
    /// AI 助手：组字停顿后把拼音和前文发给用户配置的模型，回答作为一个候选放在固定座位上，等待时那一格显示「…」，没有回答时显示「—」。接口地址、模型和密钥不在这里。
    pub ai_assistant: bool,
    /// 全局主题，候选窗、悬浮工具栏、菜单和触屏键盘共用。只有 `custom` 画下面两个槽位里的皮肤；内置主题（shuishan、light、paper、night、ink）在两种明暗下都画自己的固定配色，system 跟随系统。
    pub global_theme: Theme,
    /// 浅色模式用的候选窗皮肤（list_candidate_skins 的 id），candidate_skin_dark 为 null 时深色模式也用它；null 表示没有。主题不是 custom 时这里的选择保留着但不画。
    pub candidate_skin: Option<String>,
    /// 深色模式用的候选窗皮肤，null 时深色模式取 candidate_skin。
    pub candidate_skin_dark: Option<String>,
}

impl From<&PreferencesSnapshot> for PreferencesView {
    fn from(snapshot: &PreferencesSnapshot) -> Self {
        let preferences = &snapshot.preferences;
        Self {
            revision: snapshot.revision,
            scheme: preferences.scheme.into(),
            shuangpin_profile: preferences.shuangpin_profile.into(),
            shuangpin_custom_profile: (!preferences.shuangpin_custom_profile.is_empty())
                .then(|| CustomShuangpinTable::from(&preferences.shuangpin_custom_profile)),
            nine_key_shuangpin_profile: preferences.nine_key_shuangpin_profile.into(),
            nine_key_shuangpin_custom_profile: (!preferences
                .nine_key_shuangpin_custom_profile
                .is_empty())
            .then(|| NineKeyShuangpinTable::from(&preferences.nine_key_shuangpin_custom_profile)),
            candidate_page_size: preferences.candidate_page_size,
            candidate_font_size: preferences.candidate_font_size,
            candidate_scale_percent: preferences.candidate_scale_percent,
            candidate_opacity_percent: preferences.candidate_opacity_percent,
            candidate_corner_radius: preferences.candidate_corner_radius,
            candidate_padding: preferences.candidate_padding,
            candidate_layout: preferences.candidate_layout.into(),
            candidate_follow_cursor: preferences.candidate_follow_cursor,
            number_row_selection: preferences.number_row_selection,
            input_mode_hud: preferences.input_mode_hud,
            show_app_logo: preferences.show_app_logo,
            show_candidate_page_number: preferences.show_candidate_page_number,
            fuzzy_pinyin: preferences.fuzzy_pinyin.enabled,
            default_ime_mode: preferences.default_ime_mode.into(),
            app_input_mode_rules: preferences
                .app_input_mode_rules
                .iter()
                .map(|(id, rule)| (id.clone(), (*rule).into()))
                .collect(),
            toggle_english_candidates: preferences.keybindings.toggle_english_candidates.clone(),
            toggle_full_width: preferences.keybindings.toggle_full_width.clone(),
            toggle_punctuation: preferences.keybindings.toggle_punctuation.clone(),
            vim_mode_apps: preferences.vim_mode_apps.clone(),
            character_width: preferences.character_width.into(),
            chinese_punctuation: preferences.chinese_punctuation,
            smart_punctuation: preferences.smart_punctuation,
            traditional_chinese_output: preferences.traditional_chinese_output,
            wubi_profile: preferences.wubi_profile.into(),
            wubi_mixed_pinyin: preferences.wubi_mixed_pinyin,
            wubi_code_hint: preferences.wubi_code_hint,
            shuangpin_keymap_hint: preferences.shuangpin_keymap_hint,
            wubi_auto_commit_unique: preferences.wubi_auto_commit_unique,
            wubi_z_wildcard: preferences.wubi_z_wildcard,
            wubi_fifth_code_top_commit: preferences.wubi_fifth_code_top_commit,
            diagnostic_log_server: preferences.diagnostic_log.server,
            diagnostic_log_tsf: preferences.diagnostic_log.tsf,
            game_candidate_overlay: preferences.game_compatibility.candidate_overlay,
            learning: preferences.learning,
            ai_assistant: preferences.ai_assistant.enabled,
            global_theme: preferences.global_theme.into(),
            candidate_skin: preferences.custom_theme.candidate_skin.clone(),
            candidate_skin_dark: preferences.custom_theme.candidate_skin_dark.clone(),
        }
    }
}

/// What `update_preferences` accepts. Absent fields are left as they are.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct PreferencesChange {
    /// The revision `get_preferences` returned. The change is refused if the preferences have changed since; read them again and retry.
    pub expected_revision: u64,
    pub scheme: Option<ChineseSchemeChoice>,
    /// `custom` 需要一张可用的键位表：已经保存着，或者在 `shuangpin_custom_profile` 里一起给出。
    pub shuangpin_profile: Option<Profile>,
    /// 整张替换自定义双拼键位表。不能用的表会被拒绝，并说明哪里不对。
    pub shuangpin_custom_profile: Option<CustomShuangpinTable>,
    /// `custom` 需要一张可用的九键双拼表：已经保存着，或者在 `nine_key_shuangpin_custom_profile` 里一起给出。只换表，不切换键盘布局。
    pub nine_key_shuangpin_profile: Option<NineKeyProfile>,
    /// 整张替换九键双拼自定义表。不能用的表会被拒绝，并说明哪一项不对。
    pub nine_key_shuangpin_custom_profile: Option<NineKeyShuangpinTable>,
    /// 1 到 10。无论存的是多少，Windows、iOS 和鸿蒙最多显示 9 个。
    pub candidate_page_size: Option<u8>,
    /// 12 to 32.
    pub candidate_font_size: Option<u8>,
    /// 50 to 200.
    pub candidate_scale_percent: Option<u16>,
    /// 50 to 100.
    pub candidate_opacity_percent: Option<u8>,
    /// 0 to 32, or null to return to the skin's or the platform's own radius.
    #[serde(default, deserialize_with = "present")]
    pub candidate_corner_radius: Option<Option<u8>>,
    /// 0 to 16, or null to return to the skin's or the platform's own padding.
    #[serde(default, deserialize_with = "present")]
    pub candidate_padding: Option<Option<u8>>,
    pub candidate_layout: Option<Layout>,
    pub candidate_follow_cursor: Option<bool>,
    pub number_row_selection: Option<bool>,
    pub input_mode_hud: Option<bool>,
    pub show_app_logo: Option<bool>,
    pub show_candidate_page_number: Option<bool>,
    /// Turning fuzzy pinyin on for the first time also turns on every fuzzy rule, as the settings page does.
    pub fuzzy_pinyin: Option<bool>,
    pub default_ime_mode: Option<StartMode>,
    /// 整张替换应用例外表，`{}` 清空。最多 32 条；标识非空、不超过 64 字节、两端没有空白、不含控制字符和 `/` `\`，按 ASCII 不分大小写不能重复。
    pub app_input_mode_rules: Option<BTreeMap<String, StartMode>>,
    /// 中英切换（英文候选模式）热键；写法同设置页录入（`Ctrl`/`Alt`/`Shift`/`Meta` 加主键），`""` 解绑。非法写法或与已有绑定冲突时整次更新被拒绝。
    pub toggle_english_candidates: Option<String>,
    /// 全角 / 半角切换热键，规则同上。
    pub toggle_full_width: Option<String>,
    /// 中英文标点切换热键，规则同上。
    pub toggle_punctuation: Option<String>,
    /// 整张替换 Vim 模式应用名单，`[]` 清空。条数与标识口径同 `app_input_mode_rules`。
    pub vim_mode_apps: Option<Vec<String>>,
    pub character_width: Option<Width>,
    pub chinese_punctuation: Option<bool>,
    pub smart_punctuation: Option<bool>,
    pub traditional_chinese_output: Option<bool>,
    pub wubi_profile: Option<WubiVersion>,
    pub wubi_mixed_pinyin: Option<bool>,
    pub wubi_code_hint: Option<bool>,
    /// `null` clears the shared choice so macOS can again consult its legacy host preference.
    #[serde(default, deserialize_with = "present")]
    pub shuangpin_keymap_hint: Option<Option<bool>>,
    pub wubi_auto_commit_unique: Option<bool>,
    pub wubi_z_wildcard: Option<bool>,
    pub wubi_fifth_code_top_commit: Option<bool>,
    /// Turn on to look into a problem the user reports, and off again once it is understood.
    pub diagnostic_log_server: Option<bool>,
    /// Windows only; the other platforms keep it for the Windows settings to find.
    pub diagnostic_log_tsf: Option<bool>,
    /// Windows only. Turn on when the candidate window is missing in a game; turn off when it replaces a game's own candidates.
    pub game_candidate_overlay: Option<bool>,
    /// 只能关：用户嫌 AI 候选慢、占位置或不想用时关掉它。打开会把用户打的字发给外部服务并产生费用，由用户在设置里自己打开，这里给 true 会被拒绝。
    pub ai_assistant: Option<bool>,
    /// 换全局主题。选内置主题就在两种明暗下都画它；要画皮肤用 custom，或者直接给 candidate_skin / candidate_skin_dark。
    pub global_theme: Option<Theme>,
    /// 浅色模式的皮肤：已安装、能用的皮肤 id（list_candidate_skins 列出的），它的 base 必须画在浅色下（system 或浅色的内置主题）；null 取下。没给 global_theme 时，选上皮肤会把全局主题换成 custom，与设置页一样。
    #[serde(default, deserialize_with = "present")]
    pub candidate_skin: Option<Option<String>>,
    /// 深色模式的皮肤，规则同 candidate_skin，base 必须画在深色下（system 或深色的内置主题）；null 取下，深色模式改用 candidate_skin。
    #[serde(default, deserialize_with = "present")]
    pub candidate_skin_dark: Option<Option<String>>,
}

impl PreferencesChange {
    pub(crate) fn is_empty(&self) -> bool {
        self.scheme.is_none()
            && self.shuangpin_profile.is_none()
            && self.shuangpin_custom_profile.is_none()
            && self.nine_key_shuangpin_profile.is_none()
            && self.nine_key_shuangpin_custom_profile.is_none()
            && self.candidate_page_size.is_none()
            && self.candidate_font_size.is_none()
            && self.candidate_scale_percent.is_none()
            && self.candidate_opacity_percent.is_none()
            && self.candidate_corner_radius.is_none()
            && self.candidate_padding.is_none()
            && self.candidate_layout.is_none()
            && self.candidate_follow_cursor.is_none()
            && self.number_row_selection.is_none()
            && self.input_mode_hud.is_none()
            && self.show_app_logo.is_none()
            && self.show_candidate_page_number.is_none()
            && self.fuzzy_pinyin.is_none()
            && self.default_ime_mode.is_none()
            && self.app_input_mode_rules.is_none()
            && self.toggle_english_candidates.is_none()
            && self.toggle_full_width.is_none()
            && self.toggle_punctuation.is_none()
            && self.vim_mode_apps.is_none()
            && self.character_width.is_none()
            && self.chinese_punctuation.is_none()
            && self.smart_punctuation.is_none()
            && self.traditional_chinese_output.is_none()
            && self.wubi_profile.is_none()
            && self.wubi_mixed_pinyin.is_none()
            && self.wubi_code_hint.is_none()
            && self.shuangpin_keymap_hint.is_none()
            && self.wubi_auto_commit_unique.is_none()
            && self.wubi_z_wildcard.is_none()
            && self.wubi_fifth_code_top_commit.is_none()
            && self.diagnostic_log_server.is_none()
            && self.diagnostic_log_tsf.is_none()
            && self.game_candidate_overlay.is_none()
            && self.ai_assistant.is_none()
            && self.global_theme.is_none()
            && self.candidate_skin.is_none()
            && self.candidate_skin_dark.is_none()
    }

    /// 这次修改选上的皮肤，按槽位：`(槽位名, 皮肤 id, 是否深色槽位)`。
    fn selected_skins(&self) -> impl Iterator<Item = (&'static str, &str, bool)> {
        [
            ("candidate_skin", &self.candidate_skin, false),
            ("candidate_skin_dark", &self.candidate_skin_dark, true),
        ]
        .into_iter()
        .filter_map(|(slot, skin, dark)| {
            skin.as_ref()
                .and_then(Option::as_deref)
                .map(|id| (slot, id, dark))
        })
    }

    fn apply(&self, preferences: &mut Preferences) {
        if let Some(choice) = self.scheme {
            let (scheme, chinese) = choice.into();
            preferences.scheme = scheme;
            // The scheme Japanese returns to; left pointing elsewhere, leaving Japanese would undo this change.
            preferences.last_chinese_scheme = Some(chinese);
        }
        if let Some(profile) = self.shuangpin_profile {
            preferences.shuangpin_profile = profile.into();
        }
        if let Some(table) = &self.shuangpin_custom_profile {
            preferences.shuangpin_custom_profile = table.into();
        }
        if let Some(profile) = self.nine_key_shuangpin_profile {
            preferences.nine_key_shuangpin_profile = profile.into();
        }
        if let Some(table) = &self.nine_key_shuangpin_custom_profile {
            preferences.nine_key_shuangpin_custom_profile = table.into();
        }
        if let Some(size) = self.candidate_page_size {
            preferences.candidate_page_size = size;
        }
        if let Some(size) = self.candidate_font_size {
            preferences.candidate_font_size = size;
        }
        if let Some(scale) = self.candidate_scale_percent {
            preferences.candidate_scale_percent = scale;
        }
        if let Some(opacity) = self.candidate_opacity_percent {
            preferences.candidate_opacity_percent = opacity;
        }
        if let Some(radius) = self.candidate_corner_radius {
            preferences.candidate_corner_radius = radius;
        }
        if let Some(padding) = self.candidate_padding {
            preferences.candidate_padding = padding;
        }
        if let Some(layout) = self.candidate_layout {
            preferences.candidate_layout = layout.into();
        }
        if let Some(value) = self.candidate_follow_cursor {
            preferences.candidate_follow_cursor = value;
        }
        if let Some(value) = self.number_row_selection {
            preferences.number_row_selection = value;
        }
        if let Some(value) = self.input_mode_hud {
            preferences.input_mode_hud = value;
        }
        if let Some(value) = self.show_app_logo {
            preferences.show_app_logo = value;
        }
        if let Some(value) = self.show_candidate_page_number {
            preferences.show_candidate_page_number = value;
        }
        if let Some(value) = self.fuzzy_pinyin {
            preferences.fuzzy_pinyin.enabled = value;
        }
        if let Some(mode) = self.default_ime_mode {
            preferences.default_ime_mode = mode.into();
        }
        if let Some(rules) = &self.app_input_mode_rules {
            preferences.app_input_mode_rules = rules
                .iter()
                .map(|(id, mode)| (id.clone(), (*mode).into()))
                .collect();
        }
        if let Some(chord) = &self.toggle_english_candidates {
            preferences.keybindings.toggle_english_candidates = chord.clone();
        }
        if let Some(chord) = &self.toggle_full_width {
            preferences.keybindings.toggle_full_width = chord.clone();
        }
        if let Some(chord) = &self.toggle_punctuation {
            preferences.keybindings.toggle_punctuation = chord.clone();
        }
        if let Some(apps) = &self.vim_mode_apps {
            preferences.vim_mode_apps = apps.clone();
        }
        if let Some(width) = self.character_width {
            preferences.character_width = width.into();
        }
        if let Some(value) = self.chinese_punctuation {
            preferences.chinese_punctuation = value;
        }
        // 与设置页的总开关一样：数字后直出和字母后直出是智能标点真正起作用的两半（`punctuation::route` 要其中之一为真才走 ASCII），所以总开关被改时两半跟着写成同一个值，否则关过两半的文档再打开总开关什么都不变。
        if let Some(value) = self.smart_punctuation {
            preferences.smart_punctuation = value;
            preferences.smart_punctuation_direct_digit = value;
            preferences.smart_punctuation_direct_letter = value;
        }
        if let Some(value) = self.traditional_chinese_output {
            preferences.traditional_chinese_output = value;
        }
        if let Some(value) = self.wubi_profile {
            preferences.wubi_profile = value.into();
        }
        if let Some(value) = self.wubi_mixed_pinyin {
            preferences.wubi_mixed_pinyin = value;
        }
        if let Some(value) = self.wubi_code_hint {
            preferences.wubi_code_hint = value;
        }
        if let Some(value) = self.shuangpin_keymap_hint {
            preferences.shuangpin_keymap_hint = value;
        }
        if let Some(value) = self.wubi_auto_commit_unique {
            preferences.wubi_auto_commit_unique = value;
        }
        if let Some(value) = self.wubi_z_wildcard {
            preferences.wubi_z_wildcard = value;
        }
        if let Some(value) = self.wubi_fifth_code_top_commit {
            preferences.wubi_fifth_code_top_commit = value;
        }
        if let Some(value) = self.diagnostic_log_server {
            preferences.diagnostic_log.server = value;
        }
        if let Some(value) = self.diagnostic_log_tsf {
            preferences.diagnostic_log.tsf = value;
        }
        if let Some(value) = self.game_candidate_overlay {
            preferences.game_compatibility.candidate_overlay = value;
        }
        if let Some(value) = self.ai_assistant {
            preferences.ai_assistant.enabled = value;
        }
        let custom = &mut preferences.custom_theme;
        let had_skin = custom.candidate_skin.is_some() || custom.candidate_skin_dark.is_some();
        if let Some(skin) = &self.candidate_skin {
            custom.candidate_skin = skin.clone();
        }
        if let Some(skin) = &self.candidate_skin_dark {
            custom.candidate_skin_dark = skin.clone();
        }
        // 与设置页取下皮肤时一样：两个槽位都空了，底改回 system。设置页应用皮肤时把包的 base 写进底，留着深色皮肤的 night 会让浅色模式也画成深色。
        if had_skin && custom.candidate_skin.is_none() && custom.candidate_skin_dark.is_none() {
            custom.base = GlobalTheme::System;
        }
        match self.global_theme {
            Some(theme) => preferences.global_theme = theme.into(),
            // 与设置页一样，选上皮肤就换到画皮肤的自定义主题。
            None if self.selected_skins().next().is_some() => {
                preferences.global_theme = GlobalTheme::Custom;
            }
            None => {}
        }
    }
}

/// A field that was given, even as null, is `Some`; only an absent field is `None`, so null can clear a value rather than leave it alone.
fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

/// `edition` 是运行时选项记录的版本：状态目录里还没有偏好文件时，读到的是这个版本的默认值。
pub fn load(state_dir: &Path, edition: &'static Edition) -> Result<PreferencesView, String> {
    let snapshot = PreferencesStore::for_edition(state_dir, edition)
        .load()
        .map_err(|error| error.to_string())?;
    Ok(PreferencesView::from(&snapshot))
}

/// 按修订号比较后保存 `change`。在 Linux 上像设置页保存时一样，把结果连同重新扫描的皮肤目录发布进 IBus 和 Fcitx5 宿主读偏好的运行时选项文档：宿主只画目录里列出的皮肤，目录放不下全部皮肤时优先留下选中的那款，所以换了选择就要按新选择重新裁剪。
///
/// `edition` 是运行时选项记录的版本：本版本不提供的方案被拒绝，例如五笔版只能选五笔。
pub fn update(
    state_dir: &Path,
    options: &Path,
    edition: &'static Edition,
    change: &PreferencesChange,
) -> Result<PreferencesView, String> {
    if change.is_empty() {
        return Err("no preference to change".into());
    }
    if let Some(choice) = change.scheme {
        let (scheme, _): (InputScheme, ChineseScheme) = choice.into();
        if !edition.offers(scheme) {
            let offered: Vec<Value> = edition
                .input_schemes
                .iter()
                .filter_map(|scheme| serde_json::to_value(scheme).ok())
                .collect();
            return Err(format!(
                "this edition of the input method does not offer that scheme; it offers {}",
                Value::Array(offered)
            ));
        }
    }
    let store = PreferencesStore::for_edition(state_dir, edition);
    let previous = store.load().map_err(|error| error.to_string())?;
    if previous.revision != change.expected_revision {
        return Err("the preferences changed since they were read; read them again".into());
    }
    if change.ai_assistant == Some(true) && !previous.preferences.ai_assistant.enabled {
        return Err("the AI assistant sends what the user types to an outside service; only the user can turn it on, in the input method's settings".into());
    }
    for (slot, id, dark) in change.selected_skins() {
        check_skin(state_dir, slot, id, dark)?;
    }
    let mut preferences = previous.preferences.clone();
    change.apply(&mut preferences);
    // 键位表合不合法只有 Engine 说了算：给了新表，或者要用自定义方案，就先校验，免得存下一张输入法用不了、只能悄悄退回小鹤的表。
    if change.shuangpin_custom_profile.is_some()
        || preferences.shuangpin_profile == ShuangpinProfile::Custom
    {
        msime_host_api::validate_custom_shuangpin_profile(&preferences.shuangpin_custom_profile)
            .map_err(|error| format!("the custom shuangpin table cannot be used: {error}"))?;
    }
    // 九键双拼的表同理，由 Engine 校验。
    if change.nine_key_shuangpin_custom_profile.is_some()
        || preferences.nine_key_shuangpin_profile == NineKeyShuangpinProfile::Custom
    {
        msime_host_api::validate_nine_key_shuangpin_custom_profile(
            &preferences.nine_key_shuangpin_custom_profile,
        )
        .map_err(|error| format!("the nine-key shuangpin table cannot be used: {error}"))?;
    }
    let snapshot = store
        .save(change.expected_revision, preferences)
        .map_err(|error| error.to_string())?;
    if cfg!(target_os = "linux") {
        let skins = state_dir.join(crate::skins::SKIN_DIRECTORY);
        if let Err(error) =
            crate::runtime_options::publish_preferences(options, &skins, &snapshot.preferences)
        {
            // A document the hosts could not read is refused along with the save that produced it, so the store and the hosts do not disagree. A concurrent writer wins over the rollback.
            let _ = store.save(snapshot.revision, previous.preferences);
            return Err(error);
        }
    }
    Ok(PreferencesView::from(&snapshot))
}

/// 槽位里要放的皮肤必须是皮肤目录里能用的那款（与 list_candidate_skins 同一条规则），而且它的 base 要画在这个槽位的明暗下：深色 base 的皮肤放进浅色槽位，浅色模式根本不画它。
fn check_skin(state_dir: &Path, slot: &str, id: &str, dark: bool) -> Result<(), String> {
    if GlobalTheme::from_id(id).is_some_and(|theme| theme != GlobalTheme::Native) {
        return Err(format!(
            "{slot}: {id} is a built-in theme, not a skin; set global_theme to {id} to use it"
        ));
    }
    let skin = catalog::load_package(state_dir.join(crate::skins::SKIN_DIRECTORY), id).map_err(
        |reason| {
            format!("{slot}: {id} is not an installed skin that can be used ({reason}); list_candidate_skins shows the installed skins")
        },
    )?;
    let mode = |appearance: ThemeAppearance| match appearance {
        ThemeAppearance::Light => "light",
        ThemeAppearance::Dark => "dark",
    };
    match skin_appearance(skin.base) {
        Some(appearance) if (appearance == ThemeAppearance::Dark) != dark => Err(format!(
            "{slot}: {id} is drawn over the {mode} base {base}, so it shows only in {mode} mode; put it in {other} instead",
            mode = mode(appearance),
            base = skin.base.id(),
            other = if dark { "candidate_skin" } else { "candidate_skin_dark" },
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_mirrors_serialize_as_the_store_does() {
        let same = |mirror: Value, store: Value| assert_eq!(mirror, store);
        for scheme in [
            InputScheme::Quanpin,
            InputScheme::Shuangpin,
            InputScheme::Wubi,
            InputScheme::Japanese,
            InputScheme::Korean,
            InputScheme::Cantonese,
            InputScheme::Zhuyin,
            InputScheme::Vietnamese,
            InputScheme::Tibetan,
            InputScheme::Stroke,
        ] {
            same(json!(Scheme::from(scheme)), json!(scheme));
        }
        for choice in [
            ChineseSchemeChoice::Quanpin,
            ChineseSchemeChoice::Shuangpin,
            ChineseSchemeChoice::Wubi,
        ] {
            let (scheme, chinese): (InputScheme, ChineseScheme) = choice.into();
            same(json!(choice), json!(scheme));
            same(json!(choice), json!(chinese));
        }
        for profile in [
            ShuangpinProfile::Xiaohe,
            ShuangpinProfile::Ziranma,
            ShuangpinProfile::Shoudao,
            ShuangpinProfile::Microsoft,
            ShuangpinProfile::SmartAbc,
            ShuangpinProfile::Custom,
        ] {
            same(json!(Profile::from(profile)), json!(profile));
            assert_eq!(ShuangpinProfile::from(Profile::from(profile)), profile);
        }
        for profile in [
            NineKeyShuangpinProfile::Migala2,
            NineKeyShuangpinProfile::Migala3,
            NineKeyShuangpinProfile::Gushi2Low,
            NineKeyShuangpinProfile::Custom,
        ] {
            same(json!(NineKeyProfile::from(profile)), json!(profile));
            assert_eq!(
                NineKeyShuangpinProfile::from(NineKeyProfile::from(profile)),
                profile
            );
        }
        for profile in [WubiProfile::Wubi86, WubiProfile::Wubi98] {
            same(json!(WubiVersion::from(profile)), json!(profile));
            assert_eq!(WubiProfile::from(WubiVersion::from(profile)), profile);
        }
        for layout in [CandidateLayout::Horizontal, CandidateLayout::Vertical] {
            same(json!(Layout::from(layout)), json!(layout));
            assert_eq!(CandidateLayout::from(Layout::from(layout)), layout);
        }
        for mode in [DefaultImeMode::Chinese, DefaultImeMode::English] {
            same(json!(StartMode::from(mode)), json!(mode));
            assert_eq!(DefaultImeMode::from(StartMode::from(mode)), mode);
        }
        for rule in [AppInputModeRule::Chinese, AppInputModeRule::English] {
            same(json!(StartMode::from(rule)), json!(rule));
            assert_eq!(AppInputModeRule::from(StartMode::from(rule)), rule);
        }
        for width in [
            CharacterWidthPreference::Halfwidth,
            CharacterWidthPreference::Fullwidth,
        ] {
            same(json!(Width::from(width)), json!(width));
            assert_eq!(CharacterWidthPreference::from(Width::from(width)), width);
        }
        for theme in GlobalTheme::ALL {
            same(json!(Theme::from(theme)), json!(theme));
            assert_eq!(GlobalTheme::from(Theme::from(theme)), theme);
        }
    }

    /// 在 `state` 的皮肤目录里放一款画在 `base` 上的最小皮肤。
    fn install_skin(state: &Path, id: &str, base: &str) {
        let folder = state.join(crate::skins::SKIN_DIRECTORY).join(id);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("skin.toml"),
            format!("schema_version = 1\nid = '{id}'\nname = '{id}'\nversion = '1.0'\nbase = '{base}'\n[supports]\nlayouts = ['horizontal', 'vertical']\nthemes = ['dark', 'light']\n[candidate_window]\nmin_width_dip = 200\n"),
        )
        .unwrap();
    }

    #[test]
    fn a_skin_is_selected_for_each_mode_after_the_catalog_accepts_it() {
        let directory = tempfile::tempdir().unwrap();
        let state = directory.path();
        let options = state.join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1}"#).unwrap();
        install_skin(state, "dusk", "night");
        install_skin(state, "dawn", "paper");
        install_skin(state, "plain", "system");
        let run = |value: Value| {
            let change: PreferencesChange = serde_json::from_value(value).unwrap();
            update(state, &options, Edition::full(), &change)
        };
        let before = load(state, Edition::full()).unwrap();
        assert_eq!(before.global_theme, Theme::System);
        assert_eq!(
            (
                before.candidate_skin.as_deref(),
                before.candidate_skin_dark.as_deref()
            ),
            (None, None)
        );

        // 选上深色皮肤，没给主题时换到 custom，与设置页一样。
        let dark =
            run(json!({ "expected_revision": before.revision, "candidate_skin_dark": "dusk" }))
                .unwrap();
        assert_eq!(dark.global_theme, Theme::Custom);
        assert_eq!(dark.candidate_skin_dark.as_deref(), Some("dusk"));
        assert_eq!(dark.candidate_skin, None);

        // 拒绝：深色皮肤放进浅色槽位、内置主题当皮肤、没装的皮肤、过期的修订号。偏好都不动。
        for (value, expected) in [
            (
                json!({ "candidate_skin": "dusk" }),
                "shows only in dark mode; put it in candidate_skin_dark",
            ),
            (
                json!({ "candidate_skin_dark": "dawn" }),
                "shows only in light mode; put it in candidate_skin",
            ),
            (
                json!({ "candidate_skin": "night" }),
                "is a built-in theme, not a skin; set global_theme to night",
            ),
            (
                json!({ "candidate_skin": "missing" }),
                "is not an installed skin",
            ),
            (
                json!({ "candidate_skin": "fluent" }),
                "is not an installed skin",
            ),
        ] {
            let mut value = value;
            value["expected_revision"] = json!(dark.revision);
            let error = run(value).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
        let error = run(json!({ "expected_revision": before.revision, "candidate_skin": "dawn" }))
            .unwrap_err();
        assert!(error.contains("changed since"), "{error}");
        assert_eq!(load(state, Edition::full()).unwrap(), dark);

        // 浅色槽位放浅色或 system 底的皮肤；给了主题时按给的主题。
        let both = run(json!({ "expected_revision": dark.revision, "candidate_skin": "dawn", "global_theme": "night" }))
            .unwrap();
        assert_eq!(both.global_theme, Theme::Night);
        assert_eq!(both.candidate_skin.as_deref(), Some("dawn"));
        assert_eq!(both.candidate_skin_dark.as_deref(), Some("dusk"));
        let plain = run(json!({ "expected_revision": both.revision, "candidate_skin": "plain", "candidate_skin_dark": "plain", "global_theme": "custom" }))
            .unwrap();
        assert_eq!(
            (
                plain.candidate_skin.as_deref(),
                plain.candidate_skin_dark.as_deref()
            ),
            (Some("plain"), Some("plain"))
        );

        // 取下最后一款皮肤时底改回 system；主题不动。
        let mut stored = PreferencesStore::new(state).load().unwrap();
        stored.preferences.custom_theme.base = GlobalTheme::Night;
        let saved = PreferencesStore::new(state)
            .save(stored.revision, stored.preferences)
            .unwrap();
        let cleared = run(json!({ "expected_revision": saved.revision, "candidate_skin": null, "candidate_skin_dark": null }))
            .unwrap();
        assert_eq!(cleared.global_theme, Theme::Custom);
        assert_eq!(
            (cleared.candidate_skin, cleared.candidate_skin_dark),
            (None, None)
        );
        let stored = PreferencesStore::new(state).load().unwrap().preferences;
        assert_eq!(stored.custom_theme.base, GlobalTheme::System);

        // 只换主题。没有皮肤的视图照样输出两个槽位（null），输出 schema 要求的字段一个不缺。
        let builtin =
            run(json!({ "expected_revision": cleared.revision, "global_theme": "paper" })).unwrap();
        assert_eq!(builtin.global_theme, Theme::Paper);
        let view = serde_json::to_value(&builtin).unwrap();
        assert_eq!(view["candidate_skin"], Value::Null);
        crate::assert_required_fields_present::<PreferencesView>(&view);
        crate::assert_required_fields_present::<PreferencesView>(
            &serde_json::to_value(&plain).unwrap(),
        );
    }

    fn change(revision: u64) -> PreferencesChange {
        PreferencesChange {
            expected_revision: revision,
            ..PreferencesChange::default()
        }
    }

    /// 小鹤的键位，只把 ing 从 k 挪到 `;`。
    fn custom_table() -> CustomShuangpinTable {
        serde_json::from_value(json!({
            "initials": { "zh": "v", "ch": "i", "sh": "u" },
            "finals": {
                "iu": "q", "ei": "w", "e": "e", "uan": "r", "ue": "t", "ve": "t", "un": "y",
                "u": "u", "i": "i", "uo": "o", "o": "o", "ie": "p", "a": "a", "ong": "s",
                "iong": "s", "ai": "d", "en": "f", "eng": "g", "ang": "h", "an": "j",
                "uai": "k", "ing": ";", "uang": "l", "iang": "l", "ou": "z", "ua": "x",
                "ia": "x", "ao": "c", "ui": "v", "v": "v", "in": "b", "iao": "n", "ian": "m"
            },
            "zero_initials": {
                "a": "aa", "ai": "ai", "an": "an", "ang": "ah", "ao": "ao", "e": "ee",
                "ei": "ei", "en": "en", "eng": "eg", "er": "er", "o": "oo", "ou": "ou"
            }
        }))
        .unwrap()
    }

    #[test]
    fn a_custom_shuangpin_profile_needs_a_table_the_engine_accepts() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1}"#).unwrap();
        let before = load(directory.path(), Edition::full()).unwrap();
        assert_eq!(before.shuangpin_custom_profile, None);
        let run = |change: &PreferencesChange| {
            update(directory.path(), &options, Edition::full(), change)
        };

        // 没有表就不能选自定义方案。
        let refused = run(&PreferencesChange {
            shuangpin_profile: Some(Profile::Custom),
            ..change(before.revision)
        })
        .unwrap_err();
        assert!(
            refused.starts_with("the custom shuangpin table cannot be used: initials unit"),
            "{refused}"
        );
        // 给出的表不能用，也拒绝，并说明哪里冲突。
        let mut colliding = custom_table();
        colliding.finals.insert("ang".into(), "k".into());
        let refused = run(&PreferencesChange {
            shuangpin_custom_profile: Some(colliding),
            ..change(before.revision)
        })
        .unwrap_err();
        assert!(refused.contains("reads as both"), "{refused}");
        assert_eq!(
            PreferencesStore::new(directory.path())
                .load()
                .unwrap()
                .revision,
            before.revision
        );

        let updated = run(&PreferencesChange {
            scheme: Some(ChineseSchemeChoice::Shuangpin),
            shuangpin_profile: Some(Profile::Custom),
            shuangpin_custom_profile: Some(custom_table()),
            ..change(before.revision)
        })
        .unwrap();
        assert_eq!(updated.shuangpin_profile, Profile::Custom);
        assert_eq!(updated.shuangpin_custom_profile, Some(custom_table()));
        let stored = PreferencesStore::new(directory.path()).load().unwrap();
        assert_eq!(
            stored.preferences.shuangpin_custom_profile,
            ShuangpinCustomProfile::from(&custom_table())
        );
    }

    #[test]
    fn a_nine_key_shuangpin_table_is_read_and_written_after_the_engine_accepts_it() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1}"#).unwrap();
        let before = load(directory.path(), Edition::full()).unwrap();
        assert_eq!(before.nine_key_shuangpin_profile, NineKeyProfile::Migala3);
        assert_eq!(before.nine_key_shuangpin_custom_profile, None);
        let run = |change: &PreferencesChange| {
            update(directory.path(), &options, Edition::full(), change)
        };
        // 内置方案直接能选。
        let updated = run(&PreferencesChange {
            nine_key_shuangpin_profile: Some(NineKeyProfile::Gushi2Low),
            ..change(before.revision)
        })
        .unwrap();
        assert_eq!(
            updated.nine_key_shuangpin_profile,
            NineKeyProfile::Gushi2Low
        );
        // 没有表就不能选自定义方案；给出的表不能用也拒绝，并指出哪一项。
        let refused = run(&PreferencesChange {
            nine_key_shuangpin_profile: Some(NineKeyProfile::Custom),
            ..change(updated.revision)
        })
        .unwrap_err();
        assert!(
            refused.starts_with("the nine-key shuangpin table cannot be used: initials unit"),
            "{refused}"
        );
        let table = NineKeyShuangpinTable::from(
            &msime_host_api::builtin_nine_key_shuangpin_custom_profile(
                NineKeyShuangpinProfile::Migala2,
            )
            .unwrap(),
        );
        let mut broken = table.clone();
        broken.zero_initials.insert("a".into(), "6".into());
        let refused = run(&PreferencesChange {
            nine_key_shuangpin_custom_profile: Some(broken),
            ..change(updated.revision)
        })
        .unwrap_err();
        assert!(
            refused.contains("zero_initials unit \"a\" has an invalid key \"6\""),
            "{refused}"
        );
        let saved = run(&PreferencesChange {
            nine_key_shuangpin_profile: Some(NineKeyProfile::Custom),
            nine_key_shuangpin_custom_profile: Some(table.clone()),
            ..change(updated.revision)
        })
        .unwrap();
        assert_eq!(saved.nine_key_shuangpin_profile, NineKeyProfile::Custom);
        assert_eq!(saved.nine_key_shuangpin_custom_profile, Some(table));
        // 26 键的自定义表不受影响。
        assert_eq!(saved.shuangpin_custom_profile, None);
    }

    #[test]
    fn toggle_hotkeys_are_read_and_written_as_canonical_chords() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1}"#).unwrap();
        let before = load(directory.path(), Edition::full()).unwrap();
        assert_eq!(before.toggle_english_candidates, "Ctrl+Shift+KeyE");
        assert_eq!(before.toggle_full_width, "Ctrl+Shift+Space");
        assert_eq!(before.toggle_punctuation, "Ctrl+Period");
        let run = |change: &PreferencesChange| {
            update(directory.path(), &options, Edition::full(), change)
        };
        // 别名写法归一成规范形落盘；空串解绑。
        let updated = run(&PreferencesChange {
            toggle_english_candidates: Some("control+alt+keyr".into()),
            toggle_punctuation: Some(String::new()),
            ..change(before.revision)
        })
        .unwrap();
        assert_eq!(updated.toggle_english_candidates, "Ctrl+Alt+KeyR");
        assert_eq!(updated.toggle_punctuation, "");
        // 非法写法和撞已有绑定的写法都被拒绝，文档不动。
        for (change, expected) in [
            (
                PreferencesChange {
                    toggle_punctuation: Some("Ctrl+".into()),
                    ..change(updated.revision)
                },
                "切换热键必须是合法的快捷键组合",
            ),
            (
                PreferencesChange {
                    toggle_punctuation: Some("Ctrl+Shift+Space".into()),
                    ..change(updated.revision)
                },
                "切换热键不能互相相同",
            ),
        ] {
            let error = run(&change).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
        assert_eq!(load(directory.path(), Edition::full()).unwrap(), updated);
    }

    #[test]
    fn smart_punctuation_writes_both_halves_with_the_master_switch() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1}"#).unwrap();
        let before = load(directory.path(), Edition::full()).unwrap();
        let mut revision = before.revision;
        for enabled in [true, false] {
            let updated = update(
                directory.path(),
                &options,
                Edition::full(),
                &PreferencesChange {
                    smart_punctuation: Some(enabled),
                    ..change(revision)
                },
            )
            .unwrap();
            assert_eq!(updated.smart_punctuation, enabled);
            revision = updated.revision;
            let stored = PreferencesStore::new(directory.path()).load().unwrap();
            assert_eq!(stored.preferences.smart_punctuation_direct_digit, enabled);
            assert_eq!(stored.preferences.smart_punctuation_direct_letter, enabled);
        }
    }

    #[test]
    fn an_update_changes_only_what_it_names_and_refuses_a_stale_revision() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1,"resources":"/resources"}"#).unwrap();
        let before = load(directory.path(), Edition::full()).unwrap();
        let untouched = PreferencesStore::new(directory.path()).load().unwrap();

        assert_eq!(
            update(
                directory.path(),
                &options,
                Edition::full(),
                &change(before.revision)
            )
            .unwrap_err(),
            "no preference to change"
        );
        let updated = update(
            directory.path(),
            &options,
            Edition::full(),
            &PreferencesChange {
                scheme: Some(ChineseSchemeChoice::Shuangpin),
                shuangpin_profile: Some(Profile::Ziranma),
                candidate_page_size: Some(7),
                fuzzy_pinyin: Some(true),
                character_width: Some(Width::Fullwidth),
                traditional_chinese_output: Some(true),
                wubi_code_hint: Some(false),
                wubi_auto_commit_unique: Some(false),
                wubi_z_wildcard: Some(true),
                wubi_fifth_code_top_commit: Some(false),
                diagnostic_log_server: Some(true),
                ..change(before.revision)
            },
        )
        .unwrap();
        assert_eq!(updated.scheme, Scheme::Shuangpin);
        assert_eq!(updated.shuangpin_profile, Profile::Ziranma);
        assert_eq!(updated.candidate_page_size, 7);
        assert!(updated.fuzzy_pinyin);
        assert_eq!(updated.character_width, Width::Fullwidth);
        assert!(updated.traditional_chinese_output);
        assert!(!updated.wubi_code_hint);
        assert!(!updated.wubi_auto_commit_unique);
        assert!(updated.wubi_z_wildcard);
        assert!(!updated.wubi_fifth_code_top_commit);
        assert!(updated.diagnostic_log_server && !updated.diagnostic_log_tsf);
        assert_eq!(updated.revision, before.revision + 1);

        let stored = PreferencesStore::new(directory.path()).load().unwrap();
        assert_eq!(
            stored.preferences.last_chinese_scheme,
            Some(ChineseScheme::Shuangpin)
        );
        // Turning fuzzy pinyin on for the first time seeds the rules, as the settings page does.
        assert!(!stored.preferences.fuzzy_pinyin.rules.is_empty());
        // Nothing outside the allowlist moved.
        let mut expected = untouched.preferences.clone();
        expected.scheme = InputScheme::Shuangpin;
        expected.last_chinese_scheme = Some(ChineseScheme::Shuangpin);
        expected.shuangpin_profile = ShuangpinProfile::Ziranma;
        expected.candidate_page_size = 7;
        expected.character_width = CharacterWidthPreference::Fullwidth;
        expected.traditional_chinese_output = true;
        expected.wubi_code_hint = false;
        expected.wubi_auto_commit_unique = false;
        expected.wubi_z_wildcard = true;
        expected.wubi_fifth_code_top_commit = false;
        expected.diagnostic_log.server = true;
        expected.fuzzy_pinyin = stored.preferences.fuzzy_pinyin.clone();
        assert_eq!(json!(stored.preferences), json!(expected));

        // A stale revision is refused, and so is a value the store refuses.
        let stale = PreferencesChange {
            candidate_page_size: Some(5),
            ..change(before.revision)
        };
        assert!(update(directory.path(), &options, Edition::full(), &stale).is_err());
        let invalid = PreferencesChange {
            candidate_page_size: Some(11),
            ..change(updated.revision)
        };
        assert!(update(directory.path(), &options, Edition::full(), &invalid).is_err());
        assert_eq!(load(directory.path(), Edition::full()).unwrap(), updated);

        // Linux 宿主从运行时选项读偏好和皮肤目录，两样随保存一起发布；文档里其余的键保留。
        let document: Value = serde_json::from_slice(&std::fs::read(&options).unwrap()).unwrap();
        if cfg!(target_os = "linux") {
            assert_eq!(document["preferences"]["candidate_page_size"], 7);
            assert_eq!(document["candidate_skin_catalog"], json!({"packages": []}));
        } else {
            assert!(document.get("preferences").is_none());
            assert!(document.get("candidate_skin_catalog").is_none());
        }
        assert_eq!(document["resources"], "/resources");
    }

    /// 五笔版只能选五笔：别的方案被拒绝，偏好不动；还没有偏好文件时读到的是五笔版的默认值。
    #[test]
    fn the_wubi_edition_refuses_a_scheme_it_does_not_offer() {
        let wubi = Edition::by_id("wubi").unwrap();
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1,"edition":"wubi"}"#).unwrap();
        let before = load(directory.path(), wubi).unwrap();
        assert_eq!(before.scheme, Scheme::Wubi);
        assert!(before.wubi_mixed_pinyin);

        for refused in [ChineseSchemeChoice::Quanpin, ChineseSchemeChoice::Shuangpin] {
            let error = update(
                directory.path(),
                &options,
                wubi,
                &PreferencesChange {
                    scheme: Some(refused),
                    candidate_page_size: Some(7),
                    ..change(before.revision)
                },
            )
            .unwrap_err();
            assert!(error.contains("does not offer"), "{error}");
            assert!(error.contains("\"wubi\""), "{error}");
        }
        assert_eq!(load(directory.path(), wubi).unwrap(), before);

        let updated = update(
            directory.path(),
            &options,
            wubi,
            &PreferencesChange {
                scheme: Some(ChineseSchemeChoice::Wubi),
                wubi_mixed_pinyin: Some(false),
                ..change(before.revision)
            },
        )
        .unwrap();
        assert_eq!(updated.scheme, Scheme::Wubi);
        assert!(!updated.wubi_mixed_pinyin);
        // 第一次保存写下的是五笔版的默认值加上这次的修改。
        let stored = PreferencesStore::new(directory.path()).load().unwrap();
        assert_eq!(
            stored.preferences.last_chinese_scheme,
            Some(ChineseScheme::Wubi)
        );
    }

    #[test]
    fn the_candidate_window_style_is_set_and_its_radius_returned_to_the_skin() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1}"#).unwrap();
        let before = load(directory.path(), Edition::full()).unwrap();
        assert_eq!(before.candidate_scale_percent, 100);
        assert_eq!(before.candidate_opacity_percent, 100);
        assert_eq!(before.candidate_corner_radius, None);

        let styled: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": before.revision,
            "candidate_scale_percent": 125,
            "candidate_opacity_percent": 80,
            "candidate_corner_radius": 12,
        }))
        .unwrap();
        let updated = update(directory.path(), &options, Edition::full(), &styled).unwrap();
        assert_eq!(updated.candidate_scale_percent, 125);
        assert_eq!(updated.candidate_opacity_percent, 80);
        assert_eq!(updated.candidate_corner_radius, Some(12));

        // An absent radius leaves it alone; null hands it back to the skin.
        let untouched: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": updated.revision,
            "candidate_scale_percent": 150,
        }))
        .unwrap();
        assert_eq!(untouched.candidate_corner_radius, None);
        let updated = update(directory.path(), &options, Edition::full(), &untouched).unwrap();
        assert_eq!(updated.candidate_corner_radius, Some(12));
        let cleared: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": updated.revision,
            "candidate_corner_radius": null,
        }))
        .unwrap();
        assert!(!cleared.is_empty());
        let updated = update(directory.path(), &options, Edition::full(), &cleared).unwrap();
        assert_eq!(updated.candidate_corner_radius, None);
        assert_eq!(updated.candidate_scale_percent, 150);

        let invalid = PreferencesChange {
            candidate_opacity_percent: Some(30),
            ..change(updated.revision)
        };
        assert!(update(directory.path(), &options, Edition::full(), &invalid).is_err());
    }

    /// 页码开关默认开着，代理能关掉它，写进共享偏好的就是宿主读的 `show_candidate_page_number`（#7316）。
    #[test]
    fn the_page_number_switch_is_read_and_written() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1}"#).unwrap();
        let before = load(directory.path(), Edition::full()).unwrap();
        assert!(before.show_candidate_page_number);

        let hidden: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": before.revision,
            "show_candidate_page_number": false,
        }))
        .unwrap();
        assert!(!hidden.is_empty());
        let updated = update(directory.path(), &options, Edition::full(), &hidden).unwrap();
        assert!(!updated.show_candidate_page_number);
        let stored = PreferencesStore::new(directory.path()).load().unwrap();
        assert!(!stored.preferences.show_candidate_page_number);
    }

    /// 卡片内边距能设、能交还皮肤，超出 0 到 16 的值被拒（#7316）。
    #[test]
    fn the_candidate_padding_is_set_and_returned_to_the_skin() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1}"#).unwrap();
        let before = load(directory.path(), Edition::full()).unwrap();
        assert_eq!(before.candidate_padding, None);

        let tight: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": before.revision,
            "candidate_padding": 2,
        }))
        .unwrap();
        assert!(!tight.is_empty());
        let updated = update(directory.path(), &options, Edition::full(), &tight).unwrap();
        assert_eq!(updated.candidate_padding, Some(2));
        let stored = PreferencesStore::new(directory.path()).load().unwrap();
        assert_eq!(stored.preferences.candidate_padding, Some(2));

        let cleared: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": updated.revision,
            "candidate_padding": null,
        }))
        .unwrap();
        assert!(!cleared.is_empty());
        let updated = update(directory.path(), &options, Edition::full(), &cleared).unwrap();
        assert_eq!(updated.candidate_padding, None);

        let invalid = PreferencesChange {
            candidate_padding: Some(Some(17)),
            ..change(updated.revision)
        };
        assert!(update(directory.path(), &options, Edition::full(), &invalid).is_err());
    }

    /// 代理只能开关游戏里的候选窗，用户写下的两张进程表原样保留。
    #[test]
    fn app_input_mode_rules_and_the_keymap_hint_are_set_and_invalid_rules_refused() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1}"#).unwrap();
        let store = PreferencesStore::new(directory.path());
        let before = load(directory.path(), Edition::full()).unwrap();
        assert!(before.app_input_mode_rules.is_empty());
        assert_eq!(before.shuangpin_keymap_hint, None);

        let set: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": before.revision,
            "app_input_mode_rules": { "code.exe": "english", "com.tencent.xinWeChat": "chinese" },
            "shuangpin_keymap_hint": true,
        }))
        .unwrap();
        assert!(!set.is_empty());
        let updated = update(directory.path(), &options, Edition::full(), &set).unwrap();
        assert_eq!(updated.app_input_mode_rules["code.exe"], StartMode::English);
        assert_eq!(updated.shuangpin_keymap_hint, Some(true));
        let stored = store.load().unwrap().preferences;
        assert_eq!(
            stored.app_input_mode_rules["com.tencent.xinWeChat"],
            AppInputModeRule::Chinese
        );
        assert_eq!(stored.shuangpin_keymap_hint, Some(true));

        // 只差大小写的两条会让 Windows 上同一个进程拿到两种模式，存储层拒绝，原来的表不变。
        let duplicate: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": updated.revision,
            "app_input_mode_rules": { "Code.exe": "chinese", "code.exe": "english" },
        }))
        .unwrap();
        assert!(update(directory.path(), &options, Edition::full(), &duplicate).is_err());
        assert_eq!(
            store.load().unwrap().preferences.app_input_mode_rules.len(),
            2
        );

        let untouched: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": updated.revision,
            "app_input_mode_rules": {},
        }))
        .unwrap();
        assert_eq!(untouched.shuangpin_keymap_hint, None);
        let untouched = update(directory.path(), &options, Edition::full(), &untouched).unwrap();
        assert_eq!(untouched.shuangpin_keymap_hint, Some(true));

        let cleared: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": untouched.revision,
            "shuangpin_keymap_hint": null,
            "app_input_mode_rules": {},
        }))
        .unwrap();
        assert_eq!(cleared.shuangpin_keymap_hint, Some(None));
        let cleared = update(directory.path(), &options, Edition::full(), &cleared).unwrap();
        assert!(cleared.app_input_mode_rules.is_empty());
        assert_eq!(cleared.shuangpin_keymap_hint, None);
    }

    /// Vim 模式名单与应用例外同一份读写通路：整张替换、`[]` 清空、非法标识拒存。
    #[test]
    fn vim_mode_apps_are_set_cleared_and_invalid_entries_refused() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1}"#).unwrap();
        let store = PreferencesStore::new(directory.path());
        let before = load(directory.path(), Edition::full()).unwrap();
        assert!(before.vim_mode_apps.is_empty());

        let set: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": before.revision,
            "vim_mode_apps": ["com.apple.Terminal", "nvim.exe"],
        }))
        .unwrap();
        assert!(!set.is_empty());
        let updated = update(directory.path(), &options, Edition::full(), &set).unwrap();
        assert_eq!(updated.vim_mode_apps, ["com.apple.Terminal", "nvim.exe"]);
        assert_eq!(
            store.load().unwrap().preferences.vim_mode_apps,
            ["com.apple.Terminal", "nvim.exe"]
        );

        // 与 valid_vim_mode_apps 同一口径：只差大小写的两条命中同一个应用，存储层拒绝，原名单不变。
        let duplicate: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": updated.revision,
            "vim_mode_apps": ["Code.exe", "code.exe"],
        }))
        .unwrap();
        assert!(update(directory.path(), &options, Edition::full(), &duplicate).is_err());
        assert_eq!(store.load().unwrap().preferences.vim_mode_apps.len(), 2);

        let cleared: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": updated.revision,
            "vim_mode_apps": [],
        }))
        .unwrap();
        let cleared = update(directory.path(), &options, Edition::full(), &cleared).unwrap();
        assert!(cleared.vim_mode_apps.is_empty());
    }

    /// AI 助手只能关不能开，关掉时接口地址、模型和密钥原样留着，用户在设置里重新打开就能接着用。
    #[test]
    fn the_ai_assistant_can_be_turned_off_but_not_on() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1}"#).unwrap();
        let store = PreferencesStore::new(directory.path());
        let mut preferences = Preferences::default();
        preferences.ai_assistant.enabled = true;
        preferences.ai_assistant.model = "synthetic-model".into();
        preferences.ai_assistant.tokens.insert(
            "https://synthetic.invalid:443".into(),
            "synthetic-token".into(),
        );
        store.save(0, preferences).unwrap();
        let before = load(directory.path(), Edition::full()).unwrap();
        assert!(before.ai_assistant);
        let view = serde_json::to_string(&before).unwrap();
        assert!(!view.contains("synthetic-token") && !view.contains("synthetic-model"));

        // 已经开着时给 true 什么也不改，不算打开。
        let unchanged: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": before.revision,
            "ai_assistant": true,
        }))
        .unwrap();
        let before = update(directory.path(), &options, Edition::full(), &unchanged).unwrap();
        assert!(before.ai_assistant);

        let off: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": before.revision,
            "ai_assistant": false,
        }))
        .unwrap();
        assert!(!off.is_empty());
        let after = update(directory.path(), &options, Edition::full(), &off).unwrap();
        assert!(!after.ai_assistant);
        let stored = store.load().unwrap().preferences.ai_assistant;
        assert!(!stored.enabled);
        assert_eq!(stored.model, "synthetic-model");
        assert_eq!(
            stored
                .tokens
                .get("https://synthetic.invalid:443")
                .map(String::as_str),
            Some("synthetic-token")
        );

        let on: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": after.revision,
            "ai_assistant": true,
        }))
        .unwrap();
        let refused = update(directory.path(), &options, Edition::full(), &on).unwrap_err();
        assert!(
            refused.contains("only the user can turn it on"),
            "{refused}"
        );
        let stored = store.load().unwrap();
        assert_eq!(stored.revision, after.revision);
        assert!(!stored.preferences.ai_assistant.enabled);
    }

    #[test]
    fn the_game_candidate_overlay_switch_leaves_the_process_lists_alone() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1}"#).unwrap();
        let store = PreferencesStore::new(directory.path());
        let mut preferences = Preferences::default();
        preferences.game_compatibility.overlay_processes = vec!["game.exe".into()];
        preferences.game_compatibility.excluded_processes = vec!["launcher.exe".into()];
        let saved = store.save(0, preferences).unwrap();
        let before = load(directory.path(), Edition::full()).unwrap();
        assert_eq!(before.revision, saved.revision);
        assert!(before.game_candidate_overlay);

        let off: PreferencesChange = serde_json::from_value(json!({
            "expected_revision": before.revision,
            "game_candidate_overlay": false,
        }))
        .unwrap();
        assert!(!off.is_empty());
        let updated = update(directory.path(), &options, Edition::full(), &off).unwrap();
        assert!(!updated.game_candidate_overlay);
        let stored = store.load().unwrap().preferences.game_compatibility;
        assert!(!stored.candidate_overlay);
        assert_eq!(stored.overlay_processes, ["game.exe"]);
        assert_eq!(stored.excluded_processes, ["launcher.exe"]);

        let on = PreferencesChange {
            game_candidate_overlay: Some(true),
            ..change(updated.revision)
        };
        assert!(
            update(directory.path(), &options, Edition::full(), &on)
                .unwrap()
                .game_candidate_overlay
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn an_unreadable_runtime_options_document_does_not_leave_preferences_saved() {
        let directory = tempfile::tempdir().unwrap();
        // 目录无法作为 runtime-options 文档打开，因此发布步骤会在偏好存储接受修改后失败。
        let before = load(directory.path(), Edition::full()).unwrap();
        let change = PreferencesChange {
            candidate_page_size: Some(7),
            ..change(before.revision)
        };

        assert_eq!(
            update(directory.path(), directory.path(), Edition::full(), &change).unwrap_err(),
            "cannot read the runtime options"
        );
        // 回滚本身也是一次按修订号的保存，revision 照常往前走（并发的写入因此能胜过它）；内容回到修改之前。
        let after = load(directory.path(), Edition::full()).unwrap();
        assert_eq!(after.revision, before.revision + 2);
        assert_eq!(
            PreferencesView {
                revision: before.revision,
                ..after
            },
            before
        );
    }
}
