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
    /// 与 `translation` 同源的 UTF-8 字节范围；空时省略，旧宿主可忽略。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub translation_runs: Vec<msime_client_core::translation::TranslationRun>,
    /// 脚本插件给这一行加的注释（≤ 32 个 UTF-16 码元，不含控制和格式字符），和引擎的 `annotation` 分开放：宿主按 `annotation` 画辅助码、韩文汉字释义，插件的注释不能混进去。没有时不出现在 JSON 里，只在桌面编译。
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_annotation: Option<String>,
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
    /// 引擎当前的组码网格：`"none"`、`"nine_key"`、`"fourteen_key"` 或 `"nine_key_shuangpin"`。宿主据此决定是否显示拼音条、读音行和三栏面板：几种网格共用 `nine_key_spellings`、`nine_key_reading`、`nine_key_single_character` 和 `nine_key_strokes`。九键双拼的数字和九键一样不当选词键，但 `nine_key` 为假（它没有英文九键，也不开注音九键）。
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
    /// 已应用的引号与方头括号样式和顿号键（`quote_style`、`bracket_style`、`enumeration_comma_*`，`corner_when_traditional` 已按繁体开关解析、两个顿号键已按「至少留一个」归一）。宿主画键面、做成对补全和英文模式下的中文标点时读它，不各自再解析偏好。和引擎同步：偏好改了要等空闲重建后才变。
    pub punctuation_marks: PunctuationMarks,
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
    /// 行内画出来的读音：日文罗马字末尾单个 `n` 留作字母（`かn`），其余与 [`View::reading`] 相同。`reading` 才是确定时上屏的读音，宿主不经引擎结束组字区、且组字区还在时应换成 `reading` 再结束（规则见 `msime_client.h` 的 `View.display_reading`）。
    pub display_reading: String,
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
    /// 这次上屏的是脚本插件在 `/` 指令里给的行（来源 14）。只给 host-api 判断要不要转全角（插件行同网址一样原样上屏），不上线缆。
    #[serde(skip)]
    pub plugin_row: bool,
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

/// 用户是经哪条路径选中候选的。
///
/// 每个取值对应 `Runtime::dispatch` 里一处把候选交给引擎的调用点。bare 的 `/`、`@` 前缀按标点或原样结束、注音把空格当拼写符号、候选为空时的 `SelectHighlighted`（笔画、藏文上屏方案自己的文本）都不是选择，不在这里。判别值是将来脚本 ABI 的一部分，只能追加，不能改号。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SelectionHow {
    /// `Action::Finish`：从高亮行结束组字。
    Finish = 0,
    /// 标点结束组字，`punctuation` 和 `punctuation_ascii` 两条路由都算；标点本身记在 `DispatchFacts::tail_chars` 里。
    PunctuationFinish = 1,
    /// 五笔四码之后再打一个字母，顶出首选。
    WubiTop = 2,
    /// 引擎交还的空格落在打开的候选列表上，选高亮行。
    SpaceOverList = 3,
    /// 数字键选当前页的一格。
    Digit = 4,
    /// 14 键的原样上屏：组码不是可上屏的字母，从高亮行结束组字。
    GridRaw = 5,
    /// `Action::Select`，即点选当前页的一行。
    Select = 6,
    /// `Action::SelectAnyCandidate`，从完整列表里点选。
    AnyCandidate = 7,
    /// `Action::SelectEdge`，以词定字取首字或尾字。
    Edge = 8,
    /// `Action::SelectHighlighted` 在有候选时选高亮行，包括韩文汉字列表的确认。
    Highlighted = 9,
    /// 五笔唯一四码在第四键自动上屏。
    WubiAuto = 10,
    /// 九宫格组码到上限后再按一键，先选掉首选再接着打。
    GridLimit = 11,
}

/// 选中之后发生了什么。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SelectionOutcome {
    /// 选中的文本上屏，组字随之结束（顶字时下一段组字是新的一段，也算这一类）。
    Committed = 0,
    /// 选中的词段暂存在 `View::phrase_prefix` 里，等整个词组完成再上屏。
    Held = 1,
    /// 整句改字里钉住一段，不上屏。
    Pinned = 2,
    /// 选中的文本已上屏，但它只用掉了一部分读音，其余仍在组字。
    Partial = 3,
}

/// 一次派发里被选中的候选。只有候选文本和引擎的上屏能对上时才会记录。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionFact {
    /// 候选在呈现顺序里的座位（整个列表的下标，不是页内下标），即宿主看到的 `Candidate::id.index`。
    pub seat: u32,
    /// 这个座位上候选的来源，同 `Candidate::source`。
    pub source: u8,
    pub how: SelectionHow,
    pub outcome: SelectionOutcome,
}

/// 撤回暂存词组里最近一次选择的方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SelectionRetraction {
    /// 退格：把这次选择用掉的读音还回来。
    Restore = 0,
    /// Ctrl+退格：删掉这一段，读音不还。
    Drop = 1,
}

/// 最近一次 `Runtime::dispatch` 或 `Runtime::focus` 里"发生了什么"。
///
/// 这是 `Transition` 的旁路：`Transition` 和它的 JSON 是所有宿主共用的契约，生命周期核心需要的额外事实不往里加，而是在调用返回后经 `Runtime::facts()` 读取。每次进入 `dispatch` 和 `focus` 时整体重置，所以读到的永远只属于上一次调用。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DispatchFacts {
    /// 本次选中的候选；没有选择、或者选择没能和上屏对上时为 `None`。
    pub selection: Option<SelectionFact>,
    /// 本次撤回了暂存词组里的一次选择。
    pub retracted: Option<SelectionRetraction>,
    /// 本次有文本进了暂存词段。普通字符键也可能让它变长，所以它本身不代表"已选择"。
    pub held_grew: bool,
    /// 暂存的词段随本次上屏一起送出（结束组字、标点、失焦）。被 Esc 丢掉时为假。
    pub held_flushed: bool,
    /// 本次上屏是原样上屏的读音（`Command::CommitRaw`，以及 bare 的 `/`、`@` 前缀），不是选词。
    pub raw: bool,
    /// 上屏文本末尾有几个字符（Unicode 标量）是接在选中文本后面的尾巴：结束组字的标点、顶字时打出的那个键。从末尾数，所以前面拼上暂存段不影响它；按字符数而不是字节数记，所以全角转换之后仍然切在同一处。
    pub tail_chars: u32,
}

impl DispatchFacts {
    /// 按 `tail_chars` 把上屏文本切成 body 和 tail。`commit` 可以是宿主做过逐字符转换（全角）之后的文本。
    pub fn split_commit<'a>(&self, commit: &'a str) -> (&'a str, &'a str) {
        let tail = self.tail_chars as usize;
        if tail == 0 {
            return (commit, "");
        }
        match commit.char_indices().rev().nth(tail - 1) {
            Some((index, _)) => commit.split_at(index),
            None => ("", commit),
        }
    }
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
    /// 参与陈旧判定：思考强度变了，在途的旧档位回答不算数（Windows 把它计入 AI 缓存键）。
    #[serde(default)]
    pub thinking: String,
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

/// 在线查询包含完整 HTTP 描述符，允许模板与请求头在 JSON 中再次转义。
pub const MAX_ONLINE_QUERY_BYTES: usize = 131_072;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OnlineQuery {
    #[serde(
        default,
        skip_serializing_if = "msime_engine::OnlineInputMode::is_pinyin"
    )]
    pub mode: msime_engine::OnlineInputMode,
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
    /// 共享层生成的 HTTP 描述符；凭据只经过平台传输，不进入签名和日志。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_request: Option<msime_client_core::cloud::candidates::CloudHttpRequest>,
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
    #[serde(rename = "ai")]
    Ai,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_assistant: Option<serde_json::Value>,
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
