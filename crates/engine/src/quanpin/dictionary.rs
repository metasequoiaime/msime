//! `QuanpinDictionary` (`R/quanpin/quanpin_dictionary.cpp`, quanpin.md §4, §6, §7.5-§7.7, §9.9, §11, §13, §14): the query pipeline and its caches, plus the canonical-pinyin phrase writer learning uses (pins, removals and frequency learning go through `user_dictionary` directly, see `ime::registry`).

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, RwLockReadGuard};
use std::time::Duration;

use crate::assets::{MAIN_DICTIONARY, NEURAL_MODEL_KEYBOARD, USER_JOURNAL};
use crate::cache::FifoCache;
use crate::dictionary::pinyin::{PinyinDatabase, INSERTED_WEIGHT};
use crate::dictionary::DictRow;
use crate::error::{EngineError, Result};
use crate::ime::online_batch::{
    replace_cached_online_candidate_batch, replace_online_candidate_batch,
};
use crate::lattice::decode::{decode_pinned, BigramMemo, PinnedSpan, SentenceNgramTables};
use crate::lattice::merge::{
    decode_lattice, decode_lattice_borrowed, merge_decoded, merge_decoded_borrowed,
    whole_sentence_insert_position, DecodedSentences,
};
use crate::lattice::neural::{
    shared_sentence_model, NeuralReranker, CONTEXT_CHARACTERS, MAX_RERANK_PATHS,
};
use crate::lattice::personal::{PersonalNgram, PersonalTransition};
use crate::lattice::{SentencePath, TypoEdgeSource};
use crate::paths::RuntimePaths;
use crate::pinyin::fuzzy::{fuzzy_segmentations, FUZZY_SEGMENTATION_LIMIT};
use crate::pinyin::graph::{
    build_syllable_graph, enumerate_complete_segmentations, SYLLABLE_GRAPH_PATH_LIMIT,
};
use crate::pinyin::jianpin::{split_initial_token_paths, QuerySource};
use crate::pinyin::segment::{
    cut_pinyin_by_mode, is_complete_pinyin_input, join_segments, split_segments, CutMode,
};
use crate::pinyin::syllables::{
    has_only_complete_pinyin_segments, normalize_umlaut_aliases, sparse_pinyin_fallback_segments,
};
use crate::text::{count_han_chars, last_characters};
use crate::time::Instant;
use crate::types::{
    autocorrect_type, CandidateSource, FirstPageRows, FuzzyPinyinOptions, PersonalDictionaryKind,
    SentenceAssociationOptions, WordItem,
};
use crate::user_dictionary::journal::record_user_insert;
use crate::user_dictionary::ngram_store::PersonalNgramStore;
use crate::user_dictionary::typo_profile::PersonalTypoProfile;

use super::series::{
    append_unique_words, fold_reading, mark_autocorrect_candidates, merge_alternative_rows,
    merge_alternative_segmentations, merge_series_by_coverage, resolve_series_query,
    series_cache_key, SeriesResolution, ALTERNATIVE_SEGMENTATION_CANDIDATE_LIMIT,
    AUTOCORRECT_CUT_KBEST, LONGER_PHRASE_EXTRA_SYLLABLES, LONGER_PHRASE_LIMIT,
    MAX_SYLLABLES_FOR_MULTIPLE_SEGMENTATIONS,
};
use super::typo_edges::collect_typo_edges;

#[cfg(test)]
mod marking_buffer_tests;

pub const CACHE_CAPACITY: usize = 128;
pub const TYPO_SPAN_CACHE_CAPACITY: usize = 512;
/// 词网格的跨度查询结果按跨度缓存的条数。九键每按一键要对几十条切分各解一次词网格，这些切分大量共用同样的跨度，下一键又会重复上一键的大部分跨度，所以容量按一次长输入里出现的不同跨度数来定。
pub const LATTICE_SPAN_CACHE_CAPACITY: usize = 4096;
/// 按切分缓存的词典原始行（`query_segments_keyed_flat`、单字母的 `query_initial`、`query_longer_phrases` 和模糊前缀的批量查询）每种的条数。`cache` 只有 128 条，九键一键的几十条切分会把它整个挤掉；这一层只存与查询方式无关的词典行，命中时与直接查库完全一样。
pub const ROW_CACHE_CAPACITY: usize = 512;
/// 按音节缓存的整句解码结果条数。九键每按一个数字要对二十条上下的切分各解一次整句，其中约三分之一是同一次刷新或上一个数字已经解过的音节串（同一条备选切分挂在好几条九键路径下）；一个长词打下来不同的音节串有两三百条。每条只存解码出的几条句子，内存很小。
pub const SENTENCE_DECODE_CACHE_CAPACITY: usize = 512;
/// `RowCacheBatch` 一段读事务最长持续的时间。词典的忙等待上限 `BUSY_TIMEOUT` 是 250 ms，这一段要远小于它，别的连接提交时才不会因为一次长刷新等到超时。
const ROW_CACHE_BATCH_SPAN: Duration = Duration::from_millis(20);
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
// k-best 纠错切分去掉主切分后最多 8 条，再加上完整音节图最多 32 条。
const ALTERNATIVE_SEGMENTATION_SEEN_CAPACITY: usize =
    AUTOCORRECT_CUT_KBEST - 1 + SYLLABLE_GRAPH_PATH_LIMIT;
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

#[derive(Clone)]
struct CachedFuzzyRows {
    rules: u32,
    limit: usize,
    typed: String,
    rows: Vec<DictRow>,
}

#[derive(Clone)]
struct CachedDecode {
    /// 解码时个人模型的版本；模型一变，条目作废。
    personal_version: u64,
    decoded: DecodedSentences,
}

#[derive(Clone)]
struct CachedLatticeSpan {
    span_limit: usize,
    span: String,
    rows: Vec<DictRow>,
}

enum ColdRows {
    Transfer,
    Borrow,
}

/// The reference answered a failed write with `ERROR_CODE` and no message; callers map any failure to their own diagnostic.
const DICTIONARY_UNAVAILABLE: &str = "Pinyin dictionary is unavailable";
const ENTRY_REJECTED: &str = "Pinyin does not spell the word one syllable per character";

/// `QuanpinDictionary::row_cache_batch` 开的批，通过它调用词典的方法；丢弃时（包括 panic 展开时）提交读事务、恢复逐条对版本。
pub struct RowCacheBatch<'a> {
    dictionary: &'a mut QuanpinDictionary,
}

impl Deref for RowCacheBatch<'_> {
    type Target = QuanpinDictionary;

    fn deref(&self) -> &QuanpinDictionary {
        self.dictionary
    }
}

impl DerefMut for RowCacheBatch<'_> {
    fn deref_mut(&mut self) -> &mut QuanpinDictionary {
        self.dictionary
    }
}

impl Drop for RowCacheBatch<'_> {
    fn drop(&mut self) {
        self.dictionary.row_cache_batch = None;
        self.dictionary.database.end_read();
    }
}

pub struct QuanpinDictionary {
    database: PinyinDatabase,
    paths: RuntimePaths,
    sentence_tables: SentenceNgramTables,
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
    /// 词网格每个跨度的 `query_lattice_span` 结果，空结果也存。哈希只做索引，命中后再核对上限和原样跨度，避免每次命中都拼接键字符串。
    lattice_span_cache: FifoCache<u64, CachedLatticeSpan>,
    /// 每条切分的 `query_segments_keyed_flat` 结果，键是切分本身；单字母的 `query_initial` 结果也存在这里，键前加 `\u{1}`。
    segment_row_cache: FifoCache<String, Vec<DictRow>>,
    /// 每条切分的 `query_longer_phrases` 结果，键是切分本身。
    longer_row_cache: FifoCache<String, Vec<DictRow>>,
    /// `fuzzy_candidates` 里每个前缀那一次批量查询的结果，键是模糊规则、路径上限和前缀切分；三者定下来，查的路径和返回的行就定了。
    fuzzy_row_cache: FifoCache<u64, CachedFuzzyRows>,
    /// 上面四个行缓存填入时的 `PRAGMA data_version`。它们只存词典行，随其他缓存一起清空；此外每次真正计算一个查询前都对一次版本，别的连接写过词典就先清掉，所以命中的行永远和此刻直接查库一样，不会像 `cache` 那样等到下次 `reset_cache_if_database_changed` 才更新。
    row_cache_version: Option<i64>,
    /// `RowCacheBatch` 存活期间为当前这段读事务开始的时刻：版本在这段开始时对过，段内的查询不再逐条对；一段超过 `ROW_CACHE_BATCH_SPAN` 就提交、重开并重新对版本。
    row_cache_batch: Option<Instant>,
    /// 词网格解码查二元分的备忘，跨查询保留；表不变它就一直有效，见 `BigramMemo`。
    bigram_memo: RefCell<BigramMemo>,
    /// 整句解码结果，键见 `sentence_decode_key`。解码只取决于音节、词典行、n 元表、个人模型和整句选项：词典行变了随行缓存清空，个人模型按条目记的版本核对，整句选项和纠错资料变了在各自的 setter 里清空。
    sentence_decode_cache: FifoCache<String, CachedDecode>,
    rerankers: Vec<NeuralReranker>,
    /// octagram `.gram` 语法模型；路径由 `set_sentence_association` 给定、经
    /// `GramDb::shared` 进程内共享，缺失或非法时为 `None` 退回无搭配打分。
    collocation_db: Option<Arc<crate::lattice::octagram::GramDb>>,
    sentence_alternatives: bool,
    sentence_association: SentenceAssociationOptions,
    /// 首页放得下几行，决定全长组的补全词是按权重并进来还是接在整组末尾；来自宿主的每页候选数（`QueryRequest::first_page_rows`）。存成 `FirstPageRows` 是为了让 0 在这里没有入口，全拼和九键的「0 当 1」只在 `FirstPageRows::new` 做一次。
    first_page_rows: FirstPageRows,
    rescoring_context: String,
    /// The reading and mask of the query being answered; marking and the typo edges read them within that query.
    pinyin_segmentation: String,
    /// Every non-primary correction cut, joined; marking compares against them on every query, cached or not.
    alternative_segmentations: Vec<String>,
    current_autocorrect_types: u32,
}

impl QuanpinDictionary {
    /// Opens the generation's `msime-pinyin.db`, loads the n-gram tables, the personal context store and the typo profile off the keystroke path, and warms the statement cache (QD:231-268).
    pub fn new(paths: &RuntimePaths) -> Self {
        let database = PinyinDatabase::open(&paths.dictionary(MAIN_DICTIONARY));
        database.warm_up();
        // 构造时预热并保留表；共享缓存淘汰不能让按键查询重新映射和检查整张表。
        let sentence_tables = SentenceNgramTables::load(paths);
        let journal = paths.user(USER_JOURNAL);
        let personal = PersonalNgramStore::for_journal(&journal);
        drop(personal.model());
        let personal_version = personal.version();
        let typo_profile = PersonalTypoProfile::shared(&journal);
        let typo_generation = typo_profile.generation();
        let mut dictionary = Self {
            database,
            paths: paths.clone(),
            sentence_tables,
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
            lattice_span_cache: FifoCache::new(LATTICE_SPAN_CACHE_CAPACITY),
            segment_row_cache: FifoCache::new(ROW_CACHE_CAPACITY),
            longer_row_cache: FifoCache::new(ROW_CACHE_CAPACITY),
            fuzzy_row_cache: FifoCache::new(ROW_CACHE_CAPACITY),
            row_cache_version: None,
            row_cache_batch: None,
            bigram_memo: RefCell::default(),
            sentence_decode_cache: FifoCache::new(SENTENCE_DECODE_CACHE_CAPACITY),
            rerankers: Vec::new(),
            collocation_db: None,
            sentence_alternatives: false,
            sentence_association: SentenceAssociationOptions::default(),
            first_page_rows: FirstPageRows::default(),
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
            // 同词的字面前缀不能遮住覆盖更多输入的模糊整词。
            merge_series_by_coverage(&mut result, rows);
            yield_stand_in_sentence_to_fuzzy_rows(&mut result);
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

    /// 九键同步借用完整行筛选；只有核对失效后的系列热页允许提前排名。
    pub(crate) fn query_filtered_rows(
        &mut self,
        raw: &str,
        segmentation: &str,
        fuzzy: FuzzyPinyinOptions,
        mut keep: impl FnMut(&WordItem, bool) -> bool,
    ) -> Vec<WordItem> {
        if fuzzy.rules != 0 {
            // 模糊合并的去重和站位必须先完成，避免被筛掉的词让另一条同文行复活。
            let mut rows = self.query(raw, segmentation, 0, fuzzy);
            rows.retain(|row| keep(row, false));
            return rows;
        }
        let mut result =
            self.query_exact_projected(raw, segmentation, 0, ColdRows::Borrow, |rows, cached| {
                match rows {
                    Cow::Owned(mut rows) => {
                        rows.retain(|row| keep(row, cached));
                        rows
                    }
                    Cow::Borrowed(rows) => {
                        // 容器预留完整行数，只跳过拒绝行的字段复制。
                        let mut result = Vec::with_capacity(rows.len());
                        result.extend(rows.iter().filter(|row| keep(row, cached)).cloned());
                        result
                    }
                }
            });
        mark_autocorrect_candidates(
            &mut result,
            raw,
            &self.pinyin_segmentation,
            &self.alternative_segmentations,
        );
        result
    }

    /// 旧筛选接口只供冻结刷新与既有分配对照使用。
    #[cfg(test)]
    pub(crate) fn query_filtered(
        &mut self,
        raw: &str,
        segmentation: &str,
        fuzzy: FuzzyPinyinOptions,
        mut keep: impl FnMut(&str, &str, &str, bool) -> bool,
    ) -> Vec<WordItem> {
        self.query_filtered_rows(raw, segmentation, fuzzy, |row, _| {
            keep(&row.word, &row.pinyin, &row.canonical_pinyin, row.fuzzy)
        })
    }

    /// 个人上下文模型的只读视图（与同一份用户日志上的输入会话共用）。九键查一次简拼时取一次，按 `PersonalNgram::word_count` 把用户用过的简拼词排到前面（#6185）；拿着它时不要记录新的使用，记录要写锁。
    pub fn personal_model(&self) -> RwLockReadGuard<'_, PersonalNgram> {
        self.personal.model()
    }

    /// 把选中的词记进个人上下文模型，26 键也写进同一份。九键选词时调用（#6185）；只开放记录这一个写操作，不交出整个存储。
    pub fn record_personal_use(&self, transitions: &[PersonalTransition]) -> Result<()> {
        self.personal.record(transitions)
    }

    /// 简拼 `codes`（同样长度，一个字母一个音节）对应的词条，按权重从高到低最多 `limit` 行，见 `PinyinDatabase::query_jianpin_codes`。`pinyin` 和 `canonical_pinyin` 都是词条的全拼键，用户自己加的词（学习写进词库的整句、自造词）也在里面。
    pub fn query_jianpin_codes(&self, codes: &[String], limit: usize) -> Vec<WordItem> {
        initial_items(self.database.query_jianpin_codes(codes, limit))
    }

    /// 每张首字母表各取前 `table_limit` 行、合起来不再截断的简拼查询，见 `PinyinDatabase::query_jianpin_codes_per_table`。
    pub fn query_jianpin_codes_per_table(
        &self,
        codes: &[String],
        table_limit: usize,
    ) -> Vec<WordItem> {
        initial_items(
            self.database
                .query_jianpin_codes_per_table(codes, table_limit),
        )
    }

    /// 首字母的 24 行候选展开为全部行（QD:506-558），同时更新行缓存与产生该列表的查询槽。
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
        let expanded = matched_items(self.database.query_initial(code, UNLIMITED_ROWS), code);
        if expanded.len() <= limited_count {
            return false;
        }
        let first_limited = candidates
            .iter()
            .position(is_limited_initial)
            .expect("limited_count checked above");
        candidates.reserve_exact(expanded.len() - limited_count);
        let mut removed = 0;
        candidates.retain(|item| {
            if is_limited_initial(item) {
                removed += 1;
                false
            } else {
                true
            }
        });
        debug_assert_eq!(removed, limited_count);
        candidates.splice(first_limited..first_limited, expanded.iter().cloned());
        self.cache.insert(code.to_string(), expanded);
        // 按参考规则写入不带 `C:` 的槽（QD:554）：单字母不纠错。根据调用方查询重算键，避免第二次光标前缀查询把整段输入的展开列表写入自己的槽。
        let cache_key =
            self.project_resolution(raw, segmentation, autocorrect_types, |resolution| {
                series_cache_key(raw, &resolution.segmentation, autocorrect_types)
            });
        let key = self.series_slot(&cache_key);
        self.series_cache.insert(key, candidates.clone());
        true
    }

    /// 切分的模糊候选按规则与读法缓存（QD:1680-1709），双拼也复用该路径。
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
        self.validate_row_caches();
        let segments: Vec<&str> = if segmentation.is_empty() {
            Vec::new()
        } else {
            let count = segmentation.bytes().filter(|&byte| byte == b'\'').count() + 1;
            let mut segments = Vec::with_capacity(count);
            segments.extend(segmentation.split('\''));
            segments
        };
        let mut result = Vec::new();
        let mut budget = FUZZY_PATH_BUDGET;
        let mut typed = segmentation;
        for count in (1..=segments.len()).rev() {
            if budget <= 1 {
                break;
            }
            if count < segments.len() {
                typed = typed.rsplit_once('\'').map_or("", |(prefix, _)| prefix);
            }
            let prefix = &segments[..count];
            let limit = FUZZY_SEGMENTATION_LIMIT.min(budget);
            // 路径照样算出来（纯计算，不查库），预算照旧按路径数扣；只有查库这一步走缓存。
            let paths = fuzzy_segmentations(prefix, options, limit);
            if paths.is_empty() {
                continue;
            }
            budget -= paths.len();
            append_cached_fuzzy_rows(
                &self.database,
                &mut self.fuzzy_row_cache,
                &paths,
                options.rules,
                limit,
                typed,
                &mut result,
            );
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
        // 按查询原规则重算键，使候选写入下次刷新读取的槽。
        let cache_key =
            self.project_resolution(raw, segmentation, autocorrect_types, |resolution| {
                match resolution {
                    Cow::Borrowed(resolution) => resolution.cache_key.clone(),
                    Cow::Owned(resolution) => resolution.cache_key,
                }
            });
        let key = self.series_slot(&cache_key);
        if let Some(list) = self.series_cache.get_mut(&key) {
            return replace_cached_online_candidate_batch(list, raw, words, source);
        }
        let mut list = Vec::new();
        if !replace_online_candidate_batch(&mut list, raw, words, source) {
            return false;
        }
        self.series_cache.insert(key, list);
        true
    }

    /// Remove one provider's rows from every cached answer while retaining the other provider and dictionary rows.
    pub fn clear_online_candidates(&mut self, source: CandidateSource) {
        self.series_cache.retain_mut(|_, rows| {
            rows.retain(|item| item.source != source);
            !rows.is_empty()
        });
    }

    pub fn find_candidate(&self, key: &str, value: &str) -> Option<WordItem> {
        self.database
            .find_weight(key, value)
            .map(|weight| WordItem::new(key, value, weight, CandidateSource::Database, key))
    }

    pub fn knows_han_char(&self, han: &str) -> bool {
        self.database.han_char_exists(han)
    }

    /// `keys`（以 `'` 连接的完整音节）各自最好那一行的权重；没有行的键不出现。滑行输入用它给一笔可能拼出的串排序。
    pub fn best_weights(&self, keys: &[String]) -> HashMap<String, i64> {
        self.database
            .query_exact_keys_per_key(keys, 1)
            .into_iter()
            .filter_map(|(key, rows)| rows.first().map(|row| (key, row.weight)))
            .collect()
    }

    /// `keys`（以 `'` 连接的完整音节）各自权重最高的 `per_key` 行的权重之和；没有行的键不出现。九宫格用它给每个音节估频度：只看最常用那一个字时，词库发版改动一个字的权重就能让两个音节的先后翻转。
    pub fn top_weight_sums(&self, keys: &[String], per_key: usize) -> HashMap<String, i64> {
        self.database
            .query_exact_keys_per_key(keys, per_key)
            .into_iter()
            .map(|(key, rows)| (key, rows.iter().map(|row| row.weight.max(0)).sum()))
            .collect()
    }

    /// 整句改字的候选：读音恰为 `span`（完整音节）的词条，按权重从高到低，至多 `limit` 条。
    pub fn conversion_rows(&mut self, span: &[String], limit: usize) -> Vec<DictRow> {
        self.validate_row_caches();
        cached_lattice_span(&self.database, &mut self.lattice_span_cache, span, limit)
    }

    /// 整句改字的重新转换：与整句候选同一套词网格、n 元表和个人模型，钉住的段原样保留（`decode_pinned`）。
    pub fn convert_pinned(&mut self, syllables: &[String], pins: &[PinnedSpan]) -> Vec<PinnedSpan> {
        self.validate_row_caches();
        let personal = Arc::clone(&self.personal);
        let model = personal.model();
        let mut options = self.sentence_tables.options(false);
        options.personal = Some(&*model);
        options.bigram_memo = Some(&self.bigram_memo);
        let span_limit = options.span_limit;
        let database = &self.database;
        let lattice_spans = &mut self.lattice_span_cache;
        let mut lookup =
            |span: &[String]| cached_lattice_span(database, lattice_spans, span, span_limit);
        decode_pinned(syllables, &mut lookup, &options, pins)
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
        self.sentence_decode_cache.clear();
    }

    /// 首页行数只决定补全词是按权重并进来还是接在整组末尾；改了就清掉按旧门槛组装的整条答案（照 `set_sentence_alternatives`）。`cache` 存的是按切分查出来的词库行，补全词是查完它才并进来的，所以这里动不了它，清掉只会白白丢掉 128 行、再查一次 SQLite。
    pub fn set_first_page_rows(&mut self, rows: FirstPageRows) {
        if self.first_page_rows == rows {
            return;
        }
        self.first_page_rows = rows;
        self.series_cache.clear();
    }

    /// Loads the keyboard sentence model from the resource bundle when its switch is on. The desktop model is not the engine's: only the runtime's settled reranker runs it. A change resets the caches (neural-association.patch:2667-2682), as shuangpin does, so a cached online row does not come back after the options are switched away and back.
    pub fn set_sentence_association(&mut self, options: SentenceAssociationOptions) {
        if self.sentence_association == options {
            return;
        }
        // 模型路径为空或不可载时回退 `None`：与 `msime-bigram.bin` 缺失的既有降级一致。
        self.collocation_db = if options.collocation_model.as_os_str().is_empty() {
            None
        } else {
            crate::lattice::octagram::GramDb::shared(&options.collocation_model)
        };
        self.sentence_association = options.clone();
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
            &self.sentence_association,
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
        self.clear_row_caches();
    }

    fn clear_row_caches(&mut self) {
        // 整句是从这些行解出来的。
        self.sentence_decode_cache.clear();
        self.lattice_span_cache.clear();
        self.segment_row_cache.clear();
        self.longer_row_cache.clear();
        self.fuzzy_row_cache.clear();
    }

    /// 九键一次刷新要连着查几十条切分。批开始时开一个读事务（见 `PinyinDatabase::begin_read`），在事务里对一次版本，之后到返回的 `RowCacheBatch` 被丢弃为止的查询都读同一份快照，行缓存也不再逐条对版本。词典是回滚日志模式，读事务拿着共享锁时别的连接提交不了，只能在 `BUSY_TIMEOUT` 内忙等，所以每段事务最多持续 `ROW_CACHE_BATCH_SPAN`，到时在下一次真正计算查询前提交、重开并重新对版本，别的连接写词典最多多等这一段。批在 `Drop` 里结束，查询中途 panic（宿主在 FFI 边界 `catch_unwind` 后继续运行）也不会把事务和共享锁留在连接上。
    pub fn row_cache_batch(&mut self) -> RowCacheBatch<'_> {
        self.row_cache_batch = None;
        self.database.begin_read();
        self.validate_row_caches();
        self.row_cache_batch = Some(Instant::now());
        RowCacheBatch { dictionary: self }
    }

    /// 只读当前版本号，不改 `database_changed` 记下的那份，所以其他缓存何时重置和以前完全一样。读不到版本（词典没打开或读取失败）时每次都清空，等于不缓存。
    fn validate_row_caches(&mut self) {
        if let Some(started) = self.row_cache_batch {
            if started.elapsed() < ROW_CACHE_BATCH_SPAN {
                return;
            }
            // 这一段读事务够长了，让别的连接有机会提交，再从新快照接着读。
            self.database.end_read();
            self.database.begin_read();
            self.row_cache_batch = Some(Instant::now());
        }
        let version = self.database.data_version();
        if version.is_none() || version != self.row_cache_version {
            self.clear_row_caches();
            self.row_cache_version = version;
        }
    }

    /// QD:320-482.
    fn query_exact(&mut self, raw: &str, segmentation: &str, types: u32) -> Vec<WordItem> {
        self.query_exact_projected(raw, segmentation, types, ColdRows::Transfer, |rows, _| {
            rows.into_owned()
        })
    }

    /// 同步投影只能筛选、复制或移交原行，不改字段和顺序；冷页全保留时可交换副本与原页。
    /// 回调的布尔值只标识核对失效后的系列热命中，借用的冷页仍为 `false`。
    fn query_exact_projected(
        &mut self,
        raw: &str,
        segmentation: &str,
        types: u32,
        cold: ColdRows,
        project: impl FnOnce(Cow<'_, [WordItem]>, bool) -> Vec<WordItem>,
    ) -> Vec<WordItem> {
        if raw.is_empty() {
            return project(Cow::Owned(Vec::new()), false);
        }
        self.current_autocorrect_types = types;
        if types & TYPO_EDGE_TYPES != 0 {
            // Only a stat per query unless the journal changed. Lists built without a mask never depend on the profile.
            self.typo_profile.refresh_if_changed();
            let generation = self.typo_profile.generation();
            if generation != self.typo_generation {
                self.typo_generation = generation;
                self.series_cache.clear();
                // 纠错整句按纠错资料规划。
                self.sentence_decode_cache.clear();
                self.personal_scored_keys.clear();
            }
        }
        let mut segments = None;
        let segmentation_cache = &mut self.segmentation_cache;
        let association = self.sentence_association.clone();
        let has_rerankers = !self.rerankers.is_empty();
        let context = &self.rescoring_context;
        let pinyin_segmentation = &mut self.pinyin_segmentation;
        let (resolution_hash, (resolution, marking_alternatives, slot)) =
            project_cached_resolution_with(
                &mut self.resolution_cache,
                raw,
                segmentation,
                types,
                || {
                    let segments = segments.get_or_insert_with(|| {
                        resolve_cached_segments(segmentation_cache, raw, segmentation)
                    });
                    resolve_series_query(raw, segments, types)
                },
                |resolution| {
                    // 热命中也发布本次标记；高成本读法仍算纠错。
                    pinyin_segmentation.clone_from(&resolution.segmentation);
                    let alternatives = resolution
                        .alternative_corrected_cuts
                        .iter()
                        .chain(&resolution.costlier_corrected_cuts)
                        .map(|cut| join_segments(cut))
                        .collect();
                    let slot = series_slot_key(
                        &resolution.cache_key,
                        &association,
                        has_rerankers,
                        context,
                    );
                    let owned = match resolution {
                        Cow::Borrowed(_) => None,
                        Cow::Owned(resolution) => Some(resolution),
                    };
                    (owned, alternatives, slot)
                },
            );
        self.alternative_segmentations = marking_alternatives;
        self.drop_personal_scored_results();
        if self.series_cache.contains(&slot) {
            if self.database.database_changed() {
                // 清空切分备忘前取得重建需要的音节，保留原先的缓存复用与分配成本。
                segments.get_or_insert_with(|| self.resolve_segments(raw, segmentation));
                self.reset_cache();
            }
            if let Some(cached) = self.series_cache.get_ref(&slot) {
                return project(Cow::Borrowed(cached), true);
            }
        }

        let segments = segments.unwrap_or_else(|| self.resolve_segments(raw, segmentation));

        // 候选槽失效不会改解析备忘；复用已核对的索引，避免重复哈希和完整键校验。
        let resolution = resolution.unwrap_or_else(|| {
            self.resolution_cache
                .get_ref(&resolution_hash)
                .expect("候选失效检查必须保留解析备忘")
                .resolution
                .clone()
        });

        // 下面每一次读词典都可能走行缓存，先确认它们还和库里一致。
        self.validate_row_caches();
        let alternatives = alternative_segmentations(raw, &segments, &resolution, types);
        let primary_segmentation = resolution.segmentation.as_str();
        let mut result;
        if resolution.corrected_input {
            // The corrected reading is the primary key, so selection and weight updates land on the right rows; the literal reading stays behind it as a fallback tail.
            result = self.query_series(raw, primary_segmentation, &resolution.corrected);
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
                    primary_segmentation,
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
            result = self.query_series(raw, primary_segmentation, &segments);
            if !alternatives.is_empty() {
                result = self.merge_alternatives(
                    raw,
                    primary_segmentation,
                    &segments,
                    &alternatives,
                    result,
                );
            }
        }
        let (cached, projected) = match cold {
            ColdRows::Transfer => {
                let cached = result.clone();
                (cached, project(Cow::Owned(result), false))
            }
            ColdRows::Borrow => {
                let projected = project(Cow::Borrowed(&result), false);
                if projected.len() == result.len() {
                    // 全保留时副本可直接入缓存，原页作为输出，省去额外缩容。
                    (projected, result)
                } else {
                    // 外层富余容量不随整页转移而长期留在 FIFO 中。
                    result.shrink_to_fit();
                    (result, projected)
                }
            }
        };
        // 单音节主路径也可能沿完整备选切分组句，需要跟随个人模型失效。
        if segments.len() >= 2
            || resolution.corrected.len() >= 2
            || alternatives
                .iter()
                .any(|path| path.len() >= 2 && is_literal_segmentation(raw, path))
        {
            self.series_cache.insert(slot.clone(), cached);
            self.remember_personal_scored_key(slot);
        } else {
            self.series_cache.insert(slot, cached);
        }
        projected
    }

    /// 完整词继续按词频合并，字面备选另外补齐前缀和整句，不能叠加一层纠错。
    fn merge_alternatives(
        &mut self,
        raw: &str,
        primary_segmentation: &str,
        primary_segments: &[String],
        alternatives: &[Vec<String>],
        result: Vec<WordItem>,
    ) -> Vec<WordItem> {
        let mut result = self.merge_alternative_full(
            raw,
            primary_segmentation,
            primary_segments,
            alternatives,
            result,
        );
        let mut supplemental = Vec::new();
        // 相同音节前缀在多条完整路径中会重复出现；它们的行和消费范围相同，只查一次。
        let mut queried_prefixes = HashMap::new();
        if is_literal_segmentation(raw, primary_segments) {
            for count in 1..primary_segments.len() {
                let key = join_segments(&primary_segments[..count]);
                if let Some(rows) = self.cache.get_ref_by(&key) {
                    queried_prefixes.insert(key, rows.len());
                }
            }
        }
        for path in alternatives {
            if is_literal_segmentation(raw, path) {
                let rows = self.query_series_with_typos(
                    raw,
                    &join_segments(path),
                    path,
                    false,
                    Some(&mut queried_prefixes),
                );
                supplemental.extend(rows);
            }
        }
        if !supplemental.is_empty() {
            merge_series_by_coverage(&mut result, supplemental);
        }
        result
    }

    /// `merge_alternative_segmentations` 的完整词查询部分，升位与合并规则在 `series`。
    fn merge_alternative_full(
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
        let cache_key = path_cache_key(raw, primary_segmentation);
        let primary_full = if let Some(primary_full) = self.cache.get_ref_by(cache_key) {
            Cow::Borrowed(primary_full.as_slice())
        } else {
            Cow::Owned(self.query_single_path_miss(
                cache_key,
                primary_segmentation,
                primary_segments,
            ))
        };
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
        match primary_full {
            Cow::Borrowed(rows) => {
                merge_alternative_rows(result, rows.iter().cloned(), alternative_full)
            }
            Cow::Owned(rows) => merge_alternative_segmentations(result, rows, alternative_full),
        }
    }

    fn append_query_rows(result: &mut Vec<WordItem>, rows: Vec<WordItem>) {
        if result.is_empty() && result.capacity() == 0 {
            *result = rows;
            return;
        }
        result.reserve(rows.len());
        result.extend(rows);
    }

    /// 对应 QD:560-730：从长到短查前缀组，再合并词网格、纠错整句与稀疏回退。
    fn query_series(
        &mut self,
        raw: &str,
        segmentation: &str,
        segments: &[String],
    ) -> Vec<WordItem> {
        self.query_series_with_typos(raw, segmentation, segments, true, None)
    }

    fn query_series_with_typos(
        &mut self,
        raw: &str,
        segmentation: &str,
        segments: &[String],
        include_typos: bool,
        mut queried_prefixes: Option<&mut HashMap<String, usize>>,
    ) -> Vec<WordItem> {
        if segments.is_empty() {
            return self.query_single_path(raw, segmentation, segments);
        }
        let mut result = Vec::new();
        let mut skipped_prefix_rows = 0usize;
        let prefix_key = series_prefix_key(segmentation, segments);
        let mut partial_segmentation = prefix_key.as_ref();
        for count in (1..=segments.len()).rev() {
            if count < segments.len() {
                partial_segmentation = partial_segmentation
                    .rsplit_once('\'')
                    .map_or("", |(prefix, _)| prefix);
            }
            let partial = &segments[..count];
            if count < segments.len() {
                if let Some(prefixes) = queried_prefixes.as_deref_mut() {
                    if let Some(&count) = prefixes.get(partial_segmentation) {
                        skipped_prefix_rows = skipped_prefix_rows.saturating_add(count);
                        continue;
                    }
                }
                let before = result.len();
                self.append_single_path(&mut result, raw, partial_segmentation, partial);
                self.append_initial_split_rows(&mut result, raw, partial);
                if let Some(prefixes) = queried_prefixes.as_deref_mut() {
                    prefixes.insert(partial_segmentation.to_owned(), result.len() - before);
                }
                continue;
            }
            let mut rows = self.query_single_path(raw, partial_segmentation, partial);
            if self.append_initial_split_rows(&mut rows, raw, partial) {
                rows.sort_by_key(|item| std::cmp::Reverse(item.weight));
            }
            if count == segments.len() {
                // 同长度表里常常只剩冷僻同音词：ping'guo 只有 苹果 1143881、评过 1180、平果 169、平锅 1，而 苹果电脑 21495、苹果公司 19725 在 tbl_4_p，同长度查询看不到，第二位就落到冷僻同音词上。所以更长的补全词按权重并入这一组，权重出自同一语料，跨表可比。`pinyin` 仍是键入的切分（上屏只消耗已键入的音节），`canonical_pinyin` 保留完整读音供持久化。
                // 但首位只留给读音和键入完全一致的同长度行里最重的那一行：补全词上屏会多出用户没打的字，权重再高也不能抢首选（#7345：dacuozi 的首选曾是 打错字了，doumeiren 的首选曾是 都没人管理）。补全词只和其余同长度行竞争第二位起的位置，苹果电脑 仍排在 平锅 前面。同长度查询没有精确键时会退到前缀区间（zhi'liang'jian 只查到 zhi'liang'jiang 的 质量奖），那一行的读音并不是键入的读音，不钉首位，和补全词一起按权重排；同长度查询一行都没有时同样按权重排，整句块随后由 `whole_sentence_insert_position` 插到首位。
                // 精确同音词自己够填首页时，补全词不再按权重挤进来，改成接在整组末尾：首页的座位仍归同音词（qi'shi 的首屏曾是 其实 骑士 其实是 企事业单位），但往后翻还翻得到它们（zhongguo、shide 的精确表本来就够一页，直接丢掉等于让「中国人民」「是的话」从候选里消失，词库以后给 ping'guo 多加几个冷僻同音词，「苹果电脑」也会整个不见）。整组本身至少就有 `first_page_rows` 行，所以接在末尾的补全词一定落在首页之后。一页几行取宿主的每页候选数（`first_page_rows`）；首页上还有整句块、AI 位、云候选位和后插入的用户词，这个门槛看不见它们，但补全词只是被推后，不再消失。
                let exact_rows = rows
                    .iter()
                    .filter(|item| item.canonical_pinyin == partial_segmentation)
                    .count();
                let longer = self.longer_phrase_candidates(partial_segmentation, partial);
                if !longer.is_empty() {
                    if exact_rows < self.first_page_rows.get() {
                        // `query_database` 和 `segment_row_cache` 都保持词典按权重降序的顺序；这里的页只来自这条路径，不能再被其它来源打乱，所以无需为长词合并重新排序。
                        let lead = usize::from(
                            rows.first()
                                .is_some_and(|item| item.canonical_pinyin == partial_segmentation),
                        );
                        rows.extend(longer);
                        rows[lead..].sort_by_key(|item| std::cmp::Reverse(item.weight));
                    } else {
                        rows.extend(longer);
                    }
                }
            }
            Self::append_query_rows(&mut result, rows);
        }

        if segments.len() >= 2 && has_only_complete_pinyin_segments(segments) {
            if include_typos {
                self.merge_sentences(&mut result, raw, segmentation, segments);
            } else {
                self.merge_sentences_with_typos(&mut result, raw, segmentation, segments, false);
            }
        }

        // 跳过的重复行仍计入原路径的稀疏门槛，不能因此额外触发读音回退。
        if result.len().saturating_add(skipped_prefix_rows) < SPARSE_FALLBACK_THRESHOLD {
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

    /// 自动切分里的双字母声母另试单字母读法；手动边界不改，完整音节不拆。
    fn append_initial_split_rows(
        &mut self,
        result: &mut Vec<WordItem>,
        raw: &str,
        segments: &[String],
    ) -> bool {
        // 单独的声母前缀仍只查单字，避免把错误拼写开头的 sh 扩成「上好」并遮住纠错关闭的边界。
        if segments.len() < 2 || raw.contains('\'') {
            return false;
        }
        let before = result.len();
        for path in split_initial_token_paths(segments, SYLLABLE_GRAPH_PATH_LIMIT) {
            let code = join_segments(&path);
            let rows = self.query_single_path(raw, &code, &path);
            append_unique_words(result, rows);
        }
        result.len() != before
    }

    /// The lattice sentence block and the typo sentence (QD:644-721, overlays.md §1.6.2). Both rank below a dictionary row that already answers the whole key, which is what `whole_sentence_insert_position` computes; inserting at index 0 once turned 百依百顺 into 白一百顺 and 颁布实施 into 版不是是.
    fn merge_sentences(
        &mut self,
        result: &mut Vec<WordItem>,
        raw: &str,
        segmentation: &str,
        segments: &[String],
    ) {
        self.merge_sentences_with_typos(result, raw, segmentation, segments, true);
    }

    fn merge_sentences_with_typos(
        &mut self,
        result: &mut Vec<WordItem>,
        raw: &str,
        segmentation: &str,
        segments: &[String],
        include_typos: bool,
    ) {
        let association = self.sentence_association.clone();
        let has_collocation =
            self.collocation_db.is_some() && association.collocation_weight != 0.0;
        // 〔搭配〕行即解码最优路径：模型在且显示开关开时才出。
        let include_collocation_best = association.collocation_rerank && has_collocation;
        if !association.word_lattice && self.rerankers.is_empty() && !include_collocation_best {
            return;
        }
        // Held for the whole merge: the options borrow the model.
        let personal = Arc::clone(&self.personal);
        let model = personal.model();
        let mut options = self.sentence_tables.options(self.sentence_alternatives);
        if !self.rerankers.is_empty() {
            options.nbest = MAX_RERANK_PATHS;
        }
        options.include_lattice_best = association.word_lattice;
        options.include_collocation_best = include_collocation_best;
        options.show_next_on_duplicate = association.show_next_on_duplicate;
        options.personal = Some(&*model);
        options.bigram_memo = Some(&self.bigram_memo);
        // 搭配项逐边叠加字级搭配分，与二元/三元在同一 Viterbi 里竞争；
        // query() 是自然对数域，乘 LN_TO_LOG10 换到主分的域。
        let scorer;
        if has_collocation {
            let db = self.collocation_db.as_ref().expect("collocation db loaded");
            let config = association.collocation_grammar;
            scorer = move |tail: &str, word: &str, is_rear: bool| {
                db.query(tail, word, is_rear, &config) * std::f64::consts::LOG10_E
            };
            options.collocation_scorer = Some(&scorer);
            options.collocation_weight = association.collocation_weight;
        }

        let typed = if segmentation.is_empty() {
            raw
        } else {
            segmentation
        };
        let types = self.current_autocorrect_types;
        let typo_eligible = include_typos && types & TYPO_EDGE_TYPES != 0 && !raw.contains('\'');
        let span_limit = options.span_limit;
        let database = &self.database;
        let span_cache = &mut self.typo_span_cache;
        let profile = &*self.typo_profile;
        let mut typo_edges = |literal_best: &SentencePath| {
            collect_typo_edges(database, span_cache, profile, segments, literal_best, types)
        };
        let typo_source: Option<&mut TypoEdgeSource<'_>> = if typo_eligible {
            Some(&mut typo_edges)
        } else {
            None
        };
        // 模型已按需重载，此刻的版本就是这次解码用的版本。
        let personal_version = personal.version();
        let decode_key = sentence_decode_key(segments, types, typo_eligible);
        let cached = self
            .sentence_decode_cache
            .get_ref(&decode_key)
            .filter(|cached| cached.personal_version == personal_version);
        let typo = if let Some(cached) = cached {
            merge_decoded_borrowed(
                result,
                segments,
                typed,
                &options,
                &mut self.rerankers,
                &self.rescoring_context,
                &cached.decoded,
            )
        } else {
            let span_count = lattice_span_count(segments.len(), options.max_phrase_syllables);
            let decoded = if span_count <= LATTICE_SPAN_CACHE_CAPACITY {
                warm_lattice_span_cache(
                    database,
                    &mut self.lattice_span_cache,
                    segments,
                    span_limit,
                    options.max_phrase_syllables,
                );
                let lattice_spans = &self.lattice_span_cache;
                let mut lookup = |span: &[String]| {
                    cached_lattice_span_ref(lattice_spans, span, span_limit)
                        .expect("warmed lattice span cache entry")
                };
                decode_lattice_borrowed(segments, &mut lookup, &options, typo_source)
            } else {
                let lattice_spans = &mut self.lattice_span_cache;
                let mut lookup = |span: &[String]| {
                    cached_lattice_span(database, lattice_spans, span, span_limit)
                };
                decode_lattice(segments, &mut lookup, &options, typo_source)
            };
            if let Some(decoded) = &decoded {
                self.sentence_decode_cache.insert(
                    decode_key,
                    CachedDecode {
                        personal_version,
                        decoded: decoded.clone(),
                    },
                );
            }
            decoded.and_then(|decoded| {
                merge_decoded(
                    result,
                    segments,
                    typed,
                    &options,
                    &mut self.rerankers,
                    &self.rescoring_context,
                    decoded,
                )
            })
        };

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

    /// 完整音节的长词续接（QD:732-749）：输入切分作为 `pinyin`，完整长键作为规范键。
    fn longer_phrase_candidates(
        &mut self,
        segmentation: &str,
        segments: &[String],
    ) -> Vec<WordItem> {
        if let Some(rows) = self.longer_row_cache.get_ref_by(segmentation) {
            return matched_items(rows.iter().cloned(), segmentation);
        }
        let rows = self.database.query_longer_phrases(
            segments,
            LONGER_PHRASE_EXTRA_SYLLABLES,
            LONGER_PHRASE_LIMIT,
        );
        self.longer_row_cache
            .insert(segmentation.to_owned(), rows.clone());
        matched_items(rows, segmentation)
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
        self.query_single_path_miss(cache_key, segmentation, segments)
    }

    /// 较短热前缀直接克隆到最终结果，避免建立随后立即搬走的中间候选页。
    fn append_single_path(
        &mut self,
        result: &mut Vec<WordItem>,
        raw: &str,
        segmentation: &str,
        segments: &[String],
    ) {
        if result.is_empty() && result.capacity() == 0 {
            *result = self.query_single_path(raw, segmentation, segments);
            return;
        }
        let cache_key = path_cache_key(raw, segmentation);
        if let Some(rows) = self.cache.get_ref_by(cache_key) {
            result.reserve(rows.len());
            result.extend(rows.iter().cloned());
            return;
        }
        let rows = self.query_single_path_miss(cache_key, segmentation, segments);
        Self::append_query_rows(result, rows);
    }

    /// 冷页仍保存紧凑副本并交出原查询结果；两个入口共用同一次未命中处理。
    fn query_single_path_miss(
        &mut self,
        cache_key: &str,
        segmentation: &str,
        segments: &[String],
    ) -> Vec<WordItem> {
        let result = self.query_database(segments, segmentation);
        self.cache.insert(cache_key.to_owned(), result.clone());
        result
    }

    /// QD:830-869：单字母查首字母表，最多 24 行；其它输入走原级联查询。
    fn query_database(&mut self, segments: &[String], segmentation: &str) -> Vec<WordItem> {
        if segments.len() == 1 && segments[0].len() == 1 {
            let matched_code = if segmentation.is_empty() {
                segments[0].as_str()
            } else {
                segmentation
            };
            // 单字节 UTF-8 音节必为 ASCII；局部键只供查找，前缀与普通切分键分开。
            let key_bytes = [1, segments[0].as_bytes()[0]];
            let key = std::str::from_utf8(&key_bytes).expect("单字节音节与缓存前缀必须是 ASCII");
            if let Some(rows) = self.segment_row_cache.get_ref_by(key) {
                return matched_items(rows.iter().cloned(), matched_code);
            }
            let rows = self
                .database
                .query_initial(&segments[0], INITIAL_CANDIDATE_LIMIT);
            self.segment_row_cache
                .insert(format!("\u{1}{}", segments[0]), rows.clone());
            return matched_items(rows, matched_code);
        }
        // 音节已在 `resolve_segments` 规范化一次，直接使用标准键查找。
        let code = segmentation.is_empty().then(|| join_segments(segments));
        let key = code.as_deref().unwrap_or(segmentation);
        if let Some(rows) = self.segment_row_cache.get_ref_by(key) {
            return matched_items(rows.iter().cloned(), key);
        }
        let rows =
            self.database
                .query_segments_keyed_flat(segments, UNLIMITED_ROWS, QuerySource::Quanpin);
        self.segment_row_cache.insert(key.to_owned(), rows.clone());
        matched_items(rows, key)
    }

    /// 显式和自动切分共用一次别名规范化，进入查询管线的音节保持标准拼写（QD:767-775）。
    fn resolve_segments(&mut self, raw: &str, segmentation: &str) -> Vec<String> {
        resolve_cached_segments(&mut self.segmentation_cache, raw, segmentation)
    }

    /// 拥有型解析入口只保留作旧查询的测试对照；产品按已核对的索引延迟取得副本。
    #[cfg(test)]
    fn resolution(
        &mut self,
        raw: &str,
        segmentation: &str,
        segments: &[String],
        types: u32,
    ) -> SeriesResolution {
        project_cached_resolution(
            &mut self.resolution_cache,
            raw,
            segmentation,
            segments,
            types,
            |resolution| resolution.into_owned(),
        )
        .1
    }

    /// 热投影不构造音节；冷未命中保留原音节备忘和解析副本，并交出计算结果。
    fn project_resolution<T>(
        &mut self,
        raw: &str,
        segmentation: &str,
        types: u32,
        project: impl FnOnce(Cow<'_, SeriesResolution>) -> T,
    ) -> T {
        let segmentation_cache = &mut self.segmentation_cache;
        project_cached_resolution_with(
            &mut self.resolution_cache,
            raw,
            segmentation,
            types,
            || {
                let segments = resolve_cached_segments(segmentation_cache, raw, segmentation);
                resolve_series_query(raw, &segments, types)
            },
            project,
        )
        .1
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

/// 按缓存字段拆分借用，使调用方能同时读取系列槽配置；完整键核对和冷副本规则集中保留。
#[cfg(test)]
fn project_cached_resolution<T>(
    cache: &mut FifoCache<u64, CachedResolution>,
    raw: &str,
    segmentation: &str,
    segments: &[String],
    types: u32,
    project: impl FnOnce(Cow<'_, SeriesResolution>) -> T,
) -> (u64, T) {
    project_cached_resolution_with(
        cache,
        raw,
        segmentation,
        types,
        || resolve_series_query(raw, segments, types),
        project,
    )
}

/// 解析热命中不调用构造闭包；常规查询因此可把音节拥有值延迟到真正未命中时。
fn project_cached_resolution_with<T>(
    cache: &mut FifoCache<u64, CachedResolution>,
    raw: &str,
    segmentation: &str,
    types: u32,
    resolve: impl FnOnce() -> SeriesResolution,
    project: impl FnOnce(Cow<'_, SeriesResolution>) -> T,
) -> (u64, T) {
    let hash = resolution_cache_hash(types, raw, segmentation);
    if let Some(cached) = cache.get_ref(&hash) {
        if resolution_cache_key_matches(&cached.key, types, raw, segmentation) {
            return (hash, project(Cow::Borrowed(&cached.resolution)));
        }
    }
    let resolution = resolve();
    let key = format!("{types}\u{1f}{raw}\u{1f}{segmentation}");
    cache.insert(
        hash,
        CachedResolution {
            key,
            resolution: resolution.clone(),
        },
    );
    (hash, project(Cow::Owned(resolution)))
}

/// 直接构造拥有型候选；缓存行可用惰性克隆迭代，避免完整的中间行页。
fn matched_items(rows: impl IntoIterator<Item = DictRow>, matched_code: &str) -> Vec<WordItem> {
    rows.into_iter()
        .map(|row| {
            WordItem::new(
                matched_code,
                row.value,
                row.weight,
                CandidateSource::Database,
                row.key,
            )
        })
        .collect()
}

/// `tbl_1_<c>` 的行转成 `WordItem(key, value, weight, Database, key)`。
fn initial_items(rows: Vec<DictRow>) -> Vec<WordItem> {
    rows.into_iter()
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

/// `query_lattice_span` 经过跨度缓存；哈希碰撞时重新查库，不混用不同跨度或上限的结果。
/// 跨度来自已切分的音节，每项不含撇号，因此分隔符编码能核对原样音节边界。
fn cached_lattice_span(
    database: &PinyinDatabase,
    cache: &mut FifoCache<u64, CachedLatticeSpan>,
    span: &[String],
    span_limit: usize,
) -> Vec<DictRow> {
    let hash = lattice_span_cache_hash(span_limit, span);
    if let Some(cached) = cache.get_ref(&hash) {
        if lattice_span_cache_key_matches(cached, span_limit, span) {
            return cached.rows.clone();
        }
    }
    let rows = database.query_lattice_span(span, span_limit);
    let mut key = String::with_capacity(span.iter().map(|syllable| syllable.len() + 1).sum());
    for syllable in span {
        key.push('\'');
        key.push_str(syllable);
    }
    cache.insert(
        hash,
        CachedLatticeSpan {
            span_limit,
            span: key,
            rows: rows.clone(),
        },
    );
    rows
}

fn ensure_cached_lattice_span(
    database: &PinyinDatabase,
    cache: &mut FifoCache<u64, CachedLatticeSpan>,
    span: &[String],
    span_limit: usize,
) {
    let hash = lattice_span_cache_hash(span_limit, span);
    if let Some(cached) = cache.get_ref(&hash) {
        if lattice_span_cache_key_matches(cached, span_limit, span) {
            // 预热批次可能命中上一轮缓存；把命中项移到队尾，避免后续冷跨度插入时把本轮早先命中的项驱逐，导致借用查询找不到刚预热的行。
            let cached = cache
                .remove(&hash)
                .expect("lattice span cache hit disappeared");
            cache.insert(hash, cached);
            return;
        }
    }
    let rows = database.query_lattice_span(span, span_limit);
    let mut key = String::with_capacity(span.iter().map(|syllable| syllable.len() + 1).sum());
    for syllable in span {
        key.push('\'');
        key.push_str(syllable);
    }
    cache.insert(
        hash,
        CachedLatticeSpan {
            span_limit,
            span: key,
            rows,
        },
    );
}

fn cached_lattice_span_ref<'a>(
    cache: &'a FifoCache<u64, CachedLatticeSpan>,
    span: &[String],
    span_limit: usize,
) -> Option<&'a [DictRow]> {
    let hash = lattice_span_cache_hash(span_limit, span);
    let cached = cache.get_ref(&hash)?;
    lattice_span_cache_key_matches(cached, span_limit, span).then_some(cached.rows.as_slice())
}

fn warm_lattice_span_cache(
    database: &PinyinDatabase,
    cache: &mut FifoCache<u64, CachedLatticeSpan>,
    syllables: &[String],
    span_limit: usize,
    max_phrase_syllables: usize,
) {
    let max_len = max_phrase_syllables.max(1);
    for start in 0..syllables.len() {
        for end in start + 1..=syllables.len().min(start + max_len) {
            ensure_cached_lattice_span(database, cache, &syllables[start..end], span_limit);
        }
    }
}

/// `sentence_decode_cache` 的键：音节串、这次查询的纠错类型和是否规划纠错整句。纠错整句只在后两者允许时才解，同一串音节在两种情况下的结果不同。
fn sentence_decode_key(segments: &[String], types: u32, typo_eligible: bool) -> String {
    let mut key = String::with_capacity(
        segments
            .iter()
            .map(|segment| segment.len() + 1)
            .sum::<usize>()
            + 12,
    );
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            key.push('\'');
        }
        key.push_str(segment);
    }
    key.push('\u{1f}');
    key.push_str(&types.to_string());
    key.push(if typo_eligible { 't' } else { '-' });
    key
}

fn lattice_span_count(syllable_count: usize, max_phrase_syllables: usize) -> usize {
    let max_len = max_phrase_syllables.max(1);
    (0..syllable_count)
        .map(|start| syllable_count.min(start.saturating_add(max_len)) - start)
        .sum()
}

fn lattice_span_cache_hash(span_limit: usize, span: &[String]) -> u64 {
    let mut hasher = DefaultHasher::new();
    span_limit.hash(&mut hasher);
    span.hash(&mut hasher);
    hasher.finish()
}

fn lattice_span_cache_key_matches(
    cached: &CachedLatticeSpan,
    span_limit: usize,
    span: &[String],
) -> bool {
    if cached.span_limit != span_limit {
        return false;
    }
    let mut remaining = cached.span.as_str();
    for syllable in span {
        let Some(rest) = remaining.strip_prefix('\'') else {
            return false;
        };
        let Some(rest) = rest.strip_prefix(syllable) else {
            return false;
        };
        remaining = rest;
    }
    remaining.is_empty()
}

// 只有参数字节恰好等于音节拼接结果才借用，保留空段、别名及嵌入撇号的旧键。
fn series_prefix_key<'a>(segmentation: &'a str, segments: &[String]) -> Cow<'a, str> {
    let mut remaining = segmentation;
    for (index, segment) in segments.iter().enumerate() {
        if index != 0 {
            let Some(rest) = remaining.strip_prefix('\'') else {
                return Cow::Owned(join_segments(segments));
            };
            remaining = rest;
        }
        let Some(rest) = remaining.strip_prefix(segment.as_str()) else {
            return Cow::Owned(join_segments(segments));
        };
        remaining = rest;
    }
    if remaining.is_empty() {
        Cow::Borrowed(segmentation)
    } else {
        Cow::Owned(join_segments(segments))
    }
}

#[cfg(test)]
fn truncate_last_segment(segmentation: &mut String) {
    if let Some(boundary) = segmentation.rfind('\'') {
        segmentation.truncate(boundary);
    } else {
        segmentation.clear();
    }
}

fn series_slot_key(
    cache_key: &str,
    options: &SentenceAssociationOptions,
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

/// 热页直接投影，冷页保留原缓存副本与 SQLite 页容量策略。
fn append_cached_fuzzy_rows<'a>(
    database: &PinyinDatabase,
    cache: &mut FifoCache<u64, CachedFuzzyRows>,
    paths: &[Vec<String>],
    rules: u32,
    limit: usize,
    typed: impl Into<Cow<'a, str>>,
    result: &mut Vec<WordItem>,
) {
    let typed = typed.into();
    let hash = fuzzy_row_cache_hash(rules, limit, &typed);
    if let Some(cached) = cache.get_ref(&hash) {
        if fuzzy_row_cache_key_matches(cached, rules, limit, &typed) {
            append_fuzzy_items(cached.rows.iter().cloned(), typed, result);
            return;
        }
    }
    let rows = database.query_exact_segmentations_keyed_flat(paths, FUZZY_ROW_LIMIT);
    cache.insert(
        hash,
        CachedFuzzyRows {
            rules,
            limit,
            typed: typed.as_ref().to_owned(),
            rows: rows.clone(),
        },
    );
    append_fuzzy_items(rows.into_iter(), typed, result);
}

/// 取得首行才拥有匹配码，已有拥有值直接接管；其余行复制同一读法。
fn append_fuzzy_items(
    mut rows: impl ExactSizeIterator<Item = DictRow>,
    typed: Cow<'_, str>,
    result: &mut Vec<WordItem>,
) {
    result.reserve(rows.len());
    if let Some(row) = rows.next() {
        let mut item = WordItem::new(
            typed.into_owned(),
            row.value,
            row.weight,
            CandidateSource::Database,
            row.key,
        );
        item.fuzzy = true;
        result.push(item);
    }
    for row in rows {
        let mut item = WordItem::new(
            result.last().expect("首个模糊候选已插入").pinyin.clone(),
            row.value,
            row.weight,
            CandidateSource::Database,
            row.key,
        );
        item.fuzzy = true;
        result.push(item);
    }
}

fn fuzzy_row_cache_hash(rules: u32, limit: usize, typed: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    rules.hash(&mut hasher);
    limit.hash(&mut hasher);
    typed.hash(&mut hasher);
    hasher.finish()
}

fn fuzzy_row_cache_key_matches(
    cached: &CachedFuzzyRows,
    rules: u32,
    limit: usize,
    typed: &str,
) -> bool {
    cached.rules == rules && cached.limit == limit && cached.typed == typed
}

fn path_cache_key<'a>(raw: &'a str, segmentation: &'a str) -> &'a str {
    if segmentation.is_empty() {
        raw
    } else {
        segmentation
    }
}

/// 自动切分的冷副本先写入备忘，别名规范化只修改返回值，保留原有缓存容量与内容。
fn resolve_cached_segments(
    cache: &mut FifoCache<String, Vec<String>>,
    raw: &str,
    segmentation: &str,
) -> Vec<String> {
    let mut segments = if segmentation.is_empty() {
        if let Some(cached) = lookup_cached_segments(cache, raw) {
            cached
        } else {
            let segments = first_correction_cut(raw).unwrap_or_default();
            cache.insert(raw.to_owned(), segments.clone());
            segments
        }
    } else {
        split_segments(segmentation)
    };
    normalize_umlaut_aliases(&mut segments);
    segments
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
    let mut alternatives = Vec::new();
    // Seeding the costlier cuts keeps them out of the frequency-competing tier; they are appended after it.
    let mut seen: [Option<String>; ALTERNATIVE_SEGMENTATION_SEEN_CAPACITY] =
        std::array::from_fn(|_| None);
    let mut seen_length = 0;
    let primary_segmentation = resolution.segmentation.as_str();
    for cut in &resolution.costlier_corrected_cuts {
        remember_segmentation_key(
            &mut seen,
            &mut seen_length,
            primary_segmentation,
            join_segments(cut),
        );
    }
    let mut append = |candidate: Vec<String>| {
        let key = join_segments(&candidate);
        if !key.is_empty()
            && alternatives.len() < SYLLABLE_GRAPH_PATH_LIMIT
            && remember_segmentation_key(&mut seen, &mut seen_length, primary_segmentation, key)
        {
            if alternatives.is_empty() {
                alternatives.reserve(SYLLABLE_GRAPH_PATH_LIMIT);
            }
            alternatives.push(candidate);
        }
    };
    // Correction-mode alternatives follow the autocorrect switch; offering them unconditionally made a misspelled "sahng" produce the corrected word with autocorrection turned off. Correction alternatives go first: they explain the letters actually typed, which the alias readings after them do not.
    if types != 0 {
        for cut in &resolution.alternative_corrected_cuts {
            append(cut.to_vec());
        }
        for cut in cut_pinyin_by_mode(raw, CutMode::Correction) {
            append(cut);
        }
    }
    if !raw.contains('\'')
        && segments.len() <= MAX_SYLLABLES_FOR_MULTIPLE_SEGMENTATIONS
        && has_only_complete_pinyin_segments(segments)
    {
        let graph = build_syllable_graph(raw);
        for path in enumerate_complete_segmentations(&graph, SYLLABLE_GRAPH_PATH_LIMIT) {
            append(path);
        }
    }
    alternatives
}

fn remember_segmentation_key(
    seen: &mut [Option<String>; ALTERNATIVE_SEGMENTATION_SEEN_CAPACITY],
    seen_length: &mut usize,
    primary: &str,
    key: String,
) -> bool {
    if key == primary
        || *seen_length == ALTERNATIVE_SEGMENTATION_SEEN_CAPACITY
        || seen[..*seen_length]
            .iter()
            .flatten()
            .any(|existing| existing == &key)
    {
        return false;
    }
    seen[*seen_length] = Some(key);
    *seen_length += 1;
    true
}

/// 仅接受没有手动边界、没有替换字母的完整切分。
fn is_literal_segmentation(raw: &str, segments: &[String]) -> bool {
    !raw.contains('\'')
        && has_only_complete_pinyin_segments(segments)
        && segments
            .iter()
            .flat_map(|segment| segment.bytes())
            .eq(raw.bytes())
}

/// Letters of a row's matched code, what the fuzzy merge sorts by.
fn matched_letters(pinyin: &str) -> usize {
    pinyin.bytes().filter(|&byte| byte != b'\'').count()
}

/// 整句只按原拼音解码，`whole_sentence_insert_position` 把它放在原拼音覆盖整个输入的词库行之后；它排到首位，通常是因为原拼音没有这样的词，这时它只是替补。开了模糊音，按模糊读法覆盖整个输入的词库行就是这个输入的整词，替补要让给它：`zongguo` 开 z=zh 时「中国」排到整句「总国」前面。同一档里只要还有原拼音覆盖整个输入的精确词库行（整词、另一种切分的整词，哪怕排在更长的补全词后面），就一行也不动，精确仍先于模糊；补全出的更长词条不算覆盖。挪动只在覆盖整个输入的那一档里做，模糊行之间、其余行之间的先后都不变。不开模糊音时不经过这里。
fn yield_stand_in_sentence_to_fuzzy_rows(result: &mut [WordItem]) {
    let Some(first) = result.first() else {
        return;
    };
    if !first.sentence_association {
        return;
    }
    let letters = matched_letters(&first.pinyin);
    let bucket = result
        .iter()
        .take_while(|item| matched_letters(&item.pinyin) == letters)
        .count();
    let typed_reading_answers = result[..bucket].iter().any(|item| {
        !item.fuzzy
            && item.source.is_dictionary()
            && matched_letters(&item.canonical_pinyin) == letters
    });
    if typed_reading_answers {
        return;
    }
    // 稳定排序：模糊行整体挪到前面，各自内部的先后不变。
    result[..bucket].sort_by_key(|item| !item.fuzzy);
}

fn first_correction_cut(pinyin: &str) -> Option<Vec<String>> {
    cut_pinyin_by_mode(pinyin, CutMode::Correction)
        .into_iter()
        .next()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "dictionary/alternative_series_tests.rs"]
mod alternative_series_tests;

#[cfg(test)]
#[path = "dictionary/lattice_span_cache_tests.rs"]
mod lattice_span_cache_tests;

#[cfg(test)]
#[path = "dictionary/sentence_decode_cache_tests.rs"]
mod sentence_decode_cache_tests;

#[cfg(test)]
#[path = "dictionary/row_projection_tests.rs"]
mod row_projection_tests;

#[cfg(test)]
#[path = "dictionary/initial_expansion_projection_tests.rs"]
mod initial_expansion_projection_tests;

#[cfg(test)]
#[path = "dictionary/fuzzy_row_projection_tests.rs"]
mod fuzzy_row_projection_tests;

#[cfg(test)]
#[path = "dictionary/resolution_projection_tests.rs"]
mod resolution_projection_tests;

#[cfg(test)]
#[path = "dictionary/primary_segmentation_tests.rs"]
mod primary_segmentation_tests;

#[cfg(test)]
#[path = "dictionary/series_hit_resolution_tests.rs"]
mod series_hit_resolution_tests;

#[cfg(test)]
#[path = "dictionary/lazy_segments_tests.rs"]
mod lazy_segments_tests;

#[cfg(test)]
#[path = "dictionary/slot_key_ownership_tests.rs"]
mod slot_key_ownership_tests;

#[cfg(test)]
#[path = "dictionary/series_prefix_borrow_tests.rs"]
mod series_prefix_borrow_tests;

#[cfg(test)]
#[path = "dictionary/initial_row_key_tests.rs"]
mod initial_row_key_tests;

#[cfg(test)]
#[path = "dictionary/fuzzy_typed_borrow_tests.rs"]
mod fuzzy_typed_borrow_tests;

#[cfg(test)]
#[path = "dictionary/fuzzy_input_borrow_tests.rs"]
mod fuzzy_input_borrow_tests;

#[cfg(test)]
#[path = "dictionary/prefix_row_stream_tests.rs"]
mod prefix_row_stream_tests;

#[cfg(test)]
mod alternative_row_stream_tests;

#[cfg(test)]
mod filtered_query_tests;

#[cfg(test)]
mod empty_projection_tests;

#[cfg(test)]
mod online_cache_update_tests;
