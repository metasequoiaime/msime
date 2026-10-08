//! T9 digit composition over quanpin syllables plus English T9 (core-session.md §8). Active while it holds digits; `Session` routes to it then.
//!
//! The digits stay the composition: a chosen spelling only rewrites its span of digits and is remembered in `locked`, and every candidate's `pinyin` is the run of digits it consumes, so selection advances the same way whichever reading produced the row.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use crate::assets;
use crate::diagnostics;
use crate::dictionary::english::EnglishDictionary;
use crate::lattice::decode::PHRASE_LENGTH_BONUS;
use crate::paths::RuntimePaths;
use crate::pinyin::syllables::intact_pinyin_list;
use crate::quanpin::QuanpinDictionary;
use crate::session::SessionSnapshot;
use crate::text::count_utf8_chars;
use crate::types::{
    CandidateSource, Command, EnglishInputOptions, FrequencyAdjustmentMode,
    FrequencyAdjustmentOptions, FuzzyPinyinOptions, KeyResult, LocalInputMode,
    PersonalDictionaryKind, SchemeType, WordItem,
};
use crate::user_dictionary::positions;
use crate::user_dictionary::ranking::{self, RankingRequest};
use crate::user_dictionary::removal;

pub const PATH_LIMIT: usize = 48;
pub const DIGIT_LIMIT: usize = 32;
pub const CANDIDATE_LIMIT: usize = 128;
// 少量查询键直接扫描已有切分路径，避免刷新时为临时哈希表分配堆内存。
const SMALL_QUERY_KEY_BATCH: usize = 64;
// 少量九宫格候选直接扫描已保留词，避免排序后为一次去重分配哈希表。
const SMALL_CANDIDATE_DEDUP: usize = 64;

/// The digit printed beside each of `a..=z` (NK:37).
const KEYPAD: &[u8; 26] = b"22233344455566677778889999";
/// The letters printed on each digit key (NK:74).
const DIGIT_LETTERS: [&str; 10] = [
    "", "", "abc", "def", "ghi", "jkl", "mno", "pqrs", "tuv", "wxyz",
];
/// T9 expansion multiplies by three or four per digit, so only the leading digits become letter prefixes and the dictionary's own prefix search carries the rest (NK:189).
const ENGLISH_PREFIX_BUDGET: usize = 64;
const ENGLISH_LIMIT: usize = 5;
const ENGLISH_CANDIDATE_CAPACITY: usize = ENGLISH_PREFIX_BUDGET * ENGLISH_LIMIT;

type Path = Vec<String>;

pub struct NineKeySession {
    paths: RuntimePaths,
    learning: bool,
    frequency: FrequencyAdjustmentOptions,
    fuzzy: FuzzyPinyinOptions,
    english_options: EnglishInputOptions,
    digits: String,
    locked: Vec<String>,
    /// 用户用 `'` 切开音节的数字位置，升序，都在已锁定的部分之后。和锁定的拼写不同，切分只定下一个音节在哪里结束，两边数字的各种读法都还保留：`94'26` 可以是 xi'an，也可以是 yi'an，但不会是 xian。
    splits: Vec<usize>,
    spellings: Vec<String>,
    /// `SessionSnapshot::nine_key_reading`，随候选一起重建。
    reading: String,
    candidates: Vec<WordItem>,
    english_only: bool,
    /// 会话允许全拼时为真。为假时九宫格只拼英文，拼音词库永远不打开。
    pinyin: bool,
    /// Opened on first use.
    dictionary: Option<QuanpinDictionary>,
    /// 每个音节及其前缀的单字频度，`SyllablePrior::from_dictionary` 在第一次查词前建好，给切分路径排序用。
    prior: Option<SyllablePrior>,
    english: Option<EnglishDictionary>,
}

impl NineKeySession {
    /// English options are fixed here; the reference never updated them afterwards. `pinyin` 是会话的 `enabled_schemes` 是否含全拼：九宫格拼的是全拼音节，不含全拼时只给英文行，也不打开拼音词库。
    pub fn new(
        paths: &RuntimePaths,
        learning: bool,
        frequency: FrequencyAdjustmentOptions,
        fuzzy: FuzzyPinyinOptions,
        english: EnglishInputOptions,
        pinyin: bool,
    ) -> Self {
        Self {
            paths: paths.clone(),
            learning,
            frequency,
            fuzzy,
            english_options: english,
            digits: String::new(),
            locked: Vec::new(),
            splits: Vec::new(),
            spellings: Vec::new(),
            reading: String::new(),
            candidates: Vec::new(),
            english_only: false,
            pinyin,
            dictionary: None,
            prior: None,
            english: None,
        }
    }

    /// Holds digits.
    pub fn active(&self) -> bool {
        !self.digits.is_empty()
    }

    /// English-only T9 is a mode the host enters deliberately, so it is not gated on the mixed-candidate setting or on the minimum prefix a mixed list needs (nine_key_session.h:22-35).
    pub fn set_english_only(&mut self, english_only: bool) {
        if self.english_only == english_only {
            return;
        }
        self.english_only = english_only;
        self.refresh();
    }

    /// `2`..=`9`；到 32 个数字时按 `NINE_KEY_DIGIT_LIMIT` 处理。组字中按 `'` 在已输入部分的末尾切开音节；在同一处再切一次，或者紧跟在锁定的拼写之后切，都没有作用。
    pub fn character(&mut self, digit: u8) -> KeyResult {
        if digit == b'\'' {
            // 英文九键的数字拼的是字母不是音节，没有可切的地方，记下的切分上屏时也只会被丢掉。
            if !self.active() || self.english_only || !self.pinyin {
                return KeyResult::unhandled();
            }
            let end = self.digits.len();
            if end > self.locked_length() && self.splits.last() != Some(&end) {
                self.splits.push(end);
                self.refresh();
            }
            return KeyResult::handled();
        }
        if !(b'2'..=b'9').contains(&digit) {
            return KeyResult::unhandled();
        }
        if self.digits.len() >= DIGIT_LIMIT {
            return KeyResult::handled()
                .with_diagnostic(Some(diagnostics::NINE_KEY_DIGIT_LIMIT.to_string()));
        }
        self.digits.push(char::from(digit));
        self.refresh();
        KeyResult::handled()
    }

    pub fn choose_spelling(&mut self, index: usize) -> KeyResult {
        let Some(spelling) = self.spellings.get(index).cloned() else {
            return KeyResult::unhandled();
        };
        let offset = self.locked_length();
        // A spelling longer than what is typed extends the digits to its whole code; the spelling list only offers ones that stay within the digit limit.
        let end = offset + spelling.len().min(self.digits.len() - offset);
        self.digits.replace_range(offset..end, &encode(&spelling));
        self.locked.push(spelling);
        let locked_length = self.locked_length();
        self.splits.retain(|&split| split > locked_length);
        self.refresh();
        KeyResult::handled()
    }

    pub fn select(&mut self, index: usize) -> KeyResult {
        let Some(selected) = self.candidates.get(index).cloned() else {
            return KeyResult::unhandled();
        };
        let diagnostic = if self.learning
            && self.frequency.mode != FrequencyAdjustmentMode::Disabled
            && index != 0
            && selected.fixed_position == 0
            && self.editable(index)
        {
            self.adjust_frequency(index, false)
        } else {
            None
        };
        self.consume(selected.pinyin.len());
        self.refresh();
        KeyResult::committed(selected.word).with_diagnostic(diagnostic)
    }

    /// Out of range commits the digits.
    pub fn finish(&mut self, first_index: usize) -> KeyResult {
        if !self.active() {
            return KeyResult::unhandled();
        }
        if first_index >= self.candidates.len() {
            return self.command(Command::CommitRaw);
        }
        let mut commit = String::new();
        let mut diagnostic = None;
        let mut index = first_index;
        while self.active() {
            if self.candidates.is_empty() {
                commit.push_str(&self.digits);
                break;
            }
            let result = self.select(index);
            if diagnostic.is_none() {
                diagnostic = result.diagnostic;
            }
            if let Some(text) = result.commit {
                commit.push_str(&text);
            }
            index = 0;
        }
        self.command(Command::Cancel);
        KeyResult::committed(commit).with_diagnostic(diagnostic)
    }

    pub fn command(&mut self, command: Command) -> KeyResult {
        if !self.active() {
            return KeyResult::unhandled();
        }
        match command {
            Command::CommitCandidate => return self.select(0),
            Command::CommitRaw => {
                let raw = self.digits.clone();
                self.command(Command::Cancel);
                return KeyResult::committed(raw);
            }
            Command::Cancel => {
                self.digits.clear();
                self.locked.clear();
                self.splits.clear();
            }
            // 末尾的切分先删，这样退格撤销的是最后按下的那个键。
            Command::Backspace => {
                if self.splits.last() == Some(&self.digits.len()) {
                    self.splits.pop();
                } else {
                    self.digits.pop();
                    while self.locked_length() > self.digits.len() {
                        self.locked.pop();
                    }
                    let length = self.digits.len();
                    self.splits.retain(|&split| split <= length);
                }
            }
            _ => return KeyResult::unhandled(),
        }
        self.refresh();
        KeyResult::handled()
    }

    pub fn pin(&mut self, index: usize) -> KeyResult {
        if !self.editable(index) {
            return KeyResult::unhandled();
        }
        let diagnostic = self.adjust_frequency(index, true);
        self.refresh();
        KeyResult::handled().with_diagnostic(diagnostic)
    }

    pub fn remove(&mut self, index: usize) -> KeyResult {
        if !self.editable(index) || count_utf8_chars(&self.candidates[index].word) <= 1 {
            return KeyResult::unhandled();
        }
        let item = &self.candidates[index];
        let deleted = removal::delete_dictionary_candidate(
            &self.paths.dictionary(assets::MAIN_DICTIONARY),
            &self.paths.user(assets::USER_JOURNAL),
            PersonalDictionaryKind::Pinyin,
            &item.canonical_pinyin,
            &item.word,
        );
        // The reference reports every failed removal with this one message and keeps the composition (NK:434-437).
        if deleted.is_err() {
            return KeyResult::handled().with_diagnostic(Some(
                diagnostics::NINE_KEY_REMOVAL_NOT_PERSISTED.to_string(),
            ));
        }
        if let Some(dictionary) = self.dictionary.as_mut() {
            dictionary.reset_cache();
        }
        self.refresh();
        KeyResult::handled()
    }

    /// 0 clears.
    pub fn set_position(&mut self, index: usize, position: i32) -> KeyResult {
        if !self.editable(index) || !(0..=5).contains(&position) {
            return KeyResult::unhandled();
        }
        let item = &self.candidates[index];
        let journal = self.paths.user(assets::USER_JOURNAL);
        let context = self.ranking_context();
        let saved = if position == 0 {
            positions::clear_fixed_position(&journal, &context, &item.canonical_pinyin, &item.word)
        } else {
            positions::set_fixed_position(
                &journal,
                &context,
                &item.canonical_pinyin,
                &item.word,
                position,
            )
        };
        // Same rule as removal: one message for every failure, composition kept (NK:453-454).
        if saved.is_err() {
            return KeyResult::handled().with_diagnostic(Some(
                diagnostics::NINE_KEY_POSITION_NOT_PERSISTED.to_string(),
            ));
        }
        self.refresh();
        KeyResult::handled()
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        let locked_length = self.locked_length();
        let mut preedit = self.locked.join("'");
        if self.active() && locked_length < self.digits.len() {
            if !preedit.is_empty() {
                preedit.push('\'');
            }
            let mut start = locked_length;
            for &split in &self.splits {
                preedit.push_str(&self.digits[start..split]);
                preedit.push('\'');
                start = split;
            }
            preedit.push_str(&self.digits[start..]);
        }
        SessionSnapshot {
            scheme: SchemeType::Quanpin,
            local_mode: LocalInputMode::None,
            // The grid's snapshot stands in for the whole session's while it is composing, so it carries the mode too: a host that draws its English keys from this flag would otherwise put Chinese ones back on the first digit.
            dedicated_english: self.english_only,
            preedit,
            candidates: self.candidates.clone(),
            editing_text: self.digits.clone(),
            caret_position: self.digits.len(),
            nine_key_spellings: self.spellings.clone(),
            nine_key_reading: self.reading.clone(),
            candidate_sources: self.candidates.iter().map(|item| item.source).collect(),
            candidate_annotations: self
                .candidates
                .iter()
                .map(|item| item.corrected_from.clone())
                .collect(),
            // The same test `select` performs: consuming the candidate's digits empties the buffer exactly when its code covers everything typed.
            candidate_answers_key: self
                .candidates
                .iter()
                .map(|item| item.pinyin.len() >= self.digits.len())
                .collect(),
            ..SessionSnapshot::default()
        }
    }

    fn refresh(&mut self) {
        self.candidates.clear();
        self.spellings.clear();
        self.reading.clear();
        if !self.active() {
            return;
        }
        if self.english_only || !self.pinyin {
            // No syllables to offer and no pinyin to look up: the digits stand for letters only.
            self.candidates = self.english_candidates(false);
            return;
        }
        let table = spelling_table();
        let locked_length = self.locked_length();
        let remaining = remaining_digits(&self.digits, locked_length);
        let splits: Vec<usize> = self
            .splits
            .iter()
            .map(|split| split - locked_length)
            .collect();
        self.spellings = table.spellings_for(remaining, locked_length, splits.first().copied());
        let alternatives = if remaining.is_empty() {
            vec![Vec::new()]
        } else {
            let dictionary = self
                .dictionary
                .get_or_insert_with(|| QuanpinDictionary::new(&self.paths));
            let prior = self
                .prior
                .get_or_insert_with(|| SyllablePrior::from_dictionary(dictionary, table));
            let mut alternatives = table.split_paths(remaining, &splits, prior);
            // Even an unfinished or invalid tail must still offer the leading syllable for partial selection.
            alternatives.extend(
                self.spellings
                    .iter()
                    .filter(|spelling| spelling.len() <= remaining.len())
                    .map(|spelling| vec![spelling.clone()]),
            );
            alternatives
        };

        let mut locked_key = String::new();
        append_path_key(&mut locked_key, "", &self.locked);
        let mut dictionary = self
            .dictionary
            .get_or_insert_with(|| QuanpinDictionary::new(&self.paths))
            .row_cache_batch();
        let mut queried = (alternatives.len() > SMALL_QUERY_KEY_BATCH)
            .then(|| HashSet::with_capacity(alternatives.len()));
        let mut candidates = Vec::with_capacity(CANDIDATE_LIMIT);
        let mut key = String::with_capacity(locked_key.len() + self.digits.len() * 4 + 1);
        // 各条切分的前缀组彼此大量重复，一次刷新会推入上万行，见 `push_ranked`。
        let mut leading: HashMap<String, RankKey> = HashMap::with_capacity(CANDIDATE_LIMIT);
        // Only a split the user typed says where a syllable ends; without one, `3` must keep 的 (a completion of d) ahead of the rarer 额 (e).
        let prefer_exact = !self.splits.is_empty();
        for (index, path) in alternatives.iter().enumerate() {
            append_path_key(&mut key, &locked_key, path);
            if key.is_empty() || !query_key_is_new(&alternatives, index, &key, queried.as_ref()) {
                continue;
            }
            let full_len = self.locked.len() + path.len();
            for mut candidate in dictionary.query(&key, &key, 0, self.fuzzy) {
                let canonical = if candidate.canonical_pinyin.is_empty() {
                    candidate.pinyin.clone()
                } else {
                    candidate.canonical_pinyin.clone()
                };
                // A row with more syllables than the path is a completion past the typed digits.
                if canonical.matches('\'').count() >= full_len {
                    continue;
                }
                let matched = if candidate.fuzzy {
                    candidate.pinyin.as_str()
                } else {
                    canonical.as_str()
                };
                let code = encode(matched);
                if code.is_empty()
                    || (!code.starts_with(&self.digits) && !self.digits.starts_with(&code))
                {
                    continue;
                }
                if !agrees_with_locked(matched, &locked_key) {
                    continue;
                }
                candidate.pinyin = self.digits[..code.len().min(self.digits.len())].to_string();
                candidate.canonical_pinyin = canonical;
                push_ranked(&mut candidates, &mut leading, candidate, prefer_exact);
            }
            if let Some(seen) = queried.as_mut() {
                seen.insert(key.clone());
            }
        }
        drop(dictionary);
        rank_candidates(&mut candidates, prefer_exact);

        // 没有任何拼音读法时（77 拼不出音节），列表本来是空的，混输开关和最短前缀保护的「拼音列表的可读性」无从谈起；这时照样给英文九键词，否则 QQ 这类词只能切到全键盘去打。
        let unanswered = candidates.is_empty();
        let mut english = self.english_candidates(unanswered);
        if !english.is_empty() {
            // Second place is ahead of every pinyin reading but the first, which is worth it for a word the user is plainly spelling and not for one the frequency table has never seen; a zero-weight word still belongs in the list, at its end (NK:311-318).
            let first = english.remove(0);
            let slot = if first.weight > 0 {
                candidates.len().min(1)
            } else {
                candidates.len()
            };
            candidates.insert(slot, first);
            candidates.extend(english);
        }
        positions::apply_fixed_positions(
            &self.paths.user(assets::USER_JOURNAL),
            &self.ranking_context(),
            &mut candidates,
            false,
            None,
            false,
        );
        // Keep the most likely reading visible without requiring a horizontal scroll.
        if let Some(front) = candidates.first() {
            let offset = if locked_key.is_empty() {
                0
            } else {
                locked_key.len() + 1
            };
            if let Some(rest) = front.canonical_pinyin.get(offset..) {
                let preferred = rest.split('\'').next().unwrap_or_default();
                if let Some(found) = self.spellings.iter().position(|s| s == preferred) {
                    self.spellings[..=found].rotate_right(1);
                }
            }
        }
        self.reading = self.reading_for(candidates.first());
        self.candidates = candidates;
    }

    /// The leading row's pinyin cut to the digits it covers, then the uncovered digits with their splits: `xi'an` for 西安 over `94'26`, `yi'c` for 遗产 over `942`. Empty when an English word or nothing leads.
    fn reading_for(&self, front: Option<&WordItem>) -> String {
        let Some(front) = front else {
            return String::new();
        };
        // A fuzzy row was matched through another spelling (知 under `94` as zi), so its canonical letters do not line up one per typed digit.
        if front.fuzzy {
            return String::new();
        }
        if front.pinyin.is_empty() || !front.pinyin.bytes().all(|byte| byte.is_ascii_digit()) {
            return String::new();
        }
        let covered = front.pinyin.len();
        let mut reading = String::new();
        let mut letters = 0;
        for character in front.canonical_pinyin.chars() {
            if letters == covered {
                break;
            }
            if character.is_ascii_lowercase() {
                letters += 1;
            }
            reading.push(character);
        }
        let reading = reading.trim_end_matches('\'').to_string();
        if reading.is_empty() {
            return String::new();
        }
        let mut reading = reading;
        let mut start = covered.min(self.digits.len());
        if start < self.digits.len() {
            reading.push('\'');
            for &split in &self.splits {
                if split > start && split < self.digits.len() {
                    reading.push_str(&self.digits[start..split]);
                    reading.push('\'');
                    start = split;
                }
            }
            reading.push_str(&self.digits[start..]);
        }
        reading
    }

    /// `unanswered`：拼音一行候选都没有，英文词就是整个答案，和英文模式一样不受混输开关和最短前缀限制；锁定了拼音时仍不给。
    fn english_candidates(&mut self, unanswered: bool) -> Vec<WordItem> {
        // In English-only mode the words are the whole answer, so neither the mixed-candidate setting nor the prefix length that keeps a mixed list readable applies: one digit already narrows the alphabet enough (NK:169-178).
        if self.english_only {
            if self.digits.is_empty() {
                return Vec::new();
            }
        } else if unanswered {
            if self.digits.is_empty() || !self.locked.is_empty() {
                return Vec::new();
            }
        } else if !self.english_options.mixed_candidates
            || !self.locked.is_empty()
            || self.digits.len() < self.english_options.minimum_prefix
        {
            return Vec::new();
        }
        let digits = self.digits.clone();
        let Some(english) = self.open_english() else {
            return Vec::new();
        };
        let prefixes = letter_prefixes(&digits, ENGLISH_PREFIX_BUDGET);
        let capacity = prefixes.len().saturating_mul(ENGLISH_LIMIT);
        let mut words = Vec::with_capacity(capacity);
        for prefix in prefixes {
            for word in english.query_prefix(&prefix, ENGLISH_LIMIT) {
                // Only a whole code that starts with the digits counts; otherwise letters beyond the expanded prefix leak in.
                // The database lookup key is the lowercase spelling in `pinyin`; `word` is the
                // display form and may intentionally contain punctuation or spaces (for example
                // the custom entry `dont` displayed as `don't`).
                if !word_matches_digits(&word.pinyin, &digits) {
                    continue;
                }
                words.push(word);
            }
        }
        deduplicate_english_words(&mut words);
        rank_english(&mut words, digits.len());
        words
    }

    /// `msime-english.db` is optional; a missing file means no English rows and is looked for again on the next refresh (NK:179-186).
    fn open_english(&mut self) -> Option<&EnglishDictionary> {
        if self.english.is_none() {
            let path = self.paths.dictionary(assets::ENGLISH_DICTIONARY);
            if !path.exists() {
                return None;
            }
            self.english = Some(EnglishDictionary::open(&path, None, None));
        }
        self.english.as_ref()
    }

    fn locked_length(&self) -> usize {
        self.locked.iter().map(String::len).sum()
    }

    /// Drop the digits a selection consumed and every locked syllable they fully cover (NK:355-361). An English row's `pinyin` is the word, which can be longer than what was typed.
    fn consume(&mut self, count: usize) {
        let count = count.min(self.digits.len());
        self.digits.drain(..count);
        self.splits = self
            .splits
            .iter()
            .filter(|&&split| split > count)
            .map(|split| split - count)
            .collect();
        let mut consumed = count;
        while let Some(front) = self.locked.first() {
            if consumed < front.len() {
                break;
            }
            consumed -= front.len();
            self.locked.remove(0);
        }
    }

    fn ranking_context(&self) -> String {
        format!("nine-key:{}:{}", self.digits, self.locked.join("'"))
    }

    fn editable(&self, index: usize) -> bool {
        self.candidates
            .get(index)
            .is_some_and(|item| !item.canonical_pinyin.is_empty() && item.source.is_dictionary())
    }

    /// Learning and explicit management use canonical dictionary keys, never digit strings.
    fn adjust_frequency(&mut self, index: usize, force_top: bool) -> Option<String> {
        let item = &self.candidates[index];
        let main_db = self.paths.dictionary(assets::MAIN_DICTIONARY);
        let user_db = self.paths.user(assets::USER_JOURNAL);
        let context = self.ranking_context();
        let request = RankingRequest {
            main_db: &main_db,
            user_db: &user_db,
            context_key: &context,
            ordered: &self.candidates,
            entry_key: &item.canonical_pinyin,
            value: &item.word,
            mode: if force_top {
                FrequencyAdjustmentMode::Pin
            } else {
                self.frequency.mode
            },
            linear_step: self.frequency.linear_step,
            trigger_count: self.frequency.trigger_count,
            force_top,
            kind: PersonalDictionaryKind::Pinyin,
        };
        match ranking::adjust_candidate_ranking(&request) {
            Ok(changed) => {
                if changed {
                    if let Some(dictionary) = self.dictionary.as_mut() {
                        dictionary.reset_cache();
                    }
                }
                None
            }
            // A failed write keeps the commit and reports this one message (NK:409-414).
            Err(_) => Some(diagnostics::NINE_KEY_FREQUENCY_NOT_PERSISTED.to_string()),
        }
    }
}

fn remaining_digits(digits: &str, locked_length: usize) -> &str {
    &digits[locked_length..]
}

fn append_path_key(output: &mut String, prefix: &str, path: &[String]) {
    output.clear();
    output.push_str(prefix);
    for (index, part) in path.iter().enumerate() {
        if !prefix.is_empty() || index != 0 {
            output.push('\'');
        }
        output.push_str(part);
    }
}

/// A row read under the locked syllables must spell them, or be a whole-syllable prefix of them.
fn agrees_with_locked(matched: &str, locked_key: &str) -> bool {
    if locked_key.is_empty() || matched == locked_key {
        return true;
    }
    let under = matched
        .strip_prefix(locked_key)
        .is_some_and(|rest| rest.starts_with('\''));
    let over = locked_key
        .strip_prefix(matched)
        .is_some_and(|rest| rest.starts_with('\''));
    under || over
}

#[cfg(test)]
fn has_candidate_word(candidates: &[WordItem], word: &str) -> bool {
    candidates.iter().any(|candidate| candidate.word == word)
}

fn query_key_is_new(
    alternatives: &[Path],
    index: usize,
    key: &str,
    queried: Option<&HashSet<String>>,
) -> bool {
    match queried {
        Some(queried) => !queried.contains(key),
        None => !alternatives[..index]
            .iter()
            .any(|alternative| alternative == &alternatives[index]),
    }
}

/// `rank_candidates` 的排序键，小的在前。
type RankKey = (Reverse<usize>, bool, bool, bool, Reverse<i64>);

/// More digits covered first. Synthesised rows (whole-sentence Generated, Fallback) score on a different scale from dictionary weights, so within one coverage bucket dictionary rows lead; then exact before fuzzy, then weight.
/// With `prefer_exact` (the user typed a split), a row the typed digits spell to its end then leads one that has to be completed past them: over `94'26` 西安 (xi'an) comes before 自从 (zi'cong), however common the longer word. Without a split the digits do not say where a syllable ends, so `3` keeps 的 (de) ahead of the rarer 额 (e) by weight.
fn rank_key(item: &WordItem, prefer_exact: bool) -> RankKey {
    let completion = prefer_exact
        && item
            .canonical_pinyin
            .bytes()
            .filter(u8::is_ascii_lowercase)
            .count()
            > item.pinyin.len();
    (
        Reverse(item.pinyin.len()),
        item.source.is_generated_or_fallback(),
        completion,
        item.fuzzy,
        Reverse(comparable_weight(item)),
    )
}

/// 整句行的分是 `log_prob * 1000`，词组边每覆盖一个音节就带一份 `PHRASE_LENGTH_BONUS`。同一串音节里比较时这无关紧要，九宫格却要比较不同音节串各自解出的整句：音节越多、越能拼成词组的读法分越高，`9436364782662` 的 xi'e'meng'suan'ma（洗噩梦算吗）就会压过 zhe'neng'suan'ma（这能算吗）。这里按路径的音节数每个扣一份奖励：同一条路径内的先后不变，多切出来的音节要靠词本身的分量才能赢。只扣词组实际拿到的那部分反而更差（整句集 top-1 降了一截），见 `.agents/notes/implemented/bug-fix/2026-10-08-nine-key-path-ranking.md`。
fn comparable_weight(item: &WordItem) -> i64 {
    if item.source != CandidateSource::Generated || item.canonical_pinyin.is_empty() {
        return item.weight;
    }
    let syllables = item.canonical_pinyin.split('\'').count() as f64;
    item.weight - (PHRASE_LENGTH_BONUS * 1000.0 * syllables) as i64
}

/// 英文九键前缀展开最多产生 320 行；用栈上借用表和索引表去重，释放借用后再原地压缩。
fn deduplicate_english_words(words: &mut Vec<WordItem>) {
    assert!(words.len() <= ENGLISH_CANDIDATE_CAPACITY);
    let mut duplicates = [0usize; ENGLISH_CANDIDATE_CAPACITY];
    let mut duplicate_len = 0;
    {
        let mut seen: [Option<&str>; ENGLISH_CANDIDATE_CAPACITY] =
            [None; ENGLISH_CANDIDATE_CAPACITY];
        let mut seen_len = 0;
        for (index, word) in words.iter().enumerate() {
            if seen[..seen_len]
                .iter()
                .flatten()
                .any(|existing| *existing == word.word.as_str())
            {
                duplicates[duplicate_len] = index;
                duplicate_len += 1;
            } else {
                seen[seen_len] = Some(word.word.as_str());
                seen_len += 1;
            }
        }
    }
    let mut next_duplicate = 0;
    let mut write = 0;
    for read in 0..words.len() {
        if next_duplicate < duplicate_len && duplicates[next_duplicate] == read {
            next_duplicate += 1;
            continue;
        }
        if write != read {
            words.swap(write, read);
        }
        write += 1;
    }
    words.truncate(write);
}

/// 推入一行，除非同一个词已有一行排得不比它靠后。`leading` 记着每个词目前排得最靠前的那一行的排序键。被跳过的行在稳定排序后必然落在那一行之后（键更大，或键相同而推入更晚），会被 `retain_unique_words` 删掉；它也不会让别的行多删或少删，因为它能挡住的行那一行同样挡得住。所以跳过与全部推入再 `rank_candidates`，结果完全相同。前提是两边用同一个 `prefer_exact`。
fn push_ranked(
    candidates: &mut Vec<WordItem>,
    leading: &mut HashMap<String, RankKey>,
    candidate: WordItem,
    prefer_exact: bool,
) {
    let key = rank_key(&candidate, prefer_exact);
    match leading.get_mut(candidate.word.as_str()) {
        Some(best) if *best <= key => return,
        Some(best) => *best = key,
        None => {
            leading.insert(candidate.word.clone(), key);
        }
    }
    candidates.push(candidate);
}

/// Stable sort by `rank_key`, dedup by word, capped (NK:283-307).
fn rank_candidates(candidates: &mut Vec<WordItem>, prefer_exact: bool) {
    candidates.sort_by_key(|item| rank_key(item, prefer_exact));
    retain_unique_words(candidates);
    candidates.truncate(CANDIDATE_LIMIT);
}

fn retain_unique_words(candidates: &mut Vec<WordItem>) {
    if candidates.len() <= SMALL_CANDIDATE_DEDUP {
        let mut write = 0;
        for read in 0..candidates.len() {
            if candidates[..write]
                .iter()
                .any(|existing| existing.word == candidates[read].word)
            {
                continue;
            }
            if write != read {
                candidates.swap(write, read);
            }
            write += 1;
        }
        candidates.truncate(write);
        return;
    }
    let mut seen = HashSet::with_capacity(candidates.len());
    let duplicates = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, item)| (!seen.insert(item.word.as_str())).then_some(index))
        .collect::<Vec<_>>();
    drop(seen);
    let mut duplicates = duplicates.into_iter().peekable();
    let mut write = 0;
    for read in 0..candidates.len() {
        if duplicates.peek() == Some(&read) {
            duplicates.next();
            continue;
        }
        if write != read {
            candidates.swap(write, read);
        }
        write += 1;
    }
    candidates.truncate(write);
}

/// A word spelling the typed code exactly leads, but only when people type it: 64426 is 你好 and `ogham` is the only five-letter word those keys spell, so a zero-weight exact word gets no privilege. Then weight, then shorter (NK:199-220).
fn rank_english(words: &mut Vec<WordItem>, typed: usize) {
    let exact = |item: &WordItem| item.word.len() == typed && item.weight > 0;
    words.sort_by(|a, b| {
        exact(b)
            .cmp(&exact(a))
            .then_with(|| b.weight.cmp(&a.weight))
            .then_with(|| a.word.len().cmp(&b.word.len()))
    });
    words.truncate(ENGLISH_LIMIT);
}

/// The digit code of a pinyin spelling; anything but `a..=z` (apostrophes) is skipped.
fn encode(pinyin: &str) -> String {
    pinyin
        .bytes()
        .filter(u8::is_ascii_lowercase)
        .map(|letter| char::from(KEYPAD[usize::from(letter - b'a')]))
        .collect()
}

/// The digit code of an English word, ignoring case; a word with any non-letter has none.
#[cfg(test)]
fn digits_for_word(word: &str) -> String {
    let lowered = word.to_ascii_lowercase();
    if !lowered.bytes().all(|byte| byte.is_ascii_lowercase()) {
        return String::new();
    }
    encode(&lowered)
}

fn word_matches_digits(word: &str, digits: &str) -> bool {
    let mut matched = 0;
    for byte in word.bytes() {
        let letter = byte.to_ascii_lowercase();
        if !letter.is_ascii_lowercase() {
            return false;
        }
        if let Some(&digit) = digits.as_bytes().get(matched) {
            if KEYPAD[usize::from(letter - b'a')] != digit {
                return false;
            }
            matched += 1;
        }
    }
    matched == digits.len()
}

fn letters_for_digit(digit: u8) -> &'static str {
    if (b'2'..=b'9').contains(&digit) {
        DIGIT_LETTERS[usize::from(digit - b'0')]
    } else {
        ""
    }
}

/// Every letter prefix of the leading digits, expanded while the count stays within `budget`.
fn letter_prefixes(digits: &str, budget: usize) -> Vec<String> {
    let mut prefixes = vec![String::new()];
    for digit in digits.bytes() {
        let letters = letters_for_digit(digit);
        if letters.is_empty() || prefixes.len() * letters.len() > budget {
            break;
        }
        prefixes = prefixes
            .iter()
            .flat_map(|prefix| {
                letters
                    .chars()
                    .map(move |letter| format!("{prefix}{letter}"))
            })
            .collect();
    }
    if prefixes.len() == 1 && prefixes[0].is_empty() {
        Vec::new()
    } else {
        prefixes
    }
}

/// The syllables and spelling pieces digits can stand for, built once from the intact syllable list.
struct SpellingTable {
    /// Every complete syllable with its code, in list order.
    syllables: Vec<(String, String)>,
    intact: HashSet<String>,
    /// Every prefix of every syllable, grouped by its code. Partial syllables are included, but never letter combinations pinyin does not have (NK:52-68).
    by_code: HashMap<String, Vec<String>>,
    longest_code: usize,
}

fn spelling_table() -> &'static SpellingTable {
    static TABLE: OnceLock<SpellingTable> = OnceLock::new();
    TABLE.get_or_init(|| SpellingTable::new(intact_pinyin_list()))
}

impl SpellingTable {
    fn new(syllables: &[&str]) -> Self {
        let prefix_capacity = syllables.iter().map(|syllable| syllable.len()).sum();
        let mut seen = HashSet::with_capacity(prefix_capacity);
        let mut by_code: HashMap<String, Vec<String>> = HashMap::with_capacity(prefix_capacity);
        let mut longest_code = 0;
        for syllable in syllables {
            for end in 1..=syllable.len() {
                let Some(prefix) = syllable.get(..end) else {
                    continue;
                };
                if seen.insert(prefix.to_string()) {
                    let code = encode(prefix);
                    longest_code = longest_code.max(code.len());
                    by_code.entry(code).or_default().push(prefix.to_string());
                }
            }
        }
        Self {
            syllables: syllables
                .iter()
                .map(|syllable| (syllable.to_string(), encode(syllable)))
                .collect(),
            intact: syllables
                .iter()
                .map(|syllable| syllable.to_string())
                .collect(),
            by_code,
            longest_code,
        }
    }

    /// Complete syllables the unlocked digits can start with, or that complete them, longest covered first (NK:238-250). Coverage is counted in digits: comparing letter counts would put a syllable that needs two digits ahead under the same digit prefix.
    /// 有切分时，只有在切分处或之前结束的音节才算。
    fn spellings_for(
        &self,
        remaining: &str,
        locked_length: usize,
        split: Option<usize>,
    ) -> Vec<String> {
        if remaining.is_empty() {
            return Vec::new();
        }
        let mut matches: Vec<&(String, String)> = self
            .syllables
            .iter()
            .filter(|(_, code)| match split {
                Some(split) => code.len() <= split && remaining.starts_with(code.as_str()),
                None => {
                    locked_length + remaining.len().max(code.len()) <= DIGIT_LIMIT
                        && (remaining.starts_with(code.as_str()) || code.starts_with(remaining))
                }
            })
            .collect();
        let covered = |code: &str| code.len().min(remaining.len());
        matches.sort_by(|(a, a_code), (b, b_code)| {
            covered(b_code).cmp(&covered(a_code)).then_with(|| a.cmp(b))
        });
        matches.into_iter().map(|(text, _)| text.clone()).collect()
    }

    /// Syllable paths spelling `digits`, built from the end. A piece may end a path early only if it is a complete syllable; each offset keeps the 48 best by fewer syllables, complete last syllable, then lexicographic (NK:122-156). 不带频度，测试用。
    #[cfg(test)]
    fn paths(&self, digits: &str) -> Vec<Path> {
        self.split_paths(digits, &[], &SyllablePrior::default())
    }

    /// 和 `paths` 一样，但每个 `splits` 位置（`digits` 里的下标）都必须有一个音节在那里结束，而且这个音节必须完整；同样音节数、同样收尾的路径按 `prior` 给的频度之和排，频度相同才按字典序。
    ///
    /// 每个位置只留 48 条，留哪些就决定了哪些读法还能查到。原先在字典序上截断，而 9 键是 wxyz、7 键是 pqrs，于是长串里 xi 开头的路径把 yi、zhe 开头的全部挤掉，`943426943426` 只剩「洗点一点」查不到「一点一点」，`7264685487` 只剩「盘后」查不到「然后」。
    fn split_paths(&self, digits: &str, splits: &[usize], prior: &SyllablePrior) -> Vec<Path> {
        let length = digits.len();
        let mut suffix: Vec<Vec<(f64, Path)>> = vec![Vec::new(); length + 1];
        suffix[length].push((0.0, Vec::new()));
        for offset in (0..length).rev() {
            let mut result = Vec::with_capacity(PATH_LIMIT);
            for end in offset + 1..=length.min(offset + self.longest_code) {
                if splits.iter().any(|&split| offset < split && split < end) {
                    break;
                }
                let Some(pieces) = self.by_code.get(&digits[offset..end]) else {
                    continue;
                };
                let must_complete = end != length || splits.contains(&end);
                for piece in pieces {
                    if must_complete && !self.intact.contains(piece) {
                        continue;
                    }
                    let score = prior.score(piece);
                    for (tail_score, tail) in &suffix[end] {
                        let mut path = Vec::with_capacity(tail.len() + 1);
                        path.push(piece.clone());
                        path.extend(tail.iter().cloned());
                        result.push((score + tail_score, path));
                    }
                }
            }
            // Pieces are visited grouped by code rather than in list order; the order below is total on distinct paths, so the kept 48 are the same either way.
            let complete = |path: &Path| path.last().is_some_and(|last| self.intact.contains(last));
            result.sort_by(|(a_score, a), (b_score, b)| {
                a.len()
                    .cmp(&b.len())
                    .then_with(|| complete(b).cmp(&complete(a)))
                    .then_with(|| b_score.total_cmp(a_score))
                    .then_with(|| a.cmp(b))
            });
            result.truncate(PATH_LIMIT);
            suffix[offset] = result;
        }
        suffix
            .swap_remove(0)
            .into_iter()
            .map(|(_, path)| path)
            .collect()
    }
}

/// 音节和音节前缀的单字频度：音节取词库里它最常用那个字的权重取对数，前缀取以它开头的音节里最高的那个。九宫格只用它决定每个位置留哪 48 条切分路径，候选本身的先后仍由词库和整句解码决定。
#[derive(Default)]
struct SyllablePrior {
    scores: HashMap<String, f64>,
}

impl SyllablePrior {
    fn from_dictionary(dictionary: &QuanpinDictionary, table: &SpellingTable) -> Self {
        let keys: Vec<String> = table
            .syllables
            .iter()
            .map(|(syllable, _)| syllable.clone())
            .collect();
        let weights = dictionary.best_weights(&keys);
        Self::from_weights(&table.syllables, &weights)
    }

    fn from_weights(syllables: &[(String, String)], weights: &HashMap<String, i64>) -> Self {
        let total: f64 = syllables
            .iter()
            .map(|(syllable, _)| weights.get(syllable).copied().unwrap_or(0).max(1) as f64)
            .sum();
        let mut scores: HashMap<String, f64> = HashMap::new();
        for (syllable, _) in syllables {
            // 没有单字行的音节和权重为 0 的一样按 1 算。
            let weight = weights.get(syllable).copied().unwrap_or(0).max(1) as f64;
            let score = (weight / total).ln();
            for end in 1..=syllable.len() {
                let Some(prefix) = syllable.get(..end) else {
                    continue;
                };
                let slot = scores.entry(prefix.to_string()).or_insert(score);
                if *slot < score {
                    *slot = score;
                }
            }
        }
        Self { scores }
    }

    fn score(&self, piece: &str) -> f64 {
        self.scores.get(piece).copied().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CandidateSource;
    use rusqlite::Connection;

    // ---- Pure helpers ----

    #[test]
    fn codes_follow_the_keypad() {
        assert_eq!(encode("ni'hao"), "64426");
        assert_eq!(encode("zhuang"), "948264");
        assert_eq!(digits_for_word("OK"), "65");
        assert_eq!(digits_for_word("ogham"), "64426");
        assert_eq!(digits_for_word("don't"), "");
        assert_eq!(digits_for_word("café"), "");
        assert!(word_matches_digits("OK", "65"));
        assert!(word_matches_digits("ogham", "64426"));
        assert!(!word_matches_digits("ogham", "64427"));
        assert!(!word_matches_digits("don't", "3668"));
        assert!(!word_matches_digits("ogham", "644260"));
    }

    #[test]
    fn letter_prefixes_stop_at_the_budget() {
        assert_eq!(letter_prefixes("6", 64), ["m", "n", "o"]);
        let prefixes = letter_prefixes("64426", 64);
        assert_eq!(prefixes.len(), 27);
        assert_eq!(prefixes.first().map(String::as_str), Some("mgg"));
        assert_eq!(prefixes.last().map(String::as_str), Some("oii"));
        assert_eq!(letter_prefixes("79", 64).len(), 16);
        assert!(letter_prefixes("1", 64).is_empty());
        assert!(letter_prefixes("", 64).is_empty());
    }

    #[test]
    fn paths_prefer_fewer_syllables_then_a_complete_tail() {
        let table = SpellingTable::new(&["ni", "mi", "o", "ma", "hao", "gao"]);
        // Fewer syllables first; an incomplete piece (`g`, `h`) may only end the path.
        assert_eq!(
            table.paths("64"),
            [vec!["mi"], vec!["ni"], vec!["o", "g"], vec!["o", "h"]]
        );
        // `o` is a syllable, `m` and `n` are only pieces of one: a complete tail comes first at equal length.
        assert_eq!(table.paths("6"), [vec!["o"], vec!["m"], vec!["n"]]);
        let paths = table.paths("64426");
        assert_eq!(
            paths[..4],
            [
                vec!["mi", "gao"],
                vec!["mi", "hao"],
                vec!["ni", "gao"],
                vec!["ni", "hao"]
            ]
        );
        // A piece may only end the path: `ni'ha'o` would need `ha`, which is not a syllable here.
        assert_eq!(paths.len(), 4);
        assert!(table.paths("11").is_empty());
    }

    #[test]
    fn paths_keep_at_most_the_limit_per_offset() {
        let table = SpellingTable::new(&["m", "n", "o"]);
        let paths = table.paths("66666");
        assert_eq!(paths.len(), PATH_LIMIT);
        assert_eq!(paths[0], ["m", "m", "m", "m", "m"]);
        assert!(paths.iter().all(|path| path.len() == 5));
    }

    #[test]
    fn the_limit_keeps_frequent_syllables_rather_than_early_letters() {
        let table = SpellingTable::new(&["xi", "yi", "dian"]);
        let weights = HashMap::from([
            ("xi".to_string(), 1_000),
            ("yi".to_string(), 1_000_000),
            ("dian".to_string(), 10_000),
        ]);
        let prior = SyllablePrior::from_weights(&table.syllables, &weights);
        // 94 3426 重复七次，每个位置都要截到 48 条：按字典序截断时第一个音节全是 xi，一点一点……不在里面。
        let digits = "943426".repeat(7);
        let paths = table.split_paths(&digits, &[], &prior);
        assert_eq!(paths.len(), PATH_LIMIT);
        assert_eq!(paths[0], ["yi", "dian"].repeat(7));
        assert!(table
            .split_paths(&digits, &[], &SyllablePrior::default())
            .iter()
            .all(|path| path[0] == "xi"));
    }

    #[test]
    fn a_prefix_scores_as_its_best_syllable() {
        let table = SpellingTable::new(&["shi", "si", "ran"]);
        let weights = HashMap::from([("shi".to_string(), 5_000), ("si".to_string(), 50)]);
        let prior = SyllablePrior::from_weights(&table.syllables, &weights);
        // 分母是各音节权重之和，没有单字行的 ran 按 1 算：5000 + 50 + 1。
        let total = 5_051f64;
        assert_eq!(prior.score("s"), (5_000f64 / total).ln());
        assert_eq!(prior.score("sh"), (5_000f64 / total).ln());
        assert_eq!(prior.score("si"), (50f64 / total).ln());
        assert_eq!(prior.score("ran"), (1f64 / total).ln());
    }

    #[test]
    fn sentences_from_different_paths_compare_without_the_phrase_bonus() {
        let bonus = (PHRASE_LENGTH_BONUS * 1000.0) as i64;
        // 五个音节的读法每个音节都在词组里，多拿一份奖励；扣掉之后四个音节的读法分更高就该排在前面。
        let mut longer = item(
            "洗噩梦算吗",
            "9436364782662",
            5 * bonus - 30_000,
            CandidateSource::Generated,
        );
        longer.canonical_pinyin = "xi'e'meng'suan'ma".into();
        let mut shorter = item(
            "这能算吗",
            "9436364782662",
            4 * bonus - 20_000,
            CandidateSource::Generated,
        );
        shorter.canonical_pinyin = "zhe'neng'suan'ma".into();
        assert!(longer.weight > shorter.weight);
        assert_eq!(
            ranked(vec![longer, shorter], false),
            ["这能算吗", "洗噩梦算吗"]
        );
        // 词库行不带这份奖励，按原权重比。
        let mut word = item("西安", "9426", 500, CandidateSource::Database);
        word.canonical_pinyin = "xi'an".into();
        assert_eq!(comparable_weight(&word), 500);
    }

    #[test]
    fn query_key_scan_uses_no_temporary_heap_state() {
        let alternatives = vec![vec!["ni".to_owned(), "hao".to_owned()]];
        let (found, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            query_key_is_new(&alternatives, 0, "ni'hao", None)
        });

        assert!(found);
        assert_eq!(allocations, 0);
    }

    #[test]
    fn spellings_sort_by_digits_covered_within_the_limit() {
        let table = SpellingTable::new(&[
            "ga", "gan", "gang", "gao", "ha", "han", "hang", "hao", "ni", "a", "ai",
        ]);
        assert_eq!(
            table.spellings_for("426", 2, None),
            ["gan", "gang", "gao", "han", "hang", "hao", "ga", "ha"]
        );
        // With 31 digits already locked, only a one-digit completion still fits in 32.
        assert_eq!(table.spellings_for("2", 31, None), ["a"]);
        assert!(table.spellings_for("", 0, None).is_empty());
        // 在两个数字之后切开，就排除了所有跨过这个位置的音节。
        assert_eq!(table.spellings_for("426", 0, Some(2)), ["ga", "ha"]);
    }

    fn item(word: &str, digits: &str, weight: i64, source: CandidateSource) -> WordItem {
        WordItem::new(digits, word, weight, source, "")
    }

    #[test]
    fn refresh_tail_is_borrowed_from_digits() {
        assert_eq!(remaining_digits("64426", 2), "426");
    }

    #[test]
    fn path_key_builder_reuses_existing_string_storage() {
        let path = vec!["hao".to_owned(), "ma".to_owned()];
        let mut key = String::with_capacity(32);
        append_path_key(&mut key, "ni", &path);
        assert_eq!(key, "ni'hao'ma");

        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            append_path_key(&mut key, "ni", &path);
        });

        assert_eq!(key, "ni'hao'ma");
        assert_eq!(allocations, 0);
    }

    #[test]
    fn candidate_word_lookup_scans_existing_rows() {
        let candidates = vec![item("old", "653", 1, CandidateSource::EnglishDictionary)];
        assert!(has_candidate_word(&candidates, "old"));
        assert!(!has_candidate_word(&candidates, "older"));
    }

    #[test]
    fn unique_word_retain_keeps_the_first_sorted_row() {
        let mut candidates = vec![
            item("你", "644", 10, CandidateSource::Database),
            item("你", "64", 1, CandidateSource::Generated),
            item("泥", "64", 2, CandidateSource::Database),
        ];
        retain_unique_words(&mut candidates);
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.word.as_str())
                .collect::<Vec<_>>(),
            ["你", "泥"]
        );
    }

    #[test]
    fn short_unique_word_retain_avoids_temporary_heap_state() {
        let mut candidates = vec![
            item("你", "644", 10, CandidateSource::Database),
            item("你", "64", 1, CandidateSource::Generated),
            item("泥", "64", 2, CandidateSource::Database),
        ];
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            retain_unique_words(&mut candidates);
        });

        assert_eq!(allocations, 0);
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.word.as_str())
                .collect::<Vec<_>>(),
            ["你", "泥"]
        );
    }

    #[test]
    fn query_key_lookup_deduplicates_paths_and_hashed_keys() {
        let alternatives = vec![
            vec!["ni".to_owned(), "hao".to_owned()],
            vec!["ni".to_owned(), "hao".to_owned()],
        ];
        assert!(query_key_is_new(&alternatives, 0, "ni'hao", None));
        assert!(!query_key_is_new(&alternatives, 1, "ni'hao", None));

        let mut queried = HashSet::new();
        assert!(query_key_is_new(&alternatives, 0, "ni'hao", Some(&queried)));
        queried.insert("ni'hao".to_owned());
        assert!(!query_key_is_new(
            &alternatives,
            1,
            "ni'hao",
            Some(&queried)
        ));
    }

    #[test]
    fn query_key_dedup_switches_to_hashing_after_small_batch_boundary() {
        let alternatives = (0..=SMALL_QUERY_KEY_BATCH)
            .map(|index| vec![format!("syllable-{}", index % 32)])
            .collect::<Vec<_>>();
        let mut queried = (alternatives.len() > SMALL_QUERY_KEY_BATCH)
            .then(|| HashSet::with_capacity(alternatives.len()));
        let mut unique = 0;
        for (index, path) in alternatives.iter().enumerate() {
            let key = path[0].as_str();
            if query_key_is_new(&alternatives, index, key, queried.as_ref()) {
                unique += 1;
                if let Some(seen) = queried.as_mut() {
                    seen.insert(key.to_owned());
                }
            }
        }
        assert_eq!(unique, 32);
        assert!(queried.is_some());
    }

    #[test]
    fn candidates_rank_by_coverage_then_dictionary_rows() {
        let mut fuzzy = item("泥", "64", 900, CandidateSource::Database);
        fuzzy.fuzzy = true;
        let mut candidates = vec![
            item("你", "64", 100, CandidateSource::Database),
            item("米好", "64426", -19113, CandidateSource::Generated),
            fuzzy,
            item("米", "64", 50, CandidateSource::UserDatabase),
            item("你好", "64426", 1000, CandidateSource::Database),
            item("你", "64", 10, CandidateSource::Database),
        ];
        rank_candidates(&mut candidates, false);
        let words: Vec<_> = candidates.iter().map(|item| item.word.as_str()).collect();
        assert_eq!(words, ["你好", "米好", "你", "米", "泥"]);
        assert_eq!(candidates[2].weight, 100);
    }

    fn spelled(
        word: &str,
        digits: &str,
        canonical: &str,
        weight: i64,
        source: CandidateSource,
    ) -> WordItem {
        let mut row = item(word, digits, weight, source);
        row.canonical_pinyin = canonical.to_owned();
        row
    }

    fn ranked(mut candidates: Vec<WordItem>, prefer_exact: bool) -> Vec<String> {
        rank_candidates(&mut candidates, prefer_exact);
        candidates.into_iter().map(|item| item.word).collect()
    }

    #[test]
    fn without_a_split_a_completion_keeps_its_weight_over_an_exact_syllable() {
        // `3` alone: 的 completes d to de, 额 spells e exactly; the more common word still leads.
        let rows = vec![
            spelled("额", "3", "e", 10, CandidateSource::Database),
            spelled("的", "3", "de", 1000, CandidateSource::Database),
        ];
        assert_eq!(ranked(rows, false), ["的", "额"]);
    }

    #[test]
    fn a_synthesised_exact_row_never_leads_a_dictionary_completion() {
        for prefer_exact in [false, true] {
            let rows = vec![
                spelled("额", "3", "e", 99_999, CandidateSource::Generated),
                spelled("的", "3", "de", 1000, CandidateSource::Database),
            ];
            assert_eq!(
                ranked(rows, prefer_exact),
                ["的", "额"],
                "prefer_exact {prefer_exact}"
            );
        }
    }

    #[test]
    fn after_a_split_an_exact_reading_leads_a_completion() {
        // `94'26`: 西安 spells xi'an to its end, 自从 has to be completed to zi'cong.
        let rows = vec![
            spelled("自从", "9426", "zi'cong", 5000, CandidateSource::Database),
            spelled("西安", "9426", "xi'an", 100, CandidateSource::Database),
        ];
        assert_eq!(ranked(rows.clone(), true), ["西安", "自从"]);
        assert_eq!(ranked(rows, false), ["自从", "西安"]);
    }

    #[test]
    fn skipping_outranked_rows_ranks_like_pushing_everything() {
        let sources = [
            CandidateSource::Database,
            CandidateSource::UserDatabase,
            CandidateSource::Generated,
            CandidateSource::Fallback,
        ];
        let mut state = 0x2545_F491_4F6C_DD1D_u64;
        let mut next = |bound: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % bound
        };
        for round in 0..200 {
            let mut rows = Vec::new();
            for _ in 0..(1 + next(300)) {
                let digits = &"64426646"[..1 + next(8) as usize];
                let mut row = item(
                    &format!("w{}", next(40)),
                    digits,
                    next(5) as i64 * 10,
                    sources[next(4) as usize],
                );
                row.fuzzy = next(3) == 0;
                row.canonical_pinyin =
                    ["e", "de", "xi'an", "zi'cong", "ni'hao'ma"][next(5) as usize].to_owned();
                rows.push(row);
            }
            let prefer_exact = round % 2 == 0;
            let mut everything = rows.clone();
            rank_candidates(&mut everything, prefer_exact);
            let mut skipped = Vec::new();
            let mut leading = HashMap::new();
            for row in rows {
                push_ranked(&mut skipped, &mut leading, row, prefer_exact);
            }
            rank_candidates(&mut skipped, prefer_exact);
            assert_eq!(skipped, everything, "round {round}");
        }
    }

    #[test]
    fn english_exact_code_leads_only_with_a_weight() {
        let english =
            |word: &str, weight| item(word, word, weight, CandidateSource::EnglishDictionary);
        let mut words = vec![
            english("old", 1000),
            english("ok", 900),
            english("older", 800),
        ];
        rank_english(&mut words, 2);
        let order: Vec<_> = words.iter().map(|item| item.word.as_str()).collect();
        assert_eq!(order, ["ok", "old", "older"]);
        let mut words = vec![english("ogham", 0), english("oh", 5), english("ogre", 5)];
        rank_english(&mut words, 5);
        let order: Vec<_> = words.iter().map(|item| item.word.as_str()).collect();
        assert_eq!(order, ["oh", "ogre", "ogham"]);
        let mut many: Vec<_> = (0..8).map(|n| english(&format!("w{n}"), n)).collect();
        rank_english(&mut many, 2);
        assert_eq!(many.len(), ENGLISH_LIMIT);
    }

    #[test]
    fn english_dedup_keeps_the_first_row_for_each_display_word() {
        let mut words = vec![
            item("one", "one", 100, CandidateSource::EnglishDictionary),
            item("two", "two", 90, CandidateSource::EnglishDictionary),
            item("one", "one", 80, CandidateSource::EnglishDictionary),
        ];

        deduplicate_english_words(&mut words);

        assert_eq!(
            words
                .iter()
                .map(|word| (word.word.as_str(), word.weight))
                .collect::<Vec<_>>(),
            [("one", 100), ("two", 90)]
        );
    }

    #[test]
    fn locked_syllables_constrain_readings() {
        assert!(agrees_with_locked("mi", ""));
        assert!(agrees_with_locked("ni", "ni"));
        assert!(agrees_with_locked("ni'hao", "ni"));
        assert!(agrees_with_locked("ni", "ni'hao"));
        assert!(!agrees_with_locked("mi", "ni"));
        assert!(!agrees_with_locked("nian", "ni"));
        assert!(!agrees_with_locked("ni", "nian"));
    }

    fn detached() -> NineKeySession {
        NineKeySession::new(
            &RuntimePaths::default(),
            false,
            FrequencyAdjustmentOptions::default(),
            FuzzyPinyinOptions::default(),
            EnglishInputOptions::default(),
            true,
        )
    }

    #[test]
    fn consuming_digits_drops_covered_locked_syllables() {
        let mut session = detached();
        session.digits = "64426".into();
        session.locked = vec!["ni".into(), "hao".into()];
        session.consume(2);
        assert_eq!(session.digits, "426");
        assert_eq!(session.locked, ["hao"]);
        // An English row's code is the word, which may run past the digits.
        session.consume(10);
        assert!(session.digits.is_empty());
        assert!(session.locked.is_empty());
    }

    #[test]
    fn snapshot_shows_locked_syllables_before_the_open_digits() {
        let mut session = detached();
        session.digits = "64426".into();
        assert_eq!(session.snapshot().preedit, "64426");
        session.locked = vec!["ni".into()];
        assert_eq!(session.snapshot().preedit, "ni'426");
        session.locked.push("hao".into());
        session.candidates = vec![
            item("你好", "64426", 1000, CandidateSource::Database),
            item("你", "64", 100, CandidateSource::Database),
        ];
        let view = session.snapshot();
        assert_eq!(view.preedit, "ni'hao");
        assert_eq!(view.editing_text, "64426");
        assert_eq!(view.caret_position, 5);
        assert_eq!(view.scheme, SchemeType::Quanpin);
        assert_eq!(view.candidate_answers_key, [true, false]);
        assert_eq!(view.candidate_sources, [CandidateSource::Database; 2]);
        assert_eq!(view.candidate_annotations, ["", ""]);
        assert_eq!(session.ranking_context(), "nine-key:64426:ni'hao");
    }

    // ---- Ported from test_nine_key_session.cpp against the session itself ----

    const MAIN_FIXTURE: &str = "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_n VALUES('ni','n','你',100);CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_m VALUES('mi','m','米',50);CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_h VALUES('hao','h','好',100);CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000);";
    const ENGLISH_FIXTURE: &str = "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);INSERT INTO english_words VALUES('ok','ok',900);INSERT INTO english_words VALUES('old','old',1000);INSERT INTO english_words VALUES('older','older',800);INSERT INTO english_words VALUES('ogham','ogham',0);INSERT INTO english_words VALUES('qq','QQ',500);";

    struct Fixture {
        directory: tempfile::TempDir,
        paths: RuntimePaths,
    }

    fn fixture() -> Fixture {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().to_path_buf();
        Connection::open(root.join(assets::MAIN_DICTIONARY))
            .and_then(|db| db.execute_batch(MAIN_FIXTURE))
            .expect("main fixture");
        Connection::open(root.join(assets::ENGLISH_DICTIONARY))
            .and_then(|db| db.execute_batch(ENGLISH_FIXTURE))
            .expect("english fixture");
        let paths = RuntimePaths {
            resources: root.clone(),
            user_data: root.clone(),
            cache: root.clone(),
            dictionaries: root,
        };
        Fixture { directory, paths }
    }

    fn open(paths: &RuntimePaths, learning: bool, english: EnglishInputOptions) -> NineKeySession {
        let frequency = FrequencyAdjustmentOptions {
            mode: if learning {
                FrequencyAdjustmentMode::Promote
            } else {
                FrequencyAdjustmentMode::Disabled
            },
            ..FrequencyAdjustmentOptions::default()
        };
        NineKeySession::new(
            paths,
            learning,
            frequency,
            FuzzyPinyinOptions::default(),
            english,
            true,
        )
    }

    fn mixed() -> EnglishInputOptions {
        EnglishInputOptions {
            mixed_candidates: true,
            ..EnglishInputOptions::default()
        }
    }

    fn type_digits(session: &mut NineKeySession, digits: &str) {
        for digit in digits.bytes() {
            assert!(session.character(digit).handled, "digit {digit} unhandled");
        }
    }

    fn index_of(session: &NineKeySession, word: &str) -> usize {
        session
            .snapshot()
            .candidates
            .iter()
            .position(|item| item.word == word)
            .unwrap_or_else(|| panic!("missing candidate {word}"))
    }

    fn words(session: &NineKeySession) -> Vec<String> {
        session
            .snapshot()
            .candidates
            .into_iter()
            .map(|item| item.word)
            .collect()
    }

    #[test]
    fn digits_offer_readings_and_ranked_english() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        assert!(!session.character(b'0').handled && !session.character(b'1').handled);
        // old is more frequent, but 65 spells ok exactly.
        type_digits(&mut session, "65");
        assert_eq!(words(&session), ["ok", "old", "older"]);
        assert_eq!(session.snapshot().nine_key_spellings, ["o"]);
        session.command(Command::Cancel);

        // 64426 is ni'hao and also the only code for ogham; spelling it exactly does not earn a zero-weight word the second slot. The two-syllable lattice adds a Generated 米好 that covers every digit, so it ranks above the two-digit dictionary rows (tests-inventory.md §2.4).
        type_digits(&mut session, "64426");
        let view = session.snapshot();
        assert_eq!(words(&session), ["你好", "米好", "你", "米", "ogham"]);
        assert_eq!(view.candidate_sources[1], CandidateSource::Generated);
        assert_eq!(view.nine_key_spellings, ["ni", "mi", "o"]);
        assert_eq!(view.candidate_answers_key[..4], [true, true, false, false]);
        // Within one coverage bucket no dictionary row follows a synthesised one.
        for pair in view.candidates.windows(2) {
            if pair[0].pinyin.len() == pair[1].pinyin.len()
                && pair[0].source.is_generated_or_fallback()
            {
                assert!(
                    !pair[1].source.is_dictionary(),
                    "{} after {}",
                    pair[1].word,
                    pair[0].word
                );
            }
        }
        session.command(Command::Cancel);

        type_digits(&mut session, "6");
        assert_eq!(
            words(&session),
            ["你", "米"],
            "mixed English waits for the minimum prefix"
        );
        assert_eq!(session.snapshot().candidates[0].pinyin, "6");
    }

    #[test]
    fn english_t9_uses_the_lookup_word_when_display_has_punctuation() {
        let fixture = fixture();
        Connection::open(fixture.paths.dictionary(assets::ENGLISH_DICTIONARY))
            .unwrap()
            .execute(
                "INSERT INTO english_words(word, display, weight) VALUES ('dont', 'don''t', 100)",
                [],
            )
            .unwrap();
        let mut session = open(
            &fixture.paths,
            false,
            EnglishInputOptions {
                mixed_candidates: false,
                ..EnglishInputOptions::default()
            },
        );
        session.set_english_only(true);
        type_digits(&mut session, "3668");
        assert!(
            words(&session).contains(&"don't".to_owned()),
            "T9 should match the lookup key even when the displayed word contains punctuation"
        );
    }

    #[test]
    fn english_digits_take_no_split() {
        let fixture = fixture();
        let mut english_only = open(&fixture.paths, false, mixed());
        english_only.set_english_only(true);
        let mut without_pinyin = NineKeySession::new(
            &fixture.paths,
            false,
            FrequencyAdjustmentOptions::default(),
            FuzzyPinyinOptions::default(),
            mixed(),
            false,
        );
        for session in [&mut english_only, &mut without_pinyin] {
            type_digits(session, "65");
            assert!(!session.character(b'\'').handled);
            assert!(session.splits.is_empty());
            assert_eq!(session.snapshot().preedit, "65");
        }
    }

    #[test]
    fn a_fuzzy_leading_row_has_no_reading() {
        let mut session = detached();
        session.digits = "94".into();
        let xi = spelled("西", "94", "xi", 100, CandidateSource::Database);
        assert_eq!(session.reading_for(Some(&xi)), "xi");
        // 知 reached through zi under `94`: cutting zhi to two letters would show zh.
        let mut zhi = spelled("知", "94", "zhi", 100, CandidateSource::Database);
        zhi.fuzzy = true;
        assert_eq!(session.reading_for(Some(&zhi)), "");
    }

    #[test]
    fn split_keeps_both_sides_open_and_backspace_removes_it_first() {
        let table = SpellingTable::new(&["xi", "yi", "an", "xian", "yan"]);
        assert_eq!(
            table
                .split_paths("9426", &[], &SyllablePrior::default())
                .first(),
            Some(&vec!["xian".to_string()])
        );
        let split = table.split_paths("9426", &[2], &SyllablePrior::default());
        assert!(!split.is_empty());
        assert!(
            split.iter().all(|path| path.len() == 2 && path[1] == "an"),
            "{split:?}"
        );

        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        assert!(!session.character(b'\'').handled, "nothing to split");
        type_digits(&mut session, "64");
        assert!(session.character(b'\'').handled);
        assert!(
            session.character(b'\'').handled,
            "a repeated split is absorbed"
        );
        assert_eq!(session.snapshot().preedit, "64'");
        assert!(
            session
                .snapshot()
                .nine_key_spellings
                .iter()
                .all(|s| s.len() <= 2),
            "no syllable runs past the split"
        );
        type_digits(&mut session, "426");
        assert_eq!(session.snapshot().preedit, "64'426");
        assert_eq!(words(&session).first().map(String::as_str), Some("你好"));
        assert_eq!(session.snapshot().nine_key_reading, "ni'hao");
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot().preedit, "64'42");
        type_digits(&mut session, "6");
        let result = session.select(index_of(&session, "你"));
        assert_eq!(result.commit.as_deref(), Some("你"));
        assert_eq!(
            session.snapshot().preedit,
            "426",
            "the split went with the consumed digits"
        );

        session.command(Command::Cancel);
        type_digits(&mut session, "64");
        session.character(b'\'');
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(
            session.snapshot().preedit,
            "64",
            "Backspace takes the split before a digit"
        );
    }

    #[test]
    fn spellings_lock_and_partial_selection_advances() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64");
        let before = session.snapshot();
        assert!(!session.select(999).handled && !session.choose_spelling(999).handled);
        assert_eq!(session.snapshot(), before);
        assert_eq!(
            before.nine_key_spellings,
            [
                "ni", "mi", "mian", "miao", "mie", "min", "ming", "miu", "nian", "niang", "niao",
                "nie", "nin", "ning", "niu", "o"
            ],
            "the preferred spelling leads"
        );
        assert!(session.choose_spelling(0).handled);
        assert_eq!(session.snapshot().preedit, "ni");
        assert_eq!(words(&session), ["你"], "the lock keeps 米 out");
        type_digits(&mut session, "426");
        let view = session.snapshot();
        assert_eq!(view.preedit, "ni'426");
        assert_eq!(
            view.nine_key_spellings,
            ["hao", "gan", "gang", "gao", "han", "hang", "ga", "ha"]
        );
        let result = session.select(index_of(&session, "你好"));
        assert_eq!(result.commit.as_deref(), Some("你好"));
        assert!(!session.active() && session.snapshot().preedit.is_empty());

        type_digits(&mut session, "64426");
        let ni = session
            .snapshot()
            .nine_key_spellings
            .iter()
            .position(|s| s == "ni")
            .expect("ni offered");
        session.choose_spelling(ni);
        session.choose_spelling(0);
        assert_eq!(session.snapshot().preedit, "ni'hao");
        assert!(session.snapshot().nine_key_spellings.is_empty());
        let result = session.select(index_of(&session, "你"));
        assert_eq!(result.commit.as_deref(), Some("你"));
        let view = session.snapshot();
        assert_eq!(view.editing_text, "426");
        assert_eq!(
            view.preedit, "hao",
            "partial selection lost the locked suffix"
        );
        let result = session.finish(0);
        assert_eq!(result.commit.as_deref(), Some("好"));
        assert!(!session.active());

        // A longer spelling extends the digits to its whole code.
        type_digits(&mut session, "6");
        let ming = session
            .snapshot()
            .nine_key_spellings
            .iter()
            .position(|s| s == "ming")
            .expect("ming offered");
        session.choose_spelling(ming);
        assert_eq!(session.snapshot().editing_text, "6464");
        assert_eq!(session.snapshot().preedit, "ming");
    }

    #[test]
    fn commands_edit_commit_and_cancel_the_digits() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        assert!(!session.command(Command::Backspace).handled && !session.finish(0).handled);
        type_digits(&mut session, "64");
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().editing_text, "6");
        session.command(Command::Backspace);
        assert!(
            !session.command(Command::Backspace).handled,
            "idle backspace"
        );

        type_digits(&mut session, "64");
        session.choose_spelling(0);
        session.command(Command::Backspace);
        assert!(
            session.locked.is_empty(),
            "backspace keeps a lock past the digits"
        );
        assert_eq!(session.snapshot().preedit, "6");
        session.command(Command::Cancel);

        type_digits(&mut session, "64");
        assert!(!session.command(Command::MoveLeft).handled);
        assert_eq!(
            session.command(Command::CommitRaw).commit.as_deref(),
            Some("64")
        );
        assert!(!session.active());
        type_digits(&mut session, "64");
        assert_eq!(session.finish(999).commit.as_deref(), Some("64"));
        type_digits(&mut session, "64426");
        assert_eq!(
            session.command(Command::CommitCandidate).commit.as_deref(),
            Some("你好")
        );
        type_digits(&mut session, "64");
        session.command(Command::Cancel);
        let view = session.snapshot();
        assert!(
            view.preedit.is_empty()
                && view.nine_key_spellings.is_empty()
                && view.candidates.is_empty()
        );

        type_digits(&mut session, &"7".repeat(DIGIT_LIMIT));
        let result = session.character(b'7');
        assert!(result.handled);
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::NINE_KEY_DIGIT_LIMIT)
        );
        assert_eq!(session.snapshot().editing_text.len(), DIGIT_LIMIT);
        assert!(session.snapshot().candidates.is_empty());
        // Finishing with nothing to choose commits the digits as typed.
        assert_eq!(
            session.finish(0).commit.as_deref(),
            Some("7".repeat(DIGIT_LIMIT).as_str())
        );
    }

    #[test]
    fn learning_and_management_use_canonical_keys() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64");
        assert_eq!(
            session.select(index_of(&session, "米")).commit.as_deref(),
            Some("米")
        );
        let mut unchanged = open(&fixture.paths, false, mixed());
        type_digits(&mut unchanged, "64");
        assert_eq!(
            words(&unchanged)[0],
            "你",
            "disabled learning changed ranking"
        );

        let mut learner = open(&fixture.paths, true, mixed());
        type_digits(&mut learner, "64");
        let learned = learner.select(index_of(&learner, "米"));
        assert_eq!(learned.commit.as_deref(), Some("米"));
        assert_eq!(learned.diagnostic, None);

        let mut managed = open(&fixture.paths, false, mixed());
        type_digits(&mut managed, "64");
        assert_eq!(words(&managed)[0], "米", "learning did not persist");
        let pinned = managed.pin(index_of(&managed, "你"));
        assert_eq!(pinned, KeyResult::handled());
        assert_eq!(managed.snapshot().editing_text, "64");
        assert_eq!(words(&managed)[0], "你");
        assert!(
            !managed.remove(index_of(&managed, "你")).handled,
            "single character was removed"
        );
        let ogham = index_of(&managed, "ogham");
        assert!(
            !managed.pin(ogham).handled,
            "an English row is not a pinyin dictionary row"
        );
        assert!(!managed.pin(999).handled && !managed.remove(999).handled);
        assert!(!managed.set_position(0, 6).handled && !managed.set_position(0, -1).handled);
        assert!(!managed.set_position(999, 0).handled);
        assert_eq!(
            managed.set_position(index_of(&managed, "米"), 1),
            KeyResult::handled()
        );
        assert_eq!(words(&managed)[0], "米", "fixed position not applied");

        let mut fixed = open(&fixture.paths, false, mixed());
        type_digits(&mut fixed, "64");
        assert_eq!(words(&fixed)[0], "米", "fixed position not persisted");
        let ni = fixed
            .snapshot()
            .nine_key_spellings
            .iter()
            .position(|s| s == "ni")
            .expect("ni offered");
        fixed.choose_spelling(ni);
        assert_eq!(
            words(&fixed)[0],
            "你",
            "fixed position escaped the spelling constraint"
        );

        assert_eq!(
            managed.set_position(index_of(&managed, "米"), 0),
            KeyResult::handled()
        );
        assert_eq!(words(&managed)[0], "你", "clear fixed position failed");
        managed.command(Command::Cancel);
        type_digits(&mut managed, "64426");
        let removed = managed.remove(index_of(&managed, "你好"));
        assert_eq!(removed, KeyResult::handled());
        assert_eq!(managed.snapshot().editing_text, "64426");

        let mut after_removal = open(&fixture.paths, false, mixed());
        type_digits(&mut after_removal, "64426");
        // Only the dictionary row is gone; the two-syllable lattice may still compose 你好 from 你 and 好.
        assert!(
            !after_removal
                .snapshot()
                .candidates
                .iter()
                .any(|item| item.word == "你好" && item.source.is_dictionary()),
            "phrase removal did not persist"
        );
    }

    #[test]
    fn failed_learning_keeps_the_commit() {
        let fixture = fixture();
        let mut paths = fixture.paths.clone();
        paths.user_data = fixture.directory.path().join("blocked");
        std::fs::create_dir_all(paths.user(assets::USER_JOURNAL)).expect("block the journal");
        let mut failing = open(&paths, true, mixed());
        type_digits(&mut failing, "64");
        let result = failing.finish(index_of(&failing, "米"));
        assert_eq!(result.commit.as_deref(), Some("米"));
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::NINE_KEY_FREQUENCY_NOT_PERSISTED)
        );
        assert!(!failing.active() && failing.snapshot().preedit.is_empty());
    }

    #[test]
    fn english_only_spells_words_from_the_first_digit() {
        let fixture = fixture();
        let options = EnglishInputOptions {
            mixed_candidates: false,
            minimum_prefix: 2,
        };
        let mut words_only = open(&fixture.paths, false, options);
        words_only.set_english_only(true);
        type_digits(&mut words_only, "65");
        let view = words_only.snapshot();
        assert_eq!(words(&words_only), ["ok", "old", "older"]);
        assert!(
            view.nine_key_spellings.is_empty(),
            "english nine-key offered pinyin spellings"
        );
        assert!(
            view.dedicated_english,
            "the grid's snapshot dropped the English mode"
        );
        words_only.command(Command::Cancel);
        type_digits(&mut words_only, "653");
        assert_eq!(words(&words_only), ["old", "older"]);
        words_only.command(Command::Cancel);

        // One digit is enough: the prefix length that quiets a mixed list means nothing when words are the whole list.
        type_digits(&mut words_only, "6");
        let result = words_only.select(index_of(&words_only, "old"));
        assert_eq!(result.commit.as_deref(), Some("old"));
        assert!(!words_only.active());

        // Leaving English brings syllables back, and English rows stay off without mixed candidates.
        words_only.set_english_only(false);
        type_digits(&mut words_only, "64");
        assert_eq!(words(&words_only), ["你", "米"]);
        assert!(!words_only.snapshot().nine_key_spellings.is_empty());
        assert!(!words_only.snapshot().dedicated_english);
        // Switching mid-composition requeries the same digits.
        words_only.set_english_only(true);
        assert_eq!(words(&words_only), ["ogham"]);
    }

    /// 77 拼不出任何音节：混输关着也给英文词，不然这串数字一行候选都没有。拼音有读法时混输开关照旧说了算。
    #[test]
    fn digits_without_a_reading_offer_english_even_with_mixing_off() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        type_digits(&mut session, "77");
        assert_eq!(words(&session), ["QQ"]);
        session.command(Command::Cancel);
        type_digits(&mut session, "64426");
        assert!(!words(&session).contains(&"ogham".to_owned()));
    }

    #[test]
    fn a_missing_english_dictionary_means_no_english_rows() {
        let fixture = fixture();
        std::fs::remove_file(fixture.paths.dictionary(assets::ENGLISH_DICTIONARY))
            .expect("remove msime-english.db");
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64");
        assert_eq!(words(&session), ["你", "米"]);
        session.set_english_only(true);
        assert!(words(&session).is_empty());
    }

    /// 会话不允许全拼时九宫格只拼英文：没有音节、没有拼音行，拼音词库也不打开。
    #[test]
    fn without_quanpin_the_grid_spells_english_only() {
        let fixture = fixture();
        let mut session = NineKeySession::new(
            &fixture.paths,
            false,
            FrequencyAdjustmentOptions::default(),
            FuzzyPinyinOptions::default(),
            mixed(),
            false,
        );
        type_digits(&mut session, "64");
        assert!(session.snapshot().nine_key_spellings.is_empty());
        assert!(!words(&session).contains(&"你".to_owned()));
        assert!(session.dictionary.is_none());

        let mut pinyin = open(&fixture.paths, false, mixed());
        type_digits(&mut pinyin, "64");
        assert!(words(&pinyin).contains(&"你".to_owned()));
        assert!(pinyin.dictionary.is_some());
    }
}
