//! Owning CXX bridge to the pinned C++ Session. No Tauri or native UI dependency.
//! Sessions remain thread-confined; no unsafe Send/Sync implementation is provided.

// The workspace denies unsafe code; the cxx bridge declares the pinned C++ Session's methods in an
// `unsafe extern "C++"` block; there is no safe spelling for a foreign vtable.
// The exemption is stated here rather than left implicit by opting out of
// the workspace lint table, which would also silently drop every other lint
// the workspace adds later.
#![allow(unsafe_code)]

mod dictionary_revision;
pub use dictionary_revision::dictionary_state_revision;
use dictionary_revision::DictionaryRevision;
mod dictionary_stage;
use dictionary_stage::DictionaryRecordStream;
pub use dictionary_stage::{stage_dictionary_state, DictionaryStateRecord, SnapshotReadError};

#[cxx::bridge(namespace = "msime")]
mod ffi {
    struct CaptureDevice {
        id: String,
        label: String,
    }
    extern "Rust" {
        type DictionaryRevision;
        fn text(self: &mut DictionaryRevision, value: &str);
        fn integer(self: &mut DictionaryRevision, value: u64);
        type DictionaryRecordStream;
        fn next(self: &mut DictionaryRecordStream) -> Result<DictionaryStateWire>;
    }
    struct DictionaryStateWire {
        record_type: u8,
        kind: DictionaryKind,
        context: String,
        key: String,
        value: String,
        number: i64,
        display: String,
        deleted: bool,
        user_inserted: bool,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DictionaryKind {
        Pinyin,
        Wubi,
        QuickPhrase,
        English,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct DictionaryEntry {
        kind: DictionaryKind,
        key: String,
        value: String,
        weight: i64,
    }
    #[derive(Debug)]
    struct DictionaryPage {
        entries: Vec<DictionaryEntry>,
        has_more: bool,
    }
    /// A row of the dictionary tables themselves. `user_inserted` rows are the user's own words; every other row shipped with the dictionary (or was learned) and only its weight can change.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct DictionaryTableEntry {
        entry: DictionaryEntry,
        user_inserted: bool,
    }
    #[derive(Debug)]
    struct DictionaryTablePage {
        entries: Vec<DictionaryTableEntry>,
        has_more: bool,
    }
    #[derive(Clone, Copy)]
    pub struct SentenceAssociationOptions {
        pub word_lattice: bool,
        pub google: bool,
        pub neural_desktop: bool,
        pub neural_keyboard: bool,
        pub show_next_on_duplicate: bool,
    }
    #[derive(Clone)]
    pub struct EngineOptions {
        pub resources: String,
        pub user_data: String,
        pub cache: String,
        pub dictionaries: String,
        pub scheme: u8,
        pub shuangpin_profile: u8,
        pub shuangpin_preedit_uses_raw: bool,
        pub learning: bool,
        pub autocorrect_transposition: bool,
        pub autocorrect_neighbor: bool,
        pub fuzzy_pinyin_rules: u32,
        pub wubi_mixed_pinyin: bool,
        pub helpcode: bool,
        pub show_helpcode: bool,
        pub helpcode_schema: String,
        pub chinese_punctuation: bool,
        pub paired_punctuation: bool,
        pub punctuation_lock: u8,
        pub frequency_mode: String,
        pub frequency_trigger_count: u8,
        pub frequency_linear_step: u8,
        pub mixed_english: bool,
        pub english_minimum_prefix: u8,
        pub mixed_emoji: bool,
        pub mixed_kaomoji: bool,
        pub local_unicode: bool,
        pub local_date_time: bool,
        pub local_quick_phrase: bool,
        pub local_emoji: bool,
        pub local_kaomoji: bool,
        pub local_super_jianpin: bool,
        pub local_temporary_english: bool,
        pub local_temporary_japanese: bool,
        pub sentence_association: SentenceAssociationOptions,
        pub rescoring_context: String,
        /// Ask the decoder for every whole-sentence reading it found rather than only its best.
        /// The runtime reorders them and crops the list, so a host that sets this must also be the
        /// one deciding what reaches the candidate page.
        pub sentence_alternatives: bool,
    }
    #[derive(Debug)]
    pub struct EngineSnapshot {
        pub local_mode: String,
        pub dedicated_english: bool,
        pub nine_key: bool,
        pub nine_key_spellings: Vec<String>,
        pub microsoft_shuangpin: bool,
        pub shuangpin_profile: String,
        pub preedit: String,
        /// Japanese kana reading shown to the user instead of the romaji editing text.
        pub reading: String,
        pub editing_text: String,
        pub caret_position: usize,
        pub segment_raw_boundaries: Vec<u64>,
        pub candidates: Vec<String>,
        pub candidate_codes: Vec<String>,
        pub scheme: u8,
        pub answered_by_pinyin_fallback: bool,
        pub wubi_unique_four_code: bool,
        pub candidate_annotations: Vec<String>,
        pub candidate_sources: Vec<u8>,
        pub candidate_positions: Vec<u8>,
        pub candidate_corrected: Vec<bool>,
        /// Whether each candidate answers the whole key, rather than a prefix of it or a completion
        /// running past it. The engine decides this to advance the composition, so it is reported
        /// rather than inferred: character count agrees only while a key has one segmentation, and
        /// `xian` reads as both 现 and 西安.
        pub candidate_answers_key: Vec<bool>,
    }
    #[derive(Debug)]
    pub struct EngineResult {
        pub handled: bool,
        pub has_commit: bool,
        pub commit: String,
        pub diagnostic: String,
    }
    pub struct DictionaryReplaySummary {
        pub applied: i32,
        pub skipped: i32,
        pub failed: i32,
        pub error: String,
    }
    #[derive(Debug)]
    pub struct OnlineQuerySnapshot {
        pub available: bool,
        pub scheme: u8,
        pub generation: u64,
        pub identity: String,
        pub query_text: String,
        pub cache_key: String,
        pub pinyin_segments: Vec<String>,
        pub cloud_eligible: bool,
        pub ai_eligible: bool,
        pub session_id: u64,
    }
    #[derive(Debug)]
    pub struct EmojiCatalogItem {
        pub text: String,
        pub annotation: String,
        pub group: String,
    }
    #[derive(Debug)]
    pub struct EmojiCatalogSlice {
        pub items: Vec<EmojiCatalogItem>,
        pub next_offset: usize,
        pub complete: bool,
    }
    #[derive(Debug)]
    pub struct EmojiSymbolGroup {
        pub parent: String,
        pub title: String,
    }
    /// One letter key of a double-pinyin face and the units it carries, already
    /// formatted for display as `initials / finals`.
    #[derive(Clone, Debug)]
    pub struct ShuangpinKeyHint {
        pub key: String,
        pub hint: String,
    }
    #[derive(Clone, Debug)]
    pub struct CandidateGlossInput {
        pub text: String,
        pub source: u8,
    }
    #[derive(Clone, Debug)]
    pub struct HandwritingPoint {
        pub stroke: u32,
        pub x: f32,
        pub y: f32,
    }
    unsafe extern "C++" {
        include!("bridge.h");
        type EngineSession;
        fn stage_dictionary_state(
            options: &EngineOptions,
            generation: &str,
            content_id: &str,
            maximum_records: usize,
            stream: &mut DictionaryRecordStream,
        ) -> Result<EngineOptions>;
        fn reset_learned_data(options: &EngineOptions) -> Result<()>;
        fn hash_dictionary_state(
            options: &EngineOptions,
            sink: &mut DictionaryRevision,
        ) -> Result<()>;
        fn create_session(options: &EngineOptions) -> Result<UniquePtr<EngineSession>>;
        /// Capture bounded mono 16 kHz PCM samples through the Engine's
        /// platform-neutral AudioCapture implementation. An empty result
        /// means the host could not open a capture device.
        fn capture_audio(milliseconds: u32) -> Vec<f32>;
        fn capture_devices() -> Vec<CaptureDevice>;
        fn dictionary_entries(
            options: &EngineOptions,
            offset: usize,
            limit: usize,
        ) -> Result<DictionaryPage>;
        fn dictionary_export_entries(
            options: &EngineOptions,
            offset: usize,
            limit: usize,
            include_learned_pinyin: bool,
        ) -> Result<DictionaryPage>;
        fn dictionary_table_entries(
            options: &EngineOptions,
            kind: u8,
            query: &str,
            offset: usize,
            limit: usize,
        ) -> Result<DictionaryTablePage>;
        fn dictionary_edit_bundled(
            options: &EngineOptions,
            previous: &DictionaryEntry,
            weight: &[i64],
            request_id: &str,
        ) -> Result<()>;
        fn english_completions(resources: &str, prefix: &str, limit: usize) -> Result<Vec<String>>;
        fn dictionary_validate(entry: &DictionaryEntry) -> Result<DictionaryEntry>;
        fn dictionary_edit(
            options: &EngineOptions,
            previous: &[DictionaryEntry],
            replacement: &[DictionaryEntry],
            request_id: &str,
        ) -> Result<()>;
        fn replay_user_dictionary(
            user_db_path: &str,
            main_db_path: &str,
            english_db_path: &str,
        ) -> DictionaryReplaySummary;
        fn prepare_options(
            resources: &str,
            user_data: &str,
            cache: &str,
            content_id: &str,
        ) -> Result<EngineOptions>;
        fn hanzi_to_pinyin(options: &EngineOptions, text: &str) -> String;
        fn normalize_full_pinyin(input: &str, expected_syllables: usize) -> String;
        fn shuangpin_key_hints(profile: &str) -> Vec<ShuangpinKeyHint>;
        fn snapshot(self: &EngineSession) -> Result<EngineSnapshot>;
        fn online_query(self: &EngineSession) -> Result<OnlineQuerySnapshot>;
        fn reset_cache(self: Pin<&mut EngineSession>);
        fn set_caret(self: Pin<&mut EngineSession>, caret: u64);
        fn prefix_end(self: &EngineSession) -> usize;
        fn pending_suffix(self: &EngineSession) -> String;
        fn reset_context(self: Pin<&mut EngineSession>);
        fn apply_online_candidate(
            self: Pin<&mut EngineSession>,
            query: &OnlineQuerySnapshot,
            candidate: &str,
            source: u8,
        ) -> Result<bool>;
        fn apply_online_candidates(
            self: Pin<&mut EngineSession>,
            query: &OnlineQuerySnapshot,
            candidates: &[String],
            source: u8,
        ) -> Result<bool>;
        fn emoji_catalog_filtered_page(
            resources: &str,
            search: &str,
            category: &str,
            group: &str,
            offset: usize,
            limit: u16,
            parent: &str,
        ) -> Result<Vec<EmojiCatalogItem>>;
        fn emoji_catalog_slice(
            resources: &str,
            search: &str,
            category: &str,
            group: &str,
            offset: usize,
            limit: u16,
            parent: &str,
        ) -> Result<EmojiCatalogSlice>;
        fn emoji_symbol_groups(resources: &str) -> Result<Vec<EmojiSymbolGroup>>;
        fn handwriting_order_candidates(candidates: &[String]) -> Result<Vec<String>>;
        fn emoji_catalog_groups(resources: &str, category: &str) -> Result<Vec<String>>;
        fn candidate_glosses(
            resources: &str,
            candidates: &[CandidateGlossInput],
        ) -> Result<Vec<String>>;
        fn candidate_glosses_with_user(
            resources: &str,
            user_data: &str,
            candidates: &[CandidateGlossInput],
        ) -> Result<Vec<String>>;
        fn candidate_target_glosses(
            database_path: &str,
            target_language: &str,
            candidates: &[CandidateGlossInput],
        ) -> Result<Vec<String>>;
        fn save_candidate_gloss(
            user_data: &str,
            chinese_to_english: bool,
            key: &str,
            gloss: &str,
        ) -> bool;
        #[cfg(not(any(target_os = "android", target_env = "ohos")))]
        fn handwriting_recognize(
            model_path: &str,
            points: &[HandwritingPoint],
            width: f32,
            height: f32,
        ) -> Result<Vec<String>>;
        fn character(self: Pin<&mut EngineSession>, value: u8, shift: bool)
            -> Result<EngineResult>;
        fn expand_initial_candidates(self: Pin<&mut EngineSession>) -> Result<bool>;
        fn set_nine_key_enabled(self: Pin<&mut EngineSession>, enabled: bool) -> Result<()>;
        fn choose_nine_key_spelling(
            self: Pin<&mut EngineSession>,
            index: usize,
        ) -> Result<EngineResult>;
        fn command(self: Pin<&mut EngineSession>, value: u8) -> Result<EngineResult>;
        fn commit_raw_with_policy(self: Pin<&mut EngineSession>) -> Result<EngineResult>;
        fn select(self: Pin<&mut EngineSession>, index: usize) -> Result<EngineResult>;
        fn pin_candidate(self: Pin<&mut EngineSession>, index: usize) -> Result<EngineResult>;
        fn remove_candidate(self: Pin<&mut EngineSession>, index: usize) -> Result<EngineResult>;
        fn fix_candidate_position(
            self: Pin<&mut EngineSession>,
            index: usize,
            position: u8,
        ) -> Result<EngineResult>;
        fn clear_candidate_position(
            self: Pin<&mut EngineSession>,
            index: usize,
        ) -> Result<EngineResult>;
        fn select_edge(
            self: Pin<&mut EngineSession>,
            index: usize,
            edge: u8,
        ) -> Result<EngineResult>;
        fn finish(self: Pin<&mut EngineSession>, index: usize) -> Result<EngineResult>;
        fn punctuation(self: Pin<&mut EngineSession>, value: u8) -> Result<EngineResult>;
        fn balance_paired_punctuation_after_auto_close(
            self: Pin<&mut EngineSession>,
            opening: u8,
        ) -> Result<()>;
        fn set_chinese_punctuation_enabled(
            self: Pin<&mut EngineSession>,
            enabled: bool,
        ) -> Result<()>;
        fn set_punctuation_lock(self: Pin<&mut EngineSession>, lock: u8) -> Result<()>;
        fn set_paired_punctuation_enabled(
            self: Pin<&mut EngineSession>,
            enabled: bool,
        ) -> Result<()>;
        fn set_dedicated_english(self: Pin<&mut EngineSession>, enabled: bool) -> Result<()>;
    }
}

pub use ffi::{
    CaptureDevice, DictionaryEntry, DictionaryKind, DictionaryPage, DictionaryTableEntry,
    DictionaryTablePage, EmojiCatalogItem, EngineOptions, EngineResult, EngineSnapshot,
    HandwritingPoint, OnlineQuerySnapshot, SentenceAssociationOptions,
};

/// Read a bounded page of user-inserted entries, excluding the bundled dictionary.
pub fn dictionary_entries(
    options: &EngineOptions,
    offset: usize,
    limit: usize,
) -> Result<DictionaryPage, cxx::Exception> {
    ffi::dictionary_entries(options, offset, limit)
}

/// Read a bounded page of the entries a dictionary export writes: the user-inserted entries, plus, with `include_learned_pinyin`, the pinyin entries whose weight was learned or edited. Single-character pinyin entries are left out in that mode, as the reference's pinyin export does.
pub fn dictionary_export_entries(
    options: &EngineOptions,
    offset: usize,
    limit: usize,
    include_learned_pinyin: bool,
) -> Result<DictionaryPage, cxx::Exception> {
    ffi::dictionary_export_entries(options, offset, limit, include_learned_pinyin)
}

/// Look up the working dictionary tables of one kind by code prefix, read-only, bundled rows included. Pinyin ignores separators and case; an empty query lists every quick phrase and nothing for the other kinds. User-inserted rows come first, then exact matches, then by weight.
pub fn dictionary_table_entries(
    options: &EngineOptions,
    kind: DictionaryKind,
    query: &str,
    offset: usize,
    limit: usize,
) -> Result<DictionaryTablePage, cxx::Exception> {
    let kind = match kind {
        DictionaryKind::Pinyin => 0,
        DictionaryKind::Wubi => 1,
        DictionaryKind::QuickPhrase => 2,
        DictionaryKind::English => 3,
        _ => u8::MAX,
    };
    ffi::dictionary_table_entries(options, kind, query, offset, limit)
}

/// Set the weight of (`Some`) or delete (`None`) a row that is not user-inserted, journaled so it survives replay onto a fresh dictionary. `previous` must carry the row's current weight. The caller must quiesce sessions sharing these paths before editing.
pub fn dictionary_edit_bundled(
    options: &EngineOptions,
    previous: &DictionaryEntry,
    weight: Option<i64>,
    request_id: &str,
) -> Result<(), cxx::Exception> {
    ffi::dictionary_edit_bundled(options, previous, weight.as_slice(), request_id)
}

/// Query the packaged English dictionary without creating or mutating an input session.
pub fn english_completions(
    resources: &str,
    prefix: &str,
    limit: usize,
) -> Result<Vec<String>, cxx::Exception> {
    ffi::english_completions(resources, prefix, limit)
}

/// Validate and normalize one personal-dictionary entry through the Engine.
pub fn dictionary_validate(entry: &DictionaryEntry) -> Result<DictionaryEntry, cxx::Exception> {
    ffi::dictionary_validate(entry)
}

/// Capture bounded mono 16 kHz samples through the pinned Engine audio layer.
/// An empty vector indicates that capture could not be started or produced no
/// samples; the caller owns session cancellation and provider transport.
pub fn capture_audio(milliseconds: u32) -> Vec<f32> {
    ffi::capture_audio(milliseconds)
}

/// Backend-qualified endpoint identities and display labels; never log them.
pub fn capture_devices() -> Vec<ffi::CaptureDevice> {
    ffi::capture_devices()
}

/// Resolve a pure Han phrase to the highest-ranked canonical pinyin in the
/// verified Engine dictionary for native dictionary import tooling.
pub fn hanzi_to_pinyin(options: &EngineOptions, text: &str) -> String {
    ffi::hanzi_to_pinyin(options, text)
}

/// Normalize an unsegmented full-pinyin code using the Engine's canonical
/// syllable table. An expected Han-character count resolves ambiguous cuts.
pub fn normalize_full_pinyin(input: &str, expected_syllables: usize) -> String {
    ffi::normalize_full_pinyin(input, expected_syllables)
}

/// Per-key double-pinyin hint text for one profile, read out of the Engine's own
/// profile tables. A keyboard face that keeps its own copy of the keymap drifts
/// from the scheme the session runs, so hosts ask for this instead. An unknown
/// profile name yields no hints rather than the default profile's.
pub fn shuangpin_key_hints(profile: &str) -> Vec<ffi::ShuangpinKeyHint> {
    ffi::shuangpin_key_hints(profile)
}

/// Atomically add, replace, or remove one personal-dictionary entry.
/// The caller must quiesce sessions sharing these paths before editing.
pub fn dictionary_edit(
    options: &EngineOptions,
    previous: Option<&DictionaryEntry>,
    replacement: Option<&DictionaryEntry>,
    request_id: &str,
) -> Result<(), cxx::Exception> {
    ffi::dictionary_edit(
        options,
        previous.map_or(&[], std::slice::from_ref),
        replacement.map_or(&[], std::slice::from_ref),
        request_id,
    )
}

/// Replace mutable dictionaries and the Engine journal with fresh copies of
/// the packaged state. The caller must quiesce every session using these paths.
pub fn reset_learned_data(options: &EngineOptions) -> Result<(), cxx::Exception> {
    ffi::reset_learned_data(options)
}

/// Delegate working-dictionary preparation and learning replay to the Engine.
/// Caller verifies resources first and quiesces all users of these data paths.
pub fn prepare_options(
    resources: &str,
    user_data: &str,
    cache: &str,
    content_id: &str,
) -> Result<EngineOptions, cxx::Exception> {
    ffi::prepare_options(resources, user_data, cache, content_id)
}

/// Replay the Engine-owned user dictionary journal into the freshly installed
/// dictionaries. The installer calls this only after quiescing the previous
/// Server, so the operation cannot race a live session.
pub fn replay_user_dictionary(
    user_db_path: &str,
    main_db_path: &str,
    english_db_path: &str,
) -> (i32, i32, i32, String) {
    let result = ffi::replay_user_dictionary(user_db_path, main_db_path, english_db_path);
    (result.applied, result.skipped, result.failed, result.error)
}

pub fn emoji_catalog_filtered_page(
    resources: &str,
    search: &str,
    category: &str,
    group: &str,
    offset: usize,
    limit: u16,
    parent: &str,
) -> Result<Vec<EmojiCatalogItem>, cxx::Exception> {
    ffi::emoji_catalog_filtered_page(resources, search, category, group, offset, limit, parent)
}

pub fn emoji_catalog_slice(
    resources: &str,
    search: &str,
    category: &str,
    group: &str,
    offset: usize,
    limit: u16,
    parent: &str,
) -> Result<ffi::EmojiCatalogSlice, cxx::Exception> {
    ffi::emoji_catalog_slice(resources, search, category, group, offset, limit, parent)
}

pub fn emoji_symbol_groups(resources: &str) -> Result<Vec<ffi::EmojiSymbolGroup>, cxx::Exception> {
    ffi::emoji_symbol_groups(resources)
}

pub fn emoji_catalog_groups(
    resources: &str,
    category: &str,
) -> Result<Vec<String>, cxx::Exception> {
    ffi::emoji_catalog_groups(resources, category)
}

/// Query the packaged English dictionary without creating or mutating an input session.
/// Look up display-only candidate glosses in the packaged Engine dictionary.
/// The result is parallel to `candidates`; an ineligible candidate has an empty gloss.
pub fn candidate_glosses(
    resources: &str,
    candidates: &[(String, u8)],
) -> Result<Vec<String>, cxx::Exception> {
    candidate_glosses_with_user(resources, "", candidates)
}

pub fn save_candidate_gloss(
    user_data: &str,
    chinese_to_english: bool,
    key: &str,
    gloss: &str,
) -> bool {
    ffi::save_candidate_gloss(user_data, chinese_to_english, key, gloss)
}

pub fn candidate_glosses_with_user(
    resources: &str,
    user_data: &str,
    candidates: &[(String, u8)],
) -> Result<Vec<String>, cxx::Exception> {
    let mut candidate_inputs = Vec::with_capacity(candidates.len());
    candidate_inputs.extend(
        candidates
            .iter()
            .map(|(text, source)| ffi::CandidateGlossInput {
                text: text.clone(),
                source: *source,
            }),
    );
    if user_data.is_empty() {
        ffi::candidate_glosses(resources, &candidate_inputs)
    } else {
        ffi::candidate_glosses_with_user(resources, user_data, &candidate_inputs)
    }
}

/// Look up display-only glosses of Chinese candidates in one offline `zh-<lang>.db`. The result is parallel to `candidates`; a Latin, symbol or unknown candidate has an empty gloss. Fails when the file is missing, unreadable, or not the version and language asked for.
pub fn candidate_target_glosses(
    database_path: &str,
    target_language: &str,
    candidates: &[(String, u8)],
) -> Result<Vec<String>, cxx::Exception> {
    let mut candidate_inputs = Vec::with_capacity(candidates.len());
    candidate_inputs.extend(
        candidates
            .iter()
            .map(|(text, source)| ffi::CandidateGlossInput {
                text: text.clone(),
                source: *source,
            }),
    );
    ffi::candidate_target_glosses(database_path, target_language, &candidate_inputs)
}

/// Apply Engine's shared handwriting candidate policy to provider results.
pub fn handwriting_order_candidates(candidates: &[String]) -> Result<Vec<String>, cxx::Exception> {
    ffi::handwriting_order_candidates(candidates)
}

/// Run the Engine's optional offline handwriting recognizer on copied strokes.
/// Points are flattened with their zero-based stroke index for the CXX ABI.
/// Absent on the mobile hosts, which inject their own recognizer: the build turns
/// MSIME_ENGINE_BRIDGE_HANDWRITING off there, so the symbol does not exist to link.
#[cfg(not(any(target_os = "android", target_env = "ohos")))]
pub fn handwriting_recognize(
    model_path: &str,
    strokes: &[Vec<(f32, f32)>],
    width: f32,
    height: f32,
) -> Result<Vec<String>, cxx::Exception> {
    let points: Vec<HandwritingPoint> = strokes
        .iter()
        .enumerate()
        .flat_map(|(stroke, points)| {
            points.iter().map(move |&(x, y)| HandwritingPoint {
                stroke: stroke as u32,
                x,
                y,
            })
        })
        .collect();
    ffi::handwriting_recognize(model_path, &points, width, height)
}

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum Command {
    Backspace,
    CommitCandidate,
    CommitRaw,
    Cancel,
    MoveLeft,
    MoveRight,
    MoveHome,
    MoveEnd,
    DeleteForward,
    CycleKanaVariant,
    CommitReading,
    /// Commit the letters as typed without learning them as an English word. Windows learns an entered word only on Enter (`event_listener.cpp`, `ShouldLearnEnteredEnglishWord`); a mode switch commits the keystroke buffer and learns nothing (`KeyHandler.cpp`, `_HandleToogleIMEMode`).
    CommitRawWithoutLearning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CandidateEdge {
    FirstHan = 0,
    LastHan = 1,
}

pub struct Session {
    inner: cxx::UniquePtr<ffi::EngineSession>,
    _thread_confined: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Session {
    pub fn new(options: &EngineOptions) -> Result<Self, cxx::Exception> {
        Ok(Self {
            inner: ffi::create_session(options)?,
            _thread_confined: std::marker::PhantomData,
        })
    }
    pub fn snapshot(&self) -> Result<EngineSnapshot, cxx::Exception> {
        self.inner.snapshot()
    }
    pub fn online_query(&self) -> Result<OnlineQuerySnapshot, cxx::Exception> {
        self.inner.online_query()
    }
    pub fn reset_cache(&mut self) {
        self.inner.pin_mut().reset_cache()
    }
    /// Set the composition caret used for prefix candidate decoding. `None` returns to the end.
    pub fn set_caret(&mut self, caret: Option<usize>) {
        self.inner
            .pin_mut()
            .set_caret(caret.map_or(u64::MAX, |value| value as u64));
    }
    pub fn prefix_end(&self) -> usize {
        self.inner.prefix_end()
    }
    pub fn pending_suffix(&self) -> String {
        self.inner.pending_suffix()
    }
    /// Forgets the committed words the Engine's personal context learning follows, and the recent commits learning
    /// undo tracks. Called when the text the next word lands in is no longer the text the last one went to.
    pub fn reset_context(&mut self) {
        self.inner.pin_mut().reset_context()
    }
    pub fn apply_online_candidate(
        &mut self,
        query: &OnlineQuerySnapshot,
        candidate: &str,
        source: u8,
    ) -> Result<bool, cxx::Exception> {
        self.inner
            .pin_mut()
            .apply_online_candidate(query, candidate, source)
    }
    pub fn apply_online_candidates(
        &mut self,
        query: &OnlineQuerySnapshot,
        candidates: &[String],
        source: u8,
    ) -> Result<bool, cxx::Exception> {
        self.inner
            .pin_mut()
            .apply_online_candidates(query, candidates, source)
    }
    pub fn character(&mut self, value: u8, shift: bool) -> Result<EngineResult, cxx::Exception> {
        self.inner.pin_mut().character(value, shift)
    }
    /// Ask the Engine for the candidates it withheld from a single-letter query. It caps those at
    /// twenty-four so the first page is cheap; everything below that cap is unreachable until
    /// someone asks. Answers whether the candidate list actually grew.
    pub fn expand_initial_candidates(&mut self) -> Result<bool, cxx::Exception> {
        self.inner.pin_mut().expand_initial_candidates()
    }
    pub fn set_nine_key_enabled(&mut self, enabled: bool) -> Result<(), cxx::Exception> {
        self.inner.pin_mut().set_nine_key_enabled(enabled)
    }
    pub fn choose_nine_key_spelling(
        &mut self,
        index: usize,
    ) -> Result<EngineResult, cxx::Exception> {
        self.inner.pin_mut().choose_nine_key_spelling(index)
    }
    pub fn command(&mut self, command: Command) -> Result<EngineResult, cxx::Exception> {
        if matches!(command, Command::CommitRaw) {
            self.inner.pin_mut().commit_raw_with_policy()
        } else {
            self.inner.pin_mut().command(command as u8)
        }
    }
    pub fn select(&mut self, index: usize) -> Result<EngineResult, cxx::Exception> {
        self.inner.pin_mut().select(index)
    }
    pub fn pin_candidate(&mut self, index: usize) -> Result<EngineResult, cxx::Exception> {
        self.inner.pin_mut().pin_candidate(index)
    }
    pub fn remove_candidate(&mut self, index: usize) -> Result<EngineResult, cxx::Exception> {
        self.inner.pin_mut().remove_candidate(index)
    }
    pub fn fix_candidate_position(
        &mut self,
        index: usize,
        position: u8,
    ) -> Result<EngineResult, cxx::Exception> {
        self.inner.pin_mut().fix_candidate_position(index, position)
    }
    pub fn clear_candidate_position(
        &mut self,
        index: usize,
    ) -> Result<EngineResult, cxx::Exception> {
        self.inner.pin_mut().clear_candidate_position(index)
    }
    pub fn select_edge(
        &mut self,
        index: usize,
        edge: CandidateEdge,
    ) -> Result<EngineResult, cxx::Exception> {
        self.inner.pin_mut().select_edge(index, edge as u8)
    }
    pub fn finish(&mut self, index: usize) -> Result<EngineResult, cxx::Exception> {
        self.inner.pin_mut().finish(index)
    }
    pub fn punctuation(&mut self, value: u8) -> Result<EngineResult, cxx::Exception> {
        self.inner.pin_mut().punctuation(value)
    }
    pub fn balance_paired_punctuation_after_auto_close(
        &mut self,
        opening: u8,
    ) -> Result<(), cxx::Exception> {
        self.inner
            .pin_mut()
            .balance_paired_punctuation_after_auto_close(opening)
    }
    pub fn set_chinese_punctuation_enabled(&mut self, enabled: bool) -> Result<(), cxx::Exception> {
        self.inner
            .pin_mut()
            .set_chinese_punctuation_enabled(enabled)
    }
    pub fn set_punctuation_lock(&mut self, lock: u8) -> Result<(), cxx::Exception> {
        self.inner.pin_mut().set_punctuation_lock(lock)
    }
    pub fn set_paired_punctuation_enabled(&mut self, enabled: bool) -> Result<(), cxx::Exception> {
        self.inner.pin_mut().set_paired_punctuation_enabled(enabled)
    }
    pub fn set_dedicated_english(&mut self, enabled: bool) -> Result<(), cxx::Exception> {
        self.inner.pin_mut().set_dedicated_english(enabled)
    }
}

#[cfg(test)]
mod tests;
