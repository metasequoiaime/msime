//! The values that cross the runtime's edges - candidates, the view the host draws,
//! and the provider configuration and queries the host supplies.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct CandidateId {
    pub session: u64,
    pub generation: u64,
    pub index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct NineKeySpellingId {
    pub session: u64,
    pub generation: u64,
    pub index: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Candidate {
    pub id: CandidateId,
    pub text: String,
    /// Engine input code that produced this candidate, aligned with `text`.
    /// Presentation layers may use it for scheme-specific hints without changing selection.
    pub code: String,
    /// Engine-derived display suffix, never part of selection or committed text.
    pub annotation: String,
    /// Engine candidate source, stable for the lifetime of this view.
    pub source: u8,
    /// True when Engine corrected the typed spelling for this candidate.
    /// Presentation layers may mark it without changing committed text.
    pub corrected: bool,
    /// Engine fixed-position slot, or zero when dynamically ranked.
    pub fixed_position: u8,
    pub highlighted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
}

/// The candidate fields needed by translation providers, without copying the rest of a display row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranslationCandidate {
    pub text: String,
    pub source: u8,
}

/// Lightweight snapshot used when a host is deciding which visible candidates need glosses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranslationCandidates {
    pub generation: u64,
    pub scheme: u8,
    pub local_mode: String,
    pub candidates: Vec<TranslationCandidate>,
}

/// On-demand copy of every candidate owned by one Engine generation.
///
/// Regular [`View`] values remain page-bounded so hosts do not pay to serialize
/// the complete candidate list after every input action.
#[derive(Clone, Debug, Serialize)]
pub struct CandidateSnapshot {
    pub session: u64,
    pub generation: u64,
    pub preedit: String,
    /// Engine-owned kana reading for Japanese, the composing Hangul for Korean; empty for other schemes.
    pub reading: String,
    pub candidates: Vec<Candidate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum CharacterWidth {
    Fullwidth,
    Halfwidth,
}

#[derive(Clone, Debug, Serialize)]
pub struct View {
    pub scheme: u8,
    /// The active scheme writes Chinese (`SchemeType::is_chinese`): what 中文 returns to and what the Chinese statistics count. Hosts branch on this rather than on scheme ordinals.
    pub chinese_text: bool,
    /// The host's Simplified-to-Traditional conversion applies to this view's preedit and to its commits: a scheme it applies to (`SchemeType::script_conversion_applies`) outside the `unicode` and `temporary_japanese` modes, whose text is not Chinese to convert.
    pub script_conversion: bool,
    /// 引擎的触屏九键模式：开着时数字是输入，不当选词键。只表示九键（全拼九宫格或注音九键），14 键下为假。
    pub nine_key: bool,
    /// 引擎当前的组码网格：`"none"`、`"nine_key"` 或 `"fourteen_key"`。宿主据此决定是否显示拼音条、读音行和三栏面板：两种网格共用 `nine_key_spellings`、`nine_key_reading`、`nine_key_single_character` 和 `nine_key_strokes`。
    pub key_grid: &'static str,
    pub nine_key_spellings: Vec<String>,
    /// 全拼九键组字时，首选候选覆盖的数字显示成它的拼音（`xi'an`），给键盘的读音行用；其他情况为空，包括注音九键。全拼下 `preedit` 仍是数字；注音下是转换结果加上未完成的数字。
    pub nine_key_reading: String,
    /// 九宫格候选当前只留单字（`Action::SetNineKeyFilter`）。组字结束时恢复为假。
    pub nine_key_single_character: bool,
    /// 九宫格候选当前按这几笔的笔顺前缀筛选（`hspnz`），空表示不按笔画。组字结束时清空。
    pub nine_key_strokes: String,
    /// Applied touch presentation, independent of Engine-owned Chinese nine-key digit handling.
    pub touch_keyboard_layout: TouchKeyboardLayout,
    /// Applied Engine configuration, not a newer deferred preference snapshot.
    pub character_width: CharacterWidth,
    pub microsoft_shuangpin: bool,
    pub shuangpin_profile: String,
    pub answered_by_pinyin_fallback: bool,
    /// Authoritative Engine mode, never inferred from displayed text.
    pub local_mode: String,
    /// The non-letter characters the Engine takes as input in this state: the active local mode's spelling (digits and operators in `expression`, digits in `unicode`), or with nothing composed the keys that open a mode (`/`, `@`). A host sends these as characters, and treats a digit listed here as input rather than a candidate shortcut; the runtime already routes them away from punctuation.
    pub spelling_symbols: String,
    /// Authoritative Engine English mode, independent of temporary local modes.
    pub dedicated_english: bool,
    pub session: u64,
    pub generation: u64,
    pub focused: bool,
    pub preedit: String,
    /// The already chosen part of a phrase still being composed, which the host draws ahead of the
    /// editing text rather than receiving as a commit. Empty unless the host asked for it.
    ///
    /// It is a field of its own rather than a prefix on `editing_text` because `caret_position` is
    /// an offset into that text, and the hosts each read it in their own string unit - the two have
    /// agreed so far only because the editing text is ASCII. A host prepends this itself and moves
    /// its own caret by this string's length in whatever unit it measures.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub phrase_prefix: String,
    /// Engine-owned kana reading for Japanese, the composing Hangul for Korean (the text a host marks inline); empty for other schemes.
    pub reading: String,
    pub editing_text: String,
    /// Byte offset in Engine's ASCII editing_text, not an OS UTF-16 offset.
    pub caret_position: usize,
    /// 整句改字时改好的整句，宿主在它非空时把它（接在 `phrase_prefix` 后面）画在行内代替拼音，光标画在 `conversion_focus_start` 处，`conversion_focus_start..conversion_focus_end` 是候选所替换的那一段；候选是这一段的字和词。不在改字时为空。
    pub conversion: String,
    /// 光标在 `conversion` 的第几个字之前，按 Unicode 标量计，宿主自己换算成它的字符串单位。
    pub conversion_focus_start: usize,
    /// 光标处那一段的结尾，按 Unicode 标量计；光标在句末时等于 `conversion_focus_start`，没有候选。
    pub conversion_focus_end: usize,
    pub page: usize,
    pub page_size: usize,
    pub page_count: usize,
    /// The scheme's openable candidate list (the Korean Hanja list) is showing. Hosts read this instead of inferring it from the scheme and a non-empty candidate list.
    pub candidate_list_open: bool,
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Serialize)]
pub struct OutputContext {
    pub scheme: u8,
    pub local_mode: String,
    /// Whether the host's Simplified-to-Traditional conversion applies to this commit, decided by the mode the commit was made in ([`View::script_conversion`]).
    pub script_conversion: bool,
    /// Whether the host counts this commit in typing statistics. False for text the Engine generated in the expression, command and mention modes, which the user did not type out.
    pub typing_statistics: bool,
}

#[derive(Debug, Serialize)]
pub struct Transition {
    pub handled: bool,
    pub commit: Option<String>,
    /// Mode before dispatch; committing may clear a local mode or apply deferred settings.
    pub commit_context: Option<OutputContext>,
    pub diagnostic: Option<String>,
    pub view: View,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AiAssistantProviderConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default = "default_ai_candidate_limit")]
    pub candidate_limit: u8,
    #[serde(default)]
    pub prompt_id: String,
    #[serde(default)]
    pub prompt_custom_1: String,
    #[serde(default)]
    pub prompt_custom_2: String,
    #[serde(default)]
    pub prompt_custom_3: String,
}

fn default_ai_candidate_limit() -> u8 {
    3
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OnlineQuery {
    /// Recent committed text supplied by the focused host, bounded to 1024 UTF-8 bytes.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ai_context: String,
    pub scheme: u8,
    pub generation: u64,
    pub identity: String,
    pub query_text: String,
    pub cache_key: String,
    pub pinyin_segments: Vec<String>,
    pub cloud_eligible: bool,
    pub ai_eligible: bool,
    /// Host preference controlling whether a provider may return cloud suggestions.
    #[serde(default = "default_cloud_candidates")]
    pub cloud_candidates: bool,
    pub session_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_assistant: Option<AiAssistantProviderConfig>,
    /// Ask a provider for its cached AI candidates only, never the network. Windows shows a cached AI answer as soon as the input changes and fetches only after the idle delay; the Linux hosts send this probe immediately and the ordinary request after the delay. Hosts never receive it from `online_query`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ai_cache_only: bool,
}

impl OnlineQuery {
    pub fn ai_candidate_limit(&self) -> usize {
        self.ai_assistant
            .as_ref()
            .filter(|assistant| assistant.enabled)
            .map_or(0, |assistant| {
                usize::from(assistant.candidate_limit.clamp(1, 10))
            })
    }
}

fn default_cloud_candidates() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TranslationProviderConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub api_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct NiuTransProviderConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub apikey: String,
}

/// The online translation service the user selected in 翻译服务. Exactly one is active, matching Windows `ActiveProvider()`: the hosted account is explicit, then NiuTrans wins over the custom endpoint, which wins over Tencent, and `Off` means no online translation at all. The choice travels even when the selected service is not fully configured, so a provider never falls back to a service the user did not pick.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranslationService {
    #[serde(rename = "none")]
    Off,
    #[serde(rename = "account")]
    Account,
    Tencent,
    NiuTrans,
    Custom,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// A manually requested sentence translation is deliberately separate from
/// the live candidate gloss path.  Hosts use it only after an explicit user
/// action, so a long composition never causes a network request on every
/// keystroke.
const fn default_sentence_translation() -> bool {
    false
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TranslationQuery {
    pub generation: u64,
    #[serde(default = "default_translation_target_language")]
    pub target_language: String,
    pub candidates: Vec<String>,
    #[serde(
        default = "default_sentence_translation",
        skip_serializing_if = "is_false"
    )]
    pub sentence: bool,
    /// The service the user selected; every query carries it so the provider never guesses.
    pub provider: TranslationService,
    /// The user explicitly selected the hosted MSIME translation account. Linux
    /// carries this flag through its provider socket; it is not a provider enum
    /// value because the provider owns the account credentials.
    #[serde(default, skip_serializing_if = "is_false")]
    pub translation_account: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_translation: Option<TranslationProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub niutrans: Option<NiuTransProviderConfig>,
}

/// The `/fy` request: English the user typed after the translate command, for the selected translation service to translate into Chinese. A host sends it as a one-item sentence `TranslationQuery` only when a service is selected, and hands the answer back with this value through `Runtime::apply_command_translation`, which puts it first as a row that commits the translation. It is the only request a local mode makes; `Runtime::online_query` stays `None` there.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CommandTranslation {
    pub generation: u64,
    pub session_id: u64,
    pub text: String,
}

fn default_translation_target_language() -> String {
    "en".into()
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TranslationResult {
    pub text: String,
    pub translation: String,
}

/// Result returned by a user-owned provider after it tests one configured
/// service. The provider keeps private credentials in its own process; hosts
/// receive only this bounded status.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CredentialTestResult {
    pub ok: bool,
    pub message: String,
}

/// A bounded stroke payload sent by a Linux handwriting panel to its
/// user-owned recognizer service. Coordinates are normalized panel pixels;
/// the recognizer decides how to map them to a platform model.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct HandwritingPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct HandwritingQuery {
    #[serde(default)]
    pub language: String,
    pub strokes: Vec<Vec<HandwritingPoint>>,
}

/// Search request for a standalone Linux emoji panel. The panel owns its
/// category/search UI while the provider supplies the catalog and annotations.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct EmojiPanelQuery {
    #[serde(default)]
    pub search: String,
    #[serde(default)]
    pub category: String,
    #[serde(default = "default_emoji_panel_limit")]
    pub limit: u8,
}

fn default_emoji_panel_limit() -> u8 {
    48
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct EmojiPanelItem {
    pub text: String,
    #[serde(default)]
    pub annotation: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnlineCandidate {
    pub query: OnlineQuery,
    pub text: String,
    /// 0 = cloud suggestion, 1 = AI suggestion.
    pub source: u8,
}
