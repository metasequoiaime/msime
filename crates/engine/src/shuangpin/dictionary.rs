//! `ShuangpinDictionary` (schemes-lang.md §1.7): series generation over decoded quanpin segments, the helpcode caches (overlays.md §5.1), and online rows. Pins, removals, frequency learning and phrases are written by the session through `user_dictionary` (see `ime::registry`), so the reference's word writers are not ported here. The Google sentence lines are gone; the lattice sentence goes in at `whole_sentence_insert_position` with nothing extra (overlays.md §2.2).

use std::collections::HashSet;
use std::sync::Arc;

use super::query::{remove_manual_delimiters, trim_trailing_letters_preserve_delimiters};
use super::typo_edges::{collect_shuangpin_typo_edges, SHUANGPIN_TYPO_TYPES};
use super::utils::{convert_seg_shuangpin_to_seg_complete_pinyin, pinyin_segmentation};
use super::ShuangpinProfile;
use crate::assets;
use crate::cache::FifoCache;
use crate::dictionary::pinyin::PinyinDatabase;
use crate::dictionary::DictRow;
use crate::helpcode::{
    match_single_helpcode, matches_double_helpcodes, HelpcodeKeymap, SingleHelpcodeMatch,
};
use crate::ime::online_batch::replace_online_candidate_batch;
use crate::lattice::decode::make_sentence_lattice_options;
use crate::lattice::merge::{merge_lattice_candidates, whole_sentence_insert_position};
use crate::lattice::neural::{
    shared_sentence_model, NeuralReranker, CONTEXT_CHARACTERS, MAX_RERANK_PATHS,
};
use crate::lattice::ngram::NgramTable;
use crate::lattice::{SentencePath, TypoEdgeSource};
use crate::ordering::apply_order;
use crate::paths::RuntimePaths;
use crate::pinyin::jianpin::QuerySource;
use crate::pinyin::segment::split_segments;
use crate::quanpin::dictionary::{
    CACHE_CAPACITY, INITIAL_CANDIDATE_LIMIT, TYPO_SPAN_CACHE_CAPACITY,
};
use crate::quanpin::series::fold_reading;
use crate::text::last_characters;
use crate::types::{CandidateSource, SentenceAssociationOptions, WordItem};
use crate::user_dictionary::ngram_store::PersonalNgramStore;

// 双拼前缀候选的短合并直接扫描已有词，避免临时哈希表和重复索引分配。
const SMALL_PREFIX_DEDUP: usize = 64;
// 单字辅助码的常见候选批次很短，排列索引放在栈上即可。
const SMALL_SINGLE_HELP_CODE_ORDER: usize = 64;

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
    /// 纠错整句查过的变体跨度键的词典行，空结果也存。只存词典行，随其他缓存一起清空。
    typo_span_cache: FifoCache<String, Vec<DictRow>>,
    /// 当前请求的纠错类型（会话的「拼音纠错」开关，见 `ShuangpinEngine::query`）。改变时清掉整句答案的缓存，它们是按另一个设置组出来的。
    autocorrect_types: u32,
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
            typo_span_cache: FifoCache::new(TYPO_SPAN_CACHE_CAPACITY),
            autocorrect_types: 0,
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
        append_prefix_rows(&mut candidates, prefix_rows);

        let segments = split_segments(&convert_seg_shuangpin_to_seg_complete_pinyin(
            segmentation,
            self.profile,
        ));
        self.merge_sentences(&mut candidates, &segments, segmentation, pure, rows_key);

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

    /// 整句块（SD:261-269，去掉了 Google 整句和它的重排，overlays.md §1.6.2）：`word_lattice` 开着时放词网格最优句（Generated），每个启用的神经重排器各一行，由合并本身插入。纠错开着时再接一条纠错整句，排在整句块之后。
    fn merge_sentences(
        &mut self,
        candidates: &mut Vec<WordItem>,
        segments: &[String],
        segmentation: &str,
        typed: &str,
        rows_key: &str,
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
        let codes = typo_codes(self.autocorrect_types, segments, segmentation, rows_key);
        let types = self.autocorrect_types;
        let profile = self.profile;
        let span_cache = &mut self.typo_span_cache;
        let mut typo_edges = |literal_best: &SentencePath| {
            collect_shuangpin_typo_edges(
                database,
                span_cache,
                profile,
                &codes,
                segments,
                literal_best,
                types,
            )
        };
        let typo_source: Option<&mut TypoEdgeSource<'_>> = if codes.is_empty() {
            None
        } else {
            Some(&mut typo_edges)
        };
        let typo = merge_lattice_candidates(
            candidates,
            segments,
            &mut lookup,
            typed,
            &options,
            typo_source,
            &mut self.rerankers,
            &self.rescoring_context,
        );

        // 纠错整句不抢首选：不论比字面整句好多少，一律排在全码词条和整句块之后，只作为一个可选的改正。
        let Some(typo) = typo else {
            return;
        };
        if candidates.iter().any(|item| item.word == typo.sentence) {
            return;
        }
        let Some(at) = typo_sentence_seat(candidates, segments) else {
            return;
        };
        let mut sentence = WordItem::new(
            typed,
            typo.sentence,
            (typo.score * 1000.0) as i64,
            CandidateSource::Generated,
            typo.key,
        );
        sentence.sentence_association = true;
        sentence.sentence_words = typo.words;
        sentence.corrected_from = fold_reading(typed);
        candidates.insert(at, sentence);
    }

    /// Matched groups, then the full input read as pinyin, then the unmatched rows (SD:324-375).
    fn filter_with_single_helpcode(
        &mut self,
        candidates: Vec<WordItem>,
        help_code: &str,
        raw: &str,
        keymap: &HelpcodeKeymap,
    ) -> Vec<WordItem> {
        if candidates.is_empty() || help_code.len() != 1 {
            return Vec::new();
        }
        let prefer_last = help_code.as_bytes()[0].is_ascii_uppercase();
        let normalized = help_code.to_ascii_lowercase();
        let matches: Vec<_> = candidates
            .iter()
            .map(|candidate| match_single_helpcode(&candidate.word, &normalized, keymap))
            .collect();
        let (mut result, unmatched) =
            reorder_single_helpcode_rows(candidates, &matches, prefer_last);
        // The whole raw input, the last letter read as pinyin instead of a helpcode. The reference segments the raw input as one chunk, `'` included; the conversion drops the empty pieces that leaves (SD:370-372).
        let original_segmentation = pinyin_segmentation(raw, self.profile);
        let whole = self.generate_series(raw, &original_segmentation, "");
        // The reference appended the whole-input answer and then the unmatched rows whole, so a word already among the matched rows, or in both lists, was listed again; a word already listed keeps its first seat.
        append_helpcode_rows(&mut result, whole, unmatched);
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

        // 参考实现把基础部分按去掉 `'` 的字母缓存（SD:458）。纠错整句让答案依赖用户是否手打了 `'`（手打的不纠），所以基础部分改按保留 `'` 的原始输入缓存和准入，与直接打这段基础输入得到同一份答案；没有 `'` 时它就是原来的字母键。
        let base_raw = trim_trailing_letters_preserve_delimiters(raw, help_codes.len());
        let candidates = self.generate_series(pure, segmentation, &base_raw);
        let result = match help_codes.len() {
            1 => self.filter_with_single_helpcode(candidates, help_codes, raw, keymap),
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

    /// Remove one provider's rows from every cached answer while retaining the other provider and dictionary rows.
    pub fn clear_online_candidates(&mut self, source: CandidateSource) {
        for cache in [
            &mut self.series_cache,
            &mut self.single_helpcode_cache,
            &mut self.reversed_single_helpcode_cache,
            &mut self.double_helpcode_cache,
        ] {
            cache.retain_mut(|_, rows| {
                rows.retain(|item| item.source != source);
                !rows.is_empty()
            });
        }
    }

    pub fn find_candidate(&self, key: &str, value: &str) -> Option<WordItem> {
        self.database
            .find_weight(key, value)
            .map(|weight| WordItem::new(key, value, weight, CandidateSource::Database, key))
    }

    /// 纠错类型变了，整句答案和由它们得出的辅助码答案都是按另一个设置组出来的，一起清掉；按键行和变体跨度行与设置无关，留着。
    pub fn set_autocorrect_types(&mut self, types: u32) {
        let types = types & SHUANGPIN_TYPO_TYPES;
        if self.autocorrect_types == types {
            return;
        }
        self.autocorrect_types = types;
        self.series_cache.clear();
        self.single_helpcode_cache.clear();
        self.reversed_single_helpcode_cache.clear();
        self.double_helpcode_cache.clear();
        self.personal_scored_keys.clear();
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
        self.typo_span_cache.clear();
    }
}

/// 纠错整句的座位：全码词条和整句块之后，且永远不是第一位。整句块可能是空的（词网格关着、键盘重排器又没给出一条），这时全码词条之后就是第 0 位，纠错行会顶到首选，被空格直接上屏，所以至少让出第一位；表是空的就不给纠错行，免得它成为唯一也是首个候选。
fn typo_sentence_seat(candidates: &[WordItem], segments: &[String]) -> Option<usize> {
    if candidates.is_empty() {
        return None;
    }
    let mut at = whole_sentence_insert_position(candidates, segments);
    while at < candidates.len() && candidates[at].sentence_association {
        at += 1;
    }
    Some(at.max(1))
}

/// 纠错整句要求每个音节恰好是一对键，才知道该换哪个键；用户手打的 `'` 表示按字面切分，和全拼一样不纠。`rows_key` 是这次答案的缓存键（带 `'` 的原始输入或去掉分隔的按键），所以准入只由缓存键和切分决定，命中缓存的答案与重新计算的一致。不可纠时返回空。
fn typo_codes<'a>(
    types: u32,
    segments: &[String],
    segmentation: &'a str,
    rows_key: &str,
) -> Vec<&'a str> {
    if types & SHUANGPIN_TYPO_TYPES == 0 || rows_key.contains('\'') {
        return Vec::new();
    }
    let codes: Vec<&str> = segmentation.split('\'').collect();
    if codes.len() != segments.len() || codes.iter().any(|code| code.len() != 2) {
        return Vec::new();
    }
    codes
}

fn prefix_group_count(segmentation: &str) -> usize {
    segmentation.matches('\'').count()
}

fn append_prefix_rows(candidates: &mut Vec<WordItem>, prefix_rows: Vec<Vec<WordItem>>) {
    let total = candidates.len().saturating_add(
        prefix_rows
            .iter()
            .fold(0usize, |total, rows| total.saturating_add(rows.len())),
    );
    if total <= SMALL_PREFIX_DEDUP {
        candidates.reserve(total.saturating_sub(candidates.len()));
        for rows in prefix_rows {
            for item in rows {
                if candidates.iter().any(|existing| existing.word == item.word) {
                    continue;
                }
                candidates.push(item);
            }
        }
        return;
    }
    // 先把各组移入同一缓冲，只建立一个重复索引列表；释放借用后再原地压缩。
    let added = total.saturating_sub(candidates.len());
    candidates.reserve(added);
    let original_len = candidates.len();
    for rows in prefix_rows {
        candidates.extend(rows);
    }
    let mut listed = HashSet::with_capacity(candidates.len());
    listed.extend(
        candidates[..original_len]
            .iter()
            .map(|item| item.word.as_str()),
    );
    let mut unique = Vec::with_capacity(added);
    for (index, item) in candidates.iter().enumerate().skip(original_len) {
        if listed.insert(item.word.as_str()) {
            unique.push(index);
        }
    }
    drop(listed);
    let mut write = original_len;
    for read in unique {
        if write != read {
            candidates.swap(write, read);
        }
        write += 1;
    }
    candidates.truncate(write);
}

fn append_helpcode_rows(
    result: &mut Vec<WordItem>,
    whole: Vec<WordItem>,
    unmatched: Vec<WordItem>,
) {
    let total = result
        .len()
        .saturating_add(whole.len())
        .saturating_add(unmatched.len());
    if total <= SMALL_PREFIX_DEDUP {
        result.reserve(total.saturating_sub(result.len()));
        for item in whole.into_iter().chain(unmatched) {
            if result.iter().any(|existing| existing.word == item.word) {
                continue;
            }
            result.push(item);
        }
        return;
    }
    // 先借用两组行计算唯一索引，释放借用后只为真正追加的行预留结果容量。
    let added = total.saturating_sub(result.len());
    let mut listed = HashSet::with_capacity(total);
    listed.extend(result.iter().map(|item| item.word.as_str()));
    let mut unique = Vec::with_capacity(added);
    for (index, item) in whole.iter().chain(unmatched.iter()).enumerate() {
        if listed.insert(item.word.as_str()) {
            unique.push(index);
        }
    }
    drop(listed);
    result.reserve(unique.len());
    let mut unique = unique.into_iter().peekable();
    result.extend(
        whole
            .into_iter()
            .chain(unmatched)
            .enumerate()
            .filter_map(|(index, item)| {
                if unique.peek() == Some(&index) {
                    unique.next();
                    Some(item)
                } else {
                    None
                }
            }),
    );
    debug_assert!(unique.peek().is_none());
}

/// 按首匹配、次匹配、未匹配的顺序原地排列单字辅助码候选，并把未匹配行移动到单独的尾部缓冲。
fn reorder_single_helpcode_rows(
    mut candidates: Vec<WordItem>,
    matches: &[SingleHelpcodeMatch],
    prefer_last: bool,
) -> (Vec<WordItem>, Vec<WordItem>) {
    debug_assert_eq!(candidates.len(), matches.len());
    let unmatched_count = matches
        .iter()
        .filter(|matched| **matched == SingleHelpcodeMatch::None)
        .count();
    if candidates.len() <= SMALL_SINGLE_HELP_CODE_ORDER {
        let mut order = [0usize; SMALL_SINGLE_HELP_CODE_ORDER];
        let order_len = fill_single_helpcode_order(&mut order, matches, prefer_last);
        debug_assert_eq!(order_len, candidates.len());
        debug_assert!(order_len <= order.len());
        apply_order(&mut candidates, &order[..order_len]);
    } else {
        let mut order = vec![0usize; candidates.len()];
        let order_len = fill_single_helpcode_order(&mut order, matches, prefer_last);
        debug_assert_eq!(order_len, candidates.len());
        apply_order(&mut candidates, &order[..order_len]);
    }
    let unmatched = candidates.split_off(candidates.len() - unmatched_count);
    (candidates, unmatched)
}

fn fill_single_helpcode_order(
    order: &mut [usize],
    matches: &[SingleHelpcodeMatch],
    prefer_last: bool,
) -> usize {
    let mut order_len = 0;
    for primary in [true, false] {
        for (index, matched) in matches.iter().enumerate() {
            let selected = match matched {
                SingleHelpcodeMatch::First => {
                    (!prefer_last && primary) || (prefer_last && !primary)
                }
                SingleHelpcodeMatch::Last => (prefer_last && primary) || (!prefer_last && !primary),
                SingleHelpcodeMatch::Both => primary,
                SingleHelpcodeMatch::None => false,
            };
            if selected {
                debug_assert!(order_len < order.len());
                order[order_len] = index;
                order_len += 1;
            }
        }
    }
    for (index, matched) in matches.iter().enumerate() {
        if *matched == SingleHelpcodeMatch::None {
            debug_assert!(order_len < order.len());
            order[order_len] = index;
            order_len += 1;
        }
    }
    order_len
}

#[cfg(test)]
mod tests {
    use super::append_helpcode_rows;
    use super::append_prefix_rows;
    use super::double_helpcode_cache_key;
    use super::prefix_group_count;
    use super::reorder_single_helpcode_rows;
    use super::typo_sentence_seat;
    use super::ShuangpinDictionary;
    use super::SingleHelpcodeMatch;
    use crate::helpcode::HelpcodeKeymap;
    use crate::paths::RuntimePaths;
    use crate::shuangpin::profile::profile;
    use crate::types::{CandidateSource, WordItem};

    fn row(word: &str) -> WordItem {
        WordItem::new("ni", word, 1, CandidateSource::Database, "ni")
    }

    /// 纠错整句跟在全码词条和整句块之后；整句块为空、前面又没有全码词条时也不坐第一位，表是空的就不给。
    #[test]
    fn the_typo_sentence_never_takes_the_first_seat() {
        let segments: Vec<String> = ["mei", "gen", "xi"].map(str::to_owned).into();
        let prefix =
            |word: &str| WordItem::new("mwgf", word, 1, CandidateSource::Database, "mei'gen");
        let mut sentence = WordItem::new(
            "mwgfxi",
            "没跟系",
            1,
            CandidateSource::Generated,
            "mei'gen'xi",
        );
        sentence.sentence_association = true;
        let exact = WordItem::new(
            "mwgfxi",
            "没跟戏",
            1,
            CandidateSource::Database,
            "mei'gen'xi",
        );

        assert_eq!(typo_sentence_seat(&[], &segments), None);
        // 词网格关着、重排器也没给出整句：前面只有前缀词条，原本会排到第 0 位。
        assert_eq!(
            typo_sentence_seat(&[prefix("没跟"), prefix("没")], &segments),
            Some(1)
        );
        assert_eq!(
            typo_sentence_seat(&[sentence.clone(), prefix("没跟")], &segments),
            Some(1)
        );
        assert_eq!(
            typo_sentence_seat(&[exact, sentence, prefix("没跟")], &segments),
            Some(2)
        );
    }

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

    #[test]
    fn single_helpcode_reordering_keeps_primary_secondary_and_unmatched_order() {
        let rows = vec![row("甲"), row("乙"), row("丙")];
        let matches = [
            SingleHelpcodeMatch::Last,
            SingleHelpcodeMatch::None,
            SingleHelpcodeMatch::First,
        ];

        let (result, unmatched) = reorder_single_helpcode_rows(rows, &matches, true);

        assert_eq!(words(&result), ["甲", "丙"]);
        assert_eq!(words(&unmatched), ["乙"]);
    }

    #[test]
    fn short_single_helpcode_reordering_does_not_allocate_order_storage() {
        let mut rows = Vec::with_capacity(3);
        rows.extend([row("甲"), row("乙"), row("丙")]);
        let matches = [
            SingleHelpcodeMatch::First,
            SingleHelpcodeMatch::First,
            SingleHelpcodeMatch::First,
        ];

        let ((result, unmatched), allocations) =
            crate::ime::personal_rerank::allocations::count(|| {
                reorder_single_helpcode_rows(rows, &matches, false)
            });

        assert_eq!(words(&result), ["甲", "乙", "丙"]);
        assert!(unmatched.is_empty());
        assert_eq!(
            allocations, 0,
            "短辅助码重排不应为排列索引分配临时存储：{allocations}"
        );
    }

    #[test]
    fn short_prefix_rows_are_appended_without_temporary_heap_state() {
        let mut candidates = Vec::with_capacity(8);
        candidates.push(row("你"));
        let prefix_rows = vec![vec![row("你"), row("好")], vec![row("好"), row("吗")]];
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            append_prefix_rows(&mut candidates, prefix_rows);
        });

        assert_eq!(allocations, 0);
        assert_eq!(
            candidates
                .iter()
                .map(|item| item.word.as_str())
                .collect::<Vec<_>>(),
            ["你", "好", "吗"]
        );
    }

    #[test]
    fn short_helpcode_rows_are_appended_without_temporary_heap_state() {
        let mut result = Vec::with_capacity(8);
        result.push(row("你"));
        let whole = vec![row("你"), row("好")];
        let unmatched = vec![row("好"), row("吗")];
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            append_helpcode_rows(&mut result, whole, unmatched);
        });

        assert_eq!(allocations, 0);
        assert_eq!(
            result
                .iter()
                .map(|item| item.word.as_str())
                .collect::<Vec<_>>(),
            ["你", "好", "吗"]
        );
    }

    #[test]
    fn single_helpcode_filter_uses_only_result_and_unmatched_buffers() {
        let mut dictionary = ShuangpinDictionary::new(
            profile(crate::types::ShuangpinProfileKind::Xiaohe).unwrap(),
            &RuntimePaths::default(),
        );
        let keymap = HelpcodeKeymap::from_codes(
            [
                ("你".to_owned(), "ab".to_owned()),
                ("嗯".to_owned(), "aa".to_owned()),
                ("好".to_owned(), "cd".to_owned()),
            ]
            .into_iter()
            .collect(),
        );
        let candidates = vec![row("你"), row("嗯"), row("好")];
        let (filtered, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            dictionary.filter_with_single_helpcode(candidates, "a", "", &keymap)
        });

        assert_eq!(words(&filtered), ["你", "嗯", "好"]);
        assert!(
            allocations < 12,
            "unexpected temporary allocations: {allocations}"
        );
    }

    fn words(items: &[WordItem]) -> Vec<&str> {
        items.iter().map(|item| item.word.as_str()).collect()
    }

    #[test]
    fn large_helpcode_rows_keep_first_occurrence_order() {
        let mut result = Vec::with_capacity(70);
        result.push(row("已有"));
        let whole = (0..32)
            .map(|index| row(if index == 0 { "已有" } else { "整" }))
            .collect();
        let unmatched = (0..33)
            .map(|index| row(if index == 0 { "未" } else { "整" }))
            .collect();

        append_helpcode_rows(&mut result, whole, unmatched);

        assert_eq!(
            result
                .iter()
                .map(|item| item.word.as_str())
                .collect::<Vec<_>>(),
            ["已有", "整", "未"]
        );
    }

    #[test]
    fn large_helpcode_rows_do_not_allocate_duplicate_index_lists() {
        let mut result = Vec::with_capacity(100);
        result.push(row("已有"));
        let whole = (0..40)
            .map(|index| {
                row(if index == 0 {
                    "已有"
                } else if index % 2 == 0 {
                    "整"
                } else {
                    "甲"
                })
            })
            .collect();
        let unmatched = (0..40)
            .map(|index| {
                row(if index == 0 {
                    "未"
                } else if index % 2 == 0 {
                    "整"
                } else {
                    "乙"
                })
            })
            .collect();

        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            append_helpcode_rows(&mut result, whole, unmatched);
        });

        assert_eq!(
            result
                .iter()
                .map(|item| item.word.as_str())
                .collect::<Vec<_>>(),
            ["已有", "甲", "整", "未", "乙"]
        );
        assert!(
            allocations <= 2,
            "large helpcode merge allocated {allocations} temporary buffers"
        );
    }

    #[test]
    fn large_helpcode_rows_reserve_only_unique_rows() {
        let mut result = Vec::with_capacity(1);
        result.push(row("已有"));
        let whole = (0..100).map(|_| row("新增")).collect();

        append_helpcode_rows(&mut result, whole, Vec::new());

        assert_eq!(
            result
                .iter()
                .map(|item| item.word.as_str())
                .collect::<Vec<_>>(),
            ["已有", "新增"]
        );
        // `Vec` 会为极小的增长保留实现规定的最小余量，但不应按 100 行输入扩容。
        assert!(result.capacity() <= 4);
    }

    #[test]
    fn large_prefix_rows_do_not_allocate_duplicate_index_lists() {
        let mut candidates = Vec::with_capacity(100);
        candidates.push(row("已有"));
        let prefix_rows = (0..2)
            .map(|group| {
                (0..40)
                    .map(|index| {
                        row(if index == 0 {
                            "已有"
                        } else if index % 2 == 0 {
                            if group == 0 {
                                "甲"
                            } else {
                                "乙"
                            }
                        } else {
                            "候选"
                        })
                    })
                    .collect()
            })
            .collect();

        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            append_prefix_rows(&mut candidates, prefix_rows);
        });

        assert_eq!(
            candidates
                .iter()
                .map(|item| item.word.as_str())
                .collect::<Vec<_>>(),
            ["已有", "候选", "甲", "乙"]
        );
        assert!(
            allocations <= 2,
            "large prefix merge allocated {allocations} temporary buffers"
        );
    }
}
