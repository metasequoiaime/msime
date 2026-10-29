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
pub const NINE_KEY_DIGIT_LIMIT: &str = "一次最多输入 48 个按键，请先选择候选";

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
/// Followed by the source path.
pub const RUNTIME_COPY_FAILED: &str = "Unable to copy runtime dictionary: ";
pub const RUNTIME_FINALIZE_FAILED: &str = "Cannot finalize runtime dictionary generation";
/// 后面接文件名和原因，见 `format::check_format_version`。
pub const DICTIONARY_FORMAT_UNSUPPORTED: &str = "Dictionary format version unsupported: ";
pub const INVALID_DICTIONARY_STATE: &str = "Invalid or unavailable dictionary state";
/// 拿不到词库的独占维护租约（`DictionaryAccess::try_maintenance`）：还有会话或另一次维护开着。错误码是 `ErrorCode::Busy`。
pub const DICTIONARY_MAINTENANCE_BUSY: &str = "Dictionary maintenance busy";

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
/// 双拼方案是 `custom`，但没有给表或表不合法；消息后面接着具体原因（`shuangpin::custom::CustomProfileError`）。
pub const INVALID_CUSTOM_SHUANGPIN_PROFILE: &str = "Invalid custom shuangpin profile";
pub const UNSUPPORTED_WUBI_PROFILE: &str = "Unsupported wubi profile";
pub const UNSUPPORTED_FREQUENCY_MODE: &str = "Unsupported frequency mode";
pub const UNSUPPORTED_VIETNAMESE_METHOD: &str = "Unsupported Vietnamese input method";
pub const UNSUPPORTED_VIETNAMESE_TONE_STYLE: &str = "Unsupported Vietnamese tone style";
pub const UNSUPPORTED_KOREAN_LAYOUT: &str = "Unsupported Korean layout";
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
pub const PRONUNCIATION_UNAVAILABLE: &str = "Pronunciation dictionary unavailable";
pub const PRONUNCIATION_UNREADABLE: &str = "Pronunciation dictionary unreadable";
pub const PRONUNCIATION_VERSION_UNSUPPORTED: &str = "Pronunciation dictionary version unsupported";
pub const PRONUNCIATION_KIND_MISMATCH: &str = "Pronunciation dictionary kind mismatch";
pub const PRONUNCIATION_READ_FAILED: &str = "Pronunciation dictionary read failed";
pub const LANGUAGE_DICTIONARY_UNAVAILABLE: &str = "Language dictionary unavailable";
pub const LANGUAGE_DICTIONARY_VERSION_UNSUPPORTED: &str = "Language dictionary version unsupported";
pub const INVALID_EMOJI_CATALOG_PAGE: &str = "Invalid emoji catalog page";
pub const INVALID_EMOJI_CATALOG_CURSOR: &str = "Invalid emoji catalog cursor";
pub const EMOJI_CATALOG_UNAVAILABLE: &str = "Emoji catalog unavailable";
pub const EMOJI_CATALOG_QUERY_UNAVAILABLE: &str = "Emoji catalog query unavailable";
pub const EMOJI_CATALOG_QUERY_REJECTED: &str = "Emoji catalog query rejected";
pub const EMOJI_CATALOG_READ_FAILED: &str = "Emoji catalog read failed";

// ---- 诊断码 ----

/// `KeyResult::diagnostic` 的稳定数字码（`KeyResult::diagnostic_code`），供宿主和 C ABI 按数字分支，不必比对说明文字。说明文字仍是上面那些常量，原样保留。编号一经发布只追加、不改、不复用；新加一条 KeyResult 诊断时在 [`DIAGNOSTIC_CODES`] 末尾接一个新号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum DiagnosticCode {
    EnglishWordNotLearned = 1,
    FrequencyNotPersisted = 2,
    EnglishFrequencyNotPersisted = 3,
    PinNotPersisted = 4,
    RemovalNotPersisted = 5,
    PositionNotPersisted = 6,
    PhraseNotPersisted = 7,
    SentenceNotPersisted = 8,
    TypoNotPersisted = 9,
    AutocorrectPreferenceNotPersisted = 10,
    PickTransitionNotPersisted = 11,
    PersonalContextNotPersisted = 12,
    NineKeyFrequencyNotPersisted = 13,
    NineKeyRemovalNotPersisted = 14,
    NineKeyPositionNotPersisted = 15,
    NineKeyDigitLimit = 16,
    QuickPhraseUnavailable = 17,
    QuickPhraseQueryFailed = 18,
    EmojiUnavailable = 19,
    EmojiQueryFailed = 20,
    KaomojiUnavailable = 21,
    KaomojiQueryFailed = 22,
    SuperJianpinUnavailable = 23,
    SuperJianpinQueryFailed = 24,
    /// 九键笔画筛选拿不到笔画词库（`LANGUAGE_DICTIONARY_UNAVAILABLE`）。这条常量放在宿主门面错误那段，因为它同时是查语言词库接口的错误文字。
    LanguageDictionaryUnavailable = 25,
    /// 不在映射表里的说明文字：注音的选字与按键把底层错误原文（`EngineError` 的 `Display`）放进诊断，这类文字没有自己的码。
    Other = 0xFFFF,
}

/// 诊断码与说明文字的映射表，KeyResult 诊断常量各占一行，按编号排。
pub const DIAGNOSTIC_CODES: [(DiagnosticCode, &str); 25] = [
    (
        DiagnosticCode::EnglishWordNotLearned,
        ENGLISH_WORD_NOT_LEARNED,
    ),
    (
        DiagnosticCode::FrequencyNotPersisted,
        FREQUENCY_NOT_PERSISTED,
    ),
    (
        DiagnosticCode::EnglishFrequencyNotPersisted,
        ENGLISH_FREQUENCY_NOT_PERSISTED,
    ),
    (DiagnosticCode::PinNotPersisted, PIN_NOT_PERSISTED),
    (DiagnosticCode::RemovalNotPersisted, REMOVAL_NOT_PERSISTED),
    (DiagnosticCode::PositionNotPersisted, POSITION_NOT_PERSISTED),
    (DiagnosticCode::PhraseNotPersisted, PHRASE_NOT_PERSISTED),
    (DiagnosticCode::SentenceNotPersisted, SENTENCE_NOT_PERSISTED),
    (DiagnosticCode::TypoNotPersisted, TYPO_NOT_PERSISTED),
    (
        DiagnosticCode::AutocorrectPreferenceNotPersisted,
        AUTOCORRECT_PREFERENCE_NOT_PERSISTED,
    ),
    (
        DiagnosticCode::PickTransitionNotPersisted,
        PICK_TRANSITION_NOT_PERSISTED,
    ),
    (
        DiagnosticCode::PersonalContextNotPersisted,
        PERSONAL_CONTEXT_NOT_PERSISTED,
    ),
    (
        DiagnosticCode::NineKeyFrequencyNotPersisted,
        NINE_KEY_FREQUENCY_NOT_PERSISTED,
    ),
    (
        DiagnosticCode::NineKeyRemovalNotPersisted,
        NINE_KEY_REMOVAL_NOT_PERSISTED,
    ),
    (
        DiagnosticCode::NineKeyPositionNotPersisted,
        NINE_KEY_POSITION_NOT_PERSISTED,
    ),
    (DiagnosticCode::NineKeyDigitLimit, NINE_KEY_DIGIT_LIMIT),
    (
        DiagnosticCode::QuickPhraseUnavailable,
        QUICK_PHRASE_UNAVAILABLE,
    ),
    (
        DiagnosticCode::QuickPhraseQueryFailed,
        QUICK_PHRASE_QUERY_FAILED,
    ),
    (DiagnosticCode::EmojiUnavailable, EMOJI_UNAVAILABLE),
    (DiagnosticCode::EmojiQueryFailed, EMOJI_QUERY_FAILED),
    (DiagnosticCode::KaomojiUnavailable, KAOMOJI_UNAVAILABLE),
    (DiagnosticCode::KaomojiQueryFailed, KAOMOJI_QUERY_FAILED),
    (
        DiagnosticCode::SuperJianpinUnavailable,
        SUPER_JIANPIN_UNAVAILABLE,
    ),
    (
        DiagnosticCode::SuperJianpinQueryFailed,
        SUPER_JIANPIN_QUERY_FAILED,
    ),
    (
        DiagnosticCode::LanguageDictionaryUnavailable,
        LANGUAGE_DICTIONARY_UNAVAILABLE,
    ),
];

impl DiagnosticCode {
    pub const fn value(self) -> u16 {
        self as u16
    }

    /// 一条 KeyResult 诊断文字对应的码；表外的文字是 [`DiagnosticCode::Other`]。
    pub fn from_message(message: &str) -> Self {
        DIAGNOSTIC_CODES
            .iter()
            .find(|(_, text)| *text == message)
            .map_or(Self::Other, |(code, _)| *code)
    }

    /// 这个码的说明文字；[`DiagnosticCode::Other`] 没有固定文字。
    pub fn message(self) -> Option<&'static str> {
        DIAGNOSTIC_CODES
            .iter()
            .find(|(code, _)| *code == self)
            .map(|(_, text)| *text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 编号一经发布就冻结，表按编号排、每条文字只出现一次，查表来回一致。
    #[test]
    fn diagnostic_codes_are_frozen_and_round_trip() {
        for (index, (code, message)) in DIAGNOSTIC_CODES.iter().enumerate() {
            assert_eq!(usize::from(code.value()), index + 1, "{message}");
            assert_eq!(DiagnosticCode::from_message(message), *code);
            assert_eq!(code.message(), Some(*message));
        }
        assert_eq!(DiagnosticCode::Other.value(), 0xFFFF);
        assert_eq!(DiagnosticCode::Other.message(), None);
        assert_eq!(
            DiagnosticCode::from_message("database is locked"),
            DiagnosticCode::Other
        );
        assert_eq!(
            DiagnosticCode::from_message(NINE_KEY_DIGIT_LIMIT).value(),
            16
        );
        assert_eq!(
            DiagnosticCode::from_message(SUPER_JIANPIN_QUERY_FAILED).value(),
            24
        );
        assert_eq!(
            DiagnosticCode::from_message(LANGUAGE_DICTIONARY_UNAVAILABLE).value(),
            25
        );
    }

    /// 本文件里 `pub const` 字符串常量的名字和值，值可能换到下一行。
    fn constants() -> Vec<(&'static str, &'static str)> {
        let source = include_str!("diagnostics.rs");
        let production = &source[..source.find("// ---- 诊断码").unwrap()];
        production
            .split("pub const ")
            .skip(1)
            .map(|item| {
                let name = &item[..item.find(':').unwrap()];
                let literal = &item[item.find('"').unwrap() + 1..];
                (name, &literal[..literal.find('"').unwrap()])
            })
            .collect()
    }

    /// 「KeyResult diagnostics」和「Local mode query diagnostics」两段的每个常量都有码：新加的诊断忘了给码，这里会报出它的名字。
    #[test]
    fn every_key_result_diagnostic_constant_has_a_code() {
        let source = include_str!("diagnostics.rs");
        let start = source.find("// ---- KeyResult diagnostics").unwrap();
        let end = source.find("// ---- Engine errors").unwrap();
        let section = &source[start..end];
        let mut checked = 0;
        for (name, message) in constants() {
            if section.contains(&format!("pub const {name}:")) {
                assert_ne!(
                    DiagnosticCode::from_message(message),
                    DiagnosticCode::Other,
                    "{name}"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 24);
    }

    /// 直接交给 `KeyResult::with_diagnostic` 的固定常量也都有码，不管它定义在本文件哪一段：九键笔画筛选给的 `LANGUAGE_DICTIONARY_UNAVAILABLE` 在宿主门面错误那段，按分段数漏掉过它。扫的是引擎源码里每个 `with_diagnostic(...)` 的实参（测试模块除外）。
    #[test]
    fn every_constant_passed_to_with_diagnostic_has_a_code() {
        fn sources(directory: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    sources(&path, found);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    found.push(path);
                }
            }
        }
        /// 从左括号之后到与它配对的右括号之前，跳过字符串字面量里的括号。
        fn argument(text: &str) -> &str {
            let (mut depth, mut quoted, mut escaped) = (1, false, false);
            for (index, character) in text.char_indices() {
                match character {
                    _ if escaped => escaped = false,
                    '\\' if quoted => escaped = true,
                    '"' => quoted = !quoted,
                    '(' if !quoted => depth += 1,
                    ')' if !quoted => {
                        depth -= 1;
                        if depth == 0 {
                            return &text[..index];
                        }
                    }
                    _ => {}
                }
            }
            panic!("unbalanced with_diagnostic call");
        }

        let constants = constants();
        let mut files = Vec::new();
        sources(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        let mut passed = std::collections::BTreeSet::new();
        for path in files {
            let source = std::fs::read_to_string(&path).unwrap();
            let production = source.split("\n#[cfg(test)]\nmod tests").next().unwrap();
            for (start, call) in production.match_indices("with_diagnostic(") {
                let argument = argument(&production[start + call.len()..]);
                for reference in argument.split("diagnostics::").skip(1) {
                    let name: String = reference
                        .chars()
                        .take_while(|character| {
                            character.is_ascii_uppercase()
                                || character.is_ascii_digit()
                                || *character == '_'
                        })
                        .collect();
                    if name.len() < 2 {
                        continue;
                    }
                    let message = constants
                        .iter()
                        .find(|(constant, _)| *constant == name)
                        .unwrap_or_else(|| panic!("{} uses unknown {name}", path.display()))
                        .1;
                    assert_ne!(
                        DiagnosticCode::from_message(message),
                        DiagnosticCode::Other,
                        "{} passes {name} to with_diagnostic without a code",
                        path.display()
                    );
                    passed.insert(name);
                }
            }
        }
        assert!(
            passed.contains("LANGUAGE_DICTIONARY_UNAVAILABLE")
                && passed.contains("NINE_KEY_DIGIT_LIMIT"),
            "{passed:?}"
        );
    }
}
