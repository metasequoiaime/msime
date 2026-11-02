//! Versioned, thread-confined C interface for native IME hosts.
//! A handle registry rejects stale and wrong-thread handles without dereferencing them.

// The workspace denies unsafe code; this crate is the C ABI native hosts link against, so every handle,
// pointer and string it accepts crosses a boundary the compiler cannot check.
// The exemption is stated here rather than left implicit by opting out of
// the workspace lint table, which would also silently drop every other lint
// the workspace adds later.
#![allow(unsafe_code)]

use msime_client_core::edition::Edition;
use msime_client_core::host_surface::{
    offered_input_schemes, HostCapabilities, HostPlatform, SurfaceRoute,
};
use msime_client_providers::ai::AiSuggestionRequest;
use msime_client_services::dictionary::access::DictionaryAccess;
/// 按用户的设置打一串键看候选，给 `msime-mcp candidates` 用。
pub mod candidate_preview;
pub mod clipboard_history;
pub mod cloud_clipboard;
pub mod cloud_dictionary;
pub mod mcp_clients;
pub mod settings_dictionary;
pub mod system_fonts;
use msime_client_core::preferences::{
    InputScheme, KoreanLayout, KoreanPreferences, MultilingualLanguage, NineKeyShuangpinProfile,
    Preferences, PreferencesSnapshot, PreferencesStore, ShuangpinCustomProfile, ShuangpinProfile,
    TouchKeyboardLayout, TouchKeyboardScheme, TouchShuangpinLayout, TouchWubiLayout,
    VietnameseInputMethod, VietnamesePreferences, VietnameseToneStyle, WubiProfile,
};
use msime_client_core::punctuation::{
    route as punctuation_route, PunctuationContext, PunctuationRoute,
};
use msime_client_core::resources::{ResourceSet, ResourceStore, VerifiedMarker};
use msime_client_providers::resource_packs::{self, ResourcePack};
use msime_client_providers::voice::doubao_frame::{
    audio_frame, decode_error_code, decode_json_frame, start_frame,
};
use msime_client_providers::voice::VoiceSessionState;
use msime_client_services::typing_statistics::{
    CommitEfficiency, TypingSource, TypingStatisticsStore, RANKS,
};
use msime_engine::host::{
    check_nine_key_shuangpin_table, check_shuangpin_custom_profile,
    nine_key_shuangpin_builtin_table, nine_key_shuangpin_table_key_hints, release_gloss_caches,
    shuangpin_builtin_table, Command, EngineOptions, Session,
};
use msime_engine::{
    release_idle_personal_models, release_japanese_dictionary_cache,
    resolve_eighteen_key_shuangpin, resolve_fourteen_key_shuangpin, resolve_nine_key_shuangpin,
    validate_nine_key_shuangpin_table, validate_shuangpin_custom_profile, CandidateEdge,
    NineKeyShuangpinError, NineKeyShuangpinKeyHint, NineKeyShuangpinKind, NineKeyShuangpinTable,
    SchemeSet, SchemeType, ShuangpinCustomProfileError, ShuangpinCustomTable,
    ShuangpinCustomTablePart, ShuangpinProfileKind,
};
#[cfg(unix)]
use msime_input_runtime::TranslationQuery;
#[cfg(unix)]
use msime_input_runtime::UnixSocketProvider;
use msime_input_runtime::{
    Action, AiAssistantProviderConfig, CandidateId, CharacterWidth, GlideKeyboard, GlidePoint,
    KeyGrid, NineKeySpellingId, OnlineCandidate, OnlineQuery, Reranker, Runtime, SentenceModel,
    Transition, TranslationService,
};
use msime_input_runtime::{EmojiPanelQuery, HandwritingQuery};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, CString};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
// Only the Unix socket streaming entry point takes raw callback context.
#[cfg(unix)]
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
/// `msime_client_account` 在 #8201 新增的操作。
mod account_ops;
mod bounded_file;
mod chinese_english_spacing;
/// 多分类自动云同步的驱动；C 入口在 `ffi::cloud_sync`。与 `cloud_dictionary` 一样和 C 入口模块同名，所以公开，免得遮住 `ffi` 的同名模块。
pub mod cloud_sync;
mod dictionary;
/// 「我的词条」经 WebDAV 网盘同步的宿主入口，见 `msime_client_dictionary_sync`。公开 [`dictionary_webdav::request_json`]，给直接链接这个 crate 的 Tauri 设置应用用，免得它绕一圈 C ABI。
pub mod dictionary_webdav;
/// 同一台设备上两个各有一份个人词库的进程之间交换「我的词条」，见 `msime_client_user_words_mirror`。
mod user_words_mirror;
// The exports live in `ffi`, but Rust consumers - this crate's own tests and
// examples, and anything linking the rlib - have always reached them at the
// crate root. Re-exported so moving the file changes no caller.
/// Largest preference document a host may hand to the C ABI, in bytes.
///
/// A valid document carries the custom touch-keyboard skin, whose photo alone may be 682,668 base64 characters; at the 16 KiB every other buffer uses, a document holding a photo could never be written back, which failed every later preference write from that host.
pub(crate) const PREFERENCES_DOCUMENT_LIMIT: usize = 1 << 20;
/// Largest serialized HostOptions document accepted by native entry points.
/// Preferences may carry a base64 custom keyboard photo, so the old 16 KiB
/// ABI limit rejected valid saved preferences before a session could start.
pub(crate) const HOST_OPTIONS_DOCUMENT_LIMIT: usize = PREFERENCES_DOCUMENT_LIMIT;
/// Largest dictionary-management request: one HostOptions document plus the
/// bounded personal-dictionary import payload and a small amount of framing.
pub(crate) const DICTIONARY_REQUEST_LIMIT: usize = HOST_OPTIONS_DOCUMENT_LIMIT + 1_200_000;

pub(crate) fn valid_sha256(value: &str) -> bool {
    msime_client_core::is_ascii_hex(value, 64)
}

pub(crate) fn valid_uuid_string(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

mod ffi;
pub use ffi::*;
pub mod local_session;

/// 会话实际用的每页候选数：偏好或宿主覆盖的值，按本库编译到的平台能排的上限截断（`HostPlatform::max_candidate_page_size`）。共享文档在别的设备上存了 10 时，只排得下九个的宿主仍拿到 9。
fn host_page_size(requested: u8) -> u8 {
    requested.min(ffi::host::compiled_platform().max_candidate_page_size())
}

/// 会话的候选窗是否竖排：桌面宿主按 `candidate_layout` 画候选窗，触屏宿主的候选栏总是横排，不读这一项（它的缺省是竖排，照读会让外接键盘的左右键不再选词）。
fn host_vertical_candidates(layout: msime_client_core::preferences::CandidateLayout) -> bool {
    ffi::host::compiled_platform().is_desktop()
        && layout == msime_client_core::preferences::CandidateLayout::Vertical
}

/// 会话实际用的中英混输触发字符数：本库编译到的平台不读这一项时（macOS、Windows 和 Linux 三个桌面宿主，`HostPlatform::reads_mixed_input_minimum_prefix`）固定为默认值，文档里存的数不起作用，设置页也不再给出这一行。
fn host_english_minimum_prefix(stored: u8) -> u8 {
    english_minimum_prefix_for(ffi::host::compiled_platform(), stored)
}

fn english_minimum_prefix_for(
    platform: msime_client_core::host_surface::HostPlatform,
    stored: u8,
) -> u8 {
    if platform.reads_mixed_input_minimum_prefix() {
        stored
    } else {
        msime_client_core::preferences::DEFAULT_MIXED_INPUT_MINIMUM_PREFIX
    }
}

/// 会话实际用的 AI 联想候选数量：本库编译到的平台固定了这个数（`HostPlatform::fixed_ai_candidate_limit`，macOS、Windows、Linux、鸿蒙）时用固定值，不读偏好里存的值；否则用偏好。座位配置、在线查询和请求描述都从这里取，`chat_completion_http_request` 要求请求和配置里的数量一致。
fn host_ai_candidate_limit(stored: u8) -> u8 {
    ffi::host::compiled_platform()
        .fixed_ai_candidate_limit()
        .unwrap_or(stored)
}
mod doubao_auth;
#[cfg(not(any(target_os = "android", target_env = "ohos")))]
mod handwriting_cells;
pub use doubao_auth::msime_client_doubao_auth_headers;
mod learned_translation;
mod niutrans_translation;
mod pronunciation;
mod supplementary_glosses;
mod tencent_translation;
mod word_breakdown;
pub use dictionary::{
    dictionary_request_json, dictionary_words, edit_dictionary_word, edit_user_quick_phrase,
    import_dictionary_words, lookup_candidates, msime_client_dictionary,
    msime_client_dictionary_import_entries, msime_client_dictionary_scel_text,
    msime_client_dictionary_validate, msime_client_personal_dictionary_request,
    msime_client_personal_dictionary_sync, personal_dictionary_request_json, scel_text_json,
    user_quick_phrases, CandidateOrigin, DictionaryOptions, LookupCandidate, LookupScheme, NewWord,
    QuickPhrase, QuickPhraseEdit, QuickPhrasePage, Word, WordEdit, WordImport, WordKind, WordPage,
};
mod diagnostics;
pub use diagnostics::{msime_client_set_diagnostic_sink, DiagnosticSink};
mod dictionary_snapshot;
mod dictionary_updates;
mod key_sound;
// 生命周期核心；`pub` 是为了让集成测试（零分配断言）直接调用纯推导函数。
pub mod lifecycle;
mod plugin_tables;
// 脚本插件：进程开关、每会话的实例和进程级的模块缓存、熔断。移动端是空壳。
mod scripts;
mod voice_capture;
pub use dictionary_snapshot::{
    inspect_snapshot_json, msime_client_snapshot_discard, msime_client_snapshot_inspect,
    msime_client_snapshot_prepare, msime_client_snapshot_queue, msime_client_snapshot_restore,
    msime_client_snapshot_version, snapshot_queue_json,
};

/// Capture endpoint identities paired with labels. Neither belongs in logs.
pub fn voice_capture_devices() -> Vec<(String, String)> {
    voice_capture::capture_devices()
}

/// A device listing as the shared settings page's 「可用录音设备」 reads it: `[{backend, id, label}]` in listing order, `backend` the value `voice_input.capture_backend` takes when the user picks the device and `id` the one `voice_input.capture_device` takes. The Tauri settings shell (Windows and macOS) and `msime_client_voice_capture_devices` both shape their listing here.
pub fn voice_capture_device_values(backend: &str, devices: Vec<(String, String)>) -> Vec<Value> {
    devices
        .into_iter()
        .map(|(id, label)| json!({"backend": backend, "id": id, "label": label}))
        .collect()
}

/// Capture bounded mono 16 kHz PCM for a platform host.
pub fn voice_capture_pcm(milliseconds: u32) -> Result<Vec<f32>, &'static str> {
    if !(1..=60_000).contains(&milliseconds) {
        return Err("invalid voice capture duration");
    }
    let samples = voice_capture::capture_audio(milliseconds);
    if samples.is_empty() {
        return Err("voice capture unavailable");
    }
    Ok(samples)
}

/// Run the optional offline Engine handwriting recognizer for a panel host. The caller must provide a trusted absolute model path. The shared panel uses a 420 by 420 canvas, which is also the coordinate space passed to the Engine. This path needs no provider socket, so every host can use it.
pub fn handwriting_local_candidates(
    model_path: &str,
    query: &HandwritingQuery,
) -> Result<Vec<String>, &'static str> {
    if !std::path::Path::new(model_path).is_absolute() {
        return Err("model path must be absolute");
    }
    engine_handwriting_candidates(model_path, query, 420.0, 420.0)
}

#[cfg(not(any(target_os = "android", target_env = "ohos")))]
fn engine_handwriting_candidates(
    model_path: &str,
    query: &HandwritingQuery,
    width: f32,
    height: f32,
) -> Result<Vec<String>, &'static str> {
    let mut strokes = Vec::with_capacity(query.strokes.len());
    strokes.extend(query.strokes.iter().map(|stroke| {
        let mut points = Vec::with_capacity(stroke.len());
        points.extend(stroke.iter().map(|point| (point.x, point.y)));
        points
    }));
    let recognize = |strokes: &[Vec<(f32, f32)>]| {
        msime_engine::handwriting_recognize(model_path, strokes, width, height)
            .map_err(|_| "local handwriting recognizer unavailable")
    };
    // The Engine classifies one character per call and normalises each call's own bounding box, so a written line is split into character cells and each cell is classified on its own, as the Windows Ink recognizer segments a line into a multi-character candidate.
    let cells = handwriting_cells::segment_handwriting_cells(&strokes);
    if cells.len() < 2 {
        return recognize(&strokes);
    }
    let mut per_cell = Vec::with_capacity(cells.len());
    for cell in &cells {
        let mut cell_strokes = Vec::with_capacity(cell.len());
        cell_strokes.extend(cell.iter().map(|&index| strokes[index].clone()));
        per_cell.push(recognize(&cell_strokes)?);
    }
    let combined = handwriting_cells::combine_cell_candidates(per_cell);
    if combined.is_empty() {
        return recognize(&strokes);
    }
    Ok(combined)
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
fn engine_handwriting_candidates(
    _model_path: &str,
    _query: &HandwritingQuery,
    _width: f32,
    _height: f32,
) -> Result<Vec<String>, &'static str> {
    // 共享桌面面板入口在移动端禁用；Android 注入 ML Kit，鸿蒙原生手写板通过 msime_client_handwriting 调用引擎的单字识别器。
    Err("local handwriting recognizer unavailable")
}

thread_local! {
    static SESSIONS: RefCell<HashMap<u64, HostSession>> = RefCell::new(HashMap::new());
}

struct HostSession {
    runtime: Runtime,
    options: EngineOptions,
    /// HostOptions 里宿主声明的「快捷短语混进普通候选」能力（macOS、Linux 请求）：偏好重建 `options` 时和 `mixed_input.quick_phrase` 相与；记在会话上是因为 `options` 里那个字段已经和能力与偏好相与过，认不出宿主原来要不要这项能力。
    quick_phrase_candidates_requested: bool,
    /// 原始宿主能力；偏好重建时不能从已经回退的有效布局推断它。
    korean_romaja_requested: bool,
    /// HostOptions 记录的版本：偏好里的方案只在本版本提供的方案里取，回退到本版本的默认方案。
    edition: &'static Edition,
    applied: Preferences,
    requested: Option<PreferencesSnapshot>,
    /// `requested` 的宿主状态（各个 setter、AI 配置缓存、音效设置）都已按它设好。上一次 `update` 在设到一半时出错，就还是 `false`，同一份快照再来时要完整重做，不能走「没变」的捷径。
    requested_settled: bool,
    /// Whether the requested preferences still need an Engine replacement. This decision is made
    /// when the document arrives so every keystroke does not compare the full preference tree.
    preferences_pending: bool,
    punctuation_override: Option<bool>,
    paired_punctuation_override: Option<bool>,
    punctuation_lock_override: Option<u8>,
    english_mode: bool,
    /// 宿主经 `msime_client_set_caps_lock` 报告的大写锁定状态。不报告的宿主一直是 `false`，`caps_lock_ascii_punctuation` 对它不起作用。
    caps_lock: bool,
    page_size_override: Option<u8>,
    /// 宿主经 `msime_client_set_key_grid` 或 `msime_client_set_nine_key_mode` 设下的组码网格，`Some(None)` 是宿主关掉了网格。只对设下它时的方案和布局有效，见 `apply_pending`。
    key_grid_override: Option<Option<KeyGrid>>,
    /// 宿主经 `msime_client_set_private_session` 标出的隐私会话（Android 的隐私模式和不允许学习的输入框，鸿蒙和 iOS 的隐私模式）：不记选词位置和上屏效率。与用户自己关掉的「学习」无关。
    statistics_private: bool,
    /// 只用来看候选的会话（`ffi::session::SessionUse::Preview`）：在任何平台上都不记选词位置和上屏效率。
    preview: bool,
    /// 宿主在会话选项里写了 `selection_statistics: false`：选词由另一个会话记，这个会话不记选词位置和上屏效率。
    statistics_recorded_elsewhere: bool,
    /// An AI provider credential the host keeps outside the preferences (the iOS Keychain), handed over for this session only and never written back.
    ai_credential: Option<SessionAiCredential>,
    /// Cached copy used by every online query until preferences change.
    ai_provider_cache: Option<AiAssistantProviderConfig>,
    voice: VoiceSessionState,
    /// Committing candidate selections counted but not yet written to typing statistics, indexed by one-based position minus one, with every position past a page in the last slot. See `SELECTION_BATCH`.
    pending_selections: [u64; RANKS + 1],
    /// 还没写进打字统计的上屏，和 `pending_selections` 一起写。只在 [`COUNTS_COMMIT_EFFICIENCY`] 时计；读音在写入时于后台线程查，不在输入线程上查。
    pending_efficiency: Vec<EfficiencyCandidate>,
    /// 本进程所知的打字统计开关（`TypingStatisticsStore::known_enabled`），每次交出选词时更新；`None` 是本进程还没读过统计文件，下次要计效率时再问。统计关闭时效率一项都不查。
    statistics_enabled: Option<bool>,
    /// Where this session's plugin packs, command tables and name list are read from.
    plugin_roots: key_sound::PluginRoots,
    /// The key sound, melody, commit, achievement and music settings of the newest preferences, which take effect at once rather than waiting for the composition to end: none of them is Engine state.
    sound: key_sound::SessionSound,
    /// The command-table and name-list files `options` was filled from.
    plugin_tables: plugin_tables::PluginTables,
    /// 按需下载资源包所在的状态目录（HostOptions 的 `preferences_directory`），不是绝对路径时为空。
    state_root: Option<PathBuf>,
    /// HostOptions 记录的 `language_dictionaries` 目录：资源包里没有某个词库时退回这里的那份。
    recorded_language_dictionaries: Option<PathBuf>,
    /// 会话打开后有资源包新装好（或被移除），`options` 里的词典路径已经更新，Engine 还要在输入空闲时重建。
    resources_pending: bool,
    /// HostOptions 记录的键盘模型路径（`sentence_model`），为 `None` 时用资源目录里的那份；`neural_keyboard` 在会话中途打开时按它加载逐键重排，见 [`ffi::keyboard_reranker`]。
    recorded_sentence_model: Option<String>,
    /// HostOptions 记录的落定重排模型路径（`settled_model`）：随包的模型优先于下载的资源包，见 [`settled_model_file`]。
    recorded_settled_model: Option<String>,
    /// 当前挂在 runtime 上的落定重排模型文件，没有模型时为 `None`。
    settled_model: Option<PathBuf>,
    /// runtime 上是否真挂着落定重排模型：`settled_model` 有路径而文件读不了或不是可用的模型时为 `false`。只用于诊断，不看 runtime 自己的槽，因为后台评分进行中模型被评分任务借走，槽是空的。
    settled_model_attached: bool,
    /// 聚焦时发现落定重排模型的文件变了（资源包刚装好或被移除），`settled_model` 已经更新，新模型在后台线程加载到这个槽里；槽有值后，下一次输入空闲的 `apply_pending` 把它换上并清掉这一项。为 `None` 时没有待换的模型。
    settled_model_loading: Option<SettledModelSlot>,
    /// 生命周期核心的状态，只由 `msime_client_set_lifecycle` 设上。为 `None` 时（移动端宿主从不调用它）不推导任何事件，transition JSON 逐字节不变。
    lifecycle: Option<Box<lifecycle::LifecycleState>>,
    // Declared after runtime so the Engine is dropped before releasing access.
    _dictionary_access: DictionaryAccess,
    /// 资源是下载的词库代次时占用它，设置宿主回收旧版本时不会删掉会话还在读的目录；同样放在 runtime 之后，Engine 先关掉文件再放手。
    _generation_lease: Option<msime_client_providers::dictionary_updates::GenerationLease>,
}

#[derive(PartialEq, Eq)]
struct SessionAiCredential {
    endpoint: String,
    token: String,
}

/// 本地模式由偏好开关，但它们读的词典是不可变的运行时资源。可选资源不在时，触发键不能被吞掉：在资源齐全之前，Engine 看到的这个模式必须是关的。
///
/// 版本不提供的功能同样当作资源不在：五笔版的资源锁不带日文词典和键盘神经模型，临时日文、日文混输和键盘神经联想无论偏好怎么写都是关的。资源目录按版本的锁校验，本来就放不进日文词典，这里再按版本表关一次，是为了状态目录里留有下载来的日文资源包时也不会打开它。
/// 偏好里拆字开关的取值：用户没动过时按宿主形态决定（见 `LocalModePreferences::chaizi`）——触屏宿主读作开（屏幕键盘从菜单进入），桌面宿主读作关（打开后句首和中英混打的大写 C 会进入模式）。Windows v0.11.1 的 `radical_mode` 出厂为开，与本仓桌面缺省不同，差异已记录，是否对齐留待产品决策。
const TOUCH_KEYBOARD_HOST: bool = cfg!(any(
    target_os = "android",
    target_os = "ios",
    target_env = "ohos"
));

fn chaizi_enabled(preferences: &msime_client_core::preferences::LocalModePreferences) -> bool {
    preferences.chaizi.unwrap_or(TOUCH_KEYBOARD_HOST)
}

/// 「移动光标后编辑光标后的部分」只在三个桌面宿主上交给 Engine：偏好随同步到达手机时，触屏宿主没有设置入口、也不画模式提示，所以那里一律按关。
const CARET_EDIT_SUFFIX_HOST: bool = cfg!(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
));

fn caret_edit_suffix_applies(preferences: &Preferences) -> bool {
    CARET_EDIT_SUFFIX_HOST && preferences.caret_edit_suffix
}

fn multilingual_languages_for(
    preferences: &Preferences,
    edition: &Edition,
    has_japanese_model: bool,
) -> Vec<msime_engine::multilingual::Language> {
    // 连续多语言解码仅接在全拼；五笔混拼不等于提供全拼输入方案。
    if !edition.offers(InputScheme::Quanpin) {
        return Vec::new();
    }
    let languages = preferences
        .multilingual_input
        .active_languages()
        .into_iter()
        .filter_map(|language| match language {
            MultilingualLanguage::Chinese => Some(msime_engine::multilingual::Language::Chinese),
            MultilingualLanguage::English => Some(msime_engine::multilingual::Language::English),
            MultilingualLanguage::Japanese
                if has_japanese_model && edition.features.temporary_japanese =>
            {
                Some(msime_engine::multilingual::Language::Japanese)
            }
            MultilingualLanguage::Japanese => None,
        })
        .collect::<Vec<_>>();
    if languages.len() >= 2 {
        languages
    } else {
        Vec::new()
    }
}

fn japanese_model_available(options: &EngineOptions) -> bool {
    if options.japanese_dictionary.is_empty() {
        Path::new(&options.resources)
            .join("msime-japanese.dat")
            .is_file()
    } else {
        Path::new(&options.japanese_dictionary).is_file()
    }
}

fn apply_local_mode_resource_gates(options: &mut EngineOptions, edition: &Edition) {
    let resources = std::path::Path::new(&options.resources);
    let has_emoji_catalog = resources.join("msime-others.db").is_file();
    let has_english_dictionary = resources.join("msime-english.db").is_file();
    // 下载的日文词典写在 `japanese_dictionary` 里；为空时 Engine 读资源目录里的那份。
    let has_japanese_model = if options.japanese_dictionary.is_empty() {
        resources.join("msime-japanese.dat").is_file()
    } else {
        Path::new(&options.japanese_dictionary).is_file()
    };
    options.local_emoji &= has_emoji_catalog;
    options.local_kaomoji &= has_emoji_catalog;
    options.local_temporary_english &= has_english_dictionary;
    options.local_temporary_japanese &= has_japanese_model && edition.features.temporary_japanese;
    // 日文混输和临时日文读同一份模型、同一个日文 provider（会话方案集里有 `JapaneseRomaji` 才建），门槛也一样；模型不在时只剩假名，不算转换。
    options.mixed_japanese &= has_japanese_model && edition.features.temporary_japanese;
    options.multilingual_languages.retain(|language| {
        *language != msime_engine::multilingual::Language::Japanese
            || (has_japanese_model && edition.features.temporary_japanese)
    });
    if options.multilingual_languages.len() < 2 {
        options.multilingual_languages.clear();
    }
    // 引擎里的键盘模型读的是同一份随版本资源锁走的模型；宿主不打开它（见 `engine_sentence_association`），不带模型的版本更不交给 Engine。
    options.sentence_association.neural_keyboard &= edition.features.neural_keyboard;
}

/// 整句联想「增强」挂不挂 runtime 的逐键重排器（[`ffi::keyboard_reranker`]）：偏好打开、且本版本带键盘模型。它只管 runtime 这一层，引擎里的键盘模型不跟这个开关走，见 `engine_sentence_association`。
fn keyboard_reranker_enabled(
    preferences: &msime_client_core::preferences::SentenceAssociationPreferences,
    edition: &Edition,
) -> bool {
    preferences.neural_keyboard && edition.features.neural_keyboard
}

fn punctuation_lock_code(lock: msime_client_core::preferences::PunctuationLock) -> u8 {
    match lock {
        msime_client_core::preferences::PunctuationLock::Follow => 0,
        msime_client_core::preferences::PunctuationLock::Chinese => 1,
        msime_client_core::preferences::PunctuationLock::English => 2,
    }
}

/// The Engine's sentence association switches. Neither model switch reaches it: the desktop one gates the runtime's settled reranker (`Runtime::set_settled_rerank_enabled`), the only place that model runs, and the keyboard one (整句联想「增强」) mounts the runtime's per-keystroke reranker (`keyboard_reranker_enabled`), so the Engine's own keyboard-model rerank stays off.
fn engine_sentence_association(
    preferences: &msime_client_core::preferences::SentenceAssociationPreferences,
    state_root: Option<&std::path::Path>,
) -> msime_engine::SentenceAssociationOptions {
    // 选中的语法模型装好后才有文件路径；选中但没装（或包装在别处的 state_root
    // 下没发布）时留空，引擎逐字节退回无模型基线。
    let pack = match preferences.grammar_model {
        msime_client_core::preferences::GrammarModel::None => None,
        msime_client_core::preferences::GrammarModel::Wanxiang => {
            Some(ResourcePack::GrammarWanxiang)
        }
        msime_client_core::preferences::GrammarModel::BaguwenWord => {
            Some(ResourcePack::GrammarBaguwen)
        }
        msime_client_core::preferences::GrammarModel::BaguwenCompact => {
            Some(ResourcePack::GrammarBaguwenCompact)
        }
        msime_client_core::preferences::GrammarModel::Baishuang => {
            Some(ResourcePack::GrammarBaishuang)
        }
    };
    let collocation_model = state_root
        .zip(pack)
        .and_then(|(root, pack)| {
            pack.set()
                .artifacts
                .first()
                .and_then(|artifact| resource_packs::installed_file(root, pack, &artifact.name))
        })
        .unwrap_or_default();
    msime_engine::SentenceAssociationOptions {
        word_lattice: preferences.word_lattice,
        // 引擎自己那层键盘模型（九键跨切分重排、全拼和双拼词库内重排）不跟「增强」打开：在 runtime 的逐键重排之外它几乎不加分，却让按键延迟和内存再涨一截，取舍见 .agents/notes/implemented/feature/2026-10-29-enhanced-runtime-reranker-only.md。它留给直接构造 `EngineOptions` 的评测和实验（`convert_eval`）。
        neural_keyboard: false,
        show_next_on_duplicate: preferences.show_next_on_duplicate,
        collocation_model,
        collocation_rerank: pack.is_some(),
        // 搭配调参系数逐字段透传；偏好层已把非法值回退到出厂值。
        collocation_weight: preferences.sentence_collocation_weight,
        collocation_grammar: msime_engine::octagram::GramConfig {
            collocation_max_length: preferences.sentence_collocation_max_length,
            collocation_min_length: preferences.sentence_collocation_min_length,
            collocation_penalty: preferences.sentence_collocation_penalty,
            non_collocation_penalty: preferences.sentence_collocation_non_penalty,
            weak_collocation_penalty: preferences.sentence_collocation_weak_penalty,
            rear_penalty: preferences.sentence_collocation_rear_penalty,
        },
    }
}

/// 资源包卸载前摘掉引擎进程内的 `.gram` 映射，避免 Windows 删除目录时仍被文件句柄占用。
pub fn evict_grammar_model(path: &std::path::Path) {
    msime_engine::octagram::GramDb::evict_shared(path);
}

/// 引擎的引号与方头括号样式，由 client-core 从偏好解析（`corner_when_traditional` 在那里按繁体开关定下）。View 报告的是同一个解析结果。
fn apply_punctuation_marks(
    options: &mut EngineOptions,
    preferences: &msime_client_core::preferences::Preferences,
) {
    let marks = msime_client_core::punctuation::punctuation_marks(preferences);
    options.corner_quotes = marks.quotes == msime_client_core::punctuation::QuoteMarks::Corner;
    options.hollow_brackets =
        marks.brackets == msime_client_core::preferences::BracketStyle::Hollow;
    options.enumeration_comma_backslash = marks.enumeration_comma_backslash;
    options.enumeration_comma_slash = marks.enumeration_comma_slash;
}

/// The switch the Engine is handed. Windows turns the punctuation switch on whenever punctuation is locked to Chinese, while the Engine drops Chinese punctuation whenever its switch is off, so a lock to Chinese has to carry the switch with it.
fn engine_chinese_punctuation(enabled: bool, lock: u8) -> bool {
    enabled || lock == 1
}

impl HostSession {
    /// 判断一个标点键去向所用的上下文。`msime_client_punctuation_with_context` 和大写锁定改道（[`Self::caps_lock_punctuation`]）用的是同一份。
    fn punctuation_context(&self, character: u8, preceding: Option<char>) -> PunctuationContext {
        let lock = match self.punctuation_lock_override {
            Some(1) => msime_client_core::preferences::PunctuationLock::Chinese,
            Some(2) => msime_client_core::preferences::PunctuationLock::English,
            Some(_) => msime_client_core::preferences::PunctuationLock::Follow,
            None => self.applied.punctuation_lock,
        };
        PunctuationContext {
            character,
            preceding,
            host_context_available: self
                .runtime
                .punctuation_host_context_available(self.english_mode),
            has_composition: !self.runtime.is_idle(),
            chinese_punctuation: self
                .punctuation_override
                .unwrap_or(self.applied.chinese_punctuation),
            smart_punctuation: self.applied.smart_punctuation,
            direct_digit: self.applied.smart_punctuation_direct_digit,
            direct_letter: self.applied.smart_punctuation_direct_letter,
            lock,
            caps_lock_ascii: self.caps_lock && self.applied.caps_lock_ascii_punctuation,
        }
    }

    /// 大写锁定时，把经字符或标点入口送来的标点键改走字面 ASCII 路线（#6370）。宿主大多把标点键当普通字符交给 `msime_client_character`，这条路线上没有标点判断，所以在分发前统一改道，每个报告了大写锁定的宿主都不用再写一份。只看大写锁定这一条：智能标点要读前文，不在这里判断。不改道时原样返回。
    fn caps_lock_punctuation(&self, action: Action) -> Action {
        if !(self.caps_lock && self.applied.caps_lock_ascii_punctuation) {
            return action;
        }
        let ascii = match action {
            Action::Character { value, .. } | Action::Punctuation(value)
                if value.is_ascii_punctuation() =>
            {
                value
            }
            _ => return action,
        };
        let mut context = self.punctuation_context(ascii, None);
        context.smart_punctuation = false;
        match punctuation_route(context) {
            PunctuationRoute::Ascii => Action::PunctuationAscii(ascii),
            PunctuationRoute::Engine => action,
        }
    }

    /// `engine_chinese_punctuation` for the live overrides over the applied preferences.
    fn live_engine_chinese_punctuation(&self) -> bool {
        engine_chinese_punctuation(
            self.punctuation_override
                .unwrap_or(self.applied.chinese_punctuation),
            self.punctuation_lock_override
                .unwrap_or_else(|| punctuation_lock_code(self.applied.punctuation_lock)),
        )
    }

    fn ai_provider_config(&self) -> Option<&AiAssistantProviderConfig> {
        self.ai_provider_cache.as_ref()
    }
    fn ai_query_is_current(&self, query: &OnlineQuery) -> bool {
        self.english_query_allowed(query)
            && self
                .ai_provider_config()
                .is_some_and(|config| query.ai_assistant.as_ref() == Some(config))
    }
    fn english_query_allowed(&self, query: &OnlineQuery) -> bool {
        if query.mode != msime_engine::OnlineInputMode::English {
            return true;
        }
        let enabled = self
            .requested
            .as_ref()
            .map_or(self.applied.english_online_candidates, |snapshot| {
                snapshot.preferences.english_online_candidates
            });
        enabled
            && self.runtime.online_query().is_ok_and(|live| {
                live.is_some_and(|live| {
                    live.mode == query.mode
                        && live.scheme == query.scheme
                        && live.session_id == query.session_id
                        && live.generation == query.generation
                        && live.identity == query.identity
                        && live.query_text == query.query_text
                        && live.cache_key == query.cache_key
                        && live.pinyin_segments == query.pinyin_segments
                        && live.ai_context == query.ai_context
                        && live.cloud_eligible == query.cloud_eligible
                        && live.ai_eligible == query.ai_eligible
                })
            })
    }
    fn set_ai_provider_cache(&mut self, preferences: &Preferences) {
        let ai = &preferences.ai_assistant;
        self.ai_provider_cache = ai.enabled.then(|| AiAssistantProviderConfig {
            enabled: true,
            provider: ai.provider.clone(),
            model: ai.model.clone(),
            endpoint: ai.endpoint.clone(),
            candidate_limit: host_ai_candidate_limit(ai.candidate_limit),
            thinking: ai.thinking.clone(),
            prompt_id: ai.prompt_id.clone(),
            prompt_custom_1: ai.prompt_custom_1.clone(),
            prompt_custom_2: ai.prompt_custom_2.clone(),
            prompt_custom_3: ai.prompt_custom_3.clone(),
        });
    }
    /// 以最新请求的开关与来源配置判断回包；组字结束才应用的旧偏好不能让已关闭或已切换的服务继续回答。
    fn cloud_query_is_current(&self, query: &OnlineQuery) -> bool {
        let preferences = self
            .requested
            .as_ref()
            .map_or(&self.applied, |snapshot| &snapshot.preferences);
        self.english_query_allowed(query)
            && self.cloud_candidates_enabled()
            && query.cloud_request.as_ref().map_or_else(
                || {
                    query.mode != msime_engine::OnlineInputMode::English
                        && preferences.cloud_candidate_provider == Default::default()
                },
                |request| request.provider_id == preferences.cloud_candidate_provider.identity(),
            )
    }

    fn cloud_candidates_enabled(&self) -> bool {
        self.requested
            .as_ref()
            .map_or(self.applied.cloud_candidates, |snapshot| {
                snapshot.preferences.cloud_candidates
            })
    }
    /// Android、鸿蒙与 iOS 会话是否不记统计：只看宿主经 `msime_client_set_private_session` 标出的隐私会话。用户在设置里关掉学习不算，统计开着就照常计数。只在 [`PRIVATE_SESSIONS_SKIP_STATISTICS`] 时成立，其他宿主照旧计数。只看候选的会话（`preview`）和宿主声明由别的会话记统计的会话（`statistics_recorded_elsewhere`）在任何平台上都不计。
    fn private_session(&self) -> bool {
        self.preview
            || self.statistics_recorded_elsewhere
            || (PRIVATE_SESSIONS_SKIP_STATISTICS && self.statistics_private)
    }

    /// Count a committing selection in memory, and hand the batch to the store once it is `SELECTION_BATCH` long.
    fn count_selection(&mut self, position: usize) {
        // Android 的隐私模式与不学习输入框、鸿蒙和 iOS 的隐私模式不统计选词位置。
        if self.private_session() {
            return;
        }
        // Positions are one-based; zero is not a position, and the store has always refused it.
        let Some(slot) = position.checked_sub(1) else {
            return;
        };
        let slot = &mut self.pending_selections[slot.min(RANKS)];
        *slot = slot.saturating_add(1);
        if self.pending_selections.iter().sum::<u64>() >= SELECTION_BATCH {
            self.flush_selections();
        }
    }

    /// 选中的候选在派发前的样子，供上屏后计效率：不记统计的会话、统计关闭、非 Android 宿主或找不到这个候选时为 `None`。
    fn efficiency_candidate(&mut self, action: &Action) -> Option<EfficiencyCandidate> {
        if !COUNTS_COMMIT_EFFICIENCY || self.private_session() {
            return None;
        }
        let id = match action {
            Action::Select(id) | Action::SelectAnyCandidate(id) => *id,
            _ => return None,
        };
        if !self.statistics_enabled()? {
            return None;
        }
        let view = self.runtime.view();
        let candidate = view
            .candidates
            .into_iter()
            .find(|candidate| candidate.id == id)?;
        Some(EfficiencyCandidate {
            prediction: candidate.code.is_empty() && view.preedit.is_empty(),
            sentence: msime_engine::CandidateSource::from_u8(candidate.source)
                .is_some_and(msime_engine::CandidateSource::is_sentence_learning),
            typed_keys: candidate
                .code
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .count() as u64,
            text: candidate.text,
        })
    }

    /// 打字统计目录，即 HostOptions 的 `preferences_directory`（`state_root`）：各宿主记字数和按键、统计页和上传读的都是这里的 `typing-statistics.json`，选词位置和上屏效率要写进同一份。`user_data` 是 Engine 的个人词库目录，没有宿主在那里读写统计。不是绝对路径时为 `None`，这时什么都不记。
    fn statistics_directory(&self) -> Option<&Path> {
        self.state_root.as_deref()
    }

    /// 统计开关，只看本进程所知的（上次读写统计文件时看到的），不在输入线程上读文件：这里在每次选词的路上，读一次就要取文件锁、先把本进程攒着的整批写出去再 fsync。本进程还没读过统计文件时当作打开，效率照攒，写入时统计文件自己的开关作数，关着就整批丢掉。没有统计目录时当作关闭。
    fn statistics_enabled(&mut self) -> Option<bool> {
        if let Some(enabled) = self.statistics_enabled {
            return Some(enabled);
        }
        let enabled = match self.statistics_directory() {
            Some(directory) => TypingStatisticsStore::new(directory).known_enabled(),
            None => Some(false),
        };
        self.statistics_enabled = enabled;
        Some(enabled.unwrap_or(true))
    }

    /// 记下一次上屏，等下一次写统计时一起算。上限 [`EFFICIENCY_BATCH_LIMIT`]，超出的不计。
    fn count_efficiency(&mut self, candidate: EfficiencyCandidate) {
        if self.pending_efficiency.len() < EFFICIENCY_BATCH_LIMIT {
            self.pending_efficiency.push(candidate);
        }
    }

    /// Hand the selections counted since the last flush to the process's pending typing-statistics batch for the statistics directory.
    ///
    /// 只加内存里的计数，不取统计文件锁、不碰磁盘（`TypingStatisticsStore::queue_selections`）：这里在宿主的输入线程上跑（满一批、失焦和销毁会话时），macOS 的 IMK 主线程、Linux 的 GLib 主循环和移动宿主的 UI 线程都不能整份读写统计文件再 fsync，也不能等统计工作线程手里那次写。攒着的选词和宿主经 `record` 攒的上屏一起写出：失焦和销毁随后的 `flush_typing_statistics`（Windows 失焦和桌面销毁在当前线程上，其余在后台线程上）、满间隔的 `record`，以及读统计的任何操作。尽力而为：统计不能成为按键失败的原因，store 拒收的批次丢掉，不报出去。统计开关由 store 在写出时自己看，这里不另查，免得两处不一致。
    fn flush_selections(&mut self) {
        let pending = std::mem::take(&mut self.pending_selections);
        let Some(directory) = self.statistics_directory().map(Path::to_path_buf) else {
            return;
        };
        let mut batch = Vec::with_capacity(RANKS);
        batch.extend(
            pending
                .iter()
                .enumerate()
                .filter(|(_, count)| **count > 0)
                .map(|(slot, count)| (slot + 1, *count)),
        );
        let enabled = TypingStatisticsStore::new(&directory)
            .queue_selections(&batch)
            .ok()
            .flatten();
        let commits = std::mem::take(&mut self.pending_efficiency);
        if !commits.is_empty() {
            // 查读音要打开词库，放到后台线程，不占输入线程；测试里同步写，好断言结果。
            if cfg!(test) {
                record_efficiency(&self.options, &directory, &commits);
            } else {
                record_efficiency_in_background(self.options.clone(), directory, commits);
            }
        }
        // 用户可能在设置里关掉或打开了统计：换成本进程此刻所知的开关，它随每次写统计更新。不在输入线程上为一个开关读文件。
        self.statistics_enabled = enabled;
    }

    /// Rebuild the Engine for the requested preferences once the composition is idle. The `Ok` value is why the preferred scheme was not the one applied, when it was not.
    fn apply_pending(&mut self) -> Result<Option<String>, String> {
        if self.runtime.is_idle() {
            if let Some(size) = self.page_size_override {
                // 这里只改分页本身，引擎的首屏门槛由 `Runtime::set_page_size` 一并下发（它读的是同一份 `page_size`），
                // 所以 override 的变化不必触发重建。fcitx5 的「9 个候选」和 IBus 的每页候选数菜单都会写它，见 `ffi/input.rs`。
                self.runtime
                    .set_page_size(host_page_size(size))
                    .map_err(|e| e.to_string())?;
            }
            // 落定重排模型不属于 Engine，换模型不用重建 Engine。约 25 MB 的权重由 `refresh_resource_packs` 起的后台线程加载，这里只在加载完之后、输入空闲时换上，不在输入线程上读文件；还没加载完就留到下一次。
            if let Some(model) = self
                .settled_model_loading
                .as_ref()
                .and_then(|slot| slot.get().cloned())
            {
                self.settled_model_loading = None;
                self.settled_model_attached = model.is_some();
                self.runtime.set_settled_reranker(model.map(Reranker::new));
                self.report_settled_model();
            }
        }
        if !(self.preferences_pending || self.resources_pending) || !self.runtime.is_idle() {
            return Ok(None);
        }
        // 只因资源包变化而重建时，没有新请求的偏好，按已应用的那份重建。
        let preferences = self
            .requested
            .as_ref()
            .map(|snapshot| &snapshot.preferences)
            .unwrap_or(&self.applied)
            .clone();
        let mut options = self.options.clone();
        let (scheme, scheme_fallback) = effective_scheme(
            &preferences,
            &offered_input_schemes(self.edition),
            &LanguageDictionaries::of_options(&self.options),
            self.edition.default_scheme,
        );
        options.scheme = scheme_code(scheme);
        options.vietnamese_input_method = vietnamese_input_method_code(preferences.vietnamese);
        options.vietnamese_tone_style = vietnamese_tone_style_code(preferences.vietnamese);
        options.korean_layout =
            korean_layout_code(preferences.korean, self.korean_romaja_requested);
        let profile_fallback = apply_shuangpin_profile(&mut options, &preferences);
        // 自定义双拼表不能用的原因只在真的跑双拼时才值得报告。
        let fallback =
            scheme_fallback.or(profile_fallback.filter(|_| scheme == InputScheme::Shuangpin));
        options.shuangpin_preedit_uses_raw = preferences.shuangpin_preedit_uses_raw;
        options.force_cantonese_shuangpin = scheme == InputScheme::Cantonese
            && preferences.touch_keyboard_schemes.selected
                == Some(TouchKeyboardScheme::CantoneseShuangpin);
        options.single_character_only = preferences.single_character_only;
        options.caret_edit_suffix = caret_edit_suffix_applies(&preferences);
        options.learning = preferences.learning;
        options.next_word_prediction =
            preferences.next_word_prediction && cfg!(target_os = "android");
        options.learn_english_on_enter = preferences.enter_learns_english_word;
        options.autocorrect_transposition = preferences.quanpin.autocorrect_transposition;
        options.autocorrect_neighbor = preferences.quanpin.autocorrect_neighbor;
        options.fuzzy_pinyin_rules = preferences.fuzzy_pinyin.active_rules();
        options.wubi_mixed_pinyin = preferences.wubi_mixed_pinyin;
        options.wubi_z_wildcard = preferences.wubi_z_wildcard;
        options.wubi_profile = wubi_profile_code(preferences.wubi_profile);
        options.frequency_mode = preferences.frequency.mode.as_str().into();
        options.frequency_trigger_count = preferences.frequency.trigger_count;
        options.frequency_linear_step = preferences.frequency.linear_step;
        options.mixed_candidates = preferences.mixed_input.enabled;
        options.mixed_english = preferences.mixed_input.english;
        options.english_minimum_prefix =
            host_english_minimum_prefix(preferences.mixed_input.minimum_prefix);
        options.english_first_page_count = preferences.mixed_input.first_page_count;
        options.mixed_emoji = preferences.mixed_input.emoji;
        options.mixed_kaomoji = preferences.mixed_input.kaomoji;
        options.mixed_symbols = preferences.mixed_input.symbols;
        options.mixed_japanese = preferences.mixed_input.japanese;
        options.multilingual_languages = multilingual_languages_for(
            &preferences,
            self.edition,
            japanese_model_available(&options),
        );
        options.quick_phrase_candidates =
            self.quick_phrase_candidates_requested && preferences.mixed_input.quick_phrase;
        options.quick_phrase_frequency = preferences.mixed_input.quick_phrase_frequency;
        options.local_unicode = preferences.local_modes.unicode;
        options.local_date_time = preferences.local_modes.date_time;
        options.local_date_time_inline = preferences.local_modes.date_time_inline;
        options.local_date_time_menu = preferences.local_modes.date_time_menu;
        options.local_quick_phrase = preferences.local_modes.quick_phrase;
        options.local_emoji = preferences.local_modes.emoji;
        options.local_kaomoji = preferences.local_modes.kaomoji;
        options.local_super_jianpin = preferences.local_modes.super_jianpin;
        options.local_temporary_english = preferences.local_modes.temporary_english;
        options.local_temporary_japanese = preferences.local_modes.temporary_japanese;
        options.local_expression = preferences.local_modes.expression;
        options.local_command = preferences.local_modes.command;
        options.local_mention = preferences.local_modes.mention;
        options.local_chaizi = chaizi_enabled(&preferences.local_modes);
        options.local_symbol = preferences.local_modes.symbol;
        options.local_shift_letter_entry = preferences.local_modes.shift_letter_entry;
        // 繁体输出开着时拆字行按繁体字形去重；改这个偏好同样会让 Engine 在空闲时重建。
        options.chaizi_traditional_output = preferences.traditional_chinese_output;
        // 先定下辅助码设置：插件表的戳要看当前方案的辅助码是否打开、选了哪个辅助码表包。
        let helpcode = helpcode_for_scheme(&preferences, scheme);
        options.helpcode = helpcode.enabled;
        options.strict_quanpin_helpcode = helpcode.strict_filter;
        options.show_helpcode = helpcode.show_in_candidate_window;
        options.helpcode_schema = helpcode.schema.as_str().into();
        options.helpcode_trigger_key = helpcode.trigger_key.bytes().next();
        options.shuangpin_helpcode_modes = shuangpin_helpcode_modes(helpcode.modes);
        let plugin_root = self.plugin_roots.installed.as_deref();
        let plugin_tables = plugin_tables::PluginTables::stamp(
            plugin_root,
            self.plugin_roots.builtin_packs.as_deref(),
            &options,
            &preferences.plugins,
        );
        plugin_tables.fill(&self.plugin_tables, plugin_root, &mut options);
        options.sentence_association = engine_sentence_association(
            &preferences.sentence_association,
            self.state_root.as_deref(),
        );
        // 引擎按宿主的每页候选数决定全长组要不要并入补全词；和下面交给 runtime 的值同源。
        let page_size = host_page_size(
            self.page_size_override
                .unwrap_or(preferences.candidate_page_size),
        );
        options.first_page_rows = usize::from(page_size);
        // Unconditional, because `Runtime::crop_alternative_readings` runs whether or not a model is
        // attached: the host always shows one whole-sentence reading. Asking for the rest only ever
        // gives it more to choose from, and even with no model the engine's own pick among them is
        // better than the one it makes when it searches without alternatives.
        options.sentence_alternatives = true;
        apply_local_mode_resource_gates(&mut options, self.edition);
        options.paired_punctuation = preferences.paired_punctuation;
        options.punctuation_lock = punctuation_lock_code(preferences.punctuation_lock);
        options.chinese_punctuation =
            engine_chinese_punctuation(preferences.chinese_punctuation, options.punctuation_lock);
        // 繁体输出开关一变，`corner_when_traditional` 解析出的样式也跟着变；两者都是偏好，都会在空闲时走到这里重建。
        apply_punctuation_marks(&mut options, &preferences);
        // 先构建并校验，失败时原会话仍然可用。
        let mut engine = Session::new(&options).map_err(|e| e.to_string())?;
        engine.set_prediction_allowed(!self.statistics_private);
        // 云候选席位不是 EngineOptions；新引擎默认关闭，每次重建都要恢复当前偏好。
        engine.set_cloud_candidates_enabled(preferences.cloud_candidates);
        engine.set_english_online_candidates_enabled(preferences.english_online_candidates);
        if self.punctuation_override.is_some() || self.punctuation_lock_override.is_some() {
            engine
                .set_chinese_punctuation_enabled(engine_chinese_punctuation(
                    self.punctuation_override
                        .unwrap_or(preferences.chinese_punctuation),
                    self.punctuation_lock_override
                        .unwrap_or(options.punctuation_lock),
                ))
                .map_err(|e| e.to_string())?;
        }
        if let Some(enabled) = self.paired_punctuation_override {
            engine
                .set_paired_punctuation_enabled(enabled)
                .map_err(|e| e.to_string())?;
        }
        if let Some(lock) = self.punctuation_lock_override {
            engine
                .set_punctuation_lock(lock)
                .map_err(|e| e.to_string())?;
        }
        // 双拼换表也算换布局：宿主设下的组码网格带着旧表，不能继续沿用。
        let layout_changed = preferences.touch_keyboard_layout
            != self.applied.touch_keyboard_layout
            || preferences.touch_shuangpin_layout != self.applied.touch_shuangpin_layout
            || preferences.touch_wubi_layout != self.applied.touch_wubi_layout
            || preferences.shuangpin_profile != self.applied.shuangpin_profile
            || preferences.shuangpin_custom_profile != self.applied.shuangpin_custom_profile
            || preferences.nine_key_shuangpin_profile != self.applied.nine_key_shuangpin_profile
            || preferences.nine_key_shuangpin_custom_profile
                != self.applied.nine_key_shuangpin_custom_profile;
        let scheme = SchemeType::from_u8(options.scheme);
        // 全拼、注音支持九键，双拼还支持派生的 14 键，五笔支持 14 键；具体组合由 `Runtime::set_key_grid` 校验。
        let grid_scheme = scheme.is_some_and(|value| {
            value.nine_key() || matches!(value, SchemeType::Shuangpin | SchemeType::Wubi)
        });
        // 宿主的网格覆盖只对它设下时的那个方案有效：全拼九宫格的开关带进注音，会让没选「注音 9 键」的注音也离开大千键位。
        let scheme_changed = options.scheme != self.options.scheme;
        let next_key_grid_override = if grid_scheme && !layout_changed && !scheme_changed {
            self.key_grid_override
        } else {
            None
        };
        let key_grid = if grid_scheme {
            next_key_grid_override.unwrap_or_else(|| layout_key_grid(scheme, &preferences))
        } else {
            None
        };
        if key_grid.is_some() {
            engine.set_key_grid(key_grid).map_err(|e| e.to_string())?;
        }
        // 正在用九键双拼而自定义表不能用时报告一次原因，与 26 键自定义双拼按小鹤运行时相同。
        let fallback = fallback.or_else(|| {
            matches!(key_grid, Some(KeyGrid::NineKeyShuangpin(_)))
                .then(|| nine_key_shuangpin_grid(&preferences).1)
                .flatten()
        });
        engine
            .set_dedicated_english(self.english_mode)
            .map_err(|e| e.to_string())?;
        // The places of `@` mode are not an engine option: every new engine starts with them off, so the switch is carried over on each rebuild.
        engine
            .set_mention_places(preferences.local_modes.mention_places)
            .map_err(|e| e.to_string())?;
        self.runtime
            .replace_engine_with_touch_layout(engine, page_size, preferences.touch_keyboard_layout)
            .map_err(|e| e.to_string())?;
        let desktop_turned_on = preferences.sentence_association.neural_desktop
            && !self.applied.sentence_association.neural_desktop;
        self.runtime
            .set_settled_rerank_enabled(preferences.sentence_association.neural_desktop);
        self.runtime
            .set_punctuation_marks(msime_client_core::punctuation::punctuation_marks(
                &preferences,
            ));
        // The keyboard model follows its switch the same way, but is dropped rather than idled while off: it is the one that runs inside every keystroke.
        let keyboard = keyboard_reranker_enabled(&preferences.sentence_association, self.edition);
        if keyboard != self.runtime.has_reranker() {
            self.runtime.set_candidate_scorer(ffi::keyboard_reranker(
                keyboard,
                &options.resources,
                options.helpcode_fallback_resources.as_deref(),
                self.recorded_sentence_model.as_deref(),
            ));
        }
        self.options = options;
        self.plugin_tables = plugin_tables;
        self.applied = preferences;
        self.preferences_pending = false;
        self.resources_pending = false;
        self.key_grid_override = next_key_grid_override;
        // 会话建立时用的是运行时选项里嵌的偏好快照，开关可能要等宿主随后送来的偏好才打开（#10341 里报告者的 `runtime-options.json` 就写着关），所以打开的这一刻再报告一次模型接上没有。新模型还在后台加载时不报告，加载完换上时会报告。
        if desktop_turned_on && self.settled_model_loading.is_none() {
            self.report_settled_model();
        }
        Ok(fallback)
    }

    /// 开着「桌面神经联想」的输入会话把落定重排模型接上了没有，写一行诊断（见 [`report_settled_model`]）。只看候选的会话和开关关着的会话不写。
    fn report_settled_model(&self) {
        if self.preview || !self.runtime.settled_rerank_enabled() {
            return;
        }
        report_settled_model(
            &self.options.resources,
            self.state_root.as_deref(),
            self.settled_model.as_deref(),
            self.settled_model_attached,
        );
    }

    /// 输入框获得焦点时，让 `/` 指令表、K 模式短语表、辅助码表、`@` 名单和脚本插件的 `/` 指令跟上插件目录：设置页可能刚导入了表、改了名单或改了授权。没有文件变动时什么都不读。换表失败只报告，不让聚焦失败。
    fn refresh_plugin_tables(&mut self) {
        let root = self.plugin_roots.installed.as_deref();
        let tables = plugin_tables::PluginTables::stamp(
            root,
            self.plugin_roots.builtin_packs.as_deref(),
            &self.options,
            &self.applied.plugins,
        );
        // 指令表、`@` 名单和短语表换不上也不算聚焦失败。这三个 setter 都是先把表交给 Engine，再刷新打开着的那个模式的列表；除了代次耗尽（那时随后的 `runtime.focus` 同样失败），报错只来自这次刷新，典型是 K 模式开着而数据库短语查不了。此时 Engine 已经用上新表，所以照样记进 `options`（重建的 Engine 从这里开始），戳也照常更新；把错误往上抛会让这次和之后每一次聚焦都失败（戳没更新，下次又会重试）。
        if tables.commands_differ(&self.plugin_tables) {
            let table = tables.command_table(root);
            if let Err(error) = self.runtime.set_command_table(&table) {
                diagnostics::report("command table view not refreshed", &error.to_string());
            }
            self.options.command_table = table;
        }
        if tables.mentions_differ(&self.plugin_tables) {
            let entries = tables.mention_entries(root, &self.options);
            if let Err(error) = self.runtime.set_mention_entries(&entries) {
                diagnostics::report("mention list view not refreshed", &error.to_string());
            }
            self.options.mention_entries = entries;
        }
        if tables.phrases_differ(&self.plugin_tables) {
            let table = tables.quick_phrase_table(root);
            if let Err(error) = self.runtime.set_quick_phrase_table(&table) {
                diagnostics::report("quick phrase view not refreshed", &error.to_string());
            }
            self.options.quick_phrase_table = table;
        }
        if tables.helpcode_differs(&self.plugin_tables) {
            let table = tables.helpcode_table(root, &self.options);
            // 辅助码表换不上不算聚焦失败：报错会让这次和之后每一次聚焦都失败（戳没更新，下次又会重试）。记下来，保留当前的表，照常更新戳。
            match self.runtime.set_helpcode_table(table.clone()) {
                Ok(()) => self.options.helpcode_table = table,
                Err(error) => {
                    diagnostics::report("helpcode table not replaced", &error.to_string())
                }
            }
        }
        self.plugin_tables = tables;
        // 脚本插件的 `/` 指令跟着授权文件走，它有自己的戳（`ScriptSet` 按戳重读授权），不进上面的表戳。
        self.refresh_plugin_commands();
    }

    /// 输入框获得焦点时，看设置应用是否在会话打开后装好（或移除）了日文、粤拼、注音、笔画、五笔的资源包和落定重排模型：只做几次 stat。词典路径有变就记下，等输入空闲时由 `apply_pending` 重建 Engine（重建时日文临时模式的开关按新路径重新判断）；落定重排模型的文件有变就起一个后台线程加载它（经 `sentence_model_settled` 的共享缓存），加载完之后由某次输入空闲的 `apply_pending` 换上。聚焦本身不读词典也不读模型文件。
    ///
    /// 资源包目录只在校验完、整体原子发布之后才出现，所以这里看到的文件都是完整的。
    fn refresh_resource_packs(&mut self) {
        let state_root = self.state_root.as_deref();
        let settled_model = settled_model_file(
            &self.options.resources,
            self.recorded_settled_model.as_deref(),
            state_root,
        );
        if settled_model != self.settled_model {
            self.settled_model = settled_model;
            self.settled_model_loading = Some(load_settled_model(
                &self.options.resources,
                self.settled_model.clone(),
            ));
        }
        let downloaded = state_root.map(DownloadedDictionaries::new);
        let dictionaries = LanguageDictionaries::resolve(
            downloaded.as_ref(),
            self.recorded_language_dictionaries.as_deref(),
        );
        let cantonese = path_text(dictionaries.cantonese);
        let zhuyin = path_text(dictionaries.zhuyin);
        let stroke = path_text(dictionaries.stroke);
        let japanese = path_text(japanese_dictionary(downloaded.as_ref()));
        let wubi = path_text(wubi_dictionary(
            downloaded.as_ref(),
            Path::new(&self.options.resources),
        ));
        let grammar_model =
            engine_sentence_association(&self.applied.sentence_association, state_root)
                .collocation_model;
        if cantonese == self.options.cantonese_dictionary
            && zhuyin == self.options.zhuyin_dictionary
            && stroke == self.options.stroke_dictionary
            && japanese == self.options.japanese_dictionary
            && wubi == self.options.wubi_dictionary
            && grammar_model == self.options.sentence_association.collocation_model
        {
            return;
        }
        self.options.cantonese_dictionary = cantonese;
        self.options.zhuyin_dictionary = zhuyin;
        self.options.stroke_dictionary = stroke;
        self.options.japanese_dictionary = japanese;
        self.options.wubi_dictionary = wubi;
        self.resources_pending = true;
    }

    /// 「中英文之间加空格」开关，跟着最新请求的偏好走：它只是上屏后处理，不是 Engine 状态，不必等组字结束后重建。
    fn chinese_english_spacing(&self) -> bool {
        self.requested
            .as_ref()
            .map_or(&self.applied, |snapshot| &snapshot.preferences)
            .chinese_english_spacing
    }

    fn complete_transition(&mut self, mut result: Transition) -> Transition {
        let generation = self.runtime.generation();
        let note = match self.apply_pending() {
            Ok(fallback) => fallback,
            Err(error) => Some(format!("Preferences update deferred: {error}")),
        };
        if let Some(note) = note {
            let prior = result.diagnostic.take().unwrap_or_default();
            result.diagnostic = Some(format!("{prior} {note}").trim().to_owned());
        }
        // 替换只改变 view 的代次，不改变已完成的上屏。不转全角的方案（韩文）无论宽度开关如何都写半角 ASCII 标点和数字；专用英文模式在任何方案里都按自己的规则走，所以它的上屏和中文方案下一样转全角。网址模式的网址部分始终是半角：上屏后 view 已回到 `none`，所以看上屏前记下的 `commit_context`；结束网址的那个标点不是网址能收的字符，照常转全角。
        let half_width_text = SchemeType::from_u8(result.view.scheme)
            .is_some_and(|scheme| !scheme.widens_full_width())
            && !result.view.dedicated_english
            && result.view.local_mode == "none";
        let url_commit = result
            .commit_context
            .as_ref()
            .is_some_and(|context| context.local_mode == "url");
        // 脚本插件在 `/` 指令里给的行（UUID 之类）原样上屏，不转全角：转了以后上屏的和候选窗里显示的不一样。同一次 `/` 模式里内置指令（`/rq`）的上屏照旧转全角。
        let plugin_commit = result
            .commit_context
            .as_ref()
            .is_some_and(|context| context.plugin_row);
        // 位置以原上屏的真实后缀为准；失配时不猜测编辑器里的光标偏移。
        if result.commit_cursor_suffix.as_ref().is_some_and(|suffix| {
            !result
                .commit
                .as_ref()
                .is_some_and(|commit| commit.ends_with(suffix))
        }) {
            result.commit_cursor_suffix = None;
        }
        if result.view.character_width == CharacterWidth::Fullwidth
            && !half_width_text
            && !plugin_commit
        {
            for c in result
                .commit
                .iter_mut()
                .chain(result.commit_cursor_suffix.iter_mut())
            {
                *c = c
                    .chars()
                    .map(|x| {
                        if url_commit && x.is_ascii() && msime_engine::url::accepts(x as u8) {
                            x
                        } else if x == ' ' {
                            '\u{3000}'
                        } else if ('!'..='~').contains(&x) {
                            char::from_u32(x as u32 + 0xfee0).unwrap()
                        } else {
                            x
                        }
                    })
                    .collect();
            }
        }
        // 中英文之间加空格排在全角转换之后：先加的半角空格会被转成 U+3000；全角下字母已是全角，本来就不会触发。宿主的简繁转换在拿到上屏文本之后才做，OpenCC 只换汉字、不动空格。之后的 `before_commit` 看到的是加过空格的文本。
        if self.chinese_english_spacing()
            && chinese_english_spacing::applies(result.commit_context.as_ref())
        {
            // 分别转换正文与后缀。边界新增的空格留在光标前，光标仍紧邻原后缀的首字符。
            for commit in result
                .commit
                .iter_mut()
                .chain(result.commit_cursor_suffix.iter_mut())
            {
                if let Some(spaced) = chinese_english_spacing::space(commit) {
                    *commit = spaced;
                }
            }
        }
        if result.commit_cursor_suffix.as_ref().is_some_and(|suffix| {
            !result
                .commit
                .as_ref()
                .is_some_and(|commit| commit.ends_with(suffix))
        }) {
            result.commit_cursor_suffix = None;
        }
        // Dispatch already built the view for this transition. Rebuild it only
        // when applying a deferred page-size or preference change advanced the
        // runtime generation; otherwise cloning the page again costs every key.
        if self.runtime.generation() != generation {
            result.view = self.runtime.view();
        }
        result
    }

    fn update(&mut self, snapshot: PreferencesSnapshot) -> Result<Value, String> {
        if snapshot.format_version != 1 {
            return Err("unsupported preferences format".into());
        }
        // Android 键盘显示期间每秒把读到的偏好原样交来一次，绝大多数时候和上一份逐字相同。上一份已经校验过、所有 setter 已经按它设过（这些宿主状态只有偏好会改，`msime_client_set_multilingual_output` 的临时投影除外），`preferences_pending` 也一直等于「它与 `applied` 不同」，所以这里只补上会被别处改动的那几样，不再校验、比较整棵偏好、克隆偏好和重建音效设置。
        if self.requested_settled
            && self.requested.as_ref().is_some_and(|previous| {
                previous.revision == snapshot.revision && *previous == snapshot
            })
        {
            return self.update_unchanged();
        }
        snapshot.preferences.validate().map_err(|e| e.to_string())?;
        if let Some(previous) = &self.requested {
            if snapshot.revision < previous.revision
                || (snapshot.revision == previous.revision && snapshot != *previous)
            {
                return Err("stale or conflicting preferences revision".into());
            }
        }
        let ai_changed = self
            .requested
            .as_ref()
            .map_or(&self.applied.ai_assistant, |previous| {
                &previous.preferences.ai_assistant
            })
            != &snapshot.preferences.ai_assistant;
        let cloud_disabled = self
            .requested
            .as_ref()
            .map_or(self.applied.cloud_candidates, |previous| {
                previous.preferences.cloud_candidates
            })
            && !snapshot.preferences.cloud_candidates;
        let cloud_changed = self
            .requested
            .as_ref()
            .map_or(&self.applied.cloud_candidate_provider, |previous| {
                &previous.preferences.cloud_candidate_provider
            })
            != &snapshot.preferences.cloud_candidate_provider;
        if (cloud_disabled || cloud_changed) && !self.runtime.is_idle() {
            // 关闭云候选或切换来源配置时移除旧云行，使迟到回包失效。
            self.runtime
                .clear_online_candidates(0)
                .map_err(|e| e.to_string())?;
        }
        let previous_preferences = self
            .requested
            .as_ref()
            .map_or(&self.applied, |previous| &previous.preferences);
        let translation_changed =
            translation_query_preferences_changed(previous_preferences, &snapshot.preferences);
        if translation_changed && !self.runtime.is_idle() {
            // 释义配置与服务凭据变化时清掉旧释义，使旧代次的回包不能进入新配置。
            self.runtime
                .invalidate_translations()
                .map_err(|e| e.to_string())?;
        }
        if ai_changed && !self.runtime.is_idle() {
            // AI 配置或令牌变化时先清理旧候选并使旧查询失效，再发布新偏好。
            self.runtime
                .clear_online_candidates(1)
                .map_err(|e| e.to_string())?;
        }
        self.preferences_pending = snapshot.preferences != self.applied;
        self.requested = Some(snapshot);
        self.requested_settled = false;
        let requested_preferences = self
            .requested
            .as_ref()
            .expect("requested snapshot exists")
            .preferences
            .clone();
        // 繁简只投影混合候选的中文段，不改组字，和宿主的即时输出开关同步。
        self.runtime.set_multilingual_output(
            requested_preferences.traditional_chinese_output,
            requested_preferences.traditional_chinese_variant,
        );
        self.set_ai_provider_cache(&requested_preferences);
        self.runtime
            .set_english_online_candidates_enabled(requested_preferences.english_online_candidates)
            .map_err(|e| e.to_string())?;
        self.runtime
            .set_cloud_candidates_enabled(requested_preferences.cloud_candidates)
            .map_err(|e| e.to_string())?;
        // 候选词翻译的范围只改查询和画法，跟着最新请求的偏好走；换范围时的旧释义已经在上面随释义偏好的变化清掉。
        self.runtime
            .set_translation_scope_highlighted(translation_scope_highlighted(
                &requested_preferences,
            ));
        // 宿主状态，不是 Engine 状态：跟着最新请求的偏好走，不必等这次偏好重建 Engine，也不必等组字结束。
        self.runtime
            .set_backspace_reverts_selection(requested_preferences.backspace_reverts_selection);
        // Esc 保留已选词同样是宿主状态：改完下一次 Esc 就按新开关解释。
        self.runtime
            .set_escape_keeps_selected_word(requested_preferences.escape_keeps_selected_word);
        // 左右键的候选回退（`navigation.left_right`）同样是宿主状态：改完立刻生效，左右键不必等 Engine 重建。
        self.runtime
            .set_left_right_navigation(requested_preferences.navigation.left_right);
        self.runtime
            .set_vertical_candidates(host_vertical_candidates(
                requested_preferences.candidate_layout,
            ));
        self.runtime
            .set_wubi_auto_commit_unique(requested_preferences.wubi_auto_commit_unique);
        // 首页英文候选数同样是宿主状态：在线座位整理按它给英文组留位，改完下一次刷新就生效。
        self.runtime
            .set_english_first_page_count(requested_preferences.mixed_input.first_page_count);
        // 顶字同样是宿主状态：纯判定在 Engine 与 runtime 之间，改偏好不必等组字结束。
        self.runtime
            .set_wubi_fifth_code_top_commit(requested_preferences.wubi_fifth_code_top_commit);
        self.runtime
            .set_wubi_candidate_selection(requested_preferences.wubi_candidate_selection.clone());
        // 「二三候选」键也是宿主编排状态：跟着最新请求的偏好走，下一键生效。
        self.runtime
            .set_second_third_candidate(requested_preferences.second_third_candidate);
        self.sound.update(key_sound::SoundSettings::new(
            &requested_preferences.plugins,
            &self.plugin_roots,
        ));
        self.requested_settled = true;
        let fallback = self.apply_pending()?;
        Ok(self.preferences_response(fallback))
    }

    /// `update` 收到与上一份逐字相同的快照：只做可能因别处改动而需要重做的事，回复与完整更新相同。
    fn update_unchanged(&mut self) -> Result<Value, String> {
        let snapshot = self.requested.as_ref().expect("requested snapshot exists");
        // 宿主经 `msime_client_set_multilingual_output` 临时切过繁简时，再交一次偏好照旧把输出交还给偏好；Windows 宿主在每次更新偏好后也按偏好重置自己的繁简状态，两边要一致。
        let (traditional, variant) = (
            snapshot.preferences.traditional_chinese_output,
            snapshot.preferences.traditional_chinese_variant,
        );
        self.runtime.set_multilingual_output(traditional, variant);
        // 设置页可能重新导入或删掉了正在用的音效包、效果包：只给清单盖戳，戳变了才重读效果包，和完整更新里新建的设置结果相同。
        self.sound.restamp();
        // 组字结束后待应用的偏好、资源包和落定重排模型仍在这里换上。
        let fallback = self.apply_pending()?;
        Ok(self.preferences_response(fallback))
    }

    /// `update` 的回复：宿主都会读其中的 `view`。
    fn preferences_response(&self, fallback: Option<String>) -> Value {
        let snapshot = self.requested.as_ref().expect("requested snapshot exists");
        let mut response = json!({ "revision": snapshot.revision, "deferred": snapshot.preferences != self.applied, "view": self.runtime.view(), "floating_toolbar": { "enabled": snapshot.preferences.floating_toolbar.enabled, "english_mode": snapshot.preferences.floating_toolbar.english_mode, "scale_percent": snapshot.preferences.floating_toolbar.scale_percent, "font_size": snapshot.preferences.floating_toolbar.font_size, "fullwidth": snapshot.preferences.floating_toolbar.fullwidth, "punctuation": snapshot.preferences.floating_toolbar.punctuation, "character_set": snapshot.preferences.floating_toolbar.character_set, "emoji": snapshot.preferences.floating_toolbar.emoji, "screen_keyboard": snapshot.preferences.floating_toolbar.screen_keyboard, "settings": snapshot.preferences.floating_toolbar.settings } });
        // Applied at once, so no transition will carry the reason the preferred scheme was replaced; a deferred change reports it from `complete_transition` instead.
        if let Some(fallback) = fallback {
            response["diagnostic"] = fallback.into();
        }
        // 偏好里的方案缺词库时报出要补的资源包，宿主可以自己下载（macOS 输入法），不必等用户打开设置。按会话此刻的词典路径算，资源包装好、聚焦刷新之后就不再带它。
        if let Some(pack) = missing_scheme_pack(
            snapshot.preferences.scheme,
            &offered_input_schemes(self.edition),
            &LanguageDictionaries::of_options(&self.options),
        ) {
            response["missing_resource_pack"] = pack.id().into();
        }
        response
    }
}

fn profile_code(profile: ShuangpinProfile) -> u8 {
    match profile {
        ShuangpinProfile::Xiaohe => 0,
        ShuangpinProfile::Ziranma => 1,
        ShuangpinProfile::Shoudao => 2,
        ShuangpinProfile::Microsoft => 3,
        ShuangpinProfile::SmartAbc => 5,
        ShuangpinProfile::Custom => 4,
    }
}

/// 偏好里的自定义双拼表换成 Engine 的表。
fn engine_shuangpin_custom_table(profile: &ShuangpinCustomProfile) -> ShuangpinCustomTable {
    let pairs = |part: &std::collections::BTreeMap<String, String>| {
        part.iter()
            .map(|(unit, key)| (unit.clone(), key.clone()))
            .collect()
    };
    ShuangpinCustomTable {
        initials: pairs(&profile.initials),
        finals: pairs(&profile.finals),
        zero_initials: pairs(&profile.zero_initials),
    }
}

/// 偏好里的自定义双拼表能不能用，由 Engine 判定；不能用时是给日志和代理看的英文原因。设置页、MCP 等写偏好的一方保存之前用它校验。
pub fn validate_custom_shuangpin_profile(profile: &ShuangpinCustomProfile) -> Result<(), String> {
    validate_shuangpin_custom_profile(&engine_shuangpin_custom_table(profile))
        .map_err(|error| error.to_string())
}

/// 自定义双拼表为什么不能用，按 Engine 的 `ShuangpinCustomProfileError` 逐项摊开，给设置页按种类写自己的提示、标出出错的那一格。Engine 在第一个错误处就停，所以一次只有一条。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ShuangpinCustomProblem {
    pub kind: ShuangpinCustomProblemKind,
    /// 出错的是表的哪一部分：`initials`、`finals` 或 `zero_initials`。编码冲突不属于某一部分，没有这一项；多字母声母占了别的键时是 `initials`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<&'static str>,
    /// 出错的单位，例如 `zh`、`iong`、`er`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// 不合法或已被占用的键（已转成小写）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// 两个音节撞上的那个两键编码，以及这两个音节。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub second: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShuangpinCustomProblemKind {
    UnknownUnit,
    DuplicateUnit,
    MissingUnit,
    InvalidKey,
    InitialKeyTaken,
    AmbiguousCode,
}

impl ShuangpinCustomProblem {
    fn of_kind(kind: ShuangpinCustomProblemKind) -> Self {
        Self {
            kind,
            part: None,
            unit: None,
            key: None,
            code: None,
            first: None,
            second: None,
        }
    }
}

impl From<ShuangpinCustomProfileError> for ShuangpinCustomProblem {
    fn from(error: ShuangpinCustomProfileError) -> Self {
        use ShuangpinCustomProblemKind as Kind;
        match error {
            ShuangpinCustomProfileError::UnknownUnit { part, unit } => Self {
                part: Some(part.name()),
                unit: Some(unit),
                ..Self::of_kind(Kind::UnknownUnit)
            },
            ShuangpinCustomProfileError::DuplicateUnit { part, unit } => Self {
                part: Some(part.name()),
                unit: Some(unit),
                ..Self::of_kind(Kind::DuplicateUnit)
            },
            ShuangpinCustomProfileError::MissingUnit { part, unit } => Self {
                part: Some(part.name()),
                unit: Some(unit.to_owned()),
                ..Self::of_kind(Kind::MissingUnit)
            },
            ShuangpinCustomProfileError::InvalidKey { part, unit, key } => Self {
                part: Some(part.name()),
                unit: Some(unit),
                key: Some(key),
                ..Self::of_kind(Kind::InvalidKey)
            },
            ShuangpinCustomProfileError::InitialKeyTaken { unit, key } => Self {
                part: Some(ShuangpinCustomTablePart::Initials.name()),
                unit: Some(unit.to_owned()),
                key: Some(key),
                ..Self::of_kind(Kind::InitialKeyTaken)
            },
            ShuangpinCustomProfileError::AmbiguousCode {
                code,
                first,
                second,
            } => Self {
                code: Some(code),
                first: Some(first),
                second: Some(second),
                ..Self::of_kind(Kind::AmbiguousCode)
            },
        }
    }
}

/// 编辑中的自定义双拼表有什么问题，能用时是 `None`。结论与 [`validate_custom_shuangpin_profile`] 相同，但用 Engine 不留存的检查：设置页每改一个键就问一次，每张合法的中间表都留下来就是永久泄漏。
pub fn custom_shuangpin_profile_problem(
    profile: &ShuangpinCustomProfile,
) -> Option<ShuangpinCustomProblem> {
    check_shuangpin_custom_profile(&engine_shuangpin_custom_table(profile))
        .err()
        .map(ShuangpinCustomProblem::from)
}

/// 内置双拼方案写成自定义表的样子，作为编辑自定义方案的起点；`Custom` 本身没有内置的表，返回 `None`。
pub fn builtin_shuangpin_custom_profile(
    profile: ShuangpinProfile,
) -> Option<ShuangpinCustomProfile> {
    let table =
        ShuangpinProfileKind::from_u8(profile_code(profile)).and_then(shuangpin_builtin_table)?;
    let map = |pairs: Vec<(String, String)>| pairs.into_iter().collect();
    Some(ShuangpinCustomProfile {
        initials: map(table.initials),
        finals: map(table.finals),
        zero_initials: map(table.zero_initials),
    })
}

/// 把双拼方案写进 Engine 选项。选的是自定义方案时先让 Engine 校验用户的表：表是空的或不合法时按小鹤运行，返回原因，免得一张写坏的表让 Engine 建不起来、整个输入法停在旧会话上。
fn apply_shuangpin_profile(
    options: &mut EngineOptions,
    preferences: &Preferences,
) -> Option<String> {
    options.shuangpin_custom_profile = None;
    options.shuangpin_profile = profile_code(preferences.shuangpin_profile);
    if preferences.shuangpin_profile != ShuangpinProfile::Custom {
        return None;
    }
    match validate_custom_shuangpin_profile(&preferences.shuangpin_custom_profile) {
        Ok(()) => {
            options.shuangpin_custom_profile = Some(engine_shuangpin_custom_table(
                &preferences.shuangpin_custom_profile,
            ));
            None
        }
        Err(error) => {
            options.shuangpin_profile = profile_code(ShuangpinProfile::Xiaohe);
            Some(format!(
                "Custom shuangpin profile unusable because {error}; using Xiaohe."
            ))
        }
    }
}

/// 按与实体双拼相同的方案和非法表回退派生触屏 14 键。
fn fourteen_key_shuangpin_grid(preferences: &Preferences) -> KeyGrid {
    let kind = ShuangpinProfileKind::from_u8(profile_code(preferences.shuangpin_profile))
        .expect("偏好双拼方案均有对应的引擎方案");
    let custom = (preferences.shuangpin_profile == ShuangpinProfile::Custom)
        .then(|| engine_shuangpin_custom_table(&preferences.shuangpin_custom_profile));
    let table = resolve_fourteen_key_shuangpin(kind, custom.as_ref()).unwrap_or_else(|_| {
        resolve_fourteen_key_shuangpin(ShuangpinProfileKind::Xiaohe, None)
            .expect("内置小鹤方案可派生 14 键表")
    });
    KeyGrid::FourteenKeyShuangpin(table)
}

/// 按与实体双拼相同的方案和非法表回退派生触屏 18 键。
fn eighteen_key_shuangpin_grid(preferences: &Preferences) -> KeyGrid {
    let kind = ShuangpinProfileKind::from_u8(profile_code(preferences.shuangpin_profile))
        .expect("偏好双拼方案均有对应的引擎方案");
    let custom = (preferences.shuangpin_profile == ShuangpinProfile::Custom)
        .then(|| engine_shuangpin_custom_table(&preferences.shuangpin_custom_profile));
    let table = resolve_eighteen_key_shuangpin(kind, custom.as_ref()).unwrap_or_else(|_| {
        resolve_eighteen_key_shuangpin(ShuangpinProfileKind::Xiaohe, None)
            .expect("内置小鹤方案可派生 18 键表")
    });
    KeyGrid::EighteenKeyShuangpin(table)
}

/// 偏好里的九键双拼方案对应的 Engine 方案。
pub fn nine_key_shuangpin_kind(profile: NineKeyShuangpinProfile) -> NineKeyShuangpinKind {
    match profile {
        NineKeyShuangpinProfile::Migala2 => NineKeyShuangpinKind::Migala2,
        NineKeyShuangpinProfile::Migala3 => NineKeyShuangpinKind::Migala3,
        NineKeyShuangpinProfile::Gushi2Low => NineKeyShuangpinKind::Gushi2Low,
        NineKeyShuangpinProfile::Custom => NineKeyShuangpinKind::Custom,
    }
}

/// 偏好里的九键双拼表（与 26 键的自定义表同形）换成 Engine 的表。
fn engine_nine_key_shuangpin_table(profile: &ShuangpinCustomProfile) -> NineKeyShuangpinTable {
    let pairs = |part: &std::collections::BTreeMap<String, String>| {
        part.iter()
            .map(|(unit, key)| (unit.clone(), key.clone()))
            .collect()
    };
    NineKeyShuangpinTable {
        initials: pairs(&profile.initials),
        finals: pairs(&profile.finals),
        zero_initials: pairs(&profile.zero_initials),
    }
}

/// 偏好里的九键双拼自定义表能不能用，由 Engine 判定；不能用时是给日志和代理看的英文原因。MCP 等写偏好的一方保存之前用它校验。
pub fn validate_nine_key_shuangpin_custom_profile(
    profile: &ShuangpinCustomProfile,
) -> Result<(), String> {
    validate_nine_key_shuangpin_table(&engine_nine_key_shuangpin_table(profile))
        .map(|_| ())
        .map_err(|error| error.to_string())
}

impl From<NineKeyShuangpinError> for ShuangpinCustomProblem {
    fn from(error: NineKeyShuangpinError) -> Self {
        use ShuangpinCustomProblemKind as Kind;
        match error {
            NineKeyShuangpinError::UnknownUnit { part, unit } => Self {
                part: Some(part.name()),
                unit: Some(unit),
                ..Self::of_kind(Kind::UnknownUnit)
            },
            NineKeyShuangpinError::DuplicateUnit { part, unit } => Self {
                part: Some(part.name()),
                unit: Some(unit),
                ..Self::of_kind(Kind::DuplicateUnit)
            },
            NineKeyShuangpinError::MissingUnit { part, unit } => Self {
                part: Some(part.name()),
                unit: Some(unit.to_owned()),
                ..Self::of_kind(Kind::MissingUnit)
            },
            NineKeyShuangpinError::InvalidKey { part, unit, key } => Self {
                part: Some(part.name()),
                unit: Some(unit),
                key: Some(key),
                ..Self::of_kind(Kind::InvalidKey)
            },
        }
    }
}

/// 编辑中的九键双拼表有什么问题，能用时是 `None`。问题与 26 键自定义表同一个形状（只会出现 `unknown_unit`、`duplicate_unit`、`missing_unit` 和 `invalid_key` 四种），给设置页标出出错的那一格；用 Engine 不留存的检查，每改一个键问一次也不泄漏。
pub fn nine_key_shuangpin_custom_profile_problem(
    profile: &ShuangpinCustomProfile,
) -> Option<ShuangpinCustomProblem> {
    check_nine_key_shuangpin_table(&engine_nine_key_shuangpin_table(profile))
        .err()
        .map(ShuangpinCustomProblem::from)
}

/// 内置九键双拼方案写成自定义表的样子，作为编辑的起点；`Custom` 没有内置的表，返回 `None`。
pub fn builtin_nine_key_shuangpin_custom_profile(
    profile: NineKeyShuangpinProfile,
) -> Option<ShuangpinCustomProfile> {
    let table = nine_key_shuangpin_builtin_table(nine_key_shuangpin_kind(profile))?;
    let map = |pairs: Vec<(String, String)>| pairs.into_iter().collect();
    Some(ShuangpinCustomProfile {
        initials: map(table.initials),
        finals: map(table.finals),
        zero_initials: map(table.zero_initials),
    })
}

/// 九键双拼的一个数字键在键面上写什么：这个键上的声母、韵母（`ü` 开头的韵母写成 `ü`），以及零声母编码是否从这个键开始。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NineKeyShuangpinKeyFace {
    pub key: String,
    pub initials: Vec<String>,
    pub finals: Vec<String>,
    pub zero_initial: bool,
}

impl From<NineKeyShuangpinKeyHint> for NineKeyShuangpinKeyFace {
    fn from(hint: NineKeyShuangpinKeyHint) -> Self {
        Self {
            key: hint.key.to_string(),
            initials: hint.initials,
            finals: hint.finals,
            zero_initial: hint.zero_initial,
        }
    }
}

/// 一个九键双拼方案在宿主看来的样子（`msime_client_nine_key_shuangpin_layout`）：输入法实际用的方案和它的表、键面，以及用户表的问题。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NineKeyShuangpinLayout {
    /// 输入法实际用的方案：选了 `custom` 而表不能用时是默认的内置方案，与建会话时的回退相同。
    pub effective: NineKeyShuangpinProfile,
    /// `effective` 的表，按单位到数字的三个对象。
    pub table: ShuangpinCustomProfile,
    /// 选了 `custom` 时用户表的问题，能用时为空。
    pub problem: Option<ShuangpinCustomProblem>,
    /// 按电话键盘 1 到 9、0 的顺序，每个用到的数字键的键面。
    pub keys: Vec<NineKeyShuangpinKeyFace>,
}

/// 选了 `profile`（`custom` 时用 `custom` 这张表）时输入法用哪张九键双拼表、键面写什么。不留存任何东西，设置页每改一个键都可以问。
pub fn nine_key_shuangpin_layout(
    profile: NineKeyShuangpinProfile,
    custom: &ShuangpinCustomProfile,
) -> NineKeyShuangpinLayout {
    let problem = (profile == NineKeyShuangpinProfile::Custom)
        .then(|| nine_key_shuangpin_custom_profile_problem(custom))
        .flatten();
    let effective = match profile {
        NineKeyShuangpinProfile::Custom if problem.is_some() => NineKeyShuangpinProfile::default(),
        other => other,
    };
    let table =
        builtin_nine_key_shuangpin_custom_profile(effective).unwrap_or_else(|| custom.clone());
    let keys = nine_key_shuangpin_table_key_hints(&engine_nine_key_shuangpin_table(&table))
        .map(|hints| {
            hints
                .into_iter()
                .map(NineKeyShuangpinKeyFace::from)
                .collect()
        })
        .unwrap_or_default();
    NineKeyShuangpinLayout {
        effective,
        table,
        problem,
        keys,
    }
}

/// 偏好里的九键双拼网格。选了 `custom` 而表不能用时按默认的内置方案运行，同时返回原因，免得一张写坏的表让九键双拼打不了字。
fn nine_key_shuangpin_grid(preferences: &Preferences) -> (KeyGrid, Option<String>) {
    let kind = nine_key_shuangpin_kind(preferences.nine_key_shuangpin_profile);
    let custom = engine_nine_key_shuangpin_table(&preferences.nine_key_shuangpin_custom_profile);
    match resolve_nine_key_shuangpin(kind, Some(&custom)) {
        Ok(table) => (KeyGrid::NineKeyShuangpin(table), None),
        Err(error) => {
            let fallback = nine_key_shuangpin_kind(NineKeyShuangpinProfile::default());
            let table =
                resolve_nine_key_shuangpin(fallback, None).expect("内置的九键双拼表一定合法");
            (
                KeyGrid::NineKeyShuangpin(table),
                Some(format!(
                    "Nine-key shuangpin table unusable because {error}; using {}.",
                    fallback.name()
                )),
            )
        }
    }
}

fn wubi_profile_code(profile: WubiProfile) -> u8 {
    match profile {
        WubiProfile::Wubi86 => 0,
        WubiProfile::Wubi98 => 1,
    }
}

/// 交给 Engine 的 `enabled_schemes`：本版本提供的方案，版本带临时日文时再加上它要切到的日文方案。full 提供全部方案，得到的就是 [`SchemeSet::ALL`]，Engine 照旧构造全部 provider。
fn engine_schemes(edition: &Edition) -> SchemeSet {
    let offered = offered_input_schemes(edition)
        .into_iter()
        .filter_map(|scheme| SchemeType::from_u8(scheme_code(scheme)))
        .fold(SchemeSet::EMPTY, SchemeSet::with);
    if edition.features.temporary_japanese {
        offered.with(SchemeType::JapaneseRomaji)
    } else {
        offered
    }
}

fn scheme_code(scheme: InputScheme) -> u8 {
    match scheme {
        InputScheme::Quanpin => 0,
        InputScheme::Shuangpin => 1,
        InputScheme::Wubi => 2,
        InputScheme::Japanese => 3,
        InputScheme::Korean => 4,
        InputScheme::Cantonese => 5,
        InputScheme::Zhuyin => 6,
        InputScheme::Vietnamese => 7,
        InputScheme::Tibetan => 8,
        InputScheme::Stroke => 9,
    }
}

fn vietnamese_input_method_code(vietnamese: VietnamesePreferences) -> u8 {
    match vietnamese.input_method {
        VietnameseInputMethod::Telex => 0,
        VietnameseInputMethod::Vni => 1,
    }
}

fn korean_layout_code(korean: KoreanPreferences, supported: bool) -> u8 {
    u8::from(supported && korean.layout == KoreanLayout::Romaja)
}

fn vietnamese_tone_style_code(vietnamese: VietnamesePreferences) -> u8 {
    match vietnamese.tone_style {
        VietnameseToneStyle::Modern => 0,
        VietnameseToneStyle::Classic => 1,
    }
}

/// The Cantonese, Zhuyin and Stroke dictionaries a host has installed. Each is its own artifact rather than part of the shared resource set, so any of them may be missing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LanguageDictionaries {
    pub(crate) cantonese: Option<std::path::PathBuf>,
    pub(crate) zhuyin: Option<std::path::PathBuf>,
    pub(crate) stroke: Option<std::path::PathBuf>,
    /// 五笔码表不在：资源目录不带 `msime-wubi.db`（本平台的发布包可以不带它），也没有下载的那份，见 [`wubi_dictionary_missing`]。缺省为假，五笔照常可用。
    pub(crate) wubi_missing: bool,
}

impl LanguageDictionaries {
    fn trusted_regular_file(path: &Path) -> bool {
        msime_path_trust::reject_symlinked_components(path).is_ok() && path.is_file()
    }

    /// 每个词库优先用状态目录（`downloaded`）下已下载的词库更新或资源包里的那份，没有时用 HostOptions 记录的 `recorded` 目录里的那份（随包内置或旧版本留下的），两处都没有就是缺席。
    pub(crate) fn resolve(
        downloaded: Option<&DownloadedDictionaries>,
        recorded: Option<&Path>,
    ) -> Self {
        let find = |name: &str| {
            downloaded
                .and_then(|downloaded| {
                    downloaded.updated_file(name).or_else(|| {
                        resource_packs::installed_file(
                            downloaded.root,
                            ResourcePack::LanguageDictionaries,
                            name,
                        )
                    })
                })
                .or_else(|| {
                    recorded
                        .filter(|directory| directory.is_absolute())
                        // 更新目录已在上面校验；校验失败不能再把它当作普通随包路径采用。
                        .filter(|directory| {
                            !downloaded.is_some_and(|downloaded| {
                                directory.starts_with(
                                    downloaded.canonical_root().join("dictionary-updates"),
                                )
                            })
                        })
                        .map(|directory| directory.join(name))
                        .filter(|path| Self::trusted_regular_file(path))
                })
        };
        LanguageDictionaries {
            cantonese: find("msime-cantonese.db"),
            zhuyin: find("msime-zhuyin.db"),
            stroke: find("msime-stroke.db"),
            wubi_missing: false,
        }
    }

    /// `msime-cantonese.db`, `msime-zhuyin.db` and `msime-stroke.db` in `directory`, each when it is a file.
    fn in_directory(directory: &std::path::Path) -> Self {
        let present = |name: &str| {
            let path = directory.join(name);
            Self::trusted_regular_file(&path).then_some(path)
        };
        LanguageDictionaries {
            cantonese: present("msime-cantonese.db"),
            zhuyin: present("msime-zhuyin.db"),
            stroke: present("msime-stroke.db"),
            wubi_missing: false,
        }
    }

    /// The dictionaries an Engine was configured with, where an empty path means none.
    fn of_options(options: &EngineOptions) -> Self {
        let named = |path: &str| (!path.is_empty()).then(|| std::path::PathBuf::from(path));
        LanguageDictionaries {
            cantonese: named(&options.cantonese_dictionary),
            zhuyin: named(&options.zhuyin_dictionary),
            stroke: named(&options.stroke_dictionary),
            wubi_missing: wubi_dictionary_missing(
                Path::new(&options.resources),
                &options.wubi_dictionary,
            ),
        }
    }

    fn is_empty(&self) -> bool {
        self.cantonese.is_none() && self.zhuyin.is_none() && self.stroke.is_none()
    }

    /// Whether `scheme` can run with these dictionaries: Cantonese, Zhuyin and Stroke need their own, 五笔要有码表（随包或下载的 `msime-wubi.db`）, every other scheme reads only the shared resources.
    fn serve(&self, scheme: InputScheme) -> bool {
        match scheme {
            InputScheme::Cantonese => self.cantonese.is_some(),
            InputScheme::Zhuyin => self.zhuyin.is_some(),
            InputScheme::Stroke => self.stroke.is_some(),
            InputScheme::Wubi => !self.wubi_missing,
            _ => true,
        }
    }
}

/// 已下载的日文资源包里的 `msime-japanese.dat`。`None` 时 Engine 读资源目录里的那份（完整发布包或开发环境内置的）。
fn japanese_dictionary(downloaded: Option<&DownloadedDictionaries>) -> Option<PathBuf> {
    let downloaded = downloaded?;
    downloaded.updated_file("msime-japanese.dat").or_else(|| {
        resource_packs::installed_file(
            downloaded.root,
            ResourcePack::Japanese,
            "msime-japanese.dat",
        )
    })
}

/// 五笔码表 `msime-wubi.db` 的位置，资源目录 `resources` 里有它时为 `None`（Engine 读资源目录里的那份）。macOS 发布包和 Android 的 full、拼音版瘦包不带它，这时是词库更新里的那份或已下载的 `wubi` 资源包里的那份，都没有时为 `None`。
fn wubi_dictionary(
    downloaded: Option<&DownloadedDictionaries>,
    resources: &Path,
) -> Option<PathBuf> {
    let name = msime_engine::assets::WUBI_DICTIONARY;
    if resources.join(name).is_file() {
        return None;
    }
    let downloaded = downloaded?;
    downloaded
        .updated_file(name)
        .or_else(|| resource_packs::installed_file(downloaded.root, ResourcePack::Wubi, name))
}

/// 五笔方案此时有没有码表可读：本平台的发布包可以不带 `msime-wubi.db`（[`ON_DEMAND_ARTIFACTS`]），资源目录里没有它，宿主也没有交来下载的那份（`wubi_dictionary` 为空）。别的平台的资源目录总是带着它，这里总为假。
pub(crate) fn wubi_dictionary_missing(resources: &Path, wubi_dictionary: &str) -> bool {
    let name = msime_engine::assets::WUBI_DICTIONARY;
    ON_DEMAND_ARTIFACTS
        .iter()
        .any(|group| group.contains(&name))
        && wubi_dictionary.is_empty()
        && !resources.join(name).is_file()
}

/// 一次解析（一次聚焦刷新或一次建会话）里，状态目录 `root` 下已下载词库的查找。
///
/// 一次解析要找粤拼、注音、笔画、日文和五笔最多五个文件，它们出自同一份词库更新：版本记录、资源锁、选择记录（`installed.json` 和它的逐层符号链接检查）、每个代次目录的规范化与校验，以及 `root` 的规范化，在这里各只做一次，这次要找的每个文件共用；文件本身的路径检查和长度检查仍逐个做。每一项都是第一次用到时才做，没用到的不做，与逐个文件查找时做的事相同。
///
/// 它只活在一次解析里，不跨调用保留：下一次聚焦或建会话重新读、重新做全部路径检查，所以设置应用随时换掉的选择记录下一次就会看到，没有要失效的缓存。
pub(crate) struct DownloadedDictionaries<'a> {
    root: &'a Path,
    selection: std::cell::OnceCell<Option<msime_client_providers::dictionary_updates::Selection>>,
    /// 选择记录里桌面词库代次和语言词库代次的目录，校验通过时为 `Some`。
    verified: [std::cell::OnceCell<Option<PathBuf>>; 2],
    canonical_root: std::cell::OnceCell<PathBuf>,
}

impl<'a> DownloadedDictionaries<'a> {
    pub(crate) fn new(root: &'a Path) -> Self {
        DownloadedDictionaries {
            root,
            selection: std::cell::OnceCell::new(),
            verified: Default::default(),
            canonical_root: std::cell::OnceCell::new(),
        }
    }

    /// 已选中的词库更新里的 `name`。可选词库只读已经由设置宿主完整校验、原子发布的文件，不在聚焦输入框时联网。
    fn updated_file(&self, name: &str) -> Option<PathBuf> {
        let selection = self
            .selection
            .get_or_init(|| {
                let edition = Edition::recorded_in(self.root).unwrap_or_else(Edition::full);
                let baseline = edition.resource_set().ok()?;
                msime_client_providers::dictionary_updates::selected(self.root, &baseline)
            })
            .as_ref()?;
        let (set, verified) = [Some(&selection.desktop), selection.languages.as_ref()]
            .into_iter()
            .zip(&self.verified)
            .find_map(|(set, verified)| {
                set.filter(|set| set.artifacts.iter().any(|file| file.name == name))
                    .map(|set| (set, verified))
            })?;
        let directory = verified
            .get_or_init(|| {
                let directory =
                    msime_client_providers::dictionary_updates::directory(self.root, set).ok()?;
                dictionary_updates::verify(self.root, &directory, set)?;
                Some(directory)
            })
            .as_ref()?;
        msime_client_providers::dictionary_updates::generation_file(directory, set, name)
    }

    /// 规范化后的 `root`，规范化失败时是 `root` 本身。
    fn canonical_root(&self) -> &Path {
        self.canonical_root
            .get_or_init(|| std::fs::canonicalize(self.root).unwrap_or_else(|_| self.root.into()))
    }
}

/// EngineOptions 里的路径文本，没有路径（或路径不是 UTF-8）时为空。
fn path_text(path: Option<PathBuf>) -> String {
    path.and_then(|path| path.to_str().map(str::to_owned))
        .unwrap_or_default()
}

/// 内置辅助码表的只读回退根目录（`EngineOptions::helpcode_fallback_resources`）。
///
/// 辅助码表随安装包放在资源目录的 `helpcodes/` 下，不在词库发布里，所以装上在线词库更新、`resources` 换成下载的代次目录后，那里没有这些表，Engine 只得到空表，辅助码什么也筛不掉。文档记了回退目录时用它（macOS 输入法、`prepare_host_with_updates` 准备出的文档）；没记时取更新记录里的随包资源目录，覆盖只经 `refresh_host_options` 换过 `resources` 的旧文档。`resources` 自己带着 `helpcodes/` 时不需要回退，也就不读更新记录。
fn helpcode_fallback_resources(
    recorded: Option<String>,
    resources: &Path,
    state_root: Option<&Path>,
    edition: &'static Edition,
) -> Option<String> {
    if recorded.is_some() || resources.join("helpcodes").is_dir() {
        return recorded;
    }
    recorded_helpcode_bundle(state_root?, &edition.resource_set().ok()?)
}

/// 更新记录里的随包资源目录，要求那里确实有 `helpcodes/`；`baseline` 是本版本编译进来的词库锁。
fn recorded_helpcode_bundle(state_root: &Path, baseline: &ResourceSet) -> Option<String> {
    let bundled =
        msime_client_providers::dictionary_updates::bundled_resources(state_root, baseline)?;
    if !bundled.join("helpcodes").is_dir() {
        return None;
    }
    bundled.to_str().map(str::to_owned)
}

/// HostOptions 的 `preferences_directory`，只认绝对路径：按需下载的资源包装在它下面。
fn absolute_state_root(preferences_directory: Option<&str>) -> Option<PathBuf> {
    preferences_directory
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// 按 `preferences` 交给 Engine 的方案，以及没用偏好里那个方案时的原因。本构建或本版本不提供的方案、没装词库的粤拼、注音和笔画，都回退到上一次的中文方案（它能跑时），否则回退到 `default`，所以别的宿主写下的文档不会让这个宿主没有能用的方案。偏好本身不改：词库装好后，下一个会话就跑用户选的方案。
///
/// `supported` 是运行中版本提供的方案（`offered_input_schemes`），`default` 是该版本的默认方案；full 分别是全部十个方案和全拼。所以在五笔版里，即使同步下来的偏好写着全拼，Engine 跑的也是五笔。
pub(crate) fn effective_scheme(
    preferences: &Preferences,
    supported: &[InputScheme],
    dictionaries: &LanguageDictionaries,
    default: InputScheme,
) -> (InputScheme, Option<String>) {
    let usable = |scheme: InputScheme| supported.contains(&scheme) && dictionaries.serve(scheme);
    let preferred = preferences.scheme;
    if usable(preferred) {
        return (preferred, None);
    }
    let reason = if supported.contains(&preferred) {
        "its dictionary is not installed"
    } else {
        "this host does not offer it"
    };
    let fallback = preferences
        .last_chinese_scheme
        .map(InputScheme::from)
        .filter(|scheme| usable(*scheme))
        .unwrap_or(default);
    (
        fallback,
        Some(format!(
            "Input scheme {preferred:?} unavailable because {reason}; using {fallback:?}."
        )),
    )
}

/// 偏好里的方案本版本提供、却因为本机缺它的词库跑不了时（[`effective_scheme`] 因此回退），装上哪个资源包它就能跑：五笔是 `wubi`，粤拼、注音和笔画是 `language-dictionaries`。其余情况为 `None`。
pub(crate) fn missing_scheme_pack(
    preferred: InputScheme,
    supported: &[InputScheme],
    dictionaries: &LanguageDictionaries,
) -> Option<ResourcePack> {
    if !supported.contains(&preferred) || dictionaries.serve(preferred) {
        return None;
    }
    match preferred {
        InputScheme::Wubi => Some(ResourcePack::Wubi),
        InputScheme::Cantonese | InputScheme::Zhuyin | InputScheme::Stroke => {
            Some(ResourcePack::LanguageDictionaries)
        }
        _ => None,
    }
}

/// 偏好里的双拼辅助码触发方式 → 引擎侧同名字段；写入端已归一，这里对消费端再归一一次兜底。
fn shuangpin_helpcode_modes(
    modes: msime_client_core::preferences::ShuangpinHelpcodeModes,
) -> msime_engine::ShuangpinHelpcodeModes {
    let modes = modes.normalized();
    msime_engine::ShuangpinHelpcodeModes {
        // 目前只有 macOS 宿主把 Tab 送入字符接口；其它宿主保留原触发方式。
        manual_tab: cfg!(target_os = "macos") && modes.manual_tab,
        direct: modes.direct,
        trigger_backtick: modes.trigger_backtick,
        trigger_semicolon: modes.trigger_semicolon,
        trigger_uppercase: modes.trigger_uppercase,
        mid_sentence: modes.mid_sentence,
        mid_sentence_backtick: modes.mid_sentence_backtick,
        mid_sentence_semicolon: modes.mid_sentence_semicolon,
        mid_sentence_uppercase: modes.mid_sentence_uppercase,
        direct_helpcode: modes.direct_helpcode,
        direct_helpcode_slash: modes.direct_helpcode_slash,
        direct_helpcode_uppercase: modes.direct_helpcode_uppercase,
    }
}

/// The helpcode settings for the scheme actually run, which differs from `Preferences::active_helpcode` only when `effective_scheme` fell back.
fn helpcode_for_scheme(
    preferences: &Preferences,
    scheme: InputScheme,
) -> msime_client_core::preferences::HelpcodePreferences {
    if scheme == preferences.scheme {
        preferences.active_helpcode()
    } else {
        Preferences {
            scheme,
            ..preferences.clone()
        }
        .active_helpcode()
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct HostOptions {
    api_version: u32,
    resources: String,
    /// 当前宿主随包提供的内置辅助码回退资源根目录。macOS 输入法只在内存文档中注入；`prepare_host_with_updates` 在 `resources` 换成下载代次时写它；都没有时 `helpcode_fallback_resources()` 从更新记录里找。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    helpcode_fallback_resources: Option<String>,
    user_data: String,
    cache: String,
    dictionaries: String,
    preferences: Preferences,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preferences_directory: Option<String>,
    /// The host draws a half-composed phrase itself: picking a candidate that covers only part of
    /// the input leaves the chosen piece in `view.phrase_prefix` instead of committing it, and the
    /// whole phrase commits at once when the composition ends. Absent means the previous behaviour,
    /// where each piece went to the document as it was picked, because a host that does not draw
    /// the field would otherwise show nothing for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    phrase_preedit: Option<bool>,
    /// `false`：这个会话不记选词位置和上屏效率。给与另一个会话处理同一次选词的镜像会话用：Windows TIP 在应用进程里的宿主会话跟着 Server 选同一个词，两边都记就会算两遍，由 Server 的会话记。缺省照常记。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selection_statistics: Option<bool>,
    /// macOS 和 Linux 的两个宿主把普通组字里的快捷短语也作为候选显示；缺省关闭以保持其他宿主和旧文档的入口契约。建会话时写进 `EngineOptions`，偏好变更重建 Engine 时沿用同一份选项。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    quick_phrase_candidates: Option<bool>,
    /// 仅完成键盘接入的桌面宿主显式打开；缺字段时同步偏好仍按两套式运行。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    korean_romaja: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    clipboard_history_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    online_provider_socket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    translation_provider_socket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cloud_dictionary_provider_socket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cloud_clipboard_provider_socket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    voice_provider_socket: Option<String>,
    /// Absolute path to the candidate reranking model, when it is installed as its own artifact
    /// rather than placed beside the dictionaries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sentence_model: Option<String>,
    /// Absolute path to the larger model run once typing settles, for hosts that install one.
    ///
    /// Its own option rather than an entry in the dictionary lock, because the lock is shared by
    /// every platform and `ResourceStore::verify` requires a resource directory to match it
    /// exactly — a desktop-only artifact there would mean teaching the manifest, the Rust
    /// verifier, the PowerShell verifier, four staging scripts and a CMake parser what a platform
    /// is, all on the path that guarantees a shipped dictionary is intact. Twenty five megabytes
    /// inside an iOS keyboard extension is also precisely what the small preset exists to avoid.
    ///
    /// A host that wants the second model installs it where it likes and names it here; one that
    /// does not leaves this absent and behaves exactly as before. Today that is every host: the
    /// desktop platforms are the ones this is for, and they set it alongside shipping the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    settled_model: Option<String>,
    /// Absolute path to the bundle's built-in sound packs (`resources/sound-packs` in the repository), for a host whose bundle does not put them in `sound-packs` beside `resources`, the directory used when this is absent. Installed packs, command tables and the `@` name list are read from `plugins` under `preferences_directory`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sound_packs: Option<String>,
    /// 桌面安装包随带的官方插件包目录（绝对路径，`<kind>/<id>/`，构建产物是 `target/plugin-packs`）。没有时取内置音效包目录旁边的 `plugin-packs`，那里也没有就没有官方包。macOS 的 bundle 和 Linux 的安装把它放在音效包旁边，不用写；Windows 的 Server 写它，因为那里的音效包在普通用户可写的数据目录里。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plugin_packs: Option<String>,
    /// Absolute path to the directory holding `msime-cantonese.db`, `msime-zhuyin.db` and `msime-stroke.db`, for a host that installs any of them. Absent, or a directory missing one of them, means that scheme falls back as `effective_scheme` describes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    language_dictionaries: Option<String>,
    /// 产品版本（`Edition::HOST_OPTIONS_KEY`）。只有不是 full 的版本才写：full 的文档因此与引入版本之前逐字节相同，旧版输入法（本结构拒绝未知键）照样能读。缺省就是 full；不是版本表里的 id 时整份文档被拒，而不是猜成 full。
    #[serde(default, skip_serializing_if = "Option::is_none", with = "edition_id")]
    edition: Option<&'static Edition>,
}

/// 宿主选项里的偏好副本用不了时，输入运行时改用的偏好：`preferences_directory` 下有实时的 preferences.json 就读它（读不了时在内存里救下认得的设置），没有就救副本自己的，默认值取文档记录的版本。文档里没有偏好副本、也没有实时文档时返回 `None`。读不了的偏好经诊断出口报告，盘上的文件不动。
fn runtime_preferences_for(document: &Value) -> Option<Preferences> {
    let directory = document
        .get("preferences_directory")
        .and_then(Value::as_str)
        .map(std::path::PathBuf::from)
        // load_for_runtime() 会创建传入的目录；从没在那里写过偏好的宿主没有可替代的内容。
        .filter(|directory| {
            directory.is_absolute() && directory.join("preferences.json").is_file()
        });
    if let Some(directory) = directory {
        if let Ok(runtime) = PreferencesStore::new(&directory).load_for_runtime() {
            report_unreadable_preferences(&runtime);
            return Some(runtime.snapshot.preferences);
        }
    }
    let copy = document.get("preferences")?.clone();
    // 副本本身没问题、是文档别处不对时照原样交回，不当成偏好读不了去报告。
    if let Ok(preferences) = Preferences::deserialize(&copy) {
        if preferences.validate().is_ok() {
            return Some(preferences);
        }
    }
    let edition = Edition::of_host_options(document)?;
    let (preferences, not_kept) =
        Preferences::salvaged_from(copy, edition_default_preferences(edition)).ok()?;
    crate::diagnostics::report(
        "runtime options preferences unusable",
        &format!(
            "the preferences copy in the runtime options does not validate and there is no live preferences.json; running on the settings that could be kept, settings not kept: {}",
            not_kept.join(", ")
        ),
    );
    Some(preferences)
}

/// `edition` 的默认偏好，与 [`PreferencesStore`] 修复损坏文件时垫底的相同。
fn edition_default_preferences(edition: &'static Edition) -> Preferences {
    if edition.is_full() {
        Preferences::default()
    } else {
        Preferences::for_edition(edition)
    }
}

/// 读得了时什么也不做；读不了时报告输入运行时改用了在内存里救下的设置。设置界面经严格读取看到同一个错误，修不修复由用户决定。
///
/// 只报告错误的种类和没能保留的设置键，不带 serde 的错误原文：类型不对时它会引出文档里的字符串取值，那可能是用户填的地址或密钥，而诊断日志会被打进诊断包发出去。
fn report_unreadable_preferences(runtime: &msime_client_core::preferences::RuntimePreferences) {
    use msime_client_core::preferences::PreferencesError;
    let Some(error) = &runtime.load_error else {
        return;
    };
    let kind = match error {
        PreferencesError::Json(_) => {
            "the document does not parse as this version's preferences".to_owned()
        }
        other => other.to_string(),
    };
    crate::diagnostics::report(
        "preferences document unreadable",
        &format!("{kind}; settings not kept: {}; typing continues on the settings that could be kept, the file is left untouched until it is repaired in settings or rewritten by a newer version", runtime.not_kept.join(", ")),
    );
}

/// HostOptions 的 `edition` 键与版本表条目之间的转换。
mod edition_id {
    use msime_client_core::edition::Edition;
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(
        edition: &Option<&'static Edition>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match edition {
            Some(edition) => serializer.serialize_str(&edition.id),
            None => serializer.serialize_none(),
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<&'static Edition>, D::Error> {
        match Option::<String>::deserialize(deserializer)? {
            None => Ok(None),
            Some(id) => Edition::by_id(&id)
                .map(Some)
                .ok_or_else(|| serde::de::Error::custom(format!("unknown edition {id:?}"))),
        }
    }
}

impl HostOptions {
    /// 文档记录的版本，缺省是 full。
    fn edition(&self) -> &'static Edition {
        self.edition.unwrap_or_else(Edition::full)
    }

    /// 解析 HostOptions 文档。其中的 `preferences` 是准备运行时配置那一刻的副本，升级时被原样带下去（`refresh_options_file`）。#2830 退役 `autocorrect` 时，带着它的副本曾让会话、快照和词库请求全部被拒，macOS 每个按键都像在英文模式下一样直接交给应用；偏好结构现在不拒绝不认识的键，新增或退役一个键不会再让副本读不了，但取值的类型或格式变化、以及新版本校验更严（#9802 的文本编辑排列）仍会。副本解析不了或通不过校验时，改用 `preferences_directory` 下实时的 preferences.json——设置界面会保持它最新并负责修复，宿主在会话打开后本来也会应用它；实时文档也读不了时在内存里救下它认得的设置（[`PreferencesStore::load_for_runtime`]），没有实时文档时救副本自己的，都不改写任何文件。副本能用时照原样使用。
    pub(crate) fn from_document(mut document: Value) -> Option<Self> {
        if let Ok(options) = Self::deserialize(&document) {
            if options.preferences.validate().is_ok() {
                return Some(options);
            }
        }
        document["preferences"] = serde_json::to_value(runtime_preferences_for(&document)?).ok()?;
        Self::deserialize(&document).ok()
    }

    fn into_engine_options(self) -> EngineOptions {
        // 每个平台都这样查找；日文词典只有 macOS 会下载，语言词库由 macOS 和没有随包带齐它们的 Linux 安装下载，别处的状态目录里从来没有它们。
        let state_root = absolute_state_root(self.preferences_directory.as_deref());
        let downloaded = state_root.as_deref().map(DownloadedDictionaries::new);
        let dictionaries = LanguageDictionaries::resolve(
            downloaded.as_ref(),
            self.language_dictionaries.as_deref().map(Path::new),
        );
        let japanese = japanese_dictionary(downloaded.as_ref());
        let wubi = path_text(wubi_dictionary(
            downloaded.as_ref(),
            Path::new(&self.resources),
        ));
        let dictionaries = LanguageDictionaries {
            wubi_missing: wubi_dictionary_missing(Path::new(&self.resources), &wubi),
            ..dictionaries
        };
        // Session creation has no diagnostic to carry the reason; the fallback itself is what matters here.
        let edition = self.edition();
        let (scheme, _) = effective_scheme(
            &self.preferences,
            &offered_input_schemes(edition),
            &dictionaries,
            edition.default_scheme,
        );
        let helpcode = helpcode_for_scheme(&self.preferences, scheme);
        let helpcode_fallback_resources = helpcode_fallback_resources(
            self.helpcode_fallback_resources,
            Path::new(&self.resources),
            state_root.as_deref(),
            edition,
        );
        let has_japanese_model = japanese.as_deref().is_some_and(Path::is_file)
            || Path::new(&self.resources)
                .join("msime-japanese.dat")
                .is_file();
        let mut options = EngineOptions {
            resources: self.resources,
            user_data: self.user_data,
            cache: self.cache,
            dictionaries: self.dictionaries,
            // 词库维护类操作也走这里：`HostOptions` 没有 `page_size_override`，只能用偏好值。
            first_page_rows: usize::from(host_page_size(self.preferences.candidate_page_size)),
            scheme: scheme_code(scheme),
            enabled_schemes: engine_schemes(edition),
            shuangpin_profile: profile_code(self.preferences.shuangpin_profile),
            shuangpin_custom_profile: None,
            shuangpin_preedit_uses_raw: self.preferences.shuangpin_preedit_uses_raw,
            force_cantonese_shuangpin: scheme == InputScheme::Cantonese
                && self.preferences.touch_keyboard_schemes.selected
                    == Some(TouchKeyboardScheme::CantoneseShuangpin),
            single_character_only: self.preferences.single_character_only,
            caret_edit_suffix: caret_edit_suffix_applies(&self.preferences),
            learning: self.preferences.learning,
            learn_english_on_enter: self.preferences.enter_learns_english_word,
            next_word_prediction: self.preferences.next_word_prediction
                && cfg!(target_os = "android"),
            autocorrect_transposition: self.preferences.quanpin.autocorrect_transposition,
            autocorrect_neighbor: self.preferences.quanpin.autocorrect_neighbor,
            fuzzy_pinyin_rules: self.preferences.fuzzy_pinyin.active_rules(),
            wubi_mixed_pinyin: self.preferences.wubi_mixed_pinyin,
            wubi_z_wildcard: self.preferences.wubi_z_wildcard,
            wubi_profile: wubi_profile_code(self.preferences.wubi_profile),
            frequency_mode: self.preferences.frequency.mode.as_str().into(),
            frequency_trigger_count: self.preferences.frequency.trigger_count,
            frequency_linear_step: self.preferences.frequency.linear_step,
            mixed_candidates: self.preferences.mixed_input.enabled,
            mixed_english: self.preferences.mixed_input.english,
            english_minimum_prefix: host_english_minimum_prefix(
                self.preferences.mixed_input.minimum_prefix,
            ),
            english_first_page_count: self.preferences.mixed_input.first_page_count,
            mixed_emoji: self.preferences.mixed_input.emoji,
            mixed_kaomoji: self.preferences.mixed_input.kaomoji,
            mixed_symbols: self.preferences.mixed_input.symbols,
            mixed_japanese: self.preferences.mixed_input.japanese,
            multilingual_languages: multilingual_languages_for(
                &self.preferences,
                edition,
                has_japanese_model,
            ),
            local_unicode: self.preferences.local_modes.unicode,
            local_date_time: self.preferences.local_modes.date_time,
            local_date_time_inline: self.preferences.local_modes.date_time_inline,
            local_date_time_menu: self.preferences.local_modes.date_time_menu,
            local_quick_phrase: self.preferences.local_modes.quick_phrase,
            local_emoji: self.preferences.local_modes.emoji,
            local_kaomoji: self.preferences.local_modes.kaomoji,
            local_super_jianpin: self.preferences.local_modes.super_jianpin,
            local_temporary_english: self.preferences.local_modes.temporary_english,
            local_temporary_japanese: self.preferences.local_modes.temporary_japanese,
            local_expression: self.preferences.local_modes.expression,
            local_command: self.preferences.local_modes.command,
            local_mention: self.preferences.local_modes.mention,
            local_chaizi: chaizi_enabled(&self.preferences.local_modes),
            local_symbol: self.preferences.local_modes.symbol,
            local_shift_letter_entry: self.preferences.local_modes.shift_letter_entry,
            chaizi_traditional_output: self.preferences.traditional_chinese_output,
            command_table: Vec::new(),
            plugin_commands: Vec::new(),
            mention_entries: Vec::new(),
            quick_phrase_table: Vec::new(),
            // 宿主声明能力、用户在设置里打开，两个条件都满足才把快捷短语混进普通候选；`mixed_input.quick_phrase` 缺键的旧文档读作开，保住 macOS/Linux 升级前的行为。
            quick_phrase_candidates: self.quick_phrase_candidates.unwrap_or(false)
                && self.preferences.mixed_input.quick_phrase,
            quick_phrase_frequency: self.preferences.mixed_input.quick_phrase_frequency,
            helpcode_table: None,
            helpcode_fallback_resources,
            sentence_association: engine_sentence_association(
                &self.preferences.sentence_association,
                absolute_state_root(self.preferences_directory.as_deref()).as_deref(),
            ),
            rescoring_context: String::new(),
            sentence_alternatives: true,
            vietnamese_input_method: vietnamese_input_method_code(self.preferences.vietnamese),
            vietnamese_tone_style: vietnamese_tone_style_code(self.preferences.vietnamese),
            korean_layout: korean_layout_code(
                self.preferences.korean,
                self.korean_romaja.unwrap_or(false),
            ),
            cantonese_dictionary: path_text(dictionaries.cantonese),
            zhuyin_dictionary: path_text(dictionaries.zhuyin),
            stroke_dictionary: path_text(dictionaries.stroke),
            japanese_dictionary: path_text(japanese),
            wubi_dictionary: wubi,
            helpcode: helpcode.enabled,
            strict_quanpin_helpcode: helpcode.strict_filter,
            show_helpcode: helpcode.show_in_candidate_window,
            // 双拼专属的触发方式；其它方案给默认值，引擎只在双拼下读它。手改出互斥组合的文档在消费端再归一一次，与 `PreferencesStore::save` 的写入端归一一致。
            shuangpin_helpcode_modes: shuangpin_helpcode_modes(helpcode.modes),
            helpcode_schema: helpcode.schema.as_str().into(),
            helpcode_trigger_key: helpcode.trigger_key.bytes().next(),
            chinese_punctuation: engine_chinese_punctuation(
                self.preferences.chinese_punctuation,
                punctuation_lock_code(self.preferences.punctuation_lock),
            ),
            paired_punctuation: self.preferences.paired_punctuation,
            punctuation_lock: punctuation_lock_code(self.preferences.punctuation_lock),
            corner_quotes: false,
            hollow_brackets: false,
            enumeration_comma_backslash: true,
            enumeration_comma_slash: false,
        };
        apply_punctuation_marks(&mut options, &self.preferences);
        // 建会话没有地方带原因，表不能用时按小鹤建起来就够了；之后的偏好变更会在诊断里报告原因。
        let _ = apply_shuangpin_profile(&mut options, &self.preferences);
        apply_local_mode_resource_gates(&mut options, edition);
        options
    }
}

/// Bootstrap a new host using the reviewed desktop data and Engine-owned replay.
/// Call only while all sessions using state_root are stopped. Does not activate it.
/// The settled-rerank model installed beside a resource bundle, when one is there.
///
/// A sibling directory rather than a file inside `resources`, because that directory is verified
/// against `desktop-dictionary.lock.json` and must match it *exactly* — an extra file there fails
/// the check whose job is to prove a shipped dictionary is intact. The lock is also shared by all
/// six platforms, and this model is wanted by three: it buys 49 points of top-1 on the harvested
/// failure set and costs p95 153ms per keystroke, which is why it runs on the settle timer, and
/// why 25MB of it has no business inside an iOS keyboard extension.
///
/// Absence is the normal case and is not an error. A host that installs the model puts it here;
/// one that does not is left exactly as it was.
fn settled_model_beside(resources: &std::path::Path) -> Option<String> {
    let path = resources
        .parent()?
        .join("settled-model")
        .join("sentence-model-desktop.safetensors");
    path.is_file().then(|| path.to_str())??.to_owned().into()
}

/// 后台加载落定重排模型的结果槽：加载完之前是空的，加载完是模型，模型不能用（文件坏了或版本不符）或没有模型时是 `None`。
type SettledModelSlot = Arc<OnceLock<Option<Arc<SentenceModel>>>>;

/// 在后台线程里加载落定重排模型，立即返回结果槽。约 25 MB 的权重不能在输入线程上读：聚焦和按键都在那个线程上。`path` 为 `None`（模型被移除）时槽里直接是 `None`。线程起不来时同样放 `None`，会话照常输入，只是没有落定重排。
fn load_settled_model(resources: &str, path: Option<PathBuf>) -> SettledModelSlot {
    let slot: SettledModelSlot = Arc::new(OnceLock::new());
    let Some(path) = path else {
        let _ = slot.set(None);
        return slot;
    };
    let resources = resources.to_owned();
    let loading = Arc::clone(&slot);
    let spawned = std::thread::Builder::new()
        .name("msime-settled-model".into())
        .spawn(move || {
            let _ = loading.set(sentence_model_settled(&resources, path.to_str()));
        });
    if spawned.is_err() {
        let _ = slot.set(None);
    }
    slot
}

/// 随包的落定重排模型：HostOptions 记录的 `settled_model`，没记录时是资源目录里的那份（[`sentence_model_settled`] 的默认位置）。只认存在的文件。
fn bundled_settled_model(resources: &str, recorded: Option<&str>) -> Option<PathBuf> {
    let path = match recorded {
        Some(path) => PathBuf::from(path),
        None => Path::new(resources).join(SETTLED_MODEL_FILE),
    };
    if !path.is_absolute() {
        return None;
    }
    path.is_file().then_some(path)
}

/// 会话使用的落定重排模型文件：随包的优先，随包的不在时（Windows 和 macOS 的发布包不再内置它）用 `state_root` 下已下载的 `settled-model` 资源包，都没有时为 `None`。旧版本写下的 HostOptions 可能还记着已被升级删掉的随包路径，那个文件不在了也照样落到资源包。
pub(crate) fn settled_model_file(
    resources: &str,
    recorded: Option<&str>,
    state_root: Option<&Path>,
) -> Option<PathBuf> {
    bundled_settled_model(resources, recorded).or_else(|| {
        resource_packs::installed_file(state_root?, ResourcePack::SettledModel, SETTLED_MODEL_FILE)
    })
}

/// 把会话的落定重排模型接上了没有写进诊断日志：会话建立时、开关打开时和模型文件变了（资源包装好或移除）换上之后各一行，按键和评分路径上不写。只在「桌面神经联想」开着时由调用方报告，关着的用户不会看到这些行。
///
/// 没有这几行时，「没装模型」「装了但会话没拿到状态目录」「装了但读不了」在日志里一模一样，都是什么也没有（#10341）。macOS 的 `diagnostic.log` 和 Windows 的 `logs\server.log` 都只记冒号前的类别（写成 `host_api: <类别>`），路径只出现在没登记出口的 Linux stderr 里：
/// - `settled model attached`：模型已加载并挂在会话上；
/// - `settled model not loaded`：找到了模型文件却没能用上，原因在同一次加载报的 `sentence model unreadable` 或 `sentence model ignored` 里（同一个文件在本进程只加载一次，后来的会话只有这一行）；
/// - `settled model missing`：随包的和状态目录下的 `settled-model` 资源包都没有；
/// - `settled model missing without state directory`：没有随包的模型，而宿主没给绝对路径的 `preferences_directory`，所以根本没去找下载的资源包。
pub(crate) fn report_settled_model(
    resources: &str,
    state_root: Option<&Path>,
    model: Option<&Path>,
    attached: bool,
) {
    match (model, state_root) {
        (Some(model), _) if attached => {
            diagnostics::report("settled model attached", &model.display().to_string())
        }
        (Some(model), _) => {
            diagnostics::report("settled model not loaded", &model.display().to_string())
        }
        (None, Some(state_root)) => diagnostics::report(
            "settled model missing",
            &format!(
                "no settled-model pack under {}",
                resource_packs::root(state_root).display()
            ),
        ),
        (None, None) => diagnostics::report(
            "settled model missing without state directory",
            &format!("no absolute preferences_directory; resources {resources}"),
        ),
    }
}

/// HostOptions 文档（`host_options`）所描述的安装布局里随包的落定重排模型。设置应用据此决定要不要提供 `settled-model` 资源包的下载：有随包的模型时会话不会用下载的那份。
pub fn packaged_settled_model(host_options: &Value) -> Option<PathBuf> {
    bundled_settled_model(
        host_options.get("resources")?.as_str()?,
        host_options.get("settled_model").and_then(Value::as_str),
    )
}

/// HostOptions 文档（`host_options`）所描述的安装布局是否随包带齐了粤拼、注音和笔画词库，也就是 `language_dictionaries` 记录的目录里三个文件都在。设置应用据此决定 Linux 上要不要提供 `language-dictionaries` 资源包的下载：没带齐的安装（打包时没有词库、或不带它们的 Nix 包）只能靠它补上，会话里每个词库都优先用资源包里的那份。
///
/// 记录的目录是词库更新下载的语言代次（[`inside_dictionary_updates`]）时，代次本身不算：它和下载它时的词库锁绑在一起，升级换锁后会话就不再选用它。这时改看更新记录里原安装包旁边的 `language-dictionaries/`（[`recorded_bundled_resources`]），安装包本身带齐了就算带齐；找不到安装包目录（换锁后更新记录不再有效）时按没带齐处理，只有资源包能长期提供这几份词库。
pub fn packaged_language_dictionaries(host_options: &Value) -> bool {
    let recorded = host_options
        .get("language_dictionaries")
        .and_then(Value::as_str)
        .map(Path::new);
    let recorded = match (recorded, document_state_root(host_options)) {
        (Some(recorded), Some(state)) if inside_dictionary_updates(recorded, &state) => {
            recorded_bundled_resources(host_options, &state)
                .and_then(|bundled| language_dictionaries_directory(&bundled))
        }
        (recorded, _) => recorded.map(Path::to_path_buf),
    };
    let dictionaries = LanguageDictionaries::resolve(None, recorded.as_deref());
    dictionaries.cantonese.is_some()
        && dictionaries.zhuyin.is_some()
        && dictionaries.stroke.is_some()
}

/// HostOptions 文档（`host_options`）的资源目录本身是否带着五笔码表 `msime-wubi.db`（普通文件，路径上没有符号链接）。设置应用据此决定 macOS 上要不要提供并在启动时补下 `wubi` 资源包。
///
/// 资源目录是词库更新下载的代次（[`inside_dictionary_updates`]）时，代次里的码表不算随包：更新只在资源目录或资源包已有码表时才下载它的新版本，所以代次里的码表可能来自不带码表的发布包之前的那一版；代次又和下载它时的词库锁绑在一起，应用升级换锁后 `dictionary_updates::selected` 不再选用它，会话回到不带码表的发布包，五笔就回退到全拼。这时改看更新记录里的原安装包目录（[`recorded_bundled_resources`]）：发布包本身带着码表（水杉五笔）就算随包；找不到安装包目录（换锁后更新记录不再有效）时按不随包处理，只有 `wubi` 资源包能跨过换锁一直在。
pub fn packaged_wubi_dictionary(host_options: &Value) -> bool {
    let Some(resources) = host_options
        .get("resources")
        .and_then(Value::as_str)
        .map(Path::new)
        .filter(|resources| resources.is_absolute())
    else {
        return false;
    };
    let packaged = match document_state_root(host_options) {
        Some(state) if inside_dictionary_updates(resources, &state) => {
            recorded_bundled_resources(host_options, &state)
        }
        _ => Some(resources.to_path_buf()),
    };
    packaged.is_some_and(|resources| {
        LanguageDictionaries::trusted_regular_file(
            &resources.join(msime_engine::assets::WUBI_DICTIONARY),
        )
    })
}

/// 词库更新记录里、与文档版本（`edition`）当前词库锁对得上的那份原安装包资源目录，只认绝对路径下仍存在的目录。应用升级换锁后更新记录不再有效，这里为 `None`。
fn recorded_bundled_resources(host_options: &Value, state_root: &Path) -> Option<PathBuf> {
    let baseline = Edition::of_host_options(host_options)?
        .resource_set()
        .ok()?;
    msime_client_providers::dictionary_updates::bundled_resources(state_root, &baseline)
        .filter(|bundled| bundled.is_absolute() && bundled.is_dir())
}

/// HostOptions 文档的绝对 `preferences_directory`，按需下载的资源包和词库更新都在它下面。
fn document_state_root(host_options: &Value) -> Option<PathBuf> {
    absolute_state_root(
        host_options
            .get("preferences_directory")
            .and_then(Value::as_str),
    )
}

/// `path` 是否在状态目录 `state_root` 的 `dictionary-updates/` 下，也就是词库在线更新下载的代次。路径和状态目录各按字面和规范化后的写法比较，macOS 的 `/var` 与 `/private/var` 这类别名都认得出。
fn inside_dictionary_updates(path: &Path, state_root: &Path) -> bool {
    let canonical =
        |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let paths = [path.to_path_buf(), canonical(path)];
    [state_root.to_path_buf(), canonical(state_root)]
        .iter()
        .map(|state| state.join("dictionary-updates"))
        .any(|updates| paths.iter().any(|path| path.starts_with(&updates)))
}

/// The offline gloss dictionary for one non-English target language installed beside a resource bundle, when one is there: `offline-glosses/zh-<language>.db`, built by `scripts/build_offline_glosses.py` and pinned by `resources/offline-glosses.lock.json`. A sibling of `resources` for the same reason as `settled_model_beside`: the resource directory must match the shared dictionary lock exactly, and a host ships only the languages it wants. Absence is the normal case.
pub(crate) fn offline_glosses_beside(
    resources: &std::path::Path,
    language: &str,
) -> Option<std::path::PathBuf> {
    if !OFFLINE_GLOSS_LANGUAGES.contains(&language) {
        return None;
    }
    let path = resources
        .parent()?
        .join("offline-glosses")
        .join(format!("zh-{language}.db"));
    path.is_file().then_some(path)
}

/// 一种非英文目标语言的离线释义词典：资源目录旁随包的那份优先，没有时用 `state_root` 下已下载的 `offline-glosses` 资源包里的那份（Android 的发布包不再内置它们，用户打开离线释义时下载），都没有时为 `None`。
pub(crate) fn offline_glosses_file(
    resources: &Path,
    state_root: Option<&Path>,
    language: &str,
) -> Option<PathBuf> {
    offline_glosses_beside(resources, language)
        .or_else(|| {
            let state = state_root?;
            let edition = Edition::recorded_in(state).unwrap_or_else(Edition::full);
            let original = msime_client_providers::dictionary_updates::bundled_resources(
                state,
                &edition.resource_set().ok()?,
            )?;
            offline_glosses_beside(&original, language)
        })
        .or_else(|| {
            if !OFFLINE_GLOSS_LANGUAGES.contains(&language) {
                return None;
            }
            resource_packs::installed_file(
                state_root?,
                ResourcePack::OfflineGlosses,
                &format!("zh-{language}.db"),
            )
        })
}

/// The Cantonese, Zhuyin and Stroke dictionaries installed beside a resource bundle: `language-dictionaries/msime-cantonese.db`, `language-dictionaries/msime-zhuyin.db` and `language-dictionaries/msime-stroke.db`, built by `msime-dict-builder`. A sibling of `resources` for the same reason as `settled_model_beside`: the resource directory must match the shared dictionary lock exactly, and only the hosts that offer these schemes ship them. Absence is the normal case.
pub(crate) fn language_dictionaries_beside(resources: &std::path::Path) -> LanguageDictionaries {
    language_dictionaries_directory(resources)
        .map(|directory| LanguageDictionaries::in_directory(&directory))
        .unwrap_or_default()
}

fn language_dictionaries_directory(resources: &std::path::Path) -> Option<std::path::PathBuf> {
    Some(resources.parent()?.join("language-dictionaries"))
}

/// The `language_dictionaries` value HostOptions records for `resources`: the directory beside them, only when it holds a dictionary, so a host without them writes the document it always did.
pub fn installed_language_dictionaries(resources: &std::path::Path) -> Option<String> {
    if language_dictionaries_beside(resources).is_empty() {
        return None;
    }
    language_dictionaries_directory(resources)
        .and_then(|directory| directory.to_str().map(str::to_owned))
}

/// The target languages an offline gloss dictionary can exist for; English is glossed from the packaged msime-english.db instead.
pub(crate) const OFFLINE_GLOSS_LANGUAGES: [&str; 6] = ["fr", "ja", "es", "ru", "de", "ko"];

/// Drop the `\\?\` prefix Windows canonicalisation adds.
///
/// The resource path is recorded in every HostOptions document this writes, and those documents have always held the plain drive form. Keeping it means an existing document and a freshly prepared one name the same directory the same way, and a native host reading the document gets the spelling it always got.
///
/// Only the drive form is unwrapped. `\\?\UNC\server\share` means something different from
/// `\\server\share` to the filesystem, so it is left alone rather than rewritten into a path that
/// happens to parse.
fn without_verbatim_prefix(path: std::path::PathBuf) -> std::path::PathBuf {
    let text = match path.to_str() {
        Some(text) => text,
        None => return path,
    };
    let stripped = match text.strip_prefix(r"\\?\") {
        Some(stripped) => stripped,
        None => return path,
    };
    let drive = stripped.as_bytes();
    if drive.len() >= 3 && drive[0].is_ascii_alphabetic() && drive[1] == b':' && drive[2] == b'\\' {
        return std::path::PathBuf::from(stripped);
    }
    path
}

/// Hash the resource set unless the last successful verification still describes what is on disk.
///
/// This runs at every Server start, and the desktop set is 169 MB: about half a second of SHA-256
/// before the first keystroke can be served, repeated at every login. `VerifiedMarker` records what
/// was verified so the repeat is skipped while the files are untouched.
///
/// The marker lives under the state root rather than beside the resources: on Windows the
/// resources are installed under Program Files, which the Server does not get to write to.
///
/// Failing to write the marker is not failing to start. The next launch hashes again, which is the
/// behaviour this function replaces, so the cost of that miss is the cost of doing nothing here.
fn reject_symlinked_state_root(path: &Path) -> Result<(), std::io::Error> {
    msime_path_trust::reject_symlinked_components(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidInput {
            std::io::Error::new(error.kind(), "state root contains a symbolic link")
        } else {
            error
        }
    })
}

/// 当前平台发布包可以不内置、改为按需下载的资源文件组：macOS 和 Android 是日文词典与它的两份 Mozc 许可文本、以及五笔码表，iOS 是日文词典那一组，其余平台照旧全部内置。规则本身见 [`msime_client_core::resources::on_demand_artifacts`]，测试在任何主机上都能检查每个目标的取值。
pub(crate) const ON_DEMAND_ARTIFACTS: &[&[&str]] =
    msime_client_core::resources::on_demand_artifacts(std::env::consts::OS);

/// `resources` 实际按哪一份清单发货：`on_demand` 里每一组文件全部缺席时去掉这一组，否则这一组照锁文件要求。见 [`ResourceSet::as_shipped_in_groups`]。
pub(crate) fn shipped_specification(
    specification: &ResourceSet,
    resources: &Path,
    on_demand: &[&[&str]],
) -> ResourceSet {
    specification.as_shipped_in_groups(resources, on_demand)
}

fn verify_resources_once(
    resources: &std::path::Path,
    specification: &ResourceSet,
    state_root: &std::path::Path,
    on_demand: &[&[&str]],
) -> Result<(), Box<dyn std::error::Error>> {
    // 从内置日文词典的旧版本升级后，第一次启动时校验标记描述的清单变了，会重新哈希一次，这是有意的。
    let shipped = shipped_specification(specification, resources, on_demand);
    reject_symlinked_state_root(state_root)?;
    match std::fs::symlink_metadata(state_root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "state root is a symbolic link",
            )
            .into());
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "state root is not a directory",
            )
            .into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let marker_path = state_root.join("verified-resources.json");
    let current = VerifiedMarker::describe(resources, &shipped)?;
    if let (Some(current), Some(recorded)) = (&current, VerifiedMarker::read(&marker_path)) {
        if *current == recorded {
            return Ok(());
        }
    }
    ResourceStore::new(resources).verify(resources, &shipped)?;
    if let Some(current) = current {
        let _ = current.write(&marker_path);
    }
    Ok(())
}

/// 只读检查已准备配置所指向的资源；沿用宿主的版本锁、按需资源分组和校验标记，不创建输入会话。
pub fn runtime_resources_ready(document: &Value) -> bool {
    let check = || -> Option<bool> {
        let resources = Path::new(document.get("resources")?.as_str()?);
        let state = Path::new(document.get("preferences_directory")?.as_str()?);
        if !resources.is_absolute() || !state.is_absolute() {
            return Some(false);
        }
        let baseline = Edition::of_host_options(document)?.resource_set().ok()?;
        let selected = dictionary_updates::selected_resources(state, &baseline);
        let specification = selected
            .as_ref()
            .filter(|(path, _)| path == resources)
            .map(|(_, set)| set)
            .unwrap_or(&baseline);
        Some(runtime_resources_ready_with(
            resources,
            state,
            specification,
            ON_DEMAND_ARTIFACTS,
        ))
    };
    check().unwrap_or(false)
}

fn runtime_resources_ready_with(
    resources: &Path,
    state: &Path,
    specification: &ResourceSet,
    on_demand: &[&[&str]],
) -> bool {
    if reject_symlinked_state_root(state).is_err() {
        return false;
    }
    let shipped = shipped_specification(specification, resources, on_demand);
    if let (Ok(Some(current)), Some(recorded)) = (
        VerifiedMarker::describe(resources, &shipped),
        VerifiedMarker::read(&state.join("verified-resources.json")),
    ) {
        if current == recorded {
            return true;
        }
    }
    ResourceStore::new(resources)
        .verify(resources, &shipped)
        .is_ok()
}

pub fn prepare_host_configuration(
    resources: &std::path::Path,
    state_root: &std::path::Path,
) -> Result<String, Box<dyn std::error::Error>> {
    prepare_host_configuration_for_edition(resources, state_root, Edition::full())
}

/// 为 `edition` 准备宿主：[`prepare_host_configuration`] 就是 full 的这一个。
///
/// 不是 full 的版本在文档里记下 `edition`，之后的会话、[`refresh_host_options`] 和 `msime-mcp` 都从文档里读它；full 的文档不写这个键，与以前完全相同。状态目录也记下它属于哪个版本（[`Edition::record_in`]，full 不写）：各平台宿主经 C 接口、设置应用经自己的存储都只按目录读写偏好，并不知道版本，偏好文件不见了或被修复时靠这份记录回到本版本的默认偏好。状态目录里还没有偏好文件时，每个版本都会把此刻读到的偏好写成第一份偏好文件：不经偏好存储直接读文件的一方第一次读到的也是本版本的默认值；新装和从以前的版本升级也只在这一刻分得清（触屏键盘的默认方案不同，见 `PreferencesStore::write_first_document`），写下文件后结论就固定了。
pub fn prepare_host_configuration_for_edition(
    resources: &std::path::Path,
    state_root: &std::path::Path,
    edition: &'static Edition,
) -> Result<String, Box<dyn std::error::Error>> {
    let specification = edition.resource_set()?;
    prepare_host_with_updates(resources, state_root, edition, &specification)
}

/// 安装选择与随包基线共用准备流程，测试用小型真实词库验证代次切换与日志回放。
fn prepare_host_with_updates(
    resources: &Path,
    state_root: &Path,
    edition: &'static Edition,
    specification: &ResourceSet,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some((updated, selected)) =
        dictionary_updates::selected_resources(state_root, specification)
    {
        if verify_resources_once(&updated, &selected, state_root, &[]).is_ok() {
            let mut document: Value = serde_json::from_str(&prepare_shipped_host_configuration(
                &updated,
                state_root,
                &selected,
                &[],
                edition,
            )?)?;
            // 随包的独立模型、音效和插件不属于主词库代次，保留原有入口。
            let original = msime_client_providers::dictionary_updates::bundled_resources(
                state_root,
                specification,
            )
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| resources.to_path_buf());
            for (key, name) in [
                ("sound_packs", "sound-packs"),
                ("plugin_packs", "plugin-packs"),
            ] {
                if let Some(path) = original
                    .parent()
                    .map(|p| p.join(name))
                    .filter(|p| p.is_dir())
                {
                    document[key] = json!(path);
                }
            }
            if let Some(model) = settled_model_beside(&original) {
                document["settled_model"] = json!(model);
            }
            // 辅助码表也不在词库发布里，下载的代次目录没有 `helpcodes/`。这里直接记下当前安装包的目录：iOS 每次 App 更新都换安装路径，更新记录里的随包目录会失效，`helpcode_fallback_resources()` 只能从记录里找。
            if original.join("helpcodes").is_dir() {
                document["helpcode_fallback_resources"] = json!(original);
            }
            if let Some(languages) = dictionary_updates::languages(state_root, edition)
                .and_then(|p| p.to_str().map(str::to_owned))
                .or_else(|| installed_language_dictionaries(&original))
            {
                document["language_dictionaries"] = json!(languages);
            }
            return Ok(serde_json::to_string_pretty(&document)?);
        }
    }
    // 记录的 resources 已属于下载代次时，回退仍需回到安装包目录。
    let bundled =
        msime_client_providers::dictionary_updates::bundled_resources(state_root, specification)
            .filter(|p| p.is_dir());
    prepare_shipped_host_configuration(
        bundled.as_deref().unwrap_or(resources),
        state_root,
        specification,
        ON_DEMAND_ARTIFACTS,
        edition,
    )
}

/// 状态目录里 `edition` 的偏好。还没有偏好文件时，先把此刻读到的偏好写成第一份文件（[`PreferencesStore::write_first_document`]），理由见 [`prepare_host_configuration_for_edition`]。必须在建用户词库代次之前调用：没有偏好文件时，状态目录有没有被准备过决定触屏键盘按新装还是按升级前的默认方案。
///
/// 已有的偏好文件读不了（更新的版本写的取值、本版本校验更严、写坏的字节）时不让准备失败，否则用户完全打不了字（#9802 那次 iOS 键盘、HarmonyOS 和 Windows 的 Server 都是这样）：改用在内存里救下的设置（[`PreferencesStore::load_for_runtime`]），文件不动，原因经诊断出口报告。只有存储本身出错时准备才失败。
fn edition_preferences(
    state_root: &Path,
    edition: &'static Edition,
) -> Result<Preferences, Box<dyn std::error::Error>> {
    let store = PreferencesStore::for_edition(state_root, edition);
    match store.write_first_document() {
        Ok(snapshot) => Ok(snapshot.preferences),
        Err(msime_client_core::preferences::PreferencesError::Io(error)) => Err(error.into()),
        Err(_) => {
            let runtime = store.load_for_runtime()?;
            report_unreadable_preferences(&runtime);
            Ok(runtime.snapshot.preferences)
        }
    }
}

/// [`prepare_host_configuration_for_edition`] 按给定的锁文件和按需下载清单准备；测试借它在各平台上检查 macOS 的发货规则。
fn prepare_shipped_host_configuration(
    resources: &std::path::Path,
    state_root: &std::path::Path,
    specification: &ResourceSet,
    on_demand: &[&[&str]],
    edition: &'static Edition,
) -> Result<String, Box<dyn std::error::Error>> {
    let resources = without_verbatim_prefix(std::fs::canonicalize(resources)?);
    let state_root = std::path::absolute(state_root)?;
    verify_resources_once(&resources, specification, &state_root, on_demand)?;
    // 偏好要在建 `user/dictionaries` 之前读：没有偏好文件时，以前的版本准备过的状态目录按升级前的默认方案，新装按现在的默认方案，建了代次目录就分不清了。
    let preferences = edition_preferences(&state_root, edition)?;
    // 用户词库日志读不出是数据库时，代次准备会把它改名挪开、从空日志开始，否则回放每次都失败、宿主永远起不来。在这里先挪，好经诊断出口报出来；准备代次时再查一遍只是空操作。
    let journal = state_root
        .join("user")
        .join(msime_engine::assets::USER_JOURNAL);
    if let Some(moved) = msime_engine::quarantine_corrupt_journal(&journal)? {
        crate::diagnostics::report(
            "user dictionary journal moved aside",
            &format!(
                "{} is not a database; learning starts again from an empty journal",
                moved.display()
            ),
        );
    }
    // 代次按本版本的方案准备：不含全拼、双拼、五笔的版本（日文、越南文、藏文）随包不带 msime-pinyin.db，代次里也没有它；full 的方案是全部，与以前相同。
    let prepared = msime_engine::host::prepare_options_for(
        resources.to_str().ok_or("non-UTF-8 resource path")?,
        state_root
            .join("user")
            .to_str()
            .ok_or("non-UTF-8 state path")?,
        state_root
            .join("cache")
            .to_str()
            .ok_or("non-UTF-8 cache path")?,
        // 代次按完整锁文件计算，不随发布包是否内置日文词典而变：user/dictionaries/<generation> 和 refreshed_host_options 都保持原样，升级后不会重新准备，仍在运行的旧版输入法也不会。
        &specification.generation()?,
        engine_schemes(edition),
    )?;
    // 先记下状态目录属于哪个版本：之后只拿到这个目录的偏好存储（各平台的 C ABI、设置应用）靠它取本版本的默认偏好。full 什么也不写。
    edition.record_in(&state_root)?;
    Ok(serde_json::to_string_pretty(&HostOptions {
        api_version: 1,
        resources: prepared.resources,
        helpcode_fallback_resources: None,
        user_data: prepared.user_data,
        cache: prepared.cache,
        dictionaries: prepared.dictionaries,
        preferences,
        preferences_directory: Some(
            state_root
                .to_str()
                .ok_or("non-UTF-8 state path")?
                .to_owned(),
        ),
        phrase_preedit: None,
        selection_statistics: None,
        quick_phrase_candidates: None,
        korean_romaja: None,
        clipboard_history_path: None,
        online_provider_socket: None,
        translation_provider_socket: None,
        cloud_dictionary_provider_socket: None,
        cloud_clipboard_provider_socket: None,
        voice_provider_socket: None,
        // Written only when a model has actually been installed as its own artifact; a host that
        // places one beside the dictionaries needs no configuration.
        sentence_model: None,
        settled_model: settled_model_beside(&resources),
        sound_packs: None,
        plugin_packs: None,
        language_dictionaries: installed_language_dictionaries(&resources),
        edition: (!edition.is_full()).then_some(edition),
    })?)
}

/// A runtime options refresh found that the recorded resource directory does not hold the resource set this build pins.
///
/// This is the state a user who downloaded the dictionaries (rather than getting them from the package) is left in after an upgrade that raised the dictionary version: the package replaced the lock but nothing replaced the files. It is told apart from every other refresh failure because only this one has a fix the user can run, `msime-linux-setup --update --download`. The C ABI passes errors through as their `Display` text, so the stable part of the contract is the `dictionary_outdated:` prefix; what follows it is diagnostic and may name private paths.
#[derive(Debug)]
pub struct DictionaryOutdated(msime_client_core::resources::ResourceError);

impl std::fmt::Display for DictionaryOutdated {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{DICTIONARY_OUTDATED_PREFIX} {}", self.0)
    }
}

impl std::error::Error for DictionaryOutdated {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// How a [`DictionaryOutdated`] error begins once it has crossed the C ABI as text.
pub const DICTIONARY_OUTDATED_PREFIX: &str = "dictionary_outdated:";

/// Turn a verification failure out of `prepare_host_configuration` into [`DictionaryOutdated`], leaving every other error as it was. Only a length, digest or directory-content mismatch counts: an unreadable file or an invalid compiled lock is not something a download fixes.
fn outdated_resources(error: Box<dyn std::error::Error>) -> Box<dyn std::error::Error> {
    use msime_client_core::resources::ResourceError;
    match error.downcast::<ResourceError>() {
        Ok(resource) => match *resource {
            outdated @ (ResourceError::Integrity | ResourceError::ExistingGeneration(_)) => {
                Box::new(DictionaryOutdated(outdated))
            }
            other => Box::new(other),
        },
        Err(error) => error,
    }
}

fn reject_symlinked_options_parent(path: &Path) -> std::io::Result<()> {
    let mut current = path.parent();
    while let Some(candidate) = current {
        match std::fs::symlink_metadata(candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                if msime_path_trust::is_trusted_system_alias(candidate) {
                    current = candidate.parent();
                    continue;
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "options path has a symbolic-link parent",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        current = candidate.parent();
    }
    Ok(())
}

/// Bring a published HostOptions file up to the installed dictionary generation.
///
/// A package upgrade replaces the resource bundle in place but leaves each user's options pointing at working dictionaries copied from the previous bundle, so the new dictionary never reaches the Engine and the user-dictionary replay the Windows installer runs after an upgrade never happens. When the recorded dictionaries directory is not the generation the installed lock describes, this prepares that generation (the Engine copies the new dictionaries and replays the user journal into them) and rewrites only `resources` and `dictionaries`, keeping every other key a setup or the settings app wrote. A current file is only read.
///
/// Returns whether the file was rewritten. Run it before the caller's own sessions exist. The previous generation is never modified, so a host still using it keeps working until it restarts. A symlink, or a document whose paths do not follow the layout `prepare_host_configuration` produces, is left alone rather than guessed at. When the recorded resources do not match the compiled lock the error is [`DictionaryOutdated`] and the file is left as it was.
pub fn refresh_host_options(path: &std::path::Path) -> Result<bool, Box<dyn std::error::Error>> {
    refresh_options_file(path, false, None)
}

/// 给自带一份已校验资源的宿主用的 [`refresh_host_options`]（macOS 设置应用 bundle 里的 `EngineResources` 就是这样一份）：记录的资源目录与编译进来的词库锁不符（[`DictionaryOutdated`]）时，改用 `bundled` 准备新代次，此后 `resources` 指向它。
///
/// 记录的资源目录不一定是安装包会替换的那一个。手工暂存到 Application Support 的目录、在输入法「准备词库」里选的目录，都停在暂存时的代次上，之后每次升级都以 [`DictionaryOutdated`] 失败，用户既拿不到新词库，也拿不到安装包放在自带资源旁的粤语、注音与笔画词库。记录的目录仍是当前代次，或者失败是别的原因时，处理与 [`refresh_host_options`] 完全相同；这里同样不碰 `language_dictionaries`。
pub fn refresh_host_options_from(
    path: &std::path::Path,
    bundled: &std::path::Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    refresh_options_file(path, false, Some(bundled))
}

/// 在 [`refresh_host_options`] 之外，不论代次是否变化，都让 `language_dictionaries` 跟上资源目录旁实际安装的粤语、注音与笔画词库；两项都不需要改时只读一次文件。代次准备失败也不会挡住这一项：仍按记录的资源目录更新这个键，然后再返回准备失败的错误，`resources` 与 `dictionaries` 保持原样。
///
/// 只有输入法进程自己在启动时、任何会话读取这份文件之前调用它。每个输入法会话都会重读这份文件，而 `HostOptions` 拒绝未知键，往一个仍被旧版输入法读取的文件里加键会让它再也开不了会话。设置应用升级后旧版输入法可能还在运行，所以设置应用自己的刷新是 [`refresh_host_options`]；运行这段代码的输入法认识它写入的键。
pub fn refresh_host_options_with_language_dictionaries(
    path: &std::path::Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    refresh_options_file(path, true, None)
}

fn refresh_options_file(
    path: &std::path::Path,
    language_dictionaries: bool,
    bundled: Option<&std::path::Path>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let parent = path.parent().ok_or("runtime options have no directory")?;
    let name = path
        .file_name()
        .ok_or("runtime options have no file name")?;
    let directory = msime_client_core::file_lock::open_private_directory(parent)?;
    reject_symlinked_options_parent(path)?;
    let locator = std::fs::symlink_metadata(path)?;
    if locator.file_type().is_symlink() {
        return Ok(false);
    }
    let file = msime_client_core::file_lock::open_private_file_at(&directory, name)?;
    let metadata = file.metadata()?;
    // The file can be replaced or grow after symlink_metadata returns. Read through a
    // limit-aware handle so that the size check remains effective across that race.
    let bytes =
        crate::bounded_file::read(file, HOST_OPTIONS_DOCUMENT_LIMIT as u64).map_err(|error| {
            if error.kind() == std::io::ErrorKind::InvalidData {
                Box::<dyn std::error::Error>::from("runtime options exceed 1 MiB")
            } else {
                Box::<dyn std::error::Error>::from(error)
            }
        })?;
    let document: Value = serde_json::from_slice(&bytes)?;
    // 文档记录的版本，缺省是 full。新代次按同一个版本准备；`refreshed_layout` 只替换 `resources` 和 `dictionaries`，`edition` 键原样保留。
    let edition =
        Edition::of_host_options(&document).ok_or("runtime options name an unknown edition")?;
    // 已安装的代次要和 `prepare_host_configuration_for_edition` 用同一份锁比较，否则不是 full 的版本每次刷新都会被当成过期。
    let baseline = edition.resource_set()?;
    let state = document["preferences_directory"].as_str().map(Path::new);
    let updated = state
        .and_then(|state| dictionary_updates::selected_resources(state, &baseline))
        .filter(|(path, set)| {
            state.is_some_and(|state| verify_resources_once(path, set, state, &[]).is_ok())
        });
    let specification = updated.as_ref().map(|(_, set)| set).unwrap_or(&baseline);
    let fallback = state.and_then(|state| {
        msime_client_providers::dictionary_updates::bundled_resources(state, &baseline)
    });
    let prepared = refreshed_host_options(
        &document,
        &specification.generation()?,
        updated
            .as_ref()
            .map(|(path, _)| path.as_path())
            .or(bundled)
            .or(fallback.as_deref()),
        Path::is_dir,
        |resources, state| {
            Ok(serde_json::from_str(
                &prepare_host_configuration_for_edition(resources, state, edition)
                    .map_err(outdated_resources)?,
            )?)
        },
    );
    // 语言词库随安装包到来，与词库代次无关；代次准备失败（最常见的是没有任何安装包会升级的资源目录，见 `refresh_host_options_from`）不能让它们进不了配置。
    let languages = if language_dictionaries {
        let current = match &prepared {
            Ok(Some(prepared)) => prepared,
            _ => &document,
        };
        with_installed_language_dictionaries(current)?
    } else {
        None
    };
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            if let Some(languages) = languages {
                replace_options_file(&directory, name, &metadata, &languages)?;
            }
            return Err(error);
        }
    };
    let Some(refreshed) = languages.or(prepared) else {
        return Ok(false);
    };
    replace_options_file(&directory, name, &metadata, &refreshed)?;
    Ok(true)
}

/// 用已打开的父目录句柄原子替换配置文件，保留原有权限。
fn replace_options_file(
    directory: &msime_client_core::file_lock::PrivateDirectory,
    name: &std::ffi::OsStr,
    metadata: &std::fs::Metadata,
    document: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut serialized = serde_json::to_vec_pretty(document)?;
    serialized.push(b'\n');
    msime_client_core::file_lock::replace_private_file_at(
        directory,
        name,
        &serialized,
        &metadata.permissions(),
    )?;
    Ok(())
}

/// 把输入法进程这次启动准备好的词库代次写进已发布的 `runtime-options.json`，只改 `dictionaries` 一个键；文件已经是这个代次时只读不写。返回是否改写了文件。
///
/// Windows 的 Server 每次启动都重新准备宿主（`msime_client_prepare_host`），自己的会话用准备出的文档；`runtime-options.json` 却只在首次运行时写下（`FirstRun.h`）。升级换了词库锁、或者装上了在线词库更新之后，Server 的会话读新代次，而照这份文件工作的设置窗口、`msime-mcp` 和各应用进程里的 TIP 还在旧代次上：设置里新增的快捷短语和五笔词条写进旧代次，Server 要等下次启动回放日志才看得到，用户看到的就是「加了没用」（#9434）。
///
/// 只改 `dictionaries`，不动 `resources`：在线词库更新的资源目录在状态目录下，而 Server 启动时从这份文件读 `resources` 并要求它与状态目录不相交，改了会让 Server 下次起不来。词库新增、编辑和查找都只经过代次里的工作副本，`resources` 留在安装包目录不影响它们。两份文档都必须是准备布局（[`prepared_layout`]）且属于同一个状态目录，否则原样不动；写入的路径沿用文件自己的拼写，只换代次目录名。
pub fn publish_prepared_dictionaries(
    path: &Path,
    prepared: &Value,
) -> Result<bool, Box<dyn std::error::Error>> {
    let parent = path.parent().ok_or("runtime options have no directory")?;
    let name = path
        .file_name()
        .ok_or("runtime options have no file name")?;
    let directory = msime_client_core::file_lock::open_private_directory(parent)?;
    reject_symlinked_options_parent(path)?;
    if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Ok(false);
    }
    let file = msime_client_core::file_lock::open_private_file_at(&directory, name)?;
    let metadata = file.metadata()?;
    let bytes = crate::bounded_file::read(file, HOST_OPTIONS_DOCUMENT_LIMIT as u64)?;
    let mut document: Value = serde_json::from_slice(&bytes)?;
    let (Some((_, published, state)), Some((_, generation, prepared_state))) =
        (prepared_layout(&document), prepared_layout(prepared))
    else {
        return Ok(false);
    };
    // 同一个状态目录可以写成不同的字面路径（Windows 的盘符大小写、短文件名），按规范化后的路径比较；文件记的状态目录已经不在，就不是这次准备的宿主的文件。
    let same_state = std::fs::canonicalize(state).is_ok_and(|state| {
        std::fs::canonicalize(prepared_state).is_ok_and(|prepared| state == prepared)
    });
    if !same_state {
        return Ok(false);
    }
    // 两份文档的拼写可能不同，只取代次目录名，接在文件自己的 `user_data/dictionaries` 下：照搬准备出的整条路径会让 `dictionaries` 与文件里的 `user_data` 拼写不一致，文件从此不再是准备布局，之后的发布和刷新都会跳过它。
    let (Some(current), Some(generation_name)) = (published.parent(), generation.file_name())
    else {
        return Ok(false);
    };
    if published.file_name() == Some(generation_name) {
        return Ok(false);
    }
    document["dictionaries"] = Value::String(
        current
            .join(generation_name)
            .to_str()
            .ok_or("dictionary generation path is not UTF-8")?
            .to_owned(),
    );
    replace_options_file(&directory, name, &metadata, &document)?;
    Ok(true)
}

/// 随包资源所在的目录：安装包里 Engine 资源目录的上一级，设置应用在这里找背单词的内置词书（`wordbooks/`）。
///
/// 词库在线更新后，准备出的文档里 `resources` 指向状态目录下的下载代次，那里没有随包资源；这时取更新记录里的原安装包目录，与 [`prepare_host_with_updates`] 给音效包和插件包定位、[`with_installed_language_dictionaries`] 找语言词库的规则相同。文档不是准备布局、或没有可用的更新记录时用 `resources` 本身；`resources` 不是绝对路径时为 `None`。
pub fn bundled_resource_root(document: &Value) -> Option<PathBuf> {
    let resources = document
        .get("resources")
        .and_then(Value::as_str)
        .map(Path::new)
        .filter(|resources| resources.is_absolute())?;
    let bundled = prepared_layout(document)
        .and_then(|(_, _, state)| {
            let baseline = Edition::of_host_options(document)?.resource_set().ok()?;
            msime_client_providers::dictionary_updates::bundled_resources(state, &baseline)
        })
        .filter(|bundled| bundled.is_dir());
    bundled
        .as_deref()
        .unwrap_or(resources)
        .parent()
        .map(Path::to_path_buf)
}

/// The `resources`, `dictionaries` and state directory of a document in the layout `prepare_host_configuration` produces, or `None` for any other document.
fn prepared_layout(document: &Value) -> Option<(&Path, &Path, &Path)> {
    let path = |key: &str| {
        document
            .get(key)
            .and_then(Value::as_str)
            .map(Path::new)
            .filter(|path| path.is_absolute())
    };
    let (resources, user_data, dictionaries, state) = (
        path("resources")?,
        path("user_data")?,
        path("dictionaries")?,
        path("preferences_directory")?,
    );
    (user_data == state.join("user")
        && dictionaries.parent() == Some(user_data.join("dictionaries").as_path()))
    .then_some((resources, dictionaries, state))
}

/// `refresh_host_options` 会发布的配置；文件已是当前代次或不符合准备布局时为 `None`。记录的资源目录是 [`DictionaryOutdated`]、而宿主自带 `bundled` 资源时，改用它准备代次。
fn refreshed_host_options(
    document: &Value,
    generation: &str,
    bundled: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
    mut prepare: impl FnMut(&Path, &Path) -> Result<Value, Box<dyn std::error::Error>>,
) -> Result<Option<Value>, Box<dyn std::error::Error>> {
    let Some((resources, dictionaries, state)) = prepared_layout(document) else {
        return Ok(None);
    };
    // 记录的资源目录已经不存在，最常见的是用户把设置应用挪了位置（例如从 /Applications 挪到 ~/Applications）：代次没变，只看代次就会让 `resources` 一直指向不存在的旧路径，下一次开会话就找不到词库。带着 bundle 刷新时改从 bundle 准备，不论代次是否变化。
    if let Some(bundled) = bundled.filter(|bundled| *bundled != resources && !exists(resources)) {
        return refreshed_layout(document, prepare(bundled, state)?).map(Some);
    }
    if dictionaries.file_name().and_then(|name| name.to_str()) == Some(generation) {
        return Ok(None);
    }
    let prepared = match (prepare(resources, state), bundled) {
        (Err(error), Some(bundled)) if error.is::<DictionaryOutdated>() && bundled != resources => {
            prepare(bundled, state)?
        }
        (prepared, _) => prepared?,
    };
    refreshed_layout(document, prepared).map(Some)
}

/// `document` with `resources` and `dictionaries` taken from what `prepare_host_configuration` returned.
fn refreshed_layout(
    document: &Value,
    prepared: Value,
) -> Result<Value, Box<dyn std::error::Error>> {
    let mut refreshed = document.clone();
    for key in ["resources", "dictionaries"] {
        refreshed[key] = prepared
            .get(key)
            .filter(|value| value.is_string())
            .cloned()
            .ok_or("prepared options are incomplete")?;
    }
    if let Some(value) = prepared
        .get("language_dictionaries")
        .filter(|v| v.is_string())
    {
        refreshed["language_dictionaries"] = value.clone();
    } else if document["language_dictionaries"]
        .as_str()
        .is_some_and(|p| p.contains("dictionary-updates"))
    {
        refreshed
            .as_object_mut()
            .ok_or("invalid host options")?
            .remove("language_dictionaries");
    }
    for key in ["settled_model", "sound_packs", "plugin_packs"] {
        if refreshed.get(key).is_none() {
            if let Some(value) = prepared.get(key) {
                refreshed[key] = value.clone();
            }
        }
    }
    Ok(refreshed)
}

/// 为准备流程生成的文档补齐语言词库路径；无需变化或布局不匹配时返回 `None`。
///
/// 优先资源相邻目录，其次已校验的下载语言代次，最后是更新记录中的原安装包目录。
/// 主词库代次不变也需刷新；显式指定且仍存在的其他目录继续保留。
fn with_installed_language_dictionaries(
    document: &Value,
) -> Result<Option<Value>, Box<dyn std::error::Error>> {
    let Some((resources, _, state)) = prepared_layout(document) else {
        return Ok(None);
    };
    let beside = language_dictionaries_directory(resources)
        .and_then(|directory| directory.to_str().map(str::to_owned));
    let recorded = document
        .get("language_dictionaries")
        .and_then(Value::as_str);
    // 指向别处、而且那个目录还在的记录是有意为之，原样保留；已经不存在的记录（应用挪了位置后留下的旧路径）换成资源目录旁实际安装的那份。
    if recorded
        .is_some_and(|recorded| Some(recorded) != beside.as_deref() && Path::new(recorded).is_dir())
    {
        return Ok(None);
    }
    // 主词库可以位于下载代次中，语言词库仍由更新记录或原安装包提供。
    let installed = installed_language_dictionaries(resources).or_else(|| {
        let edition = Edition::of_host_options(document)?;
        dictionary_updates::languages(state, edition)
            .and_then(|path| path.to_str().map(str::to_owned))
            .or_else(|| {
                let baseline = edition.resource_set().ok()?;
                let bundled = msime_client_providers::dictionary_updates::bundled_resources(
                    state, &baseline,
                )?;
                installed_language_dictionaries(&bundled)
            })
    });
    if recorded == installed.as_deref() {
        return Ok(None);
    }
    let mut refreshed = document.clone();
    let object = refreshed
        .as_object_mut()
        .ok_or("runtime options are not an object")?;
    match installed {
        Some(directory) => {
            object.insert("language_dictionaries".to_owned(), Value::String(directory));
        }
        None => {
            object.remove("language_dictionaries");
        }
    }
    Ok(Some(refreshed))
}

/// The reason prefix for an entry this layer or the Engine refuses on its own terms. Callers map it to one error code, so it stays stable while the part after the colon says which rule failed.
pub const INVALID_DICTIONARY_ENTRY: &str = "invalid dictionary entry";

pub(crate) fn invalid_dictionary_entry(reason: &str) -> String {
    format!("{INVALID_DICTIONARY_ENTRY}: {reason}")
}

/// 本版本没有中文词库时编辑拼音、五笔或快捷短语的拒绝原因。
pub(crate) const NO_CHINESE_DICTIONARY: &str =
    "this edition of the input method has no Chinese dictionary; only English words can be edited";

/// `kind` 的词能不能在 `options` 的版本里编辑。不含全拼、双拼、五笔的版本（日文、越南文、藏文）随包不带 msime-pinyin.db，拼音、五笔和快捷短语都无处可存，只有英文词能编辑（`SchemeSet::reads_main_dictionary`）。在加锁之前就拒绝，把原因告诉调用方，而不是让 Engine 拒绝后报成笼统的「dictionary edit rejected」。
pub(crate) fn require_dictionary_kind(
    options: &EngineOptions,
    kind: msime_engine::host::DictionaryKind,
) -> Result<(), String> {
    if kind == msime_engine::host::DictionaryKind::English
        || options.enabled_schemes.reads_main_dictionary()
    {
        Ok(())
    } else {
        Err(NO_CHINESE_DICTIONARY.into())
    }
}

/// Edit only after every participating host has destroyed its sessions.
/// Busy is retryable without cancelling any composition. The host must recreate
/// sessions after success; no native/Tauri management command is exposed yet.
///
/// An entry the Engine refuses is reported as `invalid dictionary entry: <reason>` with the Engine's reason, checked before anything is locked. Those reasons are fixed sentences in `validate_personal_dictionary_entry` that never repeat the submitted entry; collapsing them into one generic "rejected" left the settings page telling the user to retry an entry that could never be saved. Every other Engine diagnostic is still withheld, since it may include user text.
pub fn edit_personal_dictionary(
    options: &EngineOptions,
    previous: Option<&msime_engine::host::DictionaryEntry>,
    replacement: Option<&msime_engine::host::DictionaryEntry>,
    request_id: &str,
) -> Result<(), String> {
    validate_personal_dictionary_edit(options, previous, replacement, request_id)?;
    let access = DictionaryAccess::try_maintenance(
        std::path::Path::new(&options.user_data),
        std::path::Path::new(&options.dictionaries),
    )
    .map_err(|_| "dictionary access unavailable")?
    .ok_or("dictionary maintenance busy")?;
    edit_personal_dictionary_with_access(&access, options, previous, replacement, request_id)
}

/// 在调用方持有维护租约时校验并应用一个词条改动。
/// 租约按借用传入，让有界批次共用一个独占窗口，不必为每个待处理词条重新取锁。
pub(crate) fn edit_personal_dictionary_with_access(
    _access: &DictionaryAccess,
    options: &EngineOptions,
    previous: Option<&msime_engine::host::DictionaryEntry>,
    replacement: Option<&msime_engine::host::DictionaryEntry>,
    request_id: &str,
) -> Result<(), String> {
    validate_personal_dictionary_edit(options, previous, replacement, request_id)?;
    msime_engine::host::dictionary_edit(options, previous, replacement, request_id)
        .map_err(|_| "dictionary edit rejected".into())
}

fn validate_personal_dictionary_edit(
    options: &EngineOptions,
    previous: Option<&msime_engine::host::DictionaryEntry>,
    replacement: Option<&msime_engine::host::DictionaryEntry>,
    request_id: &str,
) -> Result<(), String> {
    for entry in previous.iter().chain(replacement.iter()) {
        require_dictionary_kind(options, entry.kind)?;
    }
    // The previous row is one the list returned, whose weight learning may have lifted past the ceiling a new entry is held to.
    for entry in previous.iter() {
        msime_engine::host::dictionary_validate_previous(entry)
            .map_err(|error| invalid_dictionary_entry(&error.to_string()))?;
    }
    for entry in replacement.iter() {
        msime_engine::host::dictionary_validate(entry)
            .map_err(|error| invalid_dictionary_entry(&error.to_string()))?;
    }
    if request_id.is_empty() {
        return Err("dictionary request id required".into());
    }
    Ok(())
}

/// A host-owned view of one entry in the verified local Emoji catalog.
#[derive(Clone, Debug, Serialize)]
pub struct LocalEmojiCatalogItem {
    pub text: String,
    pub annotation: String,
    pub group: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalEmojiCatalogSlice {
    pub items: Vec<LocalEmojiCatalogItem>,
    pub next_offset: usize,
    pub complete: bool,
}

/// Read catalog rows without collapsing equal text from distinct categories.
// Not unix-gated: the bodies only call the engine, which builds on Windows too. The gate was a porting gap, and it left the Windows desktop falling back to the compact built-in catalog - 97 emoji against the several thousand rows in msime-others.db - behind a permanent "catalog failed to load" banner.
pub fn local_emoji_catalog_slice(
    resources: &str,
    category: &str,
    offset: usize,
    limit: u16,
) -> Result<LocalEmojiCatalogSlice, &'static str> {
    if !std::path::Path::new(resources).is_absolute() {
        return Err("resources path must be absolute");
    }
    if limit == 0 || limit > 4096 {
        return Err("invalid local emoji page size");
    }
    msime_engine::host::emoji_catalog_slice(resources, "", category, "", offset, limit, "")
        .map(|page| LocalEmojiCatalogSlice {
            items: page
                .items
                .into_iter()
                .map(|item| LocalEmojiCatalogItem {
                    text: item.text,
                    annotation: item.annotation,
                    group: item.group,
                })
                .collect(),
            next_offset: page.next_offset,
            complete: page.complete,
        })
        .map_err(|_| "local emoji catalog unavailable")
}

pub use msime_client_services::plugins::symbol_set::PluginSymbolGroup;

/// 插件目录 `root` 下已安装的符号集的全部组，供宿主追加到内置符号目录之后：`symbols` 组放在以 `pack_name` 为上级分类的分组下，`kaomoji` 组放在颜文字的 All 之后。读几个小清单：不要在按键路径上调用。
pub fn plugin_symbol_groups(root: &std::path::Path) -> Vec<PluginSymbolGroup> {
    msime_client_services::plugins::symbol_set::plugin_symbol_groups(root)
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalSymbolCatalogGroup {
    pub parent: String,
    pub title: String,
    pub items: Vec<LocalEmojiCatalogItem>,
}

/// Preserve Engine-owned symbol parent categories and subgroup order.
pub fn local_symbol_catalog(resources: &str) -> Result<Vec<LocalSymbolCatalogGroup>, &'static str> {
    if !std::path::Path::new(resources).is_absolute() {
        return Err("resources path must be absolute");
    }
    let groups = msime_engine::host::emoji_symbol_groups(resources)
        .map_err(|_| "local symbol catalog unavailable")?;
    let mut result = Vec::with_capacity(groups.len());
    let mut remaining_pages = 256usize;
    for group in groups {
        let mut items = Vec::new();
        let mut first_page = true;
        let mut offset = 0usize;
        loop {
            if remaining_pages == 0 {
                return Err("local symbol catalog exceeds limit");
            }
            remaining_pages -= 1;
            let page = msime_engine::host::emoji_catalog_slice(
                resources,
                "",
                "symbols",
                &group.title,
                offset,
                512,
                &group.parent,
            )
            .map_err(|_| "local symbol catalog unavailable")?;
            if first_page {
                // The engine exposes pages rather than a total count. Use the first
                // page's actual item count as the only reliable initial capacity.
                items = Vec::with_capacity(page.items.len());
                first_page = false;
            }
            let complete = page.complete;
            let next_offset = page.next_offset;
            items.extend(page.items.into_iter().map(|item| LocalEmojiCatalogItem {
                text: item.text,
                annotation: item.annotation,
                group: item.group,
            }));
            if complete {
                break;
            }
            if next_offset <= offset {
                return Err("local symbol catalog cursor did not advance");
            }
            offset = next_offset;
        }
        result.push(LocalSymbolCatalogGroup {
            parent: group.parent,
            title: group.title,
            items,
        });
    }
    Ok(result)
}

/// Discover the optional helper-code tables installed below a verified resource directory.
///
/// The table files are Engine-owned assets, but their display metadata belongs to the shared
/// settings surface. Keep the path check at this host boundary so neither a native caller nor the
/// Tauri shell can ask the client core to inspect an arbitrary relative location. An absent or
/// unreadable `helpcodes/custom` directory is a valid empty catalog.
pub fn list_custom_helpcode_schemas(
    resources: &str,
) -> Result<Vec<msime_client_core::helpcode::CustomHelpcodeSchema>, &'static str> {
    let path = std::path::Path::new(resources);
    if !path.is_absolute() {
        return Err("resources path must be absolute");
    }
    Ok(msime_client_core::helpcode::list_custom_helpcode_schemas(
        path,
    ))
}

fn response(operation: impl FnOnce() -> Result<Value, String>) -> *mut c_char {
    let value = match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(value)) => json!({ "ok": true, "value": value }),
        Ok(Err(error)) => json!({ "ok": false, "error": error }),
        Err(_) => json!({ "ok": false, "error": "internal runtime failure" }),
    };
    // JSON escapes embedded NUL bytes, so this cannot contain an interior NUL.
    CString::new(value.to_string())
        .expect("JSON contains no NUL")
        .into_raw()
}

fn with_session<T>(
    handle: u64,
    action: impl FnOnce(&mut HostSession) -> Result<T, String>,
) -> Result<T, String> {
    SESSIONS.with(|sessions| {
        let mut sessions = sessions
            .try_borrow_mut()
            .map_err(|_| "reentrant host call")?;
        let runtime = sessions
            .get_mut(&handle)
            .ok_or("unknown session or wrong thread")?;
        action(runtime)
    })
}

fn dispatch(handle: u64, action: Action) -> *mut c_char {
    dispatch_call(handle, action, None)
}

/// 派发一个动作。`host` 只对命令 19、20 有值：动作同 Cancel、Finish，生命周期里的类别是 `HostCommit`（结束原因记 HostCommit）或 `FocusLoss`（上屏来源记 FocusLoss，不出声不画）。
///
/// 选择加入生命周期的会话按这个顺序：调用前采样并发 `input` → `runtime.dispatch`（脚本的 `candidates` 在里面的 refresh 和展开之后）→ 统计 → 读派发事实 → `complete_transition`（全角转换、再中英文之间加空格都在这里，之后才是宿主拿到的字节，宿主再做简繁转换）→ `before_commit`（脚本可以改写 body）→ 其余事件和内置消费者 → 序列化，FEEDBACK 开着时插入 `feedback`。没选择加入的会话只走原来的几步。
fn dispatch_call(handle: u64, action: Action, host: Option<lifecycle::InputClass>) -> *mut c_char {
    response(|| dispatch_value(handle, action, host))
}

/// [`dispatch_call`] 的本体，返回值不经 C 字符串，`candidate_preview` 在本进程里打字时直接用它。
fn dispatch_value(
    handle: u64,
    action: Action,
    host: Option<lifecycle::InputClass>,
) -> Result<Value, String> {
    with_session(handle, |session| {
        // 大写锁定改道先于其余处理，后面看到的都是改道后的动作。
        let action = session.caps_lock_punctuation(action);
        let action = session.runtime.candidate_shortcut(action);
        let currency_punctuation = matches!(
            action,
            Action::Character { value: b'$', .. } | Action::Punctuation(b'$')
        );
        let pending = session.lifecycle_input(&action, host);
        // Which row the user reached for, read before dispatching because the view it is
        // relative to is gone afterwards. Every platform host routes candidate selection
        // through here, so counting it here covers all of them without a line of platform
        // code; doing it per host would have meant six chances to forget.
        let position = selected_position(&action);
        let efficiency = position.and_then(|_| session.efficiency_candidate(&action));
        let result = session.runtime.dispatch(action).map_err(|e| e.to_string());
        // 报错前 runtime 可能已经改了组字，生命周期照样收尾。
        let mut result = session.lifecycle_on_error(pending, result)?;
        // Text the Engine generated (a calculator result, a command, a mention) was picked, not typed, and stays out of the statistics the way it stays out of learning.
        let counts_as_typing = result
            .commit_context
            .as_ref()
            .is_none_or(|context| context.typing_statistics);
        if result.commit.is_some() && counts_as_typing {
            // 先记效率再记位置：位置攒满一批会立刻写盘并带走待写的效率，这一次的效率要赶上同一批，否则进程在下一批之前被杀就丢了。
            if let Some(candidate) = efficiency {
                session.count_efficiency(candidate);
            }
            if let Some(position) = position {
                session.count_selection(position);
            }
        }
        let facts = session.runtime.facts();
        // 固定标点表返回全角人民币；半角只改本次 `$` 产生的尾巴，候选和短语里的符号保持原文。
        if currency_punctuation
            && result.view.character_width == CharacterWidth::Halfwidth
            && facts.tail_chars == 1
            && result.commit.as_deref().is_some_and(|c| c.ends_with('￥'))
        {
            // 光标后缀带着同一个单字尾巴，正文以 `￥` 结尾时它也以 `￥` 结尾；两者一起改写，`complete_transition` 核对时后缀仍是正文的真后缀。
            for text in result
                .commit
                .iter_mut()
                .chain(result.commit_cursor_suffix.iter_mut())
            {
                if text.ends_with('￥') {
                    text.truncate(text.len() - '￥'.len_utf8());
                    text.push('¥');
                }
            }
        }
        let mut result = session.complete_transition(result);
        // 脚本插件可能在这里改写上屏的 body（`before_commit`），宿主拿到的是改写之后的文本；引擎学习和 AI 上文已经按原文记下。
        let feedback = session.lifecycle_after(pending, facts, &mut result);
        let mut value = serde_json::to_value(result).map_err(|e| e.to_string())?;
        if let (Some(effect), Value::Object(fields)) = (feedback, &mut value) {
            fields.insert("feedback".into(), json!({ "typing_effect": effect }));
        }
        Ok(value)
    })
}

/// The one-based position of the candidate an action is about, or `None` when it is not about one.
///
/// Both selection actions carry the absolute candidate index from the runtime's generation, so
/// adding a view page offset would double-count candidates after the first page.
fn selected_position(action: &Action) -> Option<usize> {
    match action {
        // Candidate IDs carry the absolute index in the cached generation, even
        // when the view only shows one page. Adding the page offset here counted
        // every selection after the first page twice (and pushed it into the
        // aggregate `beyond` bucket in typing statistics).
        Action::Select(id) => Some(id.index + 1),
        Action::SelectAnyCandidate(id) => Some(id.index + 1),
        _ => None,
    }
}

/// 上屏效率（少按键、联想、整句）只在 Android 上计：只有 Android 的统计页显示它，其他宿主不为它多查读音。测试里也打开，好覆盖计法。
const COUNTS_COMMIT_EFFICIENCY: bool = cfg!(any(target_os = "android", test));

/// Android 宿主在隐私模式和不学习的输入框里、鸿蒙和 iOS 宿主在隐私模式里经 `msime_client_set_private_session` 标出隐私会话；这时选词位置和上屏效率都不计（鸿蒙和 iOS 本来就不计上屏效率）。其他宿主不变。
const PRIVATE_SESSIONS_SKIP_STATISTICS: bool = cfg!(any(
    target_os = "android",
    target_env = "ohos",
    target_os = "ios",
    test
));

/// 候选词翻译开着且范围是「仅当前高亮」：翻译查询只为高亮的候选请求候选词翻译，运行时只把它画在高亮行。
pub(crate) fn translation_scope_highlighted(preferences: &Preferences) -> bool {
    preferences.candidate_translations
        && preferences.candidate_translation_scope
            == msime_client_core::preferences::CandidateTranslationScope::Highlighted
}

/// `translation_query` 使用的偏好字段；这些变化都要撤销旧代次的释义回包。
fn translation_query_preferences_changed(previous: &Preferences, next: &Preferences) -> bool {
    previous.candidate_translations != next.candidate_translations
        || previous.candidate_translation_scope != next.candidate_translation_scope
        || previous.candidate_english_gloss != next.candidate_english_gloss
        || previous.candidate_pronunciation != next.candidate_pronunciation
        || previous.translation_target_language != next.translation_target_language
        || previous.translation_secondary_language != next.translation_secondary_language
        || previous.translation_account != next.translation_account
        || previous.custom_translation != next.custom_translation
        || previous.tencent_tmt != next.tencent_tmt
        || previous.niutrans != next.niutrans
}

/// 一个会话在两次写统计之间最多记下多少次上屏。正常情况下 `SELECTION_BATCH` 次选词就会写一次，这只是兜底。
const EFFICIENCY_BATCH_LIMIT: usize = 256;

/// 等后台写进统计的上屏批次，和是否已有一个线程在写。
struct EfficiencyQueue {
    /// 每一批：查读音用的 Engine 选项、写入的统计目录和这批上屏。
    pending: Vec<(EngineOptions, PathBuf, Vec<EfficiencyCandidate>)>,
    running: bool,
}

static EFFICIENCY_QUEUE: Mutex<EfficiencyQueue> = Mutex::new(EfficiencyQueue {
    pending: Vec::new(),
    running: false,
});

fn efficiency_queue() -> std::sync::MutexGuard<'static, EfficiencyQueue> {
    EFFICIENCY_QUEUE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 同一时间只有一个线程写效率：已有线程在写时，这一批排进队列由它接着写，不再每次写统计都另开一个线程、另开一次词库。线程在队列空了时才在同一把锁下退出，所以排进去的批次不会没人写。
fn record_efficiency_in_background(
    options: EngineOptions,
    directory: PathBuf,
    commits: Vec<EfficiencyCandidate>,
) {
    {
        let mut queue = efficiency_queue();
        queue.pending.push((options, directory, commits));
        if queue.running {
            return;
        }
        queue.running = true;
    }
    /// 写的时候 panic 也要放下 `running`，否则之后的效率再也没人写。正常退出在看到队列为空的同一把锁下放下，不经过这里。
    struct Running;
    impl Drop for Running {
        fn drop(&mut self) {
            if std::thread::panicking() {
                efficiency_queue().running = false;
            }
        }
    }
    let worker = move || {
        let _running = Running;
        loop {
            let batches = {
                let mut queue = efficiency_queue();
                if queue.pending.is_empty() {
                    queue.running = false;
                    return;
                }
                std::mem::take(&mut queue.pending)
            };
            for (options, directory, commits) in batches {
                record_efficiency(&options, &directory, &commits);
            }
        }
    };
    if std::thread::Builder::new()
        .name("msime-typing-efficiency".into())
        .spawn(worker)
        .is_err()
    {
        // 开不了线程时批次留在队列里，下一次写统计再试。
        efficiency_queue().running = false;
    }
}

/// 把一批上屏算成效率计数写进统计目录 `directory`（[`HostSession::statistics_directory`]）：`typed_keys` 是候选输入码里的字母和数字，`spelled_keys` 是用拼音逐字打出这段文字要按的键数（按全拼数读音字母，双拼也以全拼为基准）；查不到读音（英文、表情、非拼音方案）时按打了多少算多少，不算少按也不算多按。尽力而为：写不进就丢掉。
fn record_efficiency(options: &EngineOptions, directory: &Path, commits: &[EfficiencyCandidate]) {
    let mut efficiency = CommitEfficiency::default();
    for commit in commits {
        let spelled_keys = spelling_keys(options, &commit.text).unwrap_or(commit.typed_keys);
        efficiency.count_commit(
            commit.typed_keys,
            spelled_keys,
            commit.sentence,
            commit.prediction,
        );
    }
    let _ = TypingStatisticsStore::new(directory).record_efficiency(&efficiency);
}

/// 用全拼打出 `text` 要按的键数，作为「少按键」的基准：只有拼音系方案（全拼含九键、双拼）有答案，双拼也按全拼计，这样统计页的「比全拼少按」对双拼用户才有意义。读音来自 Engine 的 `hanzi_to_pinyin`，它先查整词、再逐字回退。
fn spelling_keys(options: &EngineOptions, text: &str) -> Option<u64> {
    match SchemeType::from_u8(options.scheme)? {
        SchemeType::Quanpin | SchemeType::Shuangpin => {}
        _ => return None,
    }
    let reading = msime_engine::host::hanzi_to_pinyin(options, text);
    if reading.is_empty() {
        return None;
    }
    Some(
        reading
            .split('\'')
            .map(|syllable| syllable.bytes().filter(u8::is_ascii_alphabetic).count() as u64)
            .sum(),
    )
}

/// 派发选择前从视图里读出的那个候选。
struct EfficiencyCandidate {
    text: String,
    typed_keys: u64,
    sentence: bool,
    prediction: bool,
}

/// How many committing selections a session holds in memory before handing them to the process's pending typing-statistics batch.
///
/// 交出去只是加内存里的计数（`flush_selections`），写文件跟着宿主经 `record` 攒的上屏走：满间隔的 `record`、失焦和销毁、读统计的操作。会话在失焦和销毁时也会交出手里的选词，所以这个数只限定输入框用着时有多少选词赶不上定期的写，也就是进程在输入框里被杀时丢多少：iOS 键盘扩展和 Android 输入法进程可能既不失焦也不销毁就被杀掉。32 次选词是几十秒的输入，对一个只报比例的统计丢得起，也免得每次选词都去取进程级的那把锁。
const SELECTION_BATCH: u64 = 32;

#[cfg(test)]
mod tests;

/// 只看偏好时，运行 `scheme` 的会话以哪种组码网格开始（宿主的 `msime_client_set_key_grid` 和 `msime_client_set_nine_key_mode` 仍可覆盖，但覆盖只对设下它时的方案有效）。Android 和 iOS 双拼看独立的 `touch_shuangpin_layout`，14 键和 18 键按当前 26 键方案派生；其他宿主保留但忽略这个偏好。Android 和 iOS 五笔看独立的 `touch_wubi_layout`，14 键与全拼 14 键同一种网格；鸿蒙的五笔 14 键由宿主自己按偏好调用 `msime_client_set_key_grid`。全拼看 `touch_keyboard_layout`：九键是九键，14 键是 14 键，18 键只在 Android 和 iOS 上开启（鸿蒙的全拼 18 键由宿主自己按偏好调用 `msime_client_set_key_grid`，其他宿主还没有对应键面），九键双拼按 `nine_key_shuangpin_profile` 取表（自定义表不能用时按默认的内置方案）。注音只有九键，还要选了「注音 9 键」触屏方案，这个方案只有 Android 键盘会写：`touch_keyboard_layout` 是所有方案共用的一个字段，桌面宿主为全拼九宫格设它（Linux 的九键开关），那里的注音会话必须留在大千键位，不能继承九宫格。
pub(crate) fn layout_key_grid(
    scheme: Option<SchemeType>,
    preferences: &Preferences,
) -> Option<KeyGrid> {
    layout_key_grid_for(ffi::host::compiled_platform(), scheme, preferences)
}

fn layout_key_grid_for(
    platform: HostPlatform,
    scheme: Option<SchemeType>,
    preferences: &Preferences,
) -> Option<KeyGrid> {
    if matches!(platform, HostPlatform::Android | HostPlatform::Ios)
        && scheme == Some(SchemeType::Shuangpin)
        && preferences.touch_keyboard_layout != TouchKeyboardLayout::Handwriting
    {
        match preferences.touch_shuangpin_layout {
            TouchShuangpinLayout::FourteenKey => {
                return Some(fourteen_key_shuangpin_grid(preferences))
            }
            TouchShuangpinLayout::EighteenKey => {
                return Some(eighteen_key_shuangpin_grid(preferences))
            }
            TouchShuangpinLayout::TwentySixKey => {}
        }
    }
    // 五笔 14 键与全拼 14 键同一个键面，引擎按组展开码表；手写布局不画字母键。
    if scheme == Some(SchemeType::Wubi) {
        return (matches!(platform, HostPlatform::Android | HostPlatform::Ios)
            && preferences.touch_wubi_layout == TouchWubiLayout::FourteenKey
            && preferences.touch_keyboard_layout != TouchKeyboardLayout::Handwriting)
            .then_some(KeyGrid::FourteenKey);
    }
    match (preferences.touch_keyboard_layout, scheme) {
        (TouchKeyboardLayout::NineKey, Some(SchemeType::Quanpin)) => Some(KeyGrid::NineKey),
        (TouchKeyboardLayout::FourteenKey, Some(SchemeType::Quanpin)) => Some(KeyGrid::FourteenKey),
        (TouchKeyboardLayout::EighteenKey, Some(SchemeType::Quanpin))
            if matches!(platform, HostPlatform::Android | HostPlatform::Ios) =>
        {
            Some(KeyGrid::EighteenKey)
        }
        (TouchKeyboardLayout::NineKeyShuangpin, Some(SchemeType::Quanpin)) => {
            Some(nine_key_shuangpin_grid(preferences).0)
        }
        (TouchKeyboardLayout::NineKey, Some(SchemeType::Zhuyin)) => {
            let schemes = &preferences.touch_keyboard_schemes;
            // 用户改了启用列表时 Android 可能清掉 `selected`，所以启用了「注音 9 键」而没有选中项时也算。
            let zhuyin_nine_key = schemes.selected == Some(TouchKeyboardScheme::ZhuyinNineKey)
                || (schemes.selected.is_none()
                    && schemes
                        .enabled
                        .contains(&TouchKeyboardScheme::ZhuyinNineKey));
            zhuyin_nine_key.then_some(KeyGrid::NineKey)
        }
        _ => None,
    }
}
