//! `ShuangpinDictionary` (schemes-lang.md §1.7): series generation over decoded quanpin segments, the helpcode caches (overlays.md §5.1), and online rows. Pins, removals, frequency learning and phrases are written by the session through `user_dictionary` (see `ime::registry`), so the reference's word writers are not ported here. The Google sentence lines are gone; the lattice sentence goes in at `whole_sentence_insert_position` with nothing extra (overlays.md §2.2).

use std::collections::HashSet;
use std::sync::Arc;

use super::query::remove_manual_delimiters;
use super::utils::{convert_seg_shuangpin_to_seg_complete_pinyin, pinyin_segmentation};
use super::ShuangpinProfile;
use crate::assets;
use crate::cache::FifoCache;
use crate::dictionary::pinyin::PinyinDatabase;
use crate::helpcode::{
    match_single_helpcode, matches_double_helpcodes, HelpcodeKeymap, SingleHelpcodeMatch,
};
use crate::ime::online_batch::replace_online_candidate_batch;
use crate::lattice::decode::make_sentence_lattice_options;
use crate::lattice::merge::merge_lattice_candidates;
use crate::lattice::neural::{
    shared_sentence_model, NeuralReranker, CONTEXT_CHARACTERS, MAX_RERANK_PATHS,
};
use crate::lattice::ngram::NgramTable;
use crate::paths::RuntimePaths;
use crate::pinyin::jianpin::QuerySource;
use crate::pinyin::segment::split_segments;
use crate::quanpin::dictionary::{CACHE_CAPACITY, INITIAL_CANDIDATE_LIMIT};
use crate::text::last_characters;
use crate::types::{CandidateSource, SentenceAssociationOptions, WordItem};
use crate::user_dictionary::ngram_store::PersonalNgramStore;

/// The reference passed `INT_MAX` as "no limit" to the row queries (SD:899, SD:543).
const UNLIMITED_ROWS: usize = i32::MAX as usize;
/// Once the tracked personal-scored keys outgrow twice the series cache they cannot all still be cached, so the evicted ones are pruned (SD:289-301).
const PERSONAL_SCORED_KEY_LIMIT: usize = 2 * CACHE_CAPACITY;

fn double_helpcode_cache_key(pinyin: &str, help_codes: &str) -> String {
    let mut key = String::with_capacity(pinyin.len() + 1 + help_codes.len());
    key.push_str(pinyin);
    key.push(':');
    key.push_str(help_codes);
    key
}

pub struct ShuangpinDictionary {
    profile: &'static ShuangpinProfile,
    database: PinyinDatabase,
    paths: RuntimePaths,
    personal: Arc<PersonalNgramStore>,
    rerankers: Vec<NeuralReranker>,
    sentence_alternatives: bool,
    sentence_association: SentenceAssociationOptions,
    rescoring_context: String,
    /// `generate` answers per key (`_cached_buffer`).
    rows_cache: FifoCache<String, Vec<WordItem>>,
    /// `generate_series` answers per raw input including `'`, and the online rows put into them.
    series_cache: FifoCache<String, Vec<WordItem>>,
    /// Lowercase single helpcode answers per raw input.
    single_helpcode_cache: FifoCache<String, Vec<WordItem>>,
    /// Uppercase ("prefer last") single helpcode answers per raw input.
    reversed_single_helpcode_cache: FifoCache<String, Vec<WordItem>>,
    /// Double helpcode answers per `raw:codes`, so `ni`+`ab` and `ni`+`cd` never share an entry.
    double_helpcode_cache: FifoCache<String, Vec<WordItem>>,
    /// Series keys whose answer carries personal-model scores, dropped when the model changes.
    personal_scored_keys: HashSet<String>,
    personal_version: u64,
}

impl ShuangpinDictionary {
    pub fn new(profile: &'static ShuangpinProfile, paths: &RuntimePaths) -> Self {
        // READWRITE without CREATE: a missing dictionary stays missing and every query answers empty (SD:54-70).
        let mut database = PinyinDatabase::open(&paths.dictionary(assets::MAIN_DICTIONARY));
        if database.is_open() {
            database.warm_up();
            // Off the typing path: the first lattice decode would otherwise map the tables.
            NgramTable::shared(&paths.dictionary(assets::BIGRAM_TABLE));
            NgramTable::shared(&paths.dictionary(assets::TRIGRAM_TABLE));
            // Records the data version so the first cache hit has something to compare with.
            database.database_changed();
        }
        let personal = PersonalNgramStore::for_journal(&paths.user(assets::USER_JOURNAL));
        // The model loads lazily and bumps its version when it does. Left to the first lattice merge, that load would land on a keystroke and mark the answer it just scored as stale, so the next query for the same key would drop it together with any online rows put into it.
        drop(personal.model());
        let personal_version = personal.version();
        Self {
            profile,
            database,
            paths: paths.clone(),
            personal,
            rerankers: Vec::new(),
            sentence_alternatives: false,
            sentence_association: SentenceAssociationOptions::default(),
            rescoring_context: String::new(),
            rows_cache: FifoCache::new(CACHE_CAPACITY),
            series_cache: FifoCache::new(CACHE_CAPACITY),
            single_helpcode_cache: FifoCache::new(CACHE_CAPACITY),
            reversed_single_helpcode_cache: FifoCache::new(CACHE_CAPACITY),
            double_helpcode_cache: FifoCache::new(CACHE_CAPACITY),
            personal_scored_keys: HashSet::new(),
            personal_version,
        }
    }

    /// Series and helpcode answers carry the reranked sentence rows, which depend on the committed context. The C++ bypassed these caches while a model was loaded because its rows arrived asynchronously; scoring here is synchronous, so the trimmed context joins the key instead and a commit moves to fresh entries without clearing the context-free row cache (overlays.md §1.6.2). Without a reranker no answer reads the context, so the key stays the raw input.
    fn sentence_cache_key(&self, key: &str) -> String {
        if self.rerankers.is_empty() {
            key.to_string()
        } else {
            format!("{key}\u{1}{}", self.rescoring_context)
        }
    }

    /// Only a cache hit checks `PRAGMA data_version`, before returning the cached answer (SD:1189-1212).
    fn reset_cache_if_database_changed(&mut self) {
        if self.database.database_changed() {
            self.reset_cache();
        }
    }

    /// Series answers scored with an older personal model are stale (SD:89-99).
    fn drop_personal_scored_results(&mut self) {
        // Reading the model reloads it when another store or process changed the tables (SD:89-99 refreshes before comparing versions); a bare version read would keep answers scored against a stale model.
        drop(self.personal.model());
        let version = self.personal.version();
        if version == self.personal_version {
            return;
        }
        self.personal_version = version;
        for key in self.personal_scored_keys.drain() {
            self.series_cache.remove(&key);
        }
    }

    /// `tbl_1_<c>` rows for one code letter read as an initial, so xiaohe `u` lists `sh` words (SD:932-950).
    fn query_initial(&self, code: &str, limit: usize) -> Vec<WordItem> {
        if !self.database.is_open() || code.len() != 1 || !code.as_bytes()[0].is_ascii_lowercase() {
            return Vec::new();
        }
        let initial = convert_seg_shuangpin_to_seg_complete_pinyin(code, self.profile);
        self.database
            .query_initial(&initial, limit)
            .into_iter()
            .map(|row| {
                WordItem::new(
                    code,
                    row.value,
                    row.weight,
                    CandidateSource::Database,
                    row.key,
                )
            })
            .collect()
    }

    /// The cascade over the decoded quanpin segments; rows keep the typed code as their pinyin (SD:880-913).
    fn query_rows(&self, pure: &str, segmentation: &str) -> Vec<WordItem> {
        if !self.database.is_open() || segmentation.is_empty() {
            return Vec::new();
        }
        let segments = split_segments(&convert_seg_shuangpin_to_seg_complete_pinyin(
            segmentation,
            self.profile,
        ));
        if segments.is_empty() {
            return Vec::new();
        }
        self.database
            .query_segments_keyed_flat(&segments, UNLIMITED_ROWS, QuerySource::Shuangpin)
            .into_iter()
            .map(|row| {
                WordItem::new(
                    pure,
                    row.value,
                    row.weight,
                    CandidateSource::Database,
                    row.key,
                )
            })
            .collect()
    }

    /// Rows for exactly this code (SD:108-142). One letter is the capped initial list, uncached.
    fn generate(&mut self, pure: &str, segmentation: &str, cache_key: &str) -> Vec<WordItem> {
        match pure.len() {
            0 => return Vec::new(),
            1 => return self.query_initial(pure, INITIAL_CANDIDATE_LIMIT),
            _ => {}
        }
        let key = if cache_key.is_empty() {
            pure
        } else {
            cache_key
        };
        if self.rows_cache.get_ref_by(key).is_some() {
            self.reset_cache_if_database_changed();
            if let Some(cached) = self.rows_cache.get_ref_by(key) {
                return cached.clone();
            }
        }
        let rows = self.query_rows(pure, segmentation);
        self.rows_cache.insert(key.to_owned(), rows.clone());
        rows
    }

    /// Prefix groups longest first, then the lattice sentence block, cached by raw input including `'`.
    pub fn generate_series(&mut self, pure: &str, segmentation: &str, raw: &str) -> Vec<WordItem> {
        match pure.len() {
            0 => return Vec::new(),
            1 => return self.query_initial(pure, INITIAL_CANDIDATE_LIMIT),
            _ => {}
        }
        let rows_key = if raw.is_empty() { pure } else { raw };
        let mut key = (!self.rerankers.is_empty()).then(|| self.sentence_cache_key(rows_key));
        self.drop_personal_scored_results();
        let cache_hit = match key.as_ref() {
            Some(key) => self.series_cache.get_ref(key).is_some(),
            None => self.series_cache.get_ref_by(rows_key).is_some(),
        };
        if cache_hit {
            self.reset_cache_if_database_changed();
            let cached = match key.as_ref() {
                Some(key) => self.series_cache.get_ref(key),
                None => self.series_cache.get_ref_by(rows_key),
            };
            if let Some(cached) = cached {
                return cached.clone();
            }
        }

        let mut candidates = self.generate(pure, segmentation, rows_key);
        // Every shorter prefix group follows, longest first; phrase creation picks from them (SD:215-231). The reference appended each group whole, so a word several groups answer (a manual delimiter leaves empty pieces, and ni'''nn'i holds the ni group three times) was listed once per group; a word already listed keeps its first, longest-prefix seat instead.
        let mut prefix_rows = Vec::with_capacity(prefix_group_count(segmentation));
        let mut prefix = segmentation;
        while let Some(cut) = prefix.rfind('\'') {
            prefix = &prefix[..cut];
            let prefix_pure = remove_manual_delimiters(prefix);
            prefix_rows.push(self.generate(&prefix_pure, prefix, ""));
        }
        // Borrow words while calculating each group's first occurrence, then release the set before moving rows into candidates.
        let mut listed: HashSet<&str> = candidates.iter().map(|item| item.word.as_str()).collect();
        let duplicates = prefix_rows
            .iter()
            .map(|rows| {
                rows.iter()
                    .enumerate()
                    .filter_map(|(index, item)| {
                        (!listed.insert(item.word.as_str())).then_some(index)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        drop(listed);
        for (rows, duplicates) in prefix_rows.into_iter().zip(duplicates) {
            candidates.reserve(rows.len());
            let mut duplicates = duplicates.into_iter().peekable();
            candidates.extend(rows.into_iter().enumerate().filter_map(|(index, item)| {
                if duplicates.peek() == Some(&index) {
                    duplicates.next();
                    None
                } else {
                    Some(item)
                }
            }));
        }

        let segments = split_segments(&convert_seg_shuangpin_to_seg_complete_pinyin(
            segmentation,
            self.profile,
        ));
        self.merge_sentences(&mut candidates, &segments, pure);

        let key = key.take().unwrap_or_else(|| rows_key.to_owned());
        self.series_cache.insert(key.clone(), candidates.clone());
        // Only keys long enough for the lattice carry personal scores.
        if segments.len() >= 2 {
            self.personal_scored_keys.insert(key);
            if self.personal_scored_keys.len() > PERSONAL_SCORED_KEY_LIMIT {
                let series_cache = &self.series_cache;
                self.personal_scored_keys
                    .retain(|scored| series_cache.contains(scored));
            }
        }
        candidates
    }

    /// The lattice block (SD:261-269 without the Google sentence and its reordering, overlays.md §1.6.2): the lattice best as Generated when `word_lattice` is on, and one row per enabled neural reranker, inserted by the merge itself.
    fn merge_sentences(
        &mut self,
        candidates: &mut Vec<WordItem>,
        segments: &[String],
        typed: &str,
    ) {
        let word_lattice = self.sentence_association.word_lattice;
        if !word_lattice && self.rerankers.is_empty() {
            return;
        }
        let mut options = make_sentence_lattice_options(&self.paths, self.sentence_alternatives);
        if !self.rerankers.is_empty() {
            options.nbest = MAX_RERANK_PATHS;
        }
        options.include_lattice_best = word_lattice;
        options.show_next_on_duplicate = self.sentence_association.show_next_on_duplicate;
        // Held for the whole merge: the options borrow the model.
        let model = self.personal.model();
        options.personal = (!model.is_empty()).then_some(&*model);
        let database = &self.database;
        let span_limit = options.span_limit;
        let mut lookup = |span: &[String]| database.query_lattice_span(span, span_limit);
        // Shuangpin has no typo source, so no typo sentence comes back.
        merge_lattice_candidates(
            candidates,
            segments,
            &mut lookup,
            typed,
            &options,
            None,
            &mut self.rerankers,
            &self.rescoring_context,
        );
    }

    /// Matched groups, then the full input read as pinyin, then the unmatched rows (SD:324-375).
    fn filter_with_single_helpcode(
        &mut self,
        candidates: &[WordItem],
        help_code: &str,
        raw: &str,
        keymap: &HelpcodeKeymap,
    ) -> Vec<WordItem> {
        if candidates.is_empty() || help_code.len() != 1 {
            return Vec::new();
        }
        let prefer_last = help_code.as_bytes()[0].is_ascii_uppercase();
        let normalized = help_code.to_ascii_lowercase();
        let mut first = Vec::with_capacity(candidates.len());
        let mut last = Vec::with_capacity(candidates.len());
        let mut unmatched = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            match match_single_helpcode(&candidate.word, &normalized, keymap) {
                SingleHelpcodeMatch::First => first.push(candidate.clone()),
                SingleHelpcodeMatch::Last => last.push(candidate.clone()),
                SingleHelpcodeMatch::Both if prefer_last => last.push(candidate.clone()),
                SingleHelpcodeMatch::Both => first.push(candidate.clone()),
                SingleHelpcodeMatch::None => unmatched.push(candidate.clone()),
            }
        }
        let mut result = if prefer_last {
            last.extend(first);
            last
        } else {
            first.extend(last);
            first
        };
        // The whole raw input, the last letter read as pinyin instead of a helpcode. The reference segments the raw input as one chunk, `'` included; the conversion drops the empty pieces that leaves (SD:370-372).
        let original_segmentation = pinyin_segmentation(raw, self.profile);
        let whole = self.generate_series(raw, &original_segmentation, "");
        // The reference appended the whole-input answer and then the unmatched rows whole, so a word already among the matched rows, or in both lists, was listed again; a word already listed keeps its first seat.
        // Keep duplicate keys borrowed while checking both owned append lists, then move rows after releasing the set.
        let mut listed: HashSet<&str> = result.iter().map(|item| item.word.as_str()).collect();
        let rows = whole.into_iter().chain(unmatched).collect::<Vec<_>>();
        let duplicates = rows
            .iter()
            .enumerate()
            .filter_map(|(index, item)| (!listed.insert(item.word.as_str())).then_some(index))
            .collect::<Vec<_>>();
        drop(listed);
        result.reserve(rows.len());
        let mut duplicates = duplicates.into_iter().peekable();
        result.extend(rows.into_iter().enumerate().filter_map(|(index, item)| {
            if duplicates.peek() == Some(&index) {
                duplicates.next();
                None
            } else {
                Some(item)
            }
        }));
        result
    }

    /// The same with a helpcode filter (two letters) or reorder (one letter), cached per helpcode.
    pub fn generate_with_helpcodes(
        &mut self,
        pure: &str,
        segmentation: &str,
        raw: &str,
        help_codes: &str,
        keymap: &HelpcodeKeymap,
    ) -> Vec<WordItem> {
        let reversed = help_codes.len() == 1 && help_codes.as_bytes()[0].is_ascii_uppercase();
        let cache_key = match help_codes.len() {
            1 => self.sentence_cache_key(raw),
            2 => self.sentence_cache_key(&double_helpcode_cache_key(raw, help_codes)),
            _ => String::new(),
        };
        if !cache_key.is_empty()
            && self
                .helpcode_cache(help_codes.len(), reversed)
                .contains(&cache_key)
        {
            self.reset_cache_if_database_changed();
            if let Some(cached) = self
                .helpcode_cache(help_codes.len(), reversed)
                .get(&cache_key)
            {
                return cached;
            }
        }

        // The base is cached under its own letters, without the raw input's `'` (SD:458).
        let candidates = self.generate_series(pure, segmentation, "");
        let result = match help_codes.len() {
            1 => self.filter_with_single_helpcode(&candidates, help_codes, raw, keymap),
            2 => candidates
                .into_iter()
                .filter(|candidate| matches_double_helpcodes(&candidate.word, help_codes, keymap))
                .collect(),
            _ => return Vec::new(),
        };
        self.helpcode_cache_mut(help_codes.len(), reversed)
            .insert(cache_key, result.clone());
        result
    }

    fn helpcode_cache(&self, codes: usize, reversed: bool) -> &FifoCache<String, Vec<WordItem>> {
        match (codes, reversed) {
            (2, _) => &self.double_helpcode_cache,
            (_, true) => &self.reversed_single_helpcode_cache,
            _ => &self.single_helpcode_cache,
        }
    }

    fn helpcode_cache_mut(
        &mut self,
        codes: usize,
        reversed: bool,
    ) -> &mut FifoCache<String, Vec<WordItem>> {
        match (codes, reversed) {
            (2, _) => &mut self.double_helpcode_cache,
            (_, true) => &mut self.reversed_single_helpcode_cache,
            _ => &mut self.single_helpcode_cache,
        }
    }

    /// Replace the capped 24-row initial run with every row of the initial, and store the merged list under `series_key` so a refresh keeps it (SD:525-573).
    pub fn expand_initial_candidates(
        &mut self,
        code: &str,
        candidates: &mut Vec<WordItem>,
        series_key: &str,
    ) -> bool {
        if code.len() != 1 {
            return false;
        }
        let is_limited =
            |item: &WordItem| item.source == CandidateSource::Database && item.pinyin == code;
        let limited = candidates.iter().filter(|item| is_limited(item)).count();
        if limited != INITIAL_CANDIDATE_LIMIT {
            return false;
        }
        let expanded = self.query_initial(code, UNLIMITED_ROWS);
        if expanded.len() <= limited {
            return false;
        }
        let mut merged = Vec::with_capacity(candidates.len() - limited + expanded.len());
        let mut expanded = Some(expanded);
        for item in candidates.drain(..) {
            if is_limited(&item) {
                if let Some(rows) = expanded.take() {
                    merged.extend(rows);
                }
                continue;
            }
            merged.push(item);
        }
        *candidates = merged;
        if !series_key.is_empty() {
            let key = self.sentence_cache_key(series_key);
            self.series_cache.insert(key, candidates.clone());
        }
        true
    }

    /// Online rows into the series cache for `raw`.
    pub fn insert_word_to_series_cache(
        &mut self,
        raw: &str,
        words: &[String],
        source: CandidateSource,
    ) -> bool {
        if raw.is_empty() {
            return false;
        }
        let key = self.sentence_cache_key(raw);
        // An absent key starts an empty list: the rows show on the next query for `raw` (SD:1373-1384).
        let mut list = self.series_cache.get(&key).unwrap_or_default();
        if !replace_online_candidate_batch(&mut list, raw, words, source) {
            return false;
        }
        self.series_cache.insert(key, list);
        true
    }

    /// Online rows into the helpcode cache for `raw` and `help_codes`.
    pub fn insert_word_to_active_helpcode_cache(
        &mut self,
        raw: &str,
        words: &[String],
        source: CandidateSource,
        help_codes: &str,
    ) -> bool {
        // Only a cached answer takes the rows; an absent key is a failure (SD:1386-1408).
        let insert = |cache: &mut FifoCache<String, Vec<WordItem>>, key: &String| {
            let Some(mut list) = cache.get(key) else {
                return false;
            };
            if !replace_online_candidate_batch(&mut list, raw, words, source) {
                return false;
            }
            cache.insert(key.clone(), list);
            true
        };
        if !help_codes.is_empty() {
            let key = self.sentence_cache_key(&double_helpcode_cache_key(raw, help_codes));
            return insert(&mut self.double_helpcode_cache, &key);
        }
        let key = self.sentence_cache_key(raw);
        let single = insert(&mut self.single_helpcode_cache, &key);
        let reversed = insert(&mut self.reversed_single_helpcode_cache, &key);
        single || reversed
    }

    pub fn find_candidate(&self, key: &str, value: &str) -> Option<WordItem> {
        self.database
            .find_weight(key, value)
            .map(|weight| WordItem::new(key, value, weight, CandidateSource::Database, key))
    }

    /// A change clears only the series cache: those answers were assembled under the other setting (SD:151-158).
    pub fn set_sentence_alternatives(&mut self, enabled: bool) {
        if self.sentence_alternatives == enabled {
            return;
        }
        self.sentence_alternatives = enabled;
        self.series_cache.clear();
    }

    /// Loads the keyboard sentence model from the resource bundle when its switch is on; a model that fails to load contributes no rows. The desktop model is not the engine's: only the runtime's settled reranker runs it. A change resets the caches (neural-association.patch:3080-3095).
    pub fn set_sentence_association(&mut self, options: SentenceAssociationOptions) {
        if self.sentence_association == options {
            return;
        }
        self.sentence_association = options;
        self.rerankers.clear();
        if options.neural_keyboard {
            if let Some(model) =
                shared_sentence_model(&self.paths.resource(assets::NEURAL_MODEL_KEYBOARD))
            {
                self.rerankers
                    .push(NeuralReranker::new(CandidateSource::NeuralKeyboard, model));
            }
        }
        self.reset_cache();
    }

    /// Only the last 64 characters condition the models, so committed text beyond that window keys the same entries. A change clears nothing: the context is part of the sentence cache keys (`sentence_cache_key`).
    pub fn set_rescoring_context(&mut self, context: &str) {
        let trimmed = last_characters(context, CONTEXT_CHARACTERS);
        if self.rescoring_context != trimmed {
            self.rescoring_context = trimmed.to_string();
        }
    }

    pub fn reset_cache(&mut self) {
        self.rows_cache.clear();
        self.series_cache.clear();
        self.single_helpcode_cache.clear();
        self.reversed_single_helpcode_cache.clear();
        self.double_helpcode_cache.clear();
        self.personal_scored_keys.clear();
    }
}

fn prefix_group_count(segmentation: &str) -> usize {
    segmentation.matches('\'').count()
}

#[cfg(test)]
mod tests {
    use super::double_helpcode_cache_key;
    use super::prefix_group_count;

    #[test]
    fn double_helpcode_cache_keys_use_exact_string_capacity() {
        let key = double_helpcode_cache_key("ni'hao", "ab");
        assert_eq!(key, "ni'hao:ab");
        assert_eq!(key.capacity(), key.len());
    }

    #[test]
    fn prefix_group_count_matches_manual_boundaries() {
        assert_eq!(prefix_group_count("ni'hao'ba"), 2);
        assert_eq!(prefix_group_count("nihao"), 0);
    }
}
