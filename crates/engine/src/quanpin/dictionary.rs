//! `QuanpinDictionary` (`R/quanpin/quanpin_dictionary.cpp`, quanpin.md §4, §6, §7.5-§7.7, §9.9, §11, §13, §14): the query pipeline and its caches, plus the canonical-pinyin phrase writer learning uses (pins, removals and frequency learning go through `user_dictionary` directly, see `ime::registry`).

use std::borrow::Cow;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::assets::{
    BIGRAM_TABLE, MAIN_DICTIONARY, NEURAL_MODEL_KEYBOARD, TRIGRAM_TABLE, USER_JOURNAL,
};
use crate::cache::FifoCache;
use crate::dictionary::pinyin::{PinyinDatabase, INSERTED_WEIGHT};
use crate::dictionary::DictRow;
use crate::error::{EngineError, Result};
use crate::ime::online_batch::replace_online_candidate_batch;
use crate::lattice::decode::make_sentence_lattice_options;
use crate::lattice::merge::{merge_lattice_candidates, whole_sentence_insert_position};
use crate::lattice::neural::{
    shared_sentence_model, NeuralReranker, CONTEXT_CHARACTERS, MAX_RERANK_PATHS,
};
use crate::lattice::ngram::NgramTable;
use crate::lattice::{SentencePath, TypoEdgeSource};
use crate::paths::RuntimePaths;
use crate::pinyin::fuzzy::{fuzzy_segmentations, FUZZY_SEGMENTATION_LIMIT};
use crate::pinyin::graph::{
    build_syllable_graph, enumerate_complete_segmentations, SYLLABLE_GRAPH_PATH_LIMIT,
};
use crate::pinyin::jianpin::QuerySource;
use crate::pinyin::segment::{
    cut_pinyin_by_mode, is_complete_pinyin_input, join_segments, split_segments, CutMode,
};
use crate::pinyin::syllables::{
    has_only_complete_pinyin_segments, normalize_umlaut_aliases, sparse_pinyin_fallback_segments,
};
use crate::text::{count_han_chars, last_characters};
use crate::types::{
    autocorrect_type, CandidateSource, FuzzyPinyinOptions, PersonalDictionaryKind,
    SentenceAssociationOptions, WordItem,
};
use crate::user_dictionary::journal::record_user_insert;
use crate::user_dictionary::ngram_store::PersonalNgramStore;
use crate::user_dictionary::typo_profile::PersonalTypoProfile;

use super::series::{
    append_unique_words, fold_reading, mark_autocorrect_candidates,
    merge_alternative_segmentations, resolve_series_query, series_cache_key, SeriesResolution,
    ALTERNATIVE_SEGMENTATION_CANDIDATE_LIMIT, LONGER_PHRASE_EXTRA_SYLLABLES, LONGER_PHRASE_LIMIT,
    MAX_SYLLABLES_FOR_MULTIPLE_SEGMENTATIONS,
};
use super::typo_edges::collect_typo_edges;

pub const CACHE_CAPACITY: usize = 128;
pub const TYPO_SPAN_CACHE_CAPACITY: usize = 512;
/// A single-letter query returns at most this many rows until `expand_initial_candidates`.
pub const INITIAL_CANDIDATE_LIMIT: usize = 24;
pub const SPARSE_FALLBACK_THRESHOLD: usize = 8;
/// Twice the series cache's capacity: more keys than that cannot all still be cached.
pub const PERSONAL_SCORED_KEY_LIMIT: usize = 256;
/// The reference's `INT_MAX` row limit, kept as a finite value so every `LIMIT` bind stays in range.
const UNLIMITED_ROWS: usize = i32::MAX as usize;
/// The budget of fuzzy paths one `fuzzy_candidates` call may query, shared by every prefix length (QD:1691).
const FUZZY_PATH_BUDGET: usize = 128;
const FUZZY_ROW_LIMIT: usize = 128;
/// The masks whose lists depend on the typo profile and carry typo sentences.
const TYPO_EDGE_TYPES: u32 = autocorrect_type::TRANSPOSITION
    | autocorrect_type::NEIGHBOR
    | autocorrect_type::MISSING_OR_EXTRA;

#[derive(Clone)]
struct CachedResolution {
    key: String,
    resolution: SeriesResolution,
}

#[derive(Clone)]
struct CachedFuzzyCandidates {
    rules: u32,
    segmentation: String,
    candidates: Vec<WordItem>,
}

/// The reference answered a failed write with `ERROR_CODE` and no message; callers map any failure to their own diagnostic.
const DICTIONARY_UNAVAILABLE: &str = "Pinyin dictionary is unavailable";
const ENTRY_REJECTED: &str = "Pinyin does not spell the word one syllable per character";

pub struct QuanpinDictionary {
    database: PinyinDatabase,
    paths: RuntimePaths,
    typo_profile: Arc<PersonalTypoProfile>,
    /// The profile generation the cached lists were built under.
    typo_generation: u64,
    personal: Arc<PersonalNgramStore>,
    /// The personal model version the personal-scored lists were built under.
    personal_version: u64,
    /// Series keys whose lists were scored with the personal model, dropped when its version moves.
    personal_scored_keys: HashSet<String>,
    /// Rows per segmentation (`query_single_path`).
    cache: FifoCache<String, Vec<WordItem>>,
    /// Whole answers per series key, including online rows.
    series_cache: FifoCache<String, Vec<WordItem>>,
    segmentation_cache: FifoCache<String, Vec<String>>,
    /// Survives `reset_cache`: a resolution depends only on the input and the static correction tables, never on dictionary rows.
    resolution_cache: FifoCache<u64, CachedResolution>,
    /// Fuzzy rows are dictionary-dependent, but their composite key is indexed by hash so a cache hit does not allocate a formatted string.
    fuzzy_cache: FifoCache<u64, CachedFuzzyCandidates>,
    /// Rows of typo-variant span keys, empty answers included. Dictionary rows only, so it is cleared with the other caches.
    typo_span_cache: FifoCache<String, Vec<DictRow>>,
    rerankers: Vec<NeuralReranker>,
    sentence_alternatives: bool,
    sentence_association: SentenceAssociationOptions,
    rescoring_context: String,
    /// The reading and mask of the query being answered; marking and the typo edges read them within that query.
    pinyin_segmentation: String,
    /// Every non-primary correction cut, joined; marking compares against them on every query, cached or not.
    alternative_segmentations: Vec<String>,
    current_autocorrect_types: u32,
}

impl QuanpinDictionary {
    /// Opens the generation's `msime.db`, loads the n-gram tables, the personal context store and the typo profile off the keystroke path, and warms the statement cache (QD:231-268).
    pub fn new(paths: &RuntimePaths) -> Self {
        let database = PinyinDatabase::open(&paths.dictionary(MAIN_DICTIONARY));
        database.warm_up();
        // Mapping the n-gram tables and checking they are sorted is a sequential pass over about fifteen megabytes. Left to the first query that wants them it lands on a keystroke; here it joins the work of opening the dictionary, which the host does off the typing path. The same goes for the personal counts.
        NgramTable::shared(&paths.dictionary(BIGRAM_TABLE));
        NgramTable::shared(&paths.dictionary(TRIGRAM_TABLE));
        let journal = paths.user(USER_JOURNAL);
        let personal = PersonalNgramStore::for_journal(&journal);
        drop(personal.model());
        let personal_version = personal.version();
        let typo_profile = PersonalTypoProfile::shared(&journal);
        let typo_generation = typo_profile.generation();
        let mut dictionary = Self {
            database,
            paths: paths.clone(),
            typo_profile,
            typo_generation,
            personal,
            personal_version,
            personal_scored_keys: HashSet::new(),
            cache: FifoCache::new(CACHE_CAPACITY),
            series_cache: FifoCache::new(CACHE_CAPACITY),
            segmentation_cache: FifoCache::new(CACHE_CAPACITY),
            resolution_cache: FifoCache::new(CACHE_CAPACITY),
            fuzzy_cache: FifoCache::new(CACHE_CAPACITY),
            typo_span_cache: FifoCache::new(TYPO_SPAN_CACHE_CAPACITY),
            rerankers: Vec::new(),
            sentence_alternatives: false,
            sentence_association: SentenceAssociationOptions::default(),
            rescoring_context: String::new(),
            pinyin_segmentation: String::new(),
            alternative_segmentations: Vec::new(),
            current_autocorrect_types: 0,
        };
        dictionary.reset_cache_if_database_changed();
        dictionary
    }

    /// `query_exact`, the fuzzy merge and letter-count sort, then autocorrect marking (QD:1711-1735). `segmentation` may be empty.
    pub fn query(
        &mut self,
        raw: &str,
        segmentation: &str,
        autocorrect_types: u32,
        fuzzy: FuzzyPinyinOptions,
    ) -> Vec<WordItem> {
        let mut result = self.query_exact(raw, segmentation, autocorrect_types);
        if fuzzy.rules != 0 && !raw.is_empty() {
            // Fuzzy rows are merged here, not in `query_exact`, so the ordinary cache slots stay free of preference-specific rows. The split is of the request's own segmentation, not the normalised one, so a typed `lue` expands under `lue` (quanpin.md §6).
            let normalized_segments = if segmentation.is_empty() {
                self.resolve_segments(raw, segmentation)
            } else {
                Vec::new()
            };
            let typed = fuzzy_segmentation(segmentation, &normalized_segments);
            let rows = self.fuzzy_candidates(typed.as_ref(), fuzzy);
            append_unique_words(&mut result, rows);
            result.sort_by_key(|item| std::cmp::Reverse(matched_letters(&item.pinyin)));
        }
        // Marking runs after every change, including the fuzzy merge, so every row the caller sees carries its mark.
        mark_autocorrect_candidates(
            &mut result,
            raw,
            &self.pinyin_segmentation,
            &self.alternative_segmentations,
        );
        result
    }

    /// `tbl_1_<c>` rows for a one-letter code, as `WordItem(key, value, weight, Database, key)` with `pinyin = code`.
    pub fn query_initial(&self, code: &str, limit: usize) -> Vec<WordItem> {
        if code.len() != 1 {
            return Vec::new();
        }
        self.database
            .query_initial(code, limit)
            .into_iter()
            .map(|row| {
                WordItem::new(
                    row.key.clone(),
                    row.value,
                    row.weight,
                    CandidateSource::Database,
                    row.key,
                )
            })
            .collect()
    }

    /// Replace the capped 24-row run with every row of the initial (QD:506-558); updates the row cache and the series slot of the query `(raw, segmentation, autocorrect_types)` that produced `candidates`.
    pub fn expand_initial_candidates(
        &mut self,
        raw: &str,
        segmentation: &str,
        autocorrect_types: u32,
        code: &str,
        candidates: &mut Vec<WordItem>,
    ) -> bool {
        if code.len() != 1 {
            return false;
        }
        let is_limited_initial =
            |item: &WordItem| item.source == CandidateSource::Database && item.pinyin == code;
        let limited_count = candidates
            .iter()
            .filter(|item| is_limited_initial(item))
            .count();
        // Exactly the cap: fewer rows means the initial has nothing more to give.
        if limited_count != INITIAL_CANDIDATE_LIMIT {
            return false;
        }
        let mut expanded = self.query_initial(code, UNLIMITED_ROWS);
        if expanded.len() <= limited_count {
            return false;
        }
        for item in &mut expanded {
            item.canonical_pinyin = std::mem::replace(&mut item.pinyin, code.to_string());
        }

        let mut merged = Vec::with_capacity(candidates.len() - limited_count + expanded.len());
        let mut inserted = false;
        for item in candidates.drain(..) {
            if is_limited_initial(&item) {
                if !inserted {
                    merged.extend(expanded.iter().cloned());
                    inserted = true;
                }
                continue;
            }
            merged.push(item);
        }
        *candidates = merged;
        self.cache.insert(code.to_string(), expanded);
        // Keyed without the `C:` prefix, as the reference writes it (QD:554): a one-letter input is never corrected, so the key is the one its query used. The reference read the last query's fields here; the key is recomputed from the caller's query instead, because a caret-prefix decode is a second query on this dictionary and would otherwise receive the whole input's expanded list in its own slot.
        let segments = self.resolve_segments(raw, segmentation);
        let resolution = self.resolution(raw, segmentation, &segments, autocorrect_types);
        let key = self.series_slot(&series_cache_key(
            raw,
            &resolution.segmentation,
            autocorrect_types,
        ));
        self.series_cache.insert(key, candidates.clone());
        true
    }

    /// Fuzzy rows for a segmentation, cached under `fuzzy:<rules>:<segmentation>` (QD:1680-1709). Shuangpin reuses this.
    pub fn fuzzy_candidates(
        &mut self,
        segmentation: &str,
        options: FuzzyPinyinOptions,
    ) -> Vec<WordItem> {
        if options.rules == 0 || !self.database.is_open() {
            return Vec::new();
        }
        self.reset_cache_if_database_changed();
        let hash = fuzzy_cache_hash(options.rules, segmentation);
        if let Some(cached) = self.fuzzy_cache.get_ref(&hash) {
            if fuzzy_cache_key_matches(cached, options.rules, segmentation) {
                return cached.candidates.clone();
            }
        }
        let segments = split_segments(segmentation);
        let mut result = Vec::new();
        let mut budget = FUZZY_PATH_BUDGET;
        for count in (1..=segments.len()).rev() {
            if budget <= 1 {
                break;
            }
            let prefix = &segments[..count];
            let paths = fuzzy_segmentations(prefix, options, FUZZY_SEGMENTATION_LIMIT.min(budget));
            if paths.is_empty() {
                continue;
            }
            budget -= paths.len();
            let typed = join_segments(prefix);
            let rows = self
                .database
                .query_exact_segmentations_keyed_flat(&paths, FUZZY_ROW_LIMIT);
            result.reserve(rows.len());
            result.extend(rows.into_iter().map(|row| {
                let mut item = WordItem::new(
                    typed.clone(),
                    row.value,
                    row.weight,
                    CandidateSource::Database,
                    row.key,
                );
                item.fuzzy = true;
                item
            }));
        }
        self.fuzzy_cache.insert(
            hash,
            CachedFuzzyCandidates {
                rules: options.rules,
                segmentation: segmentation.to_owned(),
                candidates: result.clone(),
            },
        );
        result
    }

    /// `replace_online_candidate_batch` on the series cache slot of `(raw, segmentation, autocorrect_types)` (QD:1737-1773).
    pub fn insert_online_words(
        &mut self,
        raw: &str,
        segmentation: &str,
        autocorrect_types: u32,
        words: &[String],
        source: CandidateSource,
    ) -> bool {
        if raw.is_empty() || words.is_empty() {
            return false;
        }
        // The key is recomputed exactly as the query computed it, so the rows land in the slot the next refresh reads.
        let segments = self.resolve_segments(raw, segmentation);
        let resolution = self.resolution(raw, segmentation, &segments, autocorrect_types);
        let key = self.series_slot(&resolution.cache_key);
        let mut list = self.series_cache.get(&key).unwrap_or_default();
        if !replace_online_candidate_batch(&mut list, raw, words, source) {
            return false;
        }
        self.series_cache.insert(key, list);
        true
    }

    pub fn find_candidate(&self, key: &str, value: &str) -> Option<WordItem> {
        self.database
            .find_weight(key, value)
            .map(|weight| WordItem::new(key, value, weight, CandidateSource::Database, key))
    }

    pub fn knows_han_char(&self, han: &str) -> bool {
        self.database.han_char_exists(han)
    }

    /// Umlaut-normalised canonical key, one complete syllable per Han character, no re-cut; an existing row is OK without a journal write (QD:1215-1268).
    pub fn create_word_from_canonical_pinyin(&mut self, pinyin: &str, word: &str) -> Result<()> {
        // Online rows arrive with the typed spelling; they are stored under the nve/lve/ju keys queries resolve to.
        let mut segments = split_segments(pinyin);
        normalize_umlaut_aliases(&mut segments);
        // The caller gave an explicit segmentation, and these checks already prove one complete syllable per character. Validating through a re-cut would erase the boundaries and cut qi'e'huan as qie'huan, rejecting a valid phrase.
        if segments.is_empty()
            || segments.len() != count_han_chars(word)
            || segments
                .iter()
                .any(|segment| segment.is_empty() || !is_complete_pinyin_input(segment))
        {
            return Err(EngineError::invalid(ENTRY_REJECTED));
        }
        self.insert_new_word(&join_segments(&segments), word)
    }

    /// A change clears the row and series caches.
    pub fn set_sentence_alternatives(&mut self, enabled: bool) {
        if self.sentence_alternatives == enabled {
            return;
        }
        self.sentence_alternatives = enabled;
        // The cached lists were assembled under the previous answer.
        self.cache.clear();
        self.series_cache.clear();
    }

    /// Loads the keyboard sentence model from the resource bundle when its switch is on. The desktop model is not the engine's: only the runtime's settled reranker runs it. A change resets the caches (neural-association.patch:2667-2682), as shuangpin does, so a cached online row does not come back after the options are switched away and back.
    pub fn set_sentence_association(&mut self, options: SentenceAssociationOptions) {
        if self.sentence_association == options {
            return;
        }
        self.sentence_association = options;
        self.rerankers.clear();
        if options.neural_keyboard {
            if let Some(model) = shared_sentence_model(&self.paths.resource(NEURAL_MODEL_KEYBOARD))
            {
                self.rerankers
                    .push(NeuralReranker::new(CandidateSource::NeuralKeyboard, model));
            }
        }
        self.reset_cache();
    }

    /// Keeps the part of the committed text the rerankers read; while they run it is part of every series slot (`series_slot`).
    pub fn set_rescoring_context(&mut self, context: &str) {
        // The models only ever see the last `CONTEXT_CHARACTERS`, so committed text beyond that window changes nothing.
        let trimmed = last_characters(context, CONTEXT_CHARACTERS);
        if self.rescoring_context != trimmed {
            self.rescoring_context = trimmed.to_string();
        }
    }

    /// The series cache slot of a resolution's key (overlays.md §1.6.2). The association switches decide which sentence rows a list carries, so they are always part of the slot. The reference bypassed the cache while a neural model ran, because its scores came from an asynchronous worker; scoring here is synchronous, so a reranked list is cached under the trimmed context it was scored with and a commit moves the next query to a fresh slot instead of clearing every list.
    fn series_slot(&self, cache_key: &str) -> String {
        series_slot_key(
            cache_key,
            self.sentence_association,
            !self.rerankers.is_empty(),
            &self.rescoring_context,
        )
    }

    /// Clears everything except the resolution memo (QD:1400-1407).
    pub fn reset_cache(&mut self) {
        self.cache.clear();
        self.series_cache.clear();
        self.personal_scored_keys.clear();
        self.segmentation_cache.clear();
        self.fuzzy_cache.clear();
        self.typo_span_cache.clear();
    }

    /// QD:320-482.
    fn query_exact(&mut self, raw: &str, segmentation: &str, types: u32) -> Vec<WordItem> {
        if raw.is_empty() {
            return Vec::new();
        }
        self.current_autocorrect_types = types;
        if types & TYPO_EDGE_TYPES != 0 {
            // Only a stat per query unless the journal changed. Lists built without a mask never depend on the profile.
            self.typo_profile.refresh_if_changed();
            let generation = self.typo_profile.generation();
            if generation != self.typo_generation {
                self.typo_generation = generation;
                self.series_cache.clear();
                self.personal_scored_keys.clear();
            }
        }
        let segments = self.resolve_segments(raw, segmentation);
        let resolution = self.resolution(raw, segmentation, &segments, types);
        self.pinyin_segmentation = resolution.segmentation.clone();
        // Published even on a cache hit: marking runs after every query. Costlier readings are corrections too, so they are marked although they rank below the primary tier.
        self.alternative_segmentations = resolution
            .alternative_corrected_cuts
            .iter()
            .chain(&resolution.costlier_corrected_cuts)
            .map(|cut| join_segments(cut))
            .collect();

        self.drop_personal_scored_results();
        let slot = self.series_slot(&resolution.cache_key);
        if self.series_cache.contains(&slot) {
            self.reset_cache_if_database_changed();
            if let Some(cached) = self.series_cache.get(&slot) {
                return cached;
            }
        }

        let alternatives = alternative_segmentations(raw, &segments, &resolution, types);
        let primary_segmentation = resolution.segmentation.clone();
        let mut result;
        if resolution.corrected_input {
            // The corrected reading is the primary key, so selection and weight updates land on the right rows; the literal reading stays behind it as a fallback tail.
            result = self.query_series(raw, &primary_segmentation, &resolution.corrected);
            let literal = if segmentation.is_empty() {
                let literal_segmentation = join_segments(&segments);
                self.query_series(raw, &literal_segmentation, &segments)
            } else {
                self.query_series(raw, segmentation, &segments)
            };
            append_unique_words(&mut result, literal);
            // Same-cost readings of the typo compete with the primary cut on dictionary frequency, and the best keeps a protected slot near the top.
            if !alternatives.is_empty() {
                result = self.merge_alternatives(
                    raw,
                    &primary_segmentation,
                    &resolution.corrected,
                    &alternatives,
                    result,
                );
            }
            // Costlier readings go below the whole primary tier, so a frequent dearer correction (gau -> gai, weight 13) never precedes the cheaper one (gau -> gua, weight 10).
            for costlier in &resolution.costlier_corrected_cuts {
                let rows = self.query_series(raw, &join_segments(costlier), costlier);
                append_unique_words(&mut result, rows);
            }
        } else {
            result = self.query_series(raw, &primary_segmentation, &segments);
            if !alternatives.is_empty() {
                result = self.merge_alternatives(
                    raw,
                    &primary_segmentation,
                    &segments,
                    &alternatives,
                    result,
                );
            }
        }
        self.series_cache.insert(slot.clone(), result.clone());
        // Only keys long enough for the sentence lattice carry personal scores.
        if segments.len() >= 2 || resolution.corrected.len() >= 2 {
            self.remember_personal_scored_key(slot);
        }
        result
    }

    /// The database half of `merge_alternative_segmentations` (QD:888-945); `series` has the promotion and union rule.
    fn merge_alternatives(
        &mut self,
        raw: &str,
        primary_segmentation: &str,
        primary_segments: &[String],
        alternatives: &[Vec<String>],
        result: Vec<WordItem>,
    ) -> Vec<WordItem> {
        let rows = self.database.query_exact_segmentations_keyed_flat(
            alternatives,
            ALTERNATIVE_SEGMENTATION_CANDIDATE_LIMIT,
        );
        if rows.is_empty() {
            return result;
        }
        let primary_full = self.query_single_path(raw, primary_segmentation, primary_segments);
        let alternative_full = rows
            .into_iter()
            .map(|row| {
                WordItem::new(
                    row.key.clone(),
                    row.value,
                    row.weight,
                    CandidateSource::Database,
                    row.key,
                )
            })
            .collect();
        merge_alternative_segmentations(result, primary_full, alternative_full)
    }

    fn append_query_rows(result: &mut Vec<WordItem>, rows: Vec<WordItem>) {
        result.reserve(rows.len());
        result.extend(rows);
    }

    /// QD:560-730 without the Google sentence: prefix groups longest first, the lattice block, the typo sentence, sparse fallbacks.
    fn query_series(
        &mut self,
        raw: &str,
        segmentation: &str,
        segments: &[String],
    ) -> Vec<WordItem> {
        if segments.is_empty() {
            return self.query_single_path(raw, segmentation, segments);
        }
        let mut result = Vec::new();
        let mut partial_segmentation = join_segments(segments);
        for count in (1..=segments.len()).rev() {
            if count < segments.len() {
                truncate_last_segment(&mut partial_segmentation);
            }
            let partial = &segments[..count];
            let mut rows = self.query_single_path(raw, &partial_segmentation, partial);
            if count == segments.len() {
                // The same-length table often holds only leftovers: all of ping'guo is 苹果 1143881, 评过 1180, 平果 169, 平锅 1, while 苹果电脑 21495 and 苹果公司 19725 sit in tbl_4_p, which a same-length query never sees, so the second seat went to a rare homophone. Continuations join the group by weight, which comes from one corpus and compares across tables. `pinyin` stays the typed string (composition advance consumes only what was typed) and `canonical_pinyin` keeps the full reading for persistence.
                let longer = self.longer_phrase_candidates(&partial_segmentation, partial);
                if !longer.is_empty() {
                    rows.extend(longer);
                    rows.sort_by_key(|item| std::cmp::Reverse(item.weight));
                }
            }
            Self::append_query_rows(&mut result, rows);
        }

        if segments.len() >= 2 && has_only_complete_pinyin_segments(segments) {
            self.merge_sentences(&mut result, raw, segmentation, segments);
        }

        if result.len() < SPARSE_FALLBACK_THRESHOLD {
            for fallback in sparse_pinyin_fallback_segments(segments) {
                if fallback.is_empty() {
                    continue;
                }
                let fallback_segmentation = join_segments(&fallback);
                let rows = self.query_single_path(raw, &fallback_segmentation, &fallback);
                append_unique_words(&mut result, rows);
            }
        }
        result
    }

    /// The lattice sentence block and the typo sentence (QD:644-721, overlays.md §1.6.2). Both rank below a dictionary row that already answers the whole key, which is what `whole_sentence_insert_position` computes; inserting at index 0 once turned 百依百顺 into 白一百顺 and 颁布实施 into 版不是是.
    fn merge_sentences(
        &mut self,
        result: &mut Vec<WordItem>,
        raw: &str,
        segmentation: &str,
        segments: &[String],
    ) {
        let association = self.sentence_association;
        if !association.word_lattice && self.rerankers.is_empty() {
            return;
        }
        // Held for the whole merge: the options borrow the model.
        let personal = Arc::clone(&self.personal);
        let model = personal.model();
        let mut options = make_sentence_lattice_options(&self.paths, self.sentence_alternatives);
        if !self.rerankers.is_empty() {
            options.nbest = MAX_RERANK_PATHS;
        }
        options.include_lattice_best = association.word_lattice;
        options.show_next_on_duplicate = association.show_next_on_duplicate;
        options.personal = Some(&*model);

        let typed = if segmentation.is_empty() {
            raw
        } else {
            segmentation
        };
        let types = self.current_autocorrect_types;
        let typo_eligible = types & TYPO_EDGE_TYPES != 0 && !raw.contains('\'');
        let span_limit = options.span_limit;
        let database = &self.database;
        let span_cache = &mut self.typo_span_cache;
        let profile = &*self.typo_profile;
        let mut lookup = |span: &[String]| database.query_lattice_span(span, span_limit);
        let mut typo_edges = |literal_best: &SentencePath| {
            collect_typo_edges(database, span_cache, profile, segments, literal_best, types)
        };
        let typo_source: Option<&mut TypoEdgeSource<'_>> = if typo_eligible {
            Some(&mut typo_edges)
        } else {
            None
        };
        let typo = merge_lattice_candidates(
            result,
            segments,
            &mut lookup,
            typed,
            &options,
            typo_source,
            &mut self.rerankers,
            &self.rescoring_context,
        );

        // A sentence that reads one or more syllables as a typo of another. It leads only when it beats the literal sentence by the lead margin per typo; otherwise it follows the whole-sentence rows, so a correction is offered without displacing what was typed.
        let Some(typo) = typo else {
            return;
        };
        if result.iter().any(|item| item.word == typo.sentence) {
            return;
        }
        let mut at = whole_sentence_insert_position(result, segments);
        if !typo.leads(options.typo_lead_margin) {
            while at < result.len() && result[at].sentence_association {
                at += 1;
            }
        }
        let mut sentence = WordItem::new(
            typed,
            typo.sentence,
            (typo.score * 1000.0) as i64,
            CandidateSource::Generated,
            typo.key,
        );
        sentence.sentence_association = true;
        sentence.sentence_words = typo.words;
        sentence.corrected_from = fold_reading(raw);
        result.insert(at, sentence);
    }

    /// Whole-syllable continuations of complete segments (QD:732-749): typed segmentation as `pinyin`, the longer key as canonical.
    fn longer_phrase_candidates(&self, segmentation: &str, segments: &[String]) -> Vec<WordItem> {
        self.database
            .query_longer_phrases(segments, LONGER_PHRASE_EXTRA_SYLLABLES, LONGER_PHRASE_LIMIT)
            .into_iter()
            .map(|row| {
                WordItem::new(
                    segmentation,
                    row.value,
                    row.weight,
                    CandidateSource::Database,
                    row.key,
                )
            })
            .collect()
    }

    /// Dictionary rows for one segmentation, cached by segmentation (QD:751-765). With the Google decoder gone an empty lookup stays empty.
    fn query_single_path(
        &mut self,
        raw: &str,
        segmentation: &str,
        segments: &[String],
    ) -> Vec<WordItem> {
        let cache_key = path_cache_key(raw, segmentation);
        if let Some(cached) = self.cache.get_ref_by(cache_key) {
            return cached.clone();
        }
        let result = self.query_database(segments, segmentation);
        self.cache.insert(cache_key.to_owned(), result.clone());
        result
    }

    /// QD:830-869: a lone letter reads its initial table capped at 24 rows; anything else runs the cascade.
    fn query_database(&self, segments: &[String], segmentation: &str) -> Vec<WordItem> {
        if segments.len() == 1 && segments[0].len() == 1 {
            let matched_code = if segmentation.is_empty() {
                segments[0].as_str()
            } else {
                segmentation
            };
            let mut rows = self.query_initial(&segments[0], INITIAL_CANDIDATE_LIMIT);
            for item in &mut rows {
                item.canonical_pinyin =
                    std::mem::replace(&mut item.pinyin, matched_code.to_string());
            }
            return rows;
        }
        // The segments were normalised once in `resolve_segments`, so the lookup uses the standard keys directly.
        let rows =
            self.database
                .query_segments_keyed_flat(segments, UNLIMITED_ROWS, QuerySource::Quanpin);
        if segmentation.is_empty() {
            let code = join_segments(segments);
            rows.into_iter()
                .map(|row| {
                    WordItem::new(
                        &code,
                        row.value,
                        row.weight,
                        CandidateSource::Database,
                        row.key,
                    )
                })
                .collect()
        } else {
            rows.into_iter()
                .map(|row| {
                    WordItem::new(
                        segmentation,
                        row.value,
                        row.weight,
                        CandidateSource::Database,
                        row.key,
                    )
                })
                .collect()
        }
    }

    /// The single normalisation choke point (QD:767-775): an explicit segmentation and the automatic cut both pass here, so the segments entering the pipeline are alias-normalised exactly once.
    fn resolve_segments(&mut self, raw: &str, segmentation: &str) -> Vec<String> {
        let mut segments = if segmentation.is_empty() {
            self.computed_segments(raw)
        } else {
            split_segments(segmentation)
        };
        normalize_umlaut_aliases(&mut segments);
        segments
    }

    /// The first correction-mode cut of the raw input, cached (QD:777-788).
    fn computed_segments(&mut self, raw: &str) -> Vec<String> {
        if let Some(cached) = lookup_cached_segments(&self.segmentation_cache, raw) {
            return cached;
        }
        let segments = first_correction_cut(raw).unwrap_or_default();
        let key = raw.to_owned();
        self.segmentation_cache.insert(key, segments.clone());
        segments
    }

    /// `resolve_series_query` memoised on its whole input tuple (QD:354-364): the k-best search is the expensive part of a keystroke, and a re-typed or backspaced prefix reuses it.
    fn resolution(
        &mut self,
        raw: &str,
        segmentation: &str,
        segments: &[String],
        types: u32,
    ) -> SeriesResolution {
        let hash = resolution_cache_hash(types, raw, segmentation);
        if let Some(cached) = self.resolution_cache.get_ref(&hash) {
            if resolution_cache_key_matches(&cached.key, types, raw, segmentation) {
                return cached.resolution.clone();
            }
        }
        let resolution = resolve_series_query(raw, segments, types);
        let key = format!("{types}\u{1f}{raw}\u{1f}{segmentation}");
        self.resolution_cache.insert(
            hash,
            CachedResolution {
                key,
                resolution: resolution.clone(),
            },
        );
        resolution
    }

    /// Insert a row that is not there yet and journal it as the user's own (QD:1200-1212, QD:1234-1266). A row already present is success without a journal write.
    fn insert_new_word(&mut self, key: &str, word: &str) -> Result<()> {
        if !self.database.is_open() {
            return Err(EngineError::failed(DICTIONARY_UNAVAILABLE));
        }
        if self.database.find_weight(key, word).is_some() {
            return Ok(());
        }
        self.database.insert_word(key, word)?;
        // The reference ignored a failed journal write. The row is in the working dictionary either way, so the caches are reset first; the error still reaches the caller, because a row missing from the journal is lost at the next generation.
        let journaled = record_user_insert(
            &self.paths.user(USER_JOURNAL),
            PersonalDictionaryKind::Pinyin,
            key,
            word,
            INSERTED_WEIGHT,
            "",
        );
        self.reset_cache();
        journaled
    }

    /// QD:1409-1432.
    fn reset_cache_if_database_changed(&mut self) {
        if self.database.database_changed() {
            self.reset_cache();
        }
    }

    /// QD:283-293.
    fn drop_personal_scored_results(&mut self) {
        // Reading the model reloads it when another store or process changed the tables.
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

    /// QD:271-281.
    fn remember_personal_scored_key(&mut self, key: String) {
        self.personal_scored_keys.insert(key);
        // The series cache evicts on its own, so keys it no longer holds are dropped here; otherwise the set grows for as long as the model's version stays put.
        if self.personal_scored_keys.len() > PERSONAL_SCORED_KEY_LIMIT {
            let series_cache = &self.series_cache;
            self.personal_scored_keys
                .retain(|key| series_cache.contains(key));
        }
    }
}

fn truncate_last_segment(segmentation: &mut String) {
    if let Some(boundary) = segmentation.rfind('\'') {
        segmentation.truncate(boundary);
    } else {
        segmentation.clear();
    }
}

fn series_slot_key(
    cache_key: &str,
    options: SentenceAssociationOptions,
    has_rerankers: bool,
    rescoring_context: &str,
) -> String {
    let context_suffix = if has_rerankers {
        rescoring_context.len() + 1
    } else {
        0
    };
    let mut slot = String::with_capacity(cache_key.len() + 5 + context_suffix);
    slot.push_str(cache_key);
    slot.push('\u{1f}');
    slot.push('S');
    slot.push(if options.word_lattice { '1' } else { '0' });
    slot.push(if options.neural_keyboard { '1' } else { '0' });
    slot.push(if options.show_next_on_duplicate {
        '1'
    } else {
        '0'
    });
    if has_rerankers {
        slot.push('\u{1f}');
        slot.push_str(rescoring_context);
    }
    slot
}

fn fuzzy_segmentation<'a>(segmentation: &'a str, normalized_segments: &[String]) -> Cow<'a, str> {
    if segmentation.is_empty() {
        Cow::Owned(join_segments(normalized_segments))
    } else {
        Cow::Borrowed(segmentation)
    }
}

fn fuzzy_cache_hash(rules: u32, segmentation: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    rules.hash(&mut hasher);
    segmentation.hash(&mut hasher);
    hasher.finish()
}

fn fuzzy_cache_key_matches(cached: &CachedFuzzyCandidates, rules: u32, segmentation: &str) -> bool {
    cached.rules == rules && cached.segmentation == segmentation
}

fn path_cache_key<'a>(raw: &'a str, segmentation: &'a str) -> &'a str {
    if segmentation.is_empty() {
        raw
    } else {
        segmentation
    }
}

fn lookup_cached_segments(
    cache: &FifoCache<String, Vec<String>>,
    key: &str,
) -> Option<Vec<String>> {
    cache.get_ref_by(key).cloned()
}

fn resolution_cache_hash(types: u32, raw: &str, segmentation: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    types.hash(&mut hasher);
    raw.hash(&mut hasher);
    segmentation.hash(&mut hasher);
    hasher.finish()
}

fn resolution_cache_key_matches(key: &str, types: u32, raw: &str, segmentation: &str) -> bool {
    let mut parts = key.split('\u{1f}');
    parts.next().and_then(|part| part.parse().ok()) == Some(types)
        && parts.next() == Some(raw)
        && parts.next() == Some(segmentation)
        && parts.next().is_none()
}

/// The readings that compete with the primary one (QD:394-443): with a mask, the same-cost corrections and then every correction-mode cut; for a short all-complete input, every complete segmentation of the letters. Deduplicated against the primary and the costlier cuts, at most 32.
fn alternative_segmentations(
    raw: &str,
    segments: &[String],
    resolution: &SeriesResolution,
    types: u32,
) -> Vec<Vec<String>> {
    let mut alternatives: Vec<Vec<String>> = Vec::with_capacity(SYLLABLE_GRAPH_PATH_LIMIT);
    // Seeding the costlier cuts keeps them out of the frequency-competing tier; they are appended after it.
    let mut seen = HashSet::with_capacity(
        1usize
            .saturating_add(resolution.costlier_corrected_cuts.len())
            .saturating_add(SYLLABLE_GRAPH_PATH_LIMIT),
    );
    let primary_segmentation = resolution.segmentation.as_str();
    seen.extend(
        resolution
            .costlier_corrected_cuts
            .iter()
            .map(|cut| join_segments(cut)),
    );
    let mut append = |candidate: &[String]| {
        let key = join_segments(candidate);
        if !key.is_empty()
            && !is_duplicate_segmentation(primary_segmentation, &seen, &key)
            && alternatives.len() < SYLLABLE_GRAPH_PATH_LIMIT
        {
            seen.insert(key);
            alternatives.push(candidate.to_vec());
        }
    };
    // Correction-mode alternatives follow the autocorrect switch; offering them unconditionally made a misspelled "sahng" produce the corrected word with autocorrection turned off. Correction alternatives go first: they explain the letters actually typed, which the alias readings after them do not.
    if types != 0 {
        for cut in &resolution.alternative_corrected_cuts {
            append(cut);
        }
        for cut in cut_pinyin_by_mode(raw, CutMode::Correction) {
            append(&cut);
        }
    }
    if !raw.contains('\'')
        && segments.len() <= MAX_SYLLABLES_FOR_MULTIPLE_SEGMENTATIONS
        && has_only_complete_pinyin_segments(segments)
    {
        let graph = build_syllable_graph(raw);
        for path in enumerate_complete_segmentations(&graph, SYLLABLE_GRAPH_PATH_LIMIT) {
            append(&path);
        }
    }
    alternatives
}

fn is_duplicate_segmentation(primary: &str, seen: &HashSet<String>, key: &str) -> bool {
    key == primary || seen.contains(key)
}

/// Letters of a row's matched code, what the fuzzy merge sorts by.
fn matched_letters(pinyin: &str) -> usize {
    pinyin.bytes().filter(|&byte| byte != b'\'').count()
}

fn first_correction_cut(pinyin: &str) -> Option<Vec<String>> {
    cut_pinyin_by_mode(pinyin, CutMode::Correction)
        .into_iter()
        .next()
}

#[cfg(test)]
mod tests;
