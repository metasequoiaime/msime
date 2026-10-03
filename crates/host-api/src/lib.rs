//! Versioned, thread-confined C interface for native IME hosts.
//! A handle registry rejects stale and wrong-thread handles without dereferencing them.

// The workspace denies unsafe code; this crate is the C ABI native hosts link against, so every handle,
// pointer and string it accepts crosses a boundary the compiler cannot check.
// The exemption is stated here rather than left implicit by opting out of
// the workspace lint table, which would also silently drop every other lint
// the workspace adds later.
#![allow(unsafe_code)]

use msime_client_core::ai::AiSuggestionRequest;
use msime_client_core::dictionary::access::DictionaryAccess;
use msime_client_core::edition::Edition;
use msime_client_core::host_surface::{
    offered_input_schemes, HostCapabilities, HostPlatform, SurfaceRoute,
};
pub mod cloud_clipboard;
pub mod cloud_dictionary;
pub mod mcp_clients;
pub mod system_fonts;
use msime_client_core::preferences::{
    InputScheme, Preferences, PreferencesSnapshot, PreferencesStore, ShuangpinProfile,
    TouchKeyboardLayout, VietnameseInputMethod, VietnamesePreferences, VietnameseToneStyle,
    WubiProfile,
};
use msime_client_core::punctuation::{
    route as punctuation_route, PunctuationContext, PunctuationRoute,
};
use msime_client_core::resource_packs::{self, ResourcePack};
use msime_client_core::resources::{ResourceSet, ResourceStore, VerifiedMarker};
use msime_client_core::typing_statistics::{TypingSource, TypingStatisticsStore, RANKS};
use msime_client_core::voice::doubao_frame::{
    audio_frame, decode_error_code, decode_json_frame, start_frame,
};
use msime_client_core::voice::VoiceSessionState;
use msime_engine::host::{CandidateEdge, Command, EngineOptions, Session};
use msime_engine::{SchemeSet, SchemeType};
use msime_input_runtime::HandwritingQuery;
#[cfg(unix)]
use msime_input_runtime::UnixSocketProvider;
use msime_input_runtime::{
    Action, AiAssistantProviderConfig, CandidateId, CharacterWidth, NineKeySpellingId,
    OnlineCandidate, OnlineQuery, Reranker, Runtime, SentenceModel, Transition, TranslationService,
};
#[cfg(unix)]
use msime_input_runtime::{EmojiPanelQuery, TranslationQuery};
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
mod bounded_file;
mod dictionary;
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
mod doubao_auth;
#[cfg(not(any(target_os = "android", target_env = "ohos")))]
mod handwriting_cells;
pub use doubao_auth::msime_client_doubao_auth_headers;
mod learned_translation;
mod niutrans_translation;
mod tencent_translation;
pub use dictionary::{
    dictionary_request_json, dictionary_words, edit_dictionary_word, edit_user_quick_phrase,
    import_dictionary_words, lookup_candidates, msime_client_dictionary,
    msime_client_dictionary_import_entries, msime_client_dictionary_validate,
    msime_client_personal_dictionary_request, msime_client_personal_dictionary_sync,
    personal_dictionary_request_json, user_quick_phrases, CandidateOrigin, DictionaryOptions,
    LookupCandidate, LookupScheme, NewWord, QuickPhrase, QuickPhraseEdit, QuickPhrasePage, Word,
    WordEdit, WordImport, WordKind, WordPage,
};
mod dictionary_snapshot;
mod key_sound;
mod plugin_tables;
mod voice_capture;
pub use dictionary_snapshot::{
    msime_client_snapshot_discard, msime_client_snapshot_inspect, msime_client_snapshot_prepare,
    msime_client_snapshot_queue, msime_client_snapshot_restore, msime_client_snapshot_version,
};

/// Capture endpoint identities paired with labels. Neither belongs in logs.
pub fn voice_capture_devices() -> Vec<(String, String)> {
    voice_capture::capture_devices()
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
    // Android injects ML Kit Digital Ink through HandwritingRecognizer and HarmonyOS uses its own recognizer. Keeping this boundary unavailable there prevents zinnia and its model path from becoming an unused second recognizer in the IME process.
    Err("local handwriting recognizer unavailable")
}

thread_local! {
    static SESSIONS: RefCell<HashMap<u64, HostSession>> = RefCell::new(HashMap::new());
}

struct HostSession {
    runtime: Runtime,
    options: EngineOptions,
    /// HostOptions 记录的版本：偏好里的方案只在本版本提供的方案里取，回退到本版本的默认方案。
    edition: &'static Edition,
    applied: Preferences,
    requested: Option<PreferencesSnapshot>,
    /// Whether the requested preferences still need an Engine replacement. This decision is made
    /// when the document arrives so every keystroke does not compare the full preference tree.
    preferences_pending: bool,
    punctuation_override: Option<bool>,
    paired_punctuation_override: Option<bool>,
    punctuation_lock_override: Option<u8>,
    english_mode: bool,
    page_size_override: Option<u8>,
    nine_key_override: Option<bool>,
    /// An AI provider credential the host keeps outside the preferences (the iOS Keychain), handed over for this session only and never written back.
    ai_credential: Option<String>,
    /// Cached copy used by every online query until preferences change.
    ai_provider_cache: Option<AiAssistantProviderConfig>,
    voice: VoiceSessionState,
    /// Committing candidate selections counted but not yet written to typing statistics, indexed by one-based position minus one, with every position past a page in the last slot. See `SELECTION_BATCH`.
    pending_selections: [u64; RANKS + 1],
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
    // Declared after runtime so the Engine is dropped before releasing access.
    _dictionary_access: DictionaryAccess,
}

/// 本地模式由偏好开关，但它们读的词典是不可变的运行时资源。可选资源不在时，触发键不能被吞掉：在资源齐全之前，Engine 看到的这个模式必须是关的。
///
/// 版本不提供的功能同样当作资源不在：五笔版的资源锁不带日文词典，临时日文无论偏好怎么写都是关的。资源目录按版本的锁校验，本来就放不进日文词典，这里再按版本表关一次，是为了状态目录里留有下载来的日文资源包时也不会打开它。
fn apply_local_mode_resource_gates(options: &mut EngineOptions, edition: &Edition) {
    let resources = std::path::Path::new(&options.resources);
    let has_emoji_catalog = resources.join("others.db").is_file();
    let has_english_dictionary = resources.join("english.db").is_file();
    // 下载的日文词典写在 `japanese_dictionary` 里；为空时 Engine 读资源目录里的那份。
    let has_japanese_model = if options.japanese_dictionary.is_empty() {
        resources.join("dict_japanese.dat").is_file()
    } else {
        Path::new(&options.japanese_dictionary).is_file()
    };
    options.local_emoji &= has_emoji_catalog;
    options.local_kaomoji &= has_emoji_catalog;
    options.local_temporary_english &= has_english_dictionary;
    options.local_temporary_japanese &= has_japanese_model && edition.features.temporary_japanese;
}

fn punctuation_lock_code(lock: msime_client_core::preferences::PunctuationLock) -> u8 {
    match lock {
        msime_client_core::preferences::PunctuationLock::Follow => 0,
        msime_client_core::preferences::PunctuationLock::Chinese => 1,
        msime_client_core::preferences::PunctuationLock::English => 2,
    }
}

/// The Engine's sentence association switches. The desktop model switch is not among them: it gates the runtime's settled reranker (`Runtime::set_settled_rerank_enabled`), the only place that model runs.
fn engine_sentence_association(
    preferences: &msime_client_core::preferences::SentenceAssociationPreferences,
) -> msime_engine::host::SentenceAssociationOptions {
    msime_engine::host::SentenceAssociationOptions {
        word_lattice: preferences.word_lattice,
        neural_keyboard: preferences.neural_keyboard,
        show_next_on_duplicate: preferences.show_next_on_duplicate,
    }
}

/// The switch the Engine is handed. Windows turns the punctuation switch on whenever punctuation is locked to Chinese, while the Engine drops Chinese punctuation whenever its switch is off, so a lock to Chinese has to carry the switch with it.
fn engine_chinese_punctuation(enabled: bool, lock: u8) -> bool {
    enabled || lock == 1
}

impl HostSession {
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
        self.ai_provider_config()
            .is_some_and(|config| query.ai_assistant.as_ref() == Some(config))
    }
    fn set_ai_provider_cache(&mut self, preferences: &Preferences) {
        let ai = &preferences.ai_assistant;
        self.ai_provider_cache = ai.enabled.then(|| AiAssistantProviderConfig {
            enabled: true,
            provider: ai.provider.clone(),
            model: ai.model.clone(),
            endpoint: ai.endpoint.clone(),
            candidate_limit: ai.candidate_limit,
            prompt_id: ai.prompt_id.clone(),
            prompt_custom_1: ai.prompt_custom_1.clone(),
            prompt_custom_2: ai.prompt_custom_2.clone(),
            prompt_custom_3: ai.prompt_custom_3.clone(),
        });
    }
    fn cloud_candidates_enabled(&self) -> bool {
        self.applied.cloud_candidates
            && self
                .requested
                .as_ref()
                .is_none_or(|snapshot| snapshot.preferences.cloud_candidates)
    }
    /// Count a committing selection in memory, and hand the batch to the store once it is `SELECTION_BATCH` long.
    fn count_selection(&mut self, position: usize) {
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

    /// Write the selections counted since the last flush, in the store the host already keeps.
    ///
    /// Best effort on purpose: statistics must never be the reason a keystroke fails, so a locked or unwritable store is dropped rather than surfaced, and the batch goes with it rather than being retried on every later key. The store honours the user's switch itself, so there is no second check here to fall out of step with it. Nothing pending means nothing touches the disk.
    fn flush_selections(&mut self) {
        let pending = std::mem::take(&mut self.pending_selections);
        let directory = std::path::Path::new(&self.options.user_data);
        if !directory.is_absolute() {
            return;
        }
        let mut batch = Vec::with_capacity(RANKS);
        batch.extend(
            pending
                .iter()
                .enumerate()
                .filter(|(_, count)| **count > 0)
                .map(|(slot, count)| (slot + 1, *count)),
        );
        let _ = TypingStatisticsStore::new(directory).record_selections(&batch);
    }

    /// Rebuild the Engine for the requested preferences once the composition is idle. The `Ok` value is why the preferred scheme was not the one applied, when it was not.
    fn apply_pending(&mut self) -> Result<Option<String>, String> {
        if self.runtime.is_idle() {
            if let Some(size) = self.page_size_override {
                self.runtime
                    .set_page_size(size)
                    .map_err(|e| e.to_string())?;
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
        let (scheme, fallback) = effective_scheme(
            &preferences,
            &offered_input_schemes(self.edition),
            &LanguageDictionaries::of_options(&self.options),
            self.edition.default_scheme,
        );
        options.scheme = scheme_code(scheme);
        options.vietnamese_input_method = vietnamese_input_method_code(preferences.vietnamese);
        options.vietnamese_tone_style = vietnamese_tone_style_code(preferences.vietnamese);
        options.shuangpin_profile = profile_code(preferences.shuangpin_profile);
        options.shuangpin_preedit_uses_raw = preferences.shuangpin_preedit_uses_raw;
        options.learning = preferences.learning;
        options.autocorrect_transposition = preferences.quanpin.autocorrect_transposition;
        options.autocorrect_neighbor = preferences.quanpin.autocorrect_neighbor;
        options.fuzzy_pinyin_rules = preferences.fuzzy_pinyin.active_rules();
        options.wubi_mixed_pinyin = preferences.wubi_mixed_pinyin;
        options.wubi_profile = wubi_profile_code(preferences.wubi_profile);
        options.frequency_mode = preferences.frequency.mode.as_str().into();
        options.frequency_trigger_count = preferences.frequency.trigger_count;
        options.frequency_linear_step = preferences.frequency.linear_step;
        options.mixed_english = preferences.mixed_input.english;
        options.english_minimum_prefix = preferences.mixed_input.minimum_prefix;
        options.mixed_emoji = preferences.mixed_input.emoji;
        options.mixed_kaomoji = preferences.mixed_input.kaomoji;
        options.local_unicode = preferences.local_modes.unicode;
        options.local_date_time = preferences.local_modes.date_time;
        options.local_quick_phrase = preferences.local_modes.quick_phrase;
        options.local_emoji = preferences.local_modes.emoji;
        options.local_kaomoji = preferences.local_modes.kaomoji;
        options.local_super_jianpin = preferences.local_modes.super_jianpin;
        options.local_temporary_english = preferences.local_modes.temporary_english;
        options.local_temporary_japanese = preferences.local_modes.temporary_japanese;
        options.local_expression = preferences.local_modes.expression;
        options.local_command = preferences.local_modes.command;
        options.local_mention = preferences.local_modes.mention;
        // 先定下辅助码设置：插件表的戳要看当前方案的辅助码是否打开、选了哪个辅助码表包。
        let helpcode = helpcode_for_scheme(&preferences, scheme);
        options.helpcode = helpcode.enabled;
        options.show_helpcode = helpcode.show_in_candidate_window;
        options.helpcode_schema = helpcode.schema.as_str().into();
        let plugin_root = self.plugin_roots.installed.as_deref();
        let plugin_tables =
            plugin_tables::PluginTables::stamp(plugin_root, &options, &preferences.plugins);
        plugin_tables.fill(&self.plugin_tables, plugin_root, &mut options);
        options.sentence_association =
            engine_sentence_association(&preferences.sentence_association);
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
        // Build and validate first; errors leave the original session usable.
        let mut engine = Session::new(&options).map_err(|e| e.to_string())?;
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
        let layout_changed =
            preferences.touch_keyboard_layout != self.applied.touch_keyboard_layout;
        let nine_key_scheme = SchemeType::from_u8(options.scheme).is_some_and(SchemeType::nine_key);
        let next_nine_key_override = if nine_key_scheme && !layout_changed {
            self.nine_key_override
        } else {
            None
        };
        let nine_key_mode = nine_key_scheme
            && next_nine_key_override.unwrap_or(matches!(
                preferences.touch_keyboard_layout,
                TouchKeyboardLayout::NineKey
            ));
        if nine_key_mode {
            engine
                .set_nine_key_enabled(true)
                .map_err(|e| e.to_string())?;
        }
        engine
            .set_dedicated_english(self.english_mode)
            .map_err(|e| e.to_string())?;
        // The places of `@` mode are not an engine option: every new engine starts with them off, so the switch is carried over on each rebuild.
        engine
            .set_mention_places(preferences.local_modes.mention_places)
            .map_err(|e| e.to_string())?;
        self.runtime
            .replace_engine_with_touch_layout(
                engine,
                self.page_size_override
                    .unwrap_or(preferences.candidate_page_size),
                preferences.touch_keyboard_layout,
            )
            .map_err(|e| e.to_string())?;
        self.runtime
            .set_settled_rerank_enabled(preferences.sentence_association.neural_desktop);
        self.options = options;
        self.plugin_tables = plugin_tables;
        self.applied = preferences;
        self.preferences_pending = false;
        self.resources_pending = false;
        self.nine_key_override = next_nine_key_override;
        Ok(fallback)
    }

    /// 输入框获得焦点时，让 `/` 指令表、K 模式短语表、辅助码表和 `@` 名单跟上插件目录：设置页可能刚导入了表或改了名单。没有文件变动时什么都不读。
    fn refresh_plugin_tables(&mut self) -> Result<(), String> {
        let root = self.plugin_roots.installed.as_deref();
        let tables = plugin_tables::PluginTables::stamp(root, &self.options, &self.applied.plugins);
        if tables.commands_differ(&self.plugin_tables) {
            let table = tables.command_table(root);
            self.runtime
                .set_command_table(&table)
                .map_err(|e| e.to_string())?;
            self.options.command_table = table;
        }
        if tables.mentions_differ(&self.plugin_tables) {
            let entries = tables.mention_entries(root, &self.options);
            self.runtime
                .set_mention_entries(&entries)
                .map_err(|e| e.to_string())?;
            self.options.mention_entries = entries;
        }
        if tables.phrases_differ(&self.plugin_tables) {
            let table = tables.quick_phrase_table(root);
            self.runtime
                .set_quick_phrase_table(&table)
                .map_err(|e| e.to_string())?;
            self.options.quick_phrase_table = table;
        }
        if tables.helpcode_differs(&self.plugin_tables) {
            let table = tables.helpcode_table(root, &self.options);
            // 辅助码表换不上不算聚焦失败：报错会让这次和之后每一次聚焦都失败（戳没更新，下次又会重试）。记下来，保留当前的表，照常更新戳。
            match self.runtime.set_helpcode_table(table.clone()) {
                Ok(()) => self.options.helpcode_table = table,
                Err(error) => eprintln!("msime: helpcode table not replaced: {error}"),
            }
        }
        self.plugin_tables = tables;
        Ok(())
    }

    /// 输入框获得焦点时，看设置应用是否在会话打开后装好（或移除）了日文、粤拼和注音的资源包：只做几次 stat。路径有变就记下，等输入空闲时由 `apply_pending` 重建 Engine，重建时日文临时模式的开关按新路径重新判断。
    ///
    /// 资源包目录只在校验完、整体原子发布之后才出现，所以这里看到的文件都是完整的。
    fn refresh_resource_packs(&mut self) {
        let state_root = self.state_root.as_deref();
        let dictionaries = LanguageDictionaries::resolve(
            state_root,
            self.recorded_language_dictionaries.as_deref(),
        );
        let cantonese = path_text(dictionaries.cantonese);
        let zhuyin = path_text(dictionaries.zhuyin);
        let japanese = path_text(japanese_dictionary(state_root));
        if cantonese == self.options.cantonese_dictionary
            && zhuyin == self.options.zhuyin_dictionary
            && japanese == self.options.japanese_dictionary
        {
            return;
        }
        self.options.cantonese_dictionary = cantonese;
        self.options.zhuyin_dictionary = zhuyin;
        self.options.japanese_dictionary = japanese;
        self.resources_pending = true;
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
        // A replacement changes the view generation, never the completed commit. A scheme that does not widen (Korean) writes half-width ASCII punctuation and digits whatever the width switch says; the dedicated English mode keeps its own rules in every scheme, so its commits are widened as they are under a Chinese scheme.
        let half_width_text = SchemeType::from_u8(result.view.scheme)
            .is_some_and(|scheme| !scheme.widens_full_width())
            && !result.view.dedicated_english
            && result.view.local_mode == "none";
        if result.view.character_width == CharacterWidth::Fullwidth && !half_width_text {
            if let Some(c) = result.commit.as_mut() {
                *c = c
                    .chars()
                    .map(|x| {
                        if x == ' ' {
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
        snapshot.preferences.validate().map_err(|e| e.to_string())?;
        if let Some(previous) = &self.requested {
            if snapshot.revision < previous.revision
                || (snapshot.revision == previous.revision && snapshot != *previous)
            {
                return Err("stale or conflicting preferences revision".into());
            }
        }
        self.preferences_pending = snapshot.preferences != self.applied;
        self.requested = Some(snapshot);
        let requested_preferences = self
            .requested
            .as_ref()
            .expect("requested snapshot exists")
            .preferences
            .clone();
        self.set_ai_provider_cache(&requested_preferences);
        self.sound.update(key_sound::SoundSettings::new(
            &requested_preferences.plugins,
            &self.plugin_roots,
        ));
        let fallback = self.apply_pending()?;
        let snapshot = self.requested.as_ref().expect("requested snapshot exists");
        let mut response = json!({ "revision": snapshot.revision, "deferred": snapshot.preferences != self.applied, "view": self.runtime.view(), "floating_toolbar": { "enabled": snapshot.preferences.floating_toolbar.enabled, "english_mode": snapshot.preferences.floating_toolbar.english_mode, "scale_percent": snapshot.preferences.floating_toolbar.scale_percent, "font_size": snapshot.preferences.floating_toolbar.font_size, "fullwidth": snapshot.preferences.floating_toolbar.fullwidth, "punctuation": snapshot.preferences.floating_toolbar.punctuation, "character_set": snapshot.preferences.floating_toolbar.character_set, "emoji": snapshot.preferences.floating_toolbar.emoji, "screen_keyboard": snapshot.preferences.floating_toolbar.screen_keyboard, "settings": snapshot.preferences.floating_toolbar.settings } });
        // Applied at once, so no transition will carry the reason the preferred scheme was replaced; a deferred change reports it from `complete_transition` instead.
        if let Some(fallback) = fallback {
            response["diagnostic"] = fallback.into();
        }
        Ok(response)
    }
}

fn profile_code(profile: ShuangpinProfile) -> u8 {
    match profile {
        ShuangpinProfile::Xiaohe => 0,
        ShuangpinProfile::Ziranma => 1,
        ShuangpinProfile::Shoudao => 2,
        ShuangpinProfile::Microsoft => 3,
    }
}

fn wubi_profile_code(profile: WubiProfile) -> u8 {
    match profile {
        WubiProfile::Wubi86 => 0,
        WubiProfile::Wubi98 => 1,
    }
}

/// 交给 Engine 的 `enabled_schemes`：本版本提供的方案，版本带临时日文时再加上它要切到的日文方案。full 提供全部八个方案，得到的就是 [`SchemeSet::ALL`]，Engine 照旧构造全部 provider。
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
    }
}

fn vietnamese_input_method_code(vietnamese: VietnamesePreferences) -> u8 {
    match vietnamese.input_method {
        VietnameseInputMethod::Telex => 0,
        VietnameseInputMethod::Vni => 1,
    }
}

fn vietnamese_tone_style_code(vietnamese: VietnamesePreferences) -> u8 {
    match vietnamese.tone_style {
        VietnameseToneStyle::Modern => 0,
        VietnameseToneStyle::Classic => 1,
    }
}

/// The Cantonese and Zhuyin dictionaries a host has installed. Each is its own artifact rather than part of the shared resource set, so either may be missing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LanguageDictionaries {
    pub(crate) cantonese: Option<std::path::PathBuf>,
    pub(crate) zhuyin: Option<std::path::PathBuf>,
}

impl LanguageDictionaries {
    /// 每个词库优先用 `state_root` 下已下载的资源包里的那份，没有时用 HostOptions 记录的 `recorded` 目录里的那份（随包内置或旧版本留下的），两处都没有就是缺席。
    pub(crate) fn resolve(state_root: Option<&Path>, recorded: Option<&Path>) -> Self {
        let find = |name: &str| {
            state_root
                .and_then(|root| {
                    resource_packs::installed_file(root, ResourcePack::LanguageDictionaries, name)
                })
                .or_else(|| {
                    recorded
                        .map(|directory| directory.join(name))
                        .filter(|path| path.is_file())
                })
        };
        LanguageDictionaries {
            cantonese: find("cantonese.db"),
            zhuyin: find("zhuyin.db"),
        }
    }

    /// `cantonese.db` and `zhuyin.db` in `directory`, each when it is a file.
    fn in_directory(directory: &std::path::Path) -> Self {
        let present = |name: &str| {
            let path = directory.join(name);
            path.is_file().then_some(path)
        };
        LanguageDictionaries {
            cantonese: present("cantonese.db"),
            zhuyin: present("zhuyin.db"),
        }
    }

    /// The dictionaries an Engine was configured with, where an empty path means none.
    fn of_options(options: &EngineOptions) -> Self {
        let named = |path: &str| (!path.is_empty()).then(|| std::path::PathBuf::from(path));
        LanguageDictionaries {
            cantonese: named(&options.cantonese_dictionary),
            zhuyin: named(&options.zhuyin_dictionary),
        }
    }

    fn is_empty(&self) -> bool {
        self.cantonese.is_none() && self.zhuyin.is_none()
    }

    /// Whether `scheme` can run with these dictionaries: Cantonese and Zhuyin need their own, every other scheme reads only the shared resources.
    fn serve(&self, scheme: InputScheme) -> bool {
        match scheme {
            InputScheme::Cantonese => self.cantonese.is_some(),
            InputScheme::Zhuyin => self.zhuyin.is_some(),
            _ => true,
        }
    }
}

/// 已下载的日文资源包里的 `dict_japanese.dat`。`None` 时 Engine 读资源目录里的那份（完整发布包或开发环境内置的）。
fn japanese_dictionary(state_root: Option<&Path>) -> Option<PathBuf> {
    resource_packs::installed_file(state_root?, ResourcePack::Japanese, "dict_japanese.dat")
}

/// EngineOptions 里的路径文本，没有路径（或路径不是 UTF-8）时为空。
fn path_text(path: Option<PathBuf>) -> String {
    path.and_then(|path| path.to_str().map(str::to_owned))
        .unwrap_or_default()
}

/// HostOptions 的 `preferences_directory`，只认绝对路径：按需下载的资源包装在它下面。
fn absolute_state_root(preferences_directory: Option<&str>) -> Option<PathBuf> {
    preferences_directory
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// 按 `preferences` 交给 Engine 的方案，以及没用偏好里那个方案时的原因。本构建或本版本不提供的方案、没装词库的粤拼和注音，都回退到上一次的中文方案（它能跑时），否则回退到 `default`，所以别的宿主写下的文档不会让这个宿主没有能用的方案。偏好本身不改：词库装好后，下一个会话就跑用户选的方案。
///
/// `supported` 是运行中版本提供的方案（`offered_input_schemes`），`default` 是该版本的默认方案；full 分别是全部八个方案和全拼。所以在五笔版里，即使同步下来的偏好写着全拼，Engine 跑的也是五笔。
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
    /// Absolute path to the directory holding `cantonese.db` and `zhuyin.db`, for a host that installs either. Absent, or a directory missing one of them, means that scheme falls back as `effective_scheme` describes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    language_dictionaries: Option<String>,
    /// 产品版本（`Edition::HOST_OPTIONS_KEY`）。只有不是 full 的版本才写：full 的文档因此与引入版本之前逐字节相同，旧版输入法（本结构拒绝未知键）照样能读。缺省就是 full；不是版本表里的 id 时整份文档被拒，而不是猜成 full。
    #[serde(default, skip_serializing_if = "Option::is_none", with = "edition_id")]
    edition: Option<&'static Edition>,
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

    /// 解析 HostOptions 文档。其中的 `preferences` 是准备运行时配置那一刻的副本，升级时被原样带下去（`refresh_options_file`），所以后续版本一旦退役某个偏好字段，这份文档就读不了，会话、快照和词库请求全部被拒：#2830 退役了 `autocorrect`，macOS 输入随之失效，每个按键都像在英文模式下一样直接交给应用。副本被拒时，改用 `preferences_directory` 下实时的 preferences.json——设置界面会保持它最新并负责修复，宿主在会话打开后本来也会应用它。副本能解析时照原样使用。
    pub(crate) fn from_document(mut document: Value) -> Option<Self> {
        if let Ok(options) = Self::deserialize(&document) {
            return Some(options);
        }
        let directory = std::path::PathBuf::from(document.get("preferences_directory")?.as_str()?);
        // load() 会创建传入的目录；从没在那里写过偏好的宿主没有可替代的内容。
        if !directory.is_absolute() || !directory.join("preferences.json").is_file() {
            return None;
        }
        let snapshot = PreferencesStore::new(&directory).load().ok()?;
        document["preferences"] = serde_json::to_value(snapshot.preferences).ok()?;
        Self::deserialize(&document).ok()
    }

    fn into_engine_options(self) -> EngineOptions {
        // 每个平台都这样查找；只有 macOS 会下载资源包，别处的状态目录里从来没有它们。
        let state_root = absolute_state_root(self.preferences_directory.as_deref());
        let dictionaries = LanguageDictionaries::resolve(
            state_root.as_deref(),
            self.language_dictionaries.as_deref().map(Path::new),
        );
        let japanese = japanese_dictionary(state_root.as_deref());
        // Session creation has no diagnostic to carry the reason; the fallback itself is what matters here.
        let edition = self.edition();
        let (scheme, _) = effective_scheme(
            &self.preferences,
            &offered_input_schemes(edition),
            &dictionaries,
            edition.default_scheme,
        );
        let helpcode = helpcode_for_scheme(&self.preferences, scheme);
        let mut options = EngineOptions {
            resources: self.resources,
            user_data: self.user_data,
            cache: self.cache,
            dictionaries: self.dictionaries,
            scheme: scheme_code(scheme),
            enabled_schemes: engine_schemes(edition),
            shuangpin_profile: profile_code(self.preferences.shuangpin_profile),
            shuangpin_preedit_uses_raw: self.preferences.shuangpin_preedit_uses_raw,
            learning: self.preferences.learning,
            autocorrect_transposition: self.preferences.quanpin.autocorrect_transposition,
            autocorrect_neighbor: self.preferences.quanpin.autocorrect_neighbor,
            fuzzy_pinyin_rules: self.preferences.fuzzy_pinyin.active_rules(),
            wubi_mixed_pinyin: self.preferences.wubi_mixed_pinyin,
            wubi_profile: wubi_profile_code(self.preferences.wubi_profile),
            frequency_mode: self.preferences.frequency.mode.as_str().into(),
            frequency_trigger_count: self.preferences.frequency.trigger_count,
            frequency_linear_step: self.preferences.frequency.linear_step,
            mixed_english: self.preferences.mixed_input.english,
            english_minimum_prefix: self.preferences.mixed_input.minimum_prefix,
            mixed_emoji: self.preferences.mixed_input.emoji,
            mixed_kaomoji: self.preferences.mixed_input.kaomoji,
            local_unicode: self.preferences.local_modes.unicode,
            local_date_time: self.preferences.local_modes.date_time,
            local_quick_phrase: self.preferences.local_modes.quick_phrase,
            local_emoji: self.preferences.local_modes.emoji,
            local_kaomoji: self.preferences.local_modes.kaomoji,
            local_super_jianpin: self.preferences.local_modes.super_jianpin,
            local_temporary_english: self.preferences.local_modes.temporary_english,
            local_temporary_japanese: self.preferences.local_modes.temporary_japanese,
            local_expression: self.preferences.local_modes.expression,
            local_command: self.preferences.local_modes.command,
            local_mention: self.preferences.local_modes.mention,
            command_table: Vec::new(),
            mention_entries: Vec::new(),
            quick_phrase_table: Vec::new(),
            helpcode_table: None,
            sentence_association: engine_sentence_association(
                &self.preferences.sentence_association,
            ),
            rescoring_context: String::new(),
            sentence_alternatives: true,
            vietnamese_input_method: vietnamese_input_method_code(self.preferences.vietnamese),
            vietnamese_tone_style: vietnamese_tone_style_code(self.preferences.vietnamese),
            cantonese_dictionary: path_text(dictionaries.cantonese),
            zhuyin_dictionary: path_text(dictionaries.zhuyin),
            japanese_dictionary: path_text(japanese),
            helpcode: helpcode.enabled,
            show_helpcode: helpcode.show_in_candidate_window,
            helpcode_schema: helpcode.schema.as_str().into(),
            chinese_punctuation: engine_chinese_punctuation(
                self.preferences.chinese_punctuation,
                punctuation_lock_code(self.preferences.punctuation_lock),
            ),
            paired_punctuation: self.preferences.paired_punctuation,
            punctuation_lock: punctuation_lock_code(self.preferences.punctuation_lock),
        };
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

/// The Cantonese and Zhuyin dictionaries installed beside a resource bundle: `language-dictionaries/cantonese.db` and `language-dictionaries/zhuyin.db`, built by `msime-dict-builder`. A sibling of `resources` for the same reason as `settled_model_beside`: the resource directory must match the shared dictionary lock exactly, and only the hosts that offer these schemes ship them. Absence is the normal case.
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

/// The target languages an offline gloss dictionary can exist for; English is glossed from the packaged english.db instead.
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

/// 当前平台发布包不内置、改为按需下载的资源文件：macOS 是日文词典与它的 Mozc 许可说明，其余平台照旧全部内置。
pub(crate) const ON_DEMAND_ARTIFACTS: &[&str] = if cfg!(target_os = "macos") {
    &msime_client_core::resources::MACOS_ON_DEMAND_ARTIFACTS
} else {
    &[]
};

/// `resources` 实际按哪一份清单发货：`on_demand` 里的文件全部缺席时去掉它们，否则是完整的锁文件。见 [`ResourceSet::as_shipped_in`]。
pub(crate) fn shipped_specification(
    specification: &ResourceSet,
    resources: &Path,
    on_demand: &[&str],
) -> ResourceSet {
    specification.as_shipped_in(resources, on_demand)
}

fn verify_resources_once(
    resources: &std::path::Path,
    specification: &ResourceSet,
    state_root: &std::path::Path,
    on_demand: &[&str],
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

pub fn prepare_host_configuration(
    resources: &std::path::Path,
    state_root: &std::path::Path,
) -> Result<String, Box<dyn std::error::Error>> {
    prepare_host_configuration_for_edition(resources, state_root, Edition::full())
}

/// 为 `edition` 准备宿主：[`prepare_host_configuration`] 就是 full 的这一个。
///
/// 不是 full 的版本在文档里记下 `edition`，之后的会话、[`refresh_host_options`] 和 `msime-mcp` 都从文档里读它；full 的文档不写这个键，与以前完全相同。状态目录里还没有偏好文件时，不是 full 的版本会把本版本的默认偏好写成第一份偏好文件：各平台宿主经 C 接口按目录读取偏好，并不知道版本，没有文件时读到的是 full 的默认值（例如五笔混拼是关的）；有了这份文件，任何读取方第一次读到的都是本版本的默认值。
pub fn prepare_host_configuration_for_edition(
    resources: &std::path::Path,
    state_root: &std::path::Path,
    edition: &'static Edition,
) -> Result<String, Box<dyn std::error::Error>> {
    // 资源目录按本版本的锁校验，用户词库代次也按它计算；full 的锁就是原来那份文件。
    let specification = edition.resource_set()?;
    prepare_shipped_host_configuration(
        resources,
        state_root,
        &specification,
        ON_DEMAND_ARTIFACTS,
        edition,
    )
}

/// 状态目录里 `edition` 的偏好。不是 full 的版本在还没有偏好文件时，先把本版本的默认偏好写成第一份文件，理由见 [`prepare_host_configuration_for_edition`]。
fn edition_preferences(
    state_root: &Path,
    edition: &'static Edition,
) -> Result<Preferences, Box<dyn std::error::Error>> {
    let store = PreferencesStore::for_edition(state_root, edition);
    let snapshot = store.load()?;
    if edition.is_full() || snapshot.revision > 0 {
        return Ok(snapshot.preferences);
    }
    match store.save(0, snapshot.preferences) {
        Ok(saved) => Ok(saved.preferences),
        // 设置应用恰好在这期间写下了第一份偏好，以它为准。
        Err(msime_client_core::preferences::PreferencesError::Conflict) => {
            Ok(store.load()?.preferences)
        }
        Err(error) => Err(error.into()),
    }
}

/// [`prepare_host_configuration_for_edition`] 按给定的锁文件和按需下载清单准备；测试借它在各平台上检查 macOS 的发货规则。
fn prepare_shipped_host_configuration(
    resources: &std::path::Path,
    state_root: &std::path::Path,
    specification: &ResourceSet,
    on_demand: &[&str],
    edition: &'static Edition,
) -> Result<String, Box<dyn std::error::Error>> {
    let resources = without_verbatim_prefix(std::fs::canonicalize(resources)?);
    let state_root = std::path::absolute(state_root)?;
    verify_resources_once(&resources, specification, &state_root, on_demand)?;
    let prepared = msime_engine::host::prepare_options(
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
    )?;
    let preferences = edition_preferences(&state_root, edition)?;
    Ok(serde_json::to_string_pretty(&HostOptions {
        api_version: 1,
        resources: prepared.resources,
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
/// 记录的资源目录不一定是安装包会替换的那一个。手工暂存到 Application Support 的目录、在输入法「准备词库」里选的目录，都停在暂存时的代次上，之后每次升级都以 [`DictionaryOutdated`] 失败，用户既拿不到新词库，也拿不到安装包放在自带资源旁的粤语与注音词库。记录的目录仍是当前代次，或者失败是别的原因时，处理与 [`refresh_host_options`] 完全相同；这里同样不碰 `language_dictionaries`。
pub fn refresh_host_options_from(
    path: &std::path::Path,
    bundled: &std::path::Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    refresh_options_file(path, false, Some(bundled))
}

/// 在 [`refresh_host_options`] 之外，不论代次是否变化，都让 `language_dictionaries` 跟上资源目录旁实际安装的粤语与注音词库；两项都不需要改时只读一次文件。代次准备失败也不会挡住这一项：仍按记录的资源目录更新这个键，然后再返回准备失败的错误，`resources` 与 `dictionaries` 保持原样。
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
    reject_symlinked_options_parent(path)?;
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() {
        return Ok(false);
    }
    // The file can be replaced or grow after symlink_metadata returns. Read through a
    // limit-aware handle so that the size check remains effective across that race.
    let bytes = crate::bounded_file::read(
        std::fs::File::open(path)?,
        HOST_OPTIONS_DOCUMENT_LIMIT as u64,
    )
    .map_err(|error| {
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
    let specification = edition.resource_set()?;
    let prepared = refreshed_host_options(
        &document,
        &specification.generation()?,
        bundled,
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
                replace_options_file(path, &metadata, &languages)?;
            }
            return Err(error);
        }
    };
    let Some(refreshed) = languages.or(prepared) else {
        return Ok(false);
    };
    replace_options_file(path, &metadata, &refreshed)?;
    Ok(true)
}

/// 用 `document` 原子替换 `path` 处的配置文件，保留原有权限。
fn replace_options_file(
    path: &std::path::Path,
    metadata: &std::fs::Metadata,
    document: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write as _;
    let parent = path.parent().ok_or("runtime options have no directory")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary
        .as_file()
        .set_permissions(metadata.permissions())?;
    let mut serialized = serde_json::to_vec_pretty(document)?;
    serialized.push(b'\n');
    temporary.write_all(&serialized)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    Ok(())
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
    Ok(refreshed)
}

/// `document` with `language_dictionaries` naming what is installed beside its resources, or `None` when it already does or is not in the prepared layout.
///
/// The Cantonese and Zhuyin dictionaries arrive with a package, not with a dictionary generation, so options published by an older package are brought up to what is installed even when the generation is current, and lose the key once the dictionaries are gone. Only the directory `prepare_host_configuration` records is kept in step; a document naming another directory that still exists keeps it.
fn with_installed_language_dictionaries(
    document: &Value,
) -> Result<Option<Value>, Box<dyn std::error::Error>> {
    let Some((resources, _, _)) = prepared_layout(document) else {
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
    let installed = installed_language_dictionaries(resources);
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
    // The previous row is one the list returned, whose weight learning may have lifted past the ceiling a new entry is held to.
    for entry in previous.iter() {
        msime_engine::host::dictionary_validate_previous(entry)
            .map_err(|error| invalid_dictionary_entry(&error.to_string()))?;
    }
    for entry in replacement.iter() {
        msime_engine::host::dictionary_validate(entry)
            .map_err(|error| invalid_dictionary_entry(&error.to_string()))?;
    }
    let _access = DictionaryAccess::try_maintenance(
        std::path::Path::new(&options.user_data),
        std::path::Path::new(&options.dictionaries),
    )
    .map_err(|_| "dictionary access unavailable")?
    .ok_or("dictionary maintenance busy")?;
    if request_id.is_empty() {
        return Err("dictionary request id required".into());
    }
    msime_engine::host::dictionary_edit(options, previous, replacement, request_id)
        .map_err(|_| "dictionary edit rejected".into())
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
// Not unix-gated: the bodies only call the engine, which builds on Windows too. The gate was a porting gap, and it left the Windows desktop falling back to the compact built-in catalog - 97 emoji against the several thousand rows in others.db - behind a permanent "catalog failed to load" banner.
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

pub use msime_client_core::plugins::symbol_set::PluginSymbolGroup;

/// 插件目录 `root` 下已安装的符号集的全部组，供宿主追加到内置符号目录之后：`symbols` 组放在以 `pack_name` 为上级分类的分组下，`kaomoji` 组放在颜文字的 All 之后。读几个小清单：不要在按键路径上调用。
pub fn plugin_symbol_groups(root: &std::path::Path) -> Vec<PluginSymbolGroup> {
    msime_client_core::plugins::symbol_set::plugin_symbol_groups(root)
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

fn with_session(
    handle: u64,
    action: impl FnOnce(&mut HostSession) -> Result<Value, String>,
) -> Result<Value, String> {
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
    response(|| {
        with_session(handle, |session| {
            // Which row the user reached for, read before dispatching because the view it is
            // relative to is gone afterwards. Every platform host routes candidate selection
            // through here, so counting it here covers all of them without a line of platform
            // code; doing it per host would have meant six chances to forget.
            let position = selected_position(&action);
            let result = session
                .runtime
                .dispatch(action)
                .map_err(|e| e.to_string())?;
            // Text the Engine generated (a calculator result, a command, a mention) was picked, not typed, and stays out of the statistics the way it stays out of learning.
            let counts_as_typing = result
                .commit_context
                .as_ref()
                .is_none_or(|context| context.typing_statistics);
            if result.commit.is_some() && counts_as_typing {
                if let Some(position) = position {
                    session.count_selection(position);
                }
            }
            let result = session.complete_transition(result);
            serde_json::to_value(result).map_err(|e| e.to_string())
        })
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

/// How many committing selections a session holds in memory before writing them to typing statistics.
///
/// Writing one means locking, reading and parsing the whole document, then an fsync (F_FULLFSYNC on Apple platforms) and a rename, all on the host's input thread, which cost a few milliseconds per tapped or clicked candidate. A session also writes what it holds on focus-out and on destroy, so this only bounds what a process killed mid-field loses - the iOS keyboard extension and the Android IME process can be killed without either. 32 selections is some tens of seconds of typing, a small loss for a statistic that only ever reports a rate, and it turns 32 writes into one.
const SELECTION_BATCH: u64 = 32;

#[cfg(test)]
mod tests;
