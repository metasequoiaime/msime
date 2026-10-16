//! Online (cloud and AI) queries and their answers (core-session.md §7.2, overlays.md §7.4-§7.5).

use std::borrow::Cow;
use std::sync::atomic::{AtomicU64, Ordering};

use super::composition::resolve_shuangpin_composition_base;
use super::input::InputSession;
use crate::ime::online_batch::validate_online_candidate_batch;
use crate::local::command::TEXT_UTF16_LIMIT;
use crate::pinyin::active_helpcode::detect_active_helpcode_length;
use crate::pinyin::segment::{is_complete_pinyin_input, split_segments};
use crate::pinyin::syllables::to_google_spelling;
use crate::shuangpin::query::{
    is_complete_input, normalize_input_with_delimiters, raw_length_for_effective_prefix,
};
use crate::types::{
    CandidateSource, CommandTranslationQuery, LocalInputMode, OnlineQuery, SchemeType, WordItem,
};

/// Session ids are unique for the process, so an answer can never be applied to a different session than the one that asked.
static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

/// Stamps queries with the session id and a generation that every composition change advances, so a late answer for an older composition is refused. Session ids come from one process-wide counter starting at 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OnlineRequestGuard {
    pub session_id: u64,
    pub generation: u64,
    exhausted: bool,
}

impl OnlineRequestGuard {
    pub fn new() -> Self {
        Self {
            session_id: NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed),
            generation: 0,
            exhausted: false,
        }
    }

    pub fn invalidate(&mut self) {
        if self.exhausted {
            return;
        }
        let Some(next) = self.generation.checked_add(1) else {
            self.exhausted = true;
            return;
        };
        self.generation = next;
    }

    /// Same session, generation and every query field.
    #[cfg(test)]
    pub fn matches(&self, live: &OnlineQuery, answered: &OnlineQuery) -> bool {
        !self.exhausted
            && answered.session_id == self.session_id
            && answered.generation == self.generation
            && live.scheme == answered.scheme
            && live.identity == answered.identity
            && live.query_text == answered.query_text
            && live.cache_key == answered.cache_key
            && live.pinyin_segments == answered.pinyin_segments
            && live.cloud_eligible == answered.cloud_eligible
            && live.ai_eligible == answered.ai_eligible
    }
}

/// What the composition would ask a cloud provider (input_session_composition.cpp:913-974).
#[derive(Debug)]
struct CloudQueryState<'a> {
    should_query: bool,
    query_text: Cow<'a, str>,
    cache_key: Cow<'a, str>,
}

#[derive(Clone, Copy)]
struct OnlineAnswerState {
    cloud_eligible: bool,
    ai_eligible: bool,
}

fn online_identity_matches(identity: &str, scheme: SchemeType, input: &str) -> bool {
    let bytes = identity.as_bytes();
    bytes.len() == input.len() + 2
        && bytes[0] == b'0' + scheme as u8
        && bytes[1] == b':'
        && &bytes[2..] == input.as_bytes()
}

/// 不构造另一个 `Vec<String>`，直接比较提供者持有的分段和当前组字。返回值表示所有分段是否都是完整拼音。
fn segments_match(segments: &[String], segmentation: &str) -> Option<bool> {
    let mut complete = true;
    let mut count = 0;
    for (expected, actual) in segments.iter().zip(segmentation.split('\'')) {
        if expected != actual {
            return None;
        }
        complete &= is_complete_pinyin_input(actual);
        count += 1;
    }
    (count == segments.len() && count == segmentation.split('\'').count()).then_some(complete)
}

impl<'a> Default for CloudQueryState<'a> {
    fn default() -> Self {
        Self {
            should_query: false,
            query_text: Cow::Borrowed(""),
            cache_key: Cow::Borrowed(""),
        }
    }
}

fn online_identity(scheme: SchemeType, input: &str) -> String {
    let mut identity = String::with_capacity(2 + input.len());
    identity.push(char::from(b'0' + scheme as u8));
    identity.push(':');
    identity.push_str(input);
    identity
}

impl InputSession {
    /// input_session.cpp:631-667 over `get_cloud_query_state` (input_session_composition.cpp:913-974).
    pub(super) fn online_query(&self) -> Option<OnlineQuery> {
        if self.dedicated_english
            || self.online_requests.exhausted
            || self.local_mode != LocalInputMode::None
            || !self.has_composition()
        {
            return None;
        }
        let request = self.engine.request();
        let state = self.cloud_query_state();
        if !state.should_query {
            return None;
        }
        let input = if request.raw_input_with_cases.is_empty() {
            &request.raw_input
        } else {
            &request.raw_input_with_cases
        };
        let mut query = OnlineQuery {
            scheme: request.scheme,
            generation: self.online_requests.generation,
            identity: online_identity(request.scheme, input),
            query_text: state.query_text.into_owned(),
            cache_key: state.cache_key.into_owned(),
            session_id: self.online_requests.session_id,
            ..OnlineQuery::default()
        };
        if request.scheme == SchemeType::JapaneseRomaji {
            query.cloud_eligible = true;
            return Some(query);
        }
        if query.query_text.is_empty() {
            return None;
        }
        query.pinyin_segments = split_segments(if request.normalized_segmentation.is_empty() {
            &query.query_text
        } else {
            &request.normalized_segmentation
        });
        query.cloud_eligible = true;
        query.ai_eligible = !query.pinyin_segments.is_empty()
            && query
                .pinyin_segments
                .iter()
                .all(|segment| is_complete_pinyin_input(segment));
        Some(query)
    }

    /// The `/fy` request, the only one a local mode makes; `None` in every other state, so the cloud and AI path above stays closed to local input, and `None` again once the text has its translation, so a host that asks again for every new view does not send the same text twice.
    pub(super) fn command_translation_query(&self) -> Option<CommandTranslationQuery> {
        if self.dedicated_english || self.local_mode != LocalInputMode::Command {
            return None;
        }
        let code = self.local_preedit.get(1..)?;
        let (_, text) = self.queries.translation_source(code)?;
        // `query_command` lists the English first; anything ahead of it is the translation.
        if self
            .local_candidates
            .first()
            .is_some_and(|row| row.word != text)
        {
            return None;
        }
        Some(CommandTranslationQuery {
            session_id: self.online_requests.session_id,
            text,
        })
    }

    /// Puts a translation of the live `/fy` text first, as a row that commits the translation; false for an answer to another session or to text no longer typed, and for text a row cannot show.
    pub(super) fn apply_command_translation(
        &mut self,
        query: &CommandTranslationQuery,
        translation: &str,
    ) -> bool {
        let translation = translation.trim();
        if translation.is_empty()
            || translation.chars().any(char::is_control)
            || translation.encode_utf16().count() > TEXT_UTF16_LIMIT
            || translation == query.text
            || self.command_translation_query().as_ref() != Some(query)
        {
            return false;
        }
        let Some((trigger, _)) = self
            .local_preedit
            .get(1..)
            .and_then(|code| self.queries.translation_source(code))
        else {
            return false;
        };
        let weight = self
            .local_candidates
            .first()
            .map_or(1, |item| item.weight + 1);
        self.local_candidates.insert(
            0,
            WordItem::new(trigger, translation, weight, CandidateSource::Generated, ""),
        );
        true
    }

    pub(super) fn apply_online_candidate(
        &mut self,
        query: &OnlineQuery,
        word: &str,
        source: CandidateSource,
    ) -> bool {
        if word.is_empty() || !source.is_online() || !self.online_answer_accepted(query, source) {
            return false;
        }
        if self.candidates().iter().any(|item| item.word == word) {
            return false;
        }
        if !self
            .engine
            .apply_dynamic_candidates(&[word.to_owned()], source)
        {
            return false;
        }
        self.update_mixed_candidates();
        self.candidates()
            .iter()
            .any(|item| item.word == word && item.source == source)
    }

    pub(super) fn apply_online_candidates(
        &mut self,
        query: &OnlineQuery,
        words: &[String],
        source: CandidateSource,
    ) -> bool {
        // The batch rule is checked without constructing rows that would be discarded before anything reaches the provider cache.
        if !validate_online_candidate_batch(words, source) {
            return false;
        }
        if !self.online_answer_accepted(query, source) {
            return false;
        }
        if !self.engine.apply_dynamic_candidates(words, source) {
            return false;
        }
        self.update_mixed_candidates();
        true
    }

    /// The answer belongs to the live query and the source is one that query may take.
    fn online_answer_accepted(&self, query: &OnlineQuery, source: CandidateSource) -> bool {
        let Some(state) = self.online_answer_state(query) else {
            return false;
        };
        match source {
            CandidateSource::CloudSuggestion => state.cloud_eligible,
            CandidateSource::AiSuggestion => state.ai_eligible,
            _ => false,
        }
    }

    /// 不重建拥有型 `OnlineQuery`，直接把回包和当前组字比较。应用回包本来就要扫描候选列表；每次回调再复制身份、缓存键和分段只会增加临时缓冲。
    fn online_answer_state(&self, query: &OnlineQuery) -> Option<OnlineAnswerState> {
        if self.dedicated_english
            || self.online_requests.exhausted
            || self.local_mode != LocalInputMode::None
            || !self.has_composition()
        {
            return None;
        }
        if query.session_id != self.online_requests.session_id
            || query.generation != self.online_requests.generation
        {
            return None;
        }
        let request = self.engine.request();
        let state = self.cloud_query_state();
        if !state.should_query || state.query_text.is_empty() || query.scheme != request.scheme {
            return None;
        }
        let input = if request.raw_input_with_cases.is_empty() {
            &request.raw_input
        } else {
            &request.raw_input_with_cases
        };
        if !online_identity_matches(&query.identity, request.scheme, input)
            || query.query_text != state.query_text
            || query.cache_key != state.cache_key
        {
            return None;
        }
        if request.scheme == SchemeType::JapaneseRomaji {
            return (query.cloud_eligible
                && !query.ai_eligible
                && query.pinyin_segments.is_empty())
            .then_some(OnlineAnswerState {
                cloud_eligible: true,
                ai_eligible: false,
            });
        }
        let segmentation = if request.normalized_segmentation.is_empty() {
            state.query_text.as_ref()
        } else {
            &request.normalized_segmentation
        };
        let ai_eligible = segments_match(&query.pinyin_segments, segmentation)?;
        (query.cloud_eligible && query.ai_eligible == ai_eligible).then_some(OnlineAnswerState {
            cloud_eligible: true,
            ai_eligible,
        })
    }

    fn cloud_query_state(&self) -> CloudQueryState<'_> {
        let request = self.engine.request();
        let mut state = CloudQueryState::default();
        match self.engine.current_scheme_type() {
            SchemeType::JapaneseRomaji => {
                state.cache_key = Cow::Borrowed(&request.raw_input);
                state.should_query = !request.raw_input.is_empty();
                if state.should_query {
                    state.query_text = Cow::Borrowed(&request.raw_input);
                }
            }
            // 韩文音节、越南文单词和藏文音节串本身就是文字，没有可交给云端转换的东西。粤拼、注音和笔画只由各自的词库回答。
            SchemeType::Korean
            | SchemeType::Cantonese
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
            | SchemeType::Tibetan
            | SchemeType::Stroke => {}
            // Wubi codes are not spellings a cloud provider understands, and wubi providers cannot take dynamic rows.
            SchemeType::Wubi => state.cache_key = Cow::Borrowed(&request.normalized_input),
            SchemeType::Shuangpin => {
                let profile = self.shuangpin_profile();
                let base = resolve_shuangpin_composition_base(request, profile);
                state.cache_key = if base.helpcode_length > 0
                    && base.effective_raw_input.len() >= base.helpcode_length
                {
                    let base_length = base.effective_raw_input.len() - base.helpcode_length;
                    Cow::Owned(
                        base.raw_input
                            [..raw_length_for_effective_prefix(&base.raw_input, base_length)]
                            .to_owned(),
                    )
                } else {
                    base.raw_input
                };
                if self.has_active_helpcode() {
                    return state;
                }
                let last = base.effective_raw_input_with_cases.bytes().last();
                let ends_with_input_key = matches!(last, Some(b'a'..=b'z' | b'A'..=b'Z' | b';'));
                state.should_query =
                    ends_with_input_key && is_complete_input(&base.effective_raw_input, profile);
                if state.should_query {
                    state.query_text = Cow::Owned(to_google_spelling(
                        &normalize_input_with_delimiters(&state.cache_key, profile),
                    ));
                }
            }
            SchemeType::Quanpin => {
                state.cache_key = if request.enable_quanpin_helpcode {
                    let length = detect_active_helpcode_length(
                        &request.raw_input,
                        &request.raw_input_with_cases,
                    );
                    Cow::Borrowed(&request.raw_input[..request.raw_input.len() - length])
                } else {
                    Cow::Borrowed(&request.raw_input)
                };
                if self.has_active_helpcode() {
                    return state;
                }
                state.should_query = !request.normalized_input.is_empty();
                // The user's own apostrophes survive into the query (overlays.md §7.5), so `qi'e'huan` is not re-cut as `qie'huan`.
                state.query_text = Cow::Owned(to_google_spelling(
                    if request.normalized_segmentation.is_empty() {
                        &request.normalized_input
                    } else {
                        &request.normalized_segmentation
                    },
                ));
            }
        }
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamped(guard: &OnlineRequestGuard) -> OnlineQuery {
        OnlineQuery {
            scheme: SchemeType::Quanpin,
            generation: guard.generation,
            identity: "0:nihao".to_owned(),
            query_text: "ni'hao".to_owned(),
            cache_key: "nihao".to_owned(),
            pinyin_segments: vec!["ni".to_owned(), "hao".to_owned()],
            cloud_eligible: true,
            ai_eligible: true,
            session_id: guard.session_id,
        }
    }

    #[test]
    fn session_ids_are_distinct_and_start_above_zero() {
        let first = OnlineRequestGuard::new();
        let second = OnlineRequestGuard::new();
        assert!(first.session_id >= 1);
        assert_ne!(first.session_id, second.session_id);
        assert_eq!(first.generation, 0);
    }

    #[test]
    fn an_answer_matches_only_its_own_generation_and_fields() {
        let mut guard = OnlineRequestGuard::new();
        let query = stamped(&guard);
        assert!(guard.matches(&query, &query));

        let mut changed = query.clone();
        changed.cache_key = "niha".to_owned();
        assert!(!guard.matches(&query, &changed));
        let mut changed = query.clone();
        changed.ai_eligible = false;
        assert!(!guard.matches(&query, &changed));

        let other = OnlineRequestGuard::new();
        assert!(!other.matches(&stamped(&other), &query));

        guard.invalidate();
        let live = stamped(&guard);
        assert!(!guard.matches(&live, &query));
        assert!(guard.matches(&live, &live));
    }

    #[test]
    fn generation_exhaustion_does_not_reuse_online_request_identity() {
        let mut guard = OnlineRequestGuard::new();
        let old = stamped(&guard);
        guard.generation = u64::MAX;
        guard.invalidate();
        let live = stamped(&guard);
        assert!(!guard.matches(&live, &old));
    }
}
