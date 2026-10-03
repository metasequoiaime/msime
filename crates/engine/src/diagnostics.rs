//! Every user-visible diagnostic and error message, verbatim from the C++ engine and bridge. Goldens record diagnostics and host tests compare error text, so modules use these constants instead of writing their own wording. Messages specific to one module's validation (personal dictionary edits, snapshot staging) live with that module but follow the same rule: copy the C++ text.

// ---- KeyResult diagnostics (session, nine-key) ----
pub const ENGLISH_WORD_NOT_LEARNED: &str = "English word could not be learned.";
pub const FREQUENCY_NOT_PERSISTED: &str = "Unable to persist candidate frequency adjustment.";
pub const ENGLISH_FREQUENCY_NOT_PERSISTED: &str =
    "English candidate frequency could not be persisted.";
pub const PIN_NOT_PERSISTED: &str = "Unable to persist the pinned candidate.";
pub const REMOVAL_NOT_PERSISTED: &str = "Unable to persist candidate removal.";
pub const POSITION_NOT_PERSISTED: &str = "Unable to persist candidate position.";
pub const PHRASE_NOT_PERSISTED: &str = "Unable to persist the composed phrase.";
pub const SENTENCE_NOT_PERSISTED: &str = "Unable to persist the selected sentence.";
pub const TYPO_NOT_PERSISTED: &str = "Typo correction could not be persisted.";
pub const AUTOCORRECT_PREFERENCE_NOT_PERSISTED: &str =
    "Autocorrect preference could not be persisted.";
pub const PICK_TRANSITION_NOT_PERSISTED: &str = "Unable to persist the pick transition.";
pub const PERSONAL_CONTEXT_NOT_PERSISTED: &str = "Unable to persist personal context.";
pub const NINE_KEY_FREQUENCY_NOT_PERSISTED: &str =
    "Unable to persist nine-key candidate frequency adjustment.";
pub const NINE_KEY_REMOVAL_NOT_PERSISTED: &str = "Unable to persist nine-key candidate removal.";
pub const NINE_KEY_POSITION_NOT_PERSISTED: &str = "Unable to persist nine-key candidate position.";
pub const NINE_KEY_DIGIT_LIMIT: &str = "九键最多输入 32 位，请先选择候选";

// ---- Local mode query diagnostics ----
pub const QUICK_PHRASE_UNAVAILABLE: &str = "Quick phrase database is unavailable.";
pub const QUICK_PHRASE_QUERY_FAILED: &str = "Quick phrase database could not be queried.";
pub const EMOJI_UNAVAILABLE: &str = "Emoji database is unavailable.";
pub const EMOJI_QUERY_FAILED: &str = "Emoji database could not be queried.";
pub const KAOMOJI_UNAVAILABLE: &str = "Kaomoji database is unavailable.";
pub const KAOMOJI_QUERY_FAILED: &str = "Kaomoji database could not be queried.";
pub const SUPER_JIANPIN_UNAVAILABLE: &str = "Super-jianpin database is unavailable.";
pub const SUPER_JIANPIN_QUERY_FAILED: &str = "Super-jianpin database could not be queried.";

// ---- Engine errors (Session construction, runtime paths) ----
pub const INVALID_SESSION_OPTIONS: &str = "Invalid session options";
pub const UNKNOWN_HELPCODE_SCHEMA: &str = "Unknown helpcode schema";
pub const INVALID_PUNCTUATION_LOCK: &str = "Invalid punctuation lock";
pub const RUNTIME_DIRECTORIES_MUST_BE_ABSOLUTE: &str = "Runtime directories must be absolute";
pub const RUNTIME_ASSET_PATH_ESCAPES: &str = "Runtime asset path escapes its directory";
pub const INVALID_RUNTIME_CONTENT_ID: &str = "Invalid runtime content ID";
pub const RUNTIME_ROOTS_OVERLAP: &str =
    "Resource, user-data and cache directories must not overlap";
pub const INCOMPLETE_RUNTIME_GENERATION: &str = "Incomplete runtime dictionary generation";
/// Followed by the replay error.
pub const RUNTIME_REPLAY_FAILED: &str = "Runtime dictionary replay failed: ";
pub const RUNTIME_STAGING_EXISTS: &str = "Runtime dictionary staging directory already exists";
/// Followed by the source path.
pub const RUNTIME_COPY_FAILED: &str = "Unable to copy runtime dictionary: ";
pub const RUNTIME_FINALIZE_FAILED: &str = "Cannot finalize runtime dictionary generation";
pub const INVALID_DICTIONARY_STATE: &str = "Invalid or unavailable dictionary state";

// ---- Host facade errors (the C++ bridge) ----
pub const CHARACTER_MUST_BE_ASCII: &str = "Engine character must be ASCII";
pub const PUNCTUATION_MUST_BE_ASCII: &str = "Engine punctuation must be ASCII";
pub const PAIRED_OPENING_MUST_BE_ASCII: &str = "Paired punctuation opening must be ASCII";
pub const UNSUPPORTED_INPUT_COMMAND: &str = "Unsupported input command";
pub const INVALID_CANDIDATE_POSITION: &str = "Invalid candidate position";
pub const INVALID_CANDIDATE_EDGE: &str = "Invalid candidate edge";
pub const UNSUPPORTED_INPUT_SCHEME: &str = "Unsupported input scheme";
/// 方案合法，但不在会话的 `enabled_schemes` 里（例如五笔版的 Engine 被要求切到全拼）。
pub const INPUT_SCHEME_NOT_ENABLED: &str = "Input scheme is not enabled";
pub const UNSUPPORTED_SHUANGPIN_PROFILE: &str = "Unsupported shuangpin profile";
pub const UNSUPPORTED_WUBI_PROFILE: &str = "Unsupported wubi profile";
pub const UNSUPPORTED_FREQUENCY_MODE: &str = "Unsupported frequency mode";
pub const UNSUPPORTED_VIETNAMESE_METHOD: &str = "Unsupported Vietnamese input method";
pub const UNSUPPORTED_VIETNAMESE_TONE_STYLE: &str = "Unsupported Vietnamese tone style";
pub const UNSUPPORTED_DICTIONARY_KIND: &str = "Unsupported dictionary kind";
pub const EXPECTED_AT_MOST_ONE_ENTRY: &str = "Expected at most one dictionary entry";
pub const EXPECTED_AT_MOST_ONE_WEIGHT: &str = "Expected at most one weight";
pub const TRANSLATION_SIDECAR_FAILED: &str = "Unable to prepare custom translation sidecar";
pub const RESET_IN_PLACE: &str = "Cannot reset packaged dictionaries in place";
pub const PACKAGED_DICTIONARY_UNAVAILABLE: &str = "Packaged dictionary is unavailable";
pub const RESET_JOURNAL_FAILED: &str = "Cannot prepare empty learning journal";
pub const RESET_STAGE_DICTIONARIES_FAILED: &str = "Cannot stage fresh dictionaries";
pub const RESET_STAGE_FAILED: &str = "Cannot stage learned-data reset";
pub const RESET_PUBLISH_FAILED: &str = "Cannot publish learned-data reset";
pub const INVALID_SNAPSHOT_RECORD_LIMIT: &str = "Invalid snapshot record limit";
pub const INVALID_SNAPSHOT_RECORD_TYPE: &str = "Invalid snapshot record type";
pub const INVALID_SNAPSHOT_DICTIONARY_KIND: &str = "Invalid snapshot dictionary kind";
pub const INVALID_SNAPSHOT_POSITION: &str = "Invalid snapshot position";
pub const INVALID_SNAPSHOT_SELECTION_COUNT: &str = "Invalid snapshot selection count";
pub const SNAPSHOT_STREAM_FAILED: &str = "Snapshot record stream failed";
pub const ENGLISH_DICTIONARY_UNAVAILABLE: &str = "English dictionary unavailable";
pub const INVALID_ENGLISH_COMPLETION_LIMIT: &str = "Invalid English completion limit";
pub const INVALID_ENGLISH_COMPLETION_PREFIX: &str = "Invalid English completion prefix";
pub const CANDIDATE_GLOSS_UNAVAILABLE: &str = "Candidate gloss dictionary unavailable";
pub const OFFLINE_GLOSS_UNAVAILABLE: &str = "Offline gloss dictionary unavailable";
pub const OFFLINE_GLOSS_UNREADABLE: &str = "Offline gloss dictionary unreadable";
pub const OFFLINE_GLOSS_VERSION_UNSUPPORTED: &str = "Offline gloss dictionary version unsupported";
pub const OFFLINE_GLOSS_LANGUAGE_MISMATCH: &str = "Offline gloss dictionary language mismatch";
pub const OFFLINE_GLOSS_READ_FAILED: &str = "Offline gloss dictionary read failed";
pub const LANGUAGE_DICTIONARY_UNAVAILABLE: &str = "Language dictionary unavailable";
pub const LANGUAGE_DICTIONARY_VERSION_UNSUPPORTED: &str = "Language dictionary version unsupported";
pub const INVALID_EMOJI_CATALOG_PAGE: &str = "Invalid emoji catalog page";
pub const INVALID_EMOJI_CATALOG_CURSOR: &str = "Invalid emoji catalog cursor";
pub const EMOJI_CATALOG_UNAVAILABLE: &str = "Emoji catalog unavailable";
pub const EMOJI_CATALOG_QUERY_UNAVAILABLE: &str = "Emoji catalog query unavailable";
pub const EMOJI_CATALOG_QUERY_REJECTED: &str = "Emoji catalog query rejected";
pub const EMOJI_CATALOG_READ_FAILED: &str = "Emoji catalog read failed";
