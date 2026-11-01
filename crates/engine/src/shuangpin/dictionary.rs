//! `ShuangpinDictionary` (schemes-lang.md §1.7): series generation over decoded quanpin segments, the helpcode caches (overlays.md §5.1), and online rows. Pins, removals, frequency learning and phrases are written by the session through `user_dictionary` (see `ime::registry`), so the reference's word writers are not ported here. The Google sentence lines are gone; the lattice sentence goes in at `whole_sentence_insert_position` with nothing extra (overlays.md §2.2).

use std::collections::HashSet;
use std::sync::Arc;

use super::mid_sentence::{
    satisfies_syllable_helpcodes, syllable_helpcode_matches, syllable_helpcodes_signature,
};
use super::query::{
    raw_length_for_effective_prefix, remove_manual_delimiters,
    trim_trailing_letters_preserve_delimiters,
};
use super::typo_edges::{
    collect_priced_shuangpin_typo_edges, collect_shuangpin_typo_edges, priced_shuangpin_variants,
    touch_correction_pairs, touch_leads_correction, touch_supports_correction, LearnedTypos,
    SHUANGPIN_TYPO_TYPES,
};
use super::utils::{convert_seg_shuangpin_to_seg_complete_pinyin, pinyin_segmentation};
use super::ShuangpinProfile;
use crate::assets;
use crate::cache::FifoCache;
use crate::dictionary::pinyin::PinyinDatabase;
use crate::dictionary::DictRow;
use crate::helpcode::{
    match_single_helpcode, matches_double_helpcodes, HelpcodeKeymap, SingleHelpcodeMatch,
};
use crate::ime::online_batch::{
    replace_cached_online_candidate_batch, replace_online_candidate_batch,
};
use crate::lattice::decode::{decode_constrained_sentences, decode_sentences, SentenceNgramTables};
use crate::lattice::merge::{
    merge_decoded, merge_lattice_candidates_with_typo_policy, whole_sentence_insert_position,
    DecodedSentences,
};
use crate::lattice::neural::{
    shared_sentence_model, NeuralReranker, CONTEXT_CHARACTERS, MAX_RERANK_PATHS,
};
use crate::lattice::{SentencePath, TypoEdgeSource};
use crate::ordering::apply_order;
use crate::paths::RuntimePaths;
use crate::pinyin::jianpin::QuerySource;
use crate::pinyin::segment::split_segments;
use crate::pinyin::syllables::{has_only_complete_pinyin_segments, is_intact};
use crate::quanpin::dictionary::{
    CACHE_CAPACITY, INITIAL_CANDIDATE_LIMIT, TYPO_SPAN_CACHE_CAPACITY,
};
use crate::quanpin::series::fold_reading;
use crate::text::last_characters;
use crate::touch::{KeyGeometry, KeyTouch, TouchTuning};
use crate::types::{
    CandidateSource, SentenceAssociationOptions, ShuangpinProfileKind, SyllableHelpcode, WordItem,
};
use crate::user_dictionary::ngram_store::PersonalNgramStore;
use crate::user_dictionary::typo_profile::PersonalTypoProfile;

mod short_words;

// 双拼前缀候选的短合并直接扫描已有词，避免临时哈希表和重复索引分配。
const SMALL_PREFIX_DEDUP: usize = 64;
// 单字辅助码的常见候选批次很短，匹配分类和排列索引放在栈上即可。
const SMALL_SINGLE_HELP_CODE_BATCH: usize = 64;
/// 常见双拼纠错输入的两键分段放在栈上；更长的输入仍回退到堆容器，不改变准入范围。
const INLINE_TYPO_CODES: usize = 32;

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

/// `filter_by_syllable_helpcodes` 只筛按拼音出的汉字候选：整词都是汉字的词库/生成/云/AI 行
/// 才逐音节对应约束；英文、快捷短语、表情之类的不是拼音答案，留着。
fn is_pinyin_word(item: &WordItem) -> bool {
    matches!(
        item.source,
        CandidateSource::Database
            | CandidateSource::UserDatabase
            | CandidateSource::Generated
            | CandidateSource::Fallback
            | CandidateSource::NeuralKeyboard
            | CandidateSource::CloudSuggestion
            | CandidateSource::AiSuggestion
    ) && crate::text::count_han_chars(&item.word) == item.word.chars().count()
}

pub struct ShuangpinDictionary {
    profile: &'static ShuangpinProfile,
    database: PinyinDatabase,
    paths: RuntimePaths,
    sentence_tables: SentenceNgramTables,
    personal: Arc<PersonalNgramStore>,
    rerankers: Vec<NeuralReranker>,
    /// octagram `.gram` 语法模型；路径由 `set_sentence_association` 给定、经
    /// `GramDb::shared` 进程内共享，缺失或非法时为 `None` 退回无搭配打分。
    collocation_db: Option<Arc<crate::lattice::octagram::GramDb>>,
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
    /// 当前请求的逐音节辅助码约束（句中间接触发）。不清缓存：约束签名进缓存键，换约束等于换键
    /// （`set_syllable_helpcodes`）。
    pub(super) syllable_helpcodes: Vec<SyllableHelpcode>,
    syllable_helpcodes_signature: String,
    /// 当前请求每个按键的触点，与 `touch_input` 逐字节对齐（`set_key_touches`）；没有触点时为空，一切照旧。答案只对 `touch_input` 的前缀用得上触点，前缀的触点签名进缓存键（`touch_signature`）。
    key_touches: Vec<Option<KeyTouch>>,
    touch_input: String,
    key_geometry: Option<Arc<KeyGeometry>>,
    /// 按触点计价的参数。改变时清掉整句答案，它们按另一组参数计过价。
    touch_tuning: TouchTuning,
    /// 按两键码学到的双拼纠错（`PersonalTypoProfile`，整个进程按日志共用一份），只给触点路径打折。
    typo_profile: Arc<PersonalTypoProfile>,
    /// 带触点的答案按哪一代学习结果计的价；代次变了就清掉整句答案（`set_key_touches`）。
    typo_generation: u64,
    /// 当前请求允许纠错整句坐首选（`ShuangpinEngine` 按请求判断：整串带触点、没有辅助码、内置方案），而且 `lead_margin` 不是无穷大。为真时触点签名多一个标记，这样的答案和不允许时的答案各存各的缓存，不允许的请求（例如末尾单辅助码）读到的整句答案与没有首选规则时逐字节相同。
    lead_allowed: bool,
}

impl ShuangpinDictionary {
    pub fn new(profile: &'static ShuangpinProfile, paths: &RuntimePaths) -> Self {
        // READWRITE without CREATE: a missing dictionary stays missing and every query answers empty (SD:54-70).
        let mut database = PinyinDatabase::open(&paths.dictionary(assets::MAIN_DICTIONARY));
        let mut sentence_tables = SentenceNgramTables::default();
        if database.is_open() {
            database.warm_up();
            // 在构造时加载并保留表，避免共享淘汰后在按键路径重新读表。
            sentence_tables = SentenceNgramTables::load(paths);
            // Records the data version so the first cache hit has something to compare with.
            database.database_changed();
        }
        let personal = PersonalNgramStore::for_journal(&paths.user(assets::USER_JOURNAL));
        let typo_profile = PersonalTypoProfile::shared(&paths.user(assets::USER_JOURNAL));
        let typo_generation = typo_profile.generation();
        // The model loads lazily and bumps its version when it does. Left to the first lattice merge, that load would land on a keystroke and mark the answer it just scored as stale, so the next query for the same key would drop it together with any online rows put into it.
        drop(personal.model());
        let personal_version = personal.version();
        Self {
            profile,
            database,
            paths: paths.clone(),
            sentence_tables,
            personal,
            rerankers: Vec::new(),
            collocation_db: None,
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
            syllable_helpcodes: Vec::new(),
            syllable_helpcodes_signature: String::new(),
            key_touches: Vec::new(),
            touch_input: String::new(),
            key_geometry: None,
            touch_tuning: TouchTuning::default(),
            typo_profile,
            typo_generation,
            lead_allowed: false,
        }
    }

    /// 请求的触点跟着请求走（`ShuangpinEngine::query` 在查询之前交来）。`touches` 为空时清掉，之后的答案和缓存键与没有触点时完全相同。`lead_allowed` 是请求层面的首选规则（见字段 `lead_allowed`）。
    pub fn set_key_touches(
        &mut self,
        touches: &[Option<KeyTouch>],
        input: &str,
        geometry: Option<&Arc<KeyGeometry>>,
        tuning: TouchTuning,
        lead_allowed: bool,
    ) {
        if self.touch_tuning != tuning {
            self.touch_tuning = tuning;
            self.series_cache.clear();
            self.single_helpcode_cache.clear();
            self.reversed_single_helpcode_cache.clear();
            self.double_helpcode_cache.clear();
            self.personal_scored_keys.clear();
        }
        self.key_touches.clear();
        self.touch_input.clear();
        self.key_geometry = None;
        self.lead_allowed = false;
        let Some(geometry) = geometry.filter(|_| touches.len() == input.len()) else {
            return;
        };
        self.key_touches.extend_from_slice(touches);
        self.touch_input.push_str(input);
        self.key_geometry = Some(Arc::clone(geometry));
        self.lead_allowed = lead_allowed && tuning.lead_margin < f64::INFINITY;
        // 只有带触点的答案读学习结果：没有触点时连日志的状态也不看，桌面的缓存和以前一样。代次变了，按旧学习结果计价的整句答案作废（同全拼 `QuanpinDictionary` 按代次清缓存）。
        self.typo_profile.refresh_if_changed();
        let generation = self.typo_profile.generation();
        if generation != self.typo_generation {
            self.typo_generation = generation;
            self.series_cache.clear();
            self.single_helpcode_cache.clear();
            self.reversed_single_helpcode_cache.clear();
            self.double_helpcode_cache.clear();
            self.personal_scored_keys.clear();
        }
    }

    /// `touched` 这段输入的触点签名：几何代次和内容指纹，加上每个位置量化后的触点。代次不够：宿主可能用同一个代次下发挪过键的几何，同样的量化触点在新布局下计价不同，只认代次会把旧布局算出的答案当缓存命中。`touched` 不是带触点的那串输入的前缀（模糊音替换过的拼写、直接辅助码去掉辅码的串）或这一段一个触点都没有时为空，缓存键和没有触点时一样。
    fn touch_signature(&self, touched: &str) -> Option<String> {
        use std::fmt::Write as _;
        let touches = touch_slice(&self.key_touches, &self.touch_input, touched)?;
        let geometry = self.key_geometry.as_ref()?;
        let mut signature = String::with_capacity(32 + touches.len() * 4);
        signature.push('\u{2}');
        let _ = write!(
            signature,
            "{}:{:016x}",
            geometry.id(),
            geometry.fingerprint()
        );
        for touch in touches {
            match touch {
                Some(touch) => {
                    signature.push('\u{4}');
                    for byte in [touch.dx as u8, touch.dy as u8, touch.copy] {
                        signature.push(char::from(byte));
                    }
                }
                None => signature.push('\u{3}'),
            }
        }
        if self.lead_allowed {
            signature.push('\u{5}');
        }
        Some(signature)
    }

    /// 逐音节辅助码约束跟着请求走；签名拼在系列/辅助码缓存键后面，同一串拼音带不同约束不共用答案。
    pub fn set_syllable_helpcodes(&mut self, helpcodes: &[SyllableHelpcode]) {
        self.syllable_helpcodes = helpcodes.to_vec();
        self.syllable_helpcodes_signature = syllable_helpcodes_signature(helpcodes);
    }

    /// `rows_key + signature`：签名为空时直接借用原键，不为一次空拼接多分配。
    /// 返回值只借 `rows_key`，不会把 `self` 钉在不可变借用上。
    fn signed_rows_key<'a>(&self, rows_key: &'a str) -> std::borrow::Cow<'a, str> {
        self.signed_key(rows_key, rows_key)
    }

    /// `key` 接上逐音节辅助码约束的签名，再接上 `touched` 那段输入的触点签名（[`Self::touch_signature`]）。两者都为空时借用原键。
    fn signed_key<'a>(&self, key: &'a str, touched: &str) -> std::borrow::Cow<'a, str> {
        let touch = self.touch_signature(touched);
        if self.syllable_helpcodes_signature.is_empty() && touch.is_none() {
            std::borrow::Cow::Borrowed(key)
        } else {
            std::borrow::Cow::Owned(format!(
                "{key}{}{}",
                self.syllable_helpcodes_signature,
                touch.unwrap_or_default()
            ))
        }
    }

    /// 这个字能不能落在 helpcode 约束的音节上（`accepts_syllable_char`）。
    fn accepts_syllable_char(
        &self,
        helpcode: &SyllableHelpcode,
        character: &str,
        keymap: &HelpcodeKeymap,
    ) -> bool {
        keymap
            .code(character)
            .is_some_and(|code| syllable_helpcode_matches(helpcode, code.as_bytes()))
    }

    /// 跨度起点在 `syllable_start` 的词，每个覆盖到约束位置的字都要满足约束；跨度不含该位置时这条约束与它无关。
    pub(super) fn span_satisfies_syllable_helpcodes(
        &self,
        syllable_start: usize,
        word: &str,
        keymap: &HelpcodeKeymap,
    ) -> bool {
        let chars: Vec<char> = word.chars().collect();
        self.syllable_helpcodes.iter().all(|helpcode| {
            let index = helpcode.syllable.wrapping_sub(syllable_start);
            index >= chars.len()
                || self.accepts_syllable_char(helpcode, &chars[index].to_string(), keymap)
        })
    }

    /// 只筛按拼音出的汉字候选；英文、快捷短语之类的不是逐音节对应的，留着
    /// （`filter_by_syllable_helpcodes`）。云和 AI 拿到的是去掉段的拼音，不知道约束，结果同样要核。
    pub fn filter_by_syllable_helpcodes(
        &self,
        candidates: &mut Vec<WordItem>,
        keymap: &HelpcodeKeymap,
    ) {
        if self.syllable_helpcodes.is_empty() {
            return;
        }
        candidates.retain(|item| {
            !is_pinyin_word(item)
                || satisfies_syllable_helpcodes(&self.syllable_helpcodes, &item.word, keymap)
        });
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
        let mut rows = self.database.query_segments_keyed_flat(
            &segments,
            UNLIMITED_ROWS,
            QuerySource::Shuangpin,
        );
        if self.profile.kind == ShuangpinProfileKind::Shoudao && segmentation == "dk" {
            // 手心的 `dk` 同时对应 `dia` 和 `den`；保留原有的 `dia`，并补查 `den`。
            rows.extend(self.database.query_segments_keyed_flat(
                &["den".to_string()],
                UNLIMITED_ROWS,
                QuerySource::Shuangpin,
            ));
            rows.sort_by_key(|row| std::cmp::Reverse(row.weight));
            let mut seen = HashSet::new();
            rows.retain(|row| seen.insert(row.value.clone()));
        }
        rows.into_iter()
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
        // 同一串拼音带不同逐音节约束不是同一个答案：系列缓存键带上约束签名
        // （`generateSeries` 的 `cache_key + signature`），`generate` 内部的行缓存仍按原拼音走。
        let series_key_owned = self.signed_rows_key(rows_key);
        let series_key = series_key_owned.as_ref();
        let mut key = (!self.rerankers.is_empty()).then(|| self.sentence_cache_key(series_key));
        self.drop_personal_scored_results();
        let cache_hit = match key.as_ref() {
            Some(key) => self.series_cache.get_ref(key).is_some(),
            None => self.series_cache.get_ref_by(series_key).is_some(),
        };
        if cache_hit {
            self.reset_cache_if_database_changed();
            let cached = match key.as_ref() {
                Some(key) => self.series_cache.get_ref(key),
                None => self.series_cache.get_ref_by(series_key),
            };
            if let Some(cached) = cached {
                return cached.clone();
            }
        }

        let segments = split_segments(&convert_seg_shuangpin_to_seg_complete_pinyin(
            segmentation,
            self.profile,
        ));
        let incomplete_tail = segments.split_last().is_some_and(|(last, leading)| {
            !is_intact(last) && has_only_complete_pinyin_segments(leading)
        });
        let mut candidates = self.generate(pure, segmentation, rows_key);
        // Every shorter prefix group follows, longest first; phrase creation picks from them (SD:215-231). The reference appended each group whole, so a word several groups answer (a manual delimiter leaves empty pieces, and ni'''nn'i holds the ni group three times) was listed once per group; a word already listed keeps its first, longest-prefix seat instead.
        let mut prefix_rows = Vec::with_capacity(prefix_group_count(segmentation));
        let mut prefix = segmentation;
        while let Some(cut) = prefix.rfind('\'') {
            prefix = &prefix[..cut];
            let prefix_pure = remove_manual_delimiters(prefix);
            // 完整前缀作为补全失败时的回退，沿用它自己的缓存和按键范围。
            let rows = if incomplete_tail && prefix_rows.is_empty() {
                let end = raw_length_for_effective_prefix(rows_key, prefix_pure.len());
                self.generate_series(&prefix_pure, prefix, &rows_key[..end])
            } else {
                self.generate(&prefix_pure, prefix, "")
            };
            prefix_rows.push(rows);
        }
        append_prefix_rows(&mut candidates, prefix_rows);

        self.merge_sentences(&mut candidates, &segments, segmentation, pure, rows_key);

        let key = key.take().unwrap_or_else(|| series_key.to_owned());
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

    /// 没有直接匹配的词条时，在词网格解码前按音节位置过滤辅助码。
    pub(super) fn generate_constrained_sentences(
        &self,
        segmentation: &str,
        raw: &str,
        accepts: impl Fn(usize, &str) -> bool,
    ) -> Vec<WordItem> {
        let association = self.sentence_association.clone();
        let has_collocation =
            self.collocation_db.is_some() && association.collocation_weight != 0.0;
        let include_collocation_best = association.collocation_rerank && has_collocation;
        if !association.word_lattice && !include_collocation_best {
            return Vec::new();
        }
        let segments = split_segments(&convert_seg_shuangpin_to_seg_complete_pinyin(
            segmentation,
            self.profile,
        ));
        if !has_only_complete_pinyin_segments(&segments)
            && !segments.split_last().is_some_and(|(last, leading)| {
                !is_intact(last) && has_only_complete_pinyin_segments(leading)
            })
        {
            return Vec::new();
        }
        let mut options = self.sentence_tables.options(self.sentence_alternatives);
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
        let model = self.personal.model();
        options.personal = (!model.is_empty()).then_some(&*model);
        let mut lookup = |span: &[String]| {
            // 先按辅助码过滤再做搜索，不能用普通查询的每段截断提前丢掉目标字。
            self.database
                .query_lattice_span_with_trailing_initial(span, UNLIMITED_ROWS)
        };
        let mut paths = decode_constrained_sentences(&segments, &mut lookup, &options, accepts);
        if options.emit > 0 {
            paths.truncate(options.emit);
        }
        paths
            .into_iter()
            .map(|path| {
                let mut row = WordItem::new(
                    raw,
                    path.sentence,
                    (path.log_prob * 1000.0) as i64,
                    CandidateSource::Generated,
                    path.key,
                );
                row.sentence_association = true;
                row.sentence_words = path.words;
                row
            })
            .collect()
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
        let association = self.sentence_association.clone();
        let word_lattice = association.word_lattice;
        let has_collocation =
            self.collocation_db.is_some() && association.collocation_weight != 0.0;
        let include_collocation_best = association.collocation_rerank && has_collocation;
        if !word_lattice && self.rerankers.is_empty() && !include_collocation_best {
            return;
        }
        let mut options = self.sentence_tables.options(self.sentence_alternatives);
        if !self.rerankers.is_empty() {
            options.nbest = MAX_RERANK_PATHS;
        }
        options.include_lattice_best = word_lattice;
        options.include_collocation_best = include_collocation_best;
        options.show_next_on_duplicate = association.show_next_on_duplicate;
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
        // Held for the whole merge: the options borrow the model.
        let model = self.personal.model();
        options.personal = (!model.is_empty()).then_some(&*model);
        let database = &self.database;
        let span_limit = options.span_limit;
        if segments.split_last().is_some_and(|(last, leading)| {
            !is_intact(last) && has_only_complete_pinyin_segments(leading)
        }) {
            let mut lookup = |span: &[String]| {
                database.query_lattice_span_with_trailing_initial(span, span_limit)
            };
            let paths = decode_sentences(segments, &mut lookup, &options);
            merge_decoded(
                candidates,
                segments,
                typed,
                &options,
                &mut self.rerankers,
                &self.rescoring_context,
                DecodedSentences { paths, typo: None },
            );
            return;
        }
        let mut lookup = |span: &[String]| database.query_lattice_span(span, span_limit);
        let codes = typo_codes(self.autocorrect_types, segments, segmentation, rows_key);
        let types = self.autocorrect_types;
        let profile = self.profile;
        // 第 i 个两键码就是 `rows_key` 的第 2i、2i+1 字节（`typo_codes` 不收带 `'` 的输入），所以这段输入的触点按位置对得上。没有触点时走平面表，与桌面逐字节相同。
        let touched = touch_slice(&self.key_touches, &self.touch_input, rows_key)
            .zip(self.key_geometry.as_deref());
        let tuning = self.touch_tuning;
        // 学到的折扣只在触点路径上用；没有触点时连档案也不看。
        let learned = if touched.is_some() {
            LearnedTypos::new(&self.typo_profile, profile.kind)
        } else {
            LearnedTypos::default()
        };
        let span_cache = &mut self.typo_span_cache;
        // 按落点计价的变体第一次规划纠错边时算一次，之后判断落点是否支持纠错整句时直接复用。
        let mut priced = None;
        let mut typo_edges = |literal_best: &SentencePath| match touched {
            Some((touches, geometry)) => {
                let variants = priced.get_or_insert_with(|| {
                    priced_shuangpin_variants(
                        profile,
                        codes.as_slice(),
                        touches,
                        geometry,
                        &tuning,
                        learned,
                        types,
                    )
                });
                collect_priced_shuangpin_typo_edges(
                    database,
                    span_cache,
                    variants,
                    segments,
                    literal_best,
                )
            }
            None => collect_shuangpin_typo_edges(
                database,
                span_cache,
                profile,
                codes.as_slice(),
                segments,
                literal_best,
                types,
            ),
        };
        let typo_source: Option<&mut TypoEdgeSource<'_>> = if codes.is_empty() {
            None
        } else {
            Some(&mut typo_edges)
        };
        let typo = merge_lattice_candidates_with_typo_policy(
            candidates,
            segments,
            &mut lookup,
            typed,
            &options,
            typo_source,
            &mut self.rerankers,
            &self.rescoring_context,
            true,
        );
        // 纠错整句默认不抢首选：一律排在全码词条和整句块之后，只作为一个可选的改正。只有带触点、触点支持而且明显比字面整句好的那一行按首选规则坐第一位（`confident`）。
        let Some(typo) = typo else {
            return;
        };
        if candidates.iter().any(|item| item.word == typo.sentence) {
            return;
        }
        let Some(at) = typo_sentence_seat(candidates, segments) else {
            return;
        };
        let leads = typo.leads(tuning.lead_margin);
        let mut sentence = WordItem::new(
            typed,
            typo.sentence,
            (typo.score * 1000.0) as i64,
            CandidateSource::Generated,
            typo.key,
        );
        sentence.sentence_association = true;
        // 落点支持的改正才留在座位上：分数不比字面整句低太多，而且改动的每一键都是落点支持的改法。纠错整句来自上面的纠错边，带触点时 `priced` 已经算好。
        let touch_supported = priced.as_deref().is_some_and(|variants| {
            typo.score >= typo.literal_score - tuning.show_margin
                && touch_supports_correction(
                    variants,
                    &tuning,
                    segments,
                    sentence.canonical_pinyin.as_str(),
                )
        });
        // 首选规则（#10018 PR3），以下全部满足才坐第一位：
        // (a) 这段输入的每个键都有触点（请求整串都带触点、不是前缀，由 `ShuangpinEngine` 再核对一次）；
        // (b) 纠错整句的分数比字面整句高出每条纠错边 `lead_margin`；
        // (c) 改动的每一键在落点下的原价都不超过 `lead_penalty`；
        // (d) 字面没有全码词条，第一行就是字面整句块，坐上去以后字面整句紧跟在第二位；
        // (e) 这几对键码没被拒绝过（被抑制的输入根本不带触点）；
        // (f) 辅助码由 `ShuangpinEngine` 按请求把关（`lead_allowed`，进缓存键），本地模式由会话把关。
        // 另外它一定也是落点支持留在座位上的那一行（`touch_supported`）。`lead_allowed` 为假（`lead_margin` 是无穷大，或请求不允许）时后面几道闸一道也不算。
        let confident = touched.is_some_and(|(touches, geometry)| {
            self.lead_allowed
                && touch_supported
                && touches.iter().all(Option::is_some)
                && leads
                && whole_sentence_insert_position(candidates, segments) == 0
                && candidates
                    .first()
                    .is_some_and(|row| row.sentence_association && row.corrected_from.is_empty())
                && touch_leads_correction(
                    profile,
                    codes.as_slice(),
                    touches,
                    geometry,
                    &tuning,
                    sentence.canonical_pinyin.as_str(),
                    types,
                )
                && !self.correction_rejected(
                    rows_key,
                    touches,
                    geometry,
                    sentence.canonical_pinyin.as_str(),
                )
        });
        sentence.touch_supported = touch_supported;
        sentence.confident_correction = confident;
        sentence.sentence_words = typo.words;
        sentence.corrected_from = fold_reading(typed).into();
        candidates.insert(if confident { 0 } else { at }, sentence);
    }

    /// 首选规则 (e)：用户拒绝过这条纠错改动的某一对两键码（首选是纠错行时选了别的行，见 `session::learning`），这一对就一票否决首选，纠错行仍可排在后面。记哪一对与学习时同一个算法（`touch_correction_pairs`），对不上的（模糊音的替代拼写）当作否决。自定义方案不学，没有可查的记录，也就没法否决，所以一律不坐首选。
    fn correction_rejected(
        &self,
        typed: &str,
        touches: &[Option<KeyTouch>],
        geometry: &KeyGeometry,
        corrected_key: &str,
    ) -> bool {
        let kind = self.profile.kind;
        if kind == ShuangpinProfileKind::Custom {
            return true;
        }
        let Some(pairs) = touch_correction_pairs(
            self.profile,
            typed,
            touches,
            geometry,
            &self.touch_tuning,
            LearnedTypos::new(&self.typo_profile, kind),
            corrected_key,
            self.autocorrect_types,
        ) else {
            return true;
        };
        pairs.iter().any(|pair| {
            self.typo_profile
                .shuangpin_counts(kind, pair.typed, pair.intended)
                .rejected
                > 0
        })
    }

    /// 辅助码命中行优先；没有命中时先保留完整前缀，避免未完成尾音节让首选变短。
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
        let matches = single_helpcode_matches(&candidates, &normalized, keymap);
        let (mut result, unmatched) =
            reorder_single_helpcode_rows(candidates, matches.as_slice(), prefer_last);
        let no_helpcode_match = result.is_empty();
        // 整段输入也按普通拼音查询；参考实现把撇号一起切分，转换时丢弃空段（SD:370-372）。
        let original_segmentation = pinyin_segmentation(raw, self.profile);
        let whole = self.generate_series(raw, &original_segmentation, "");
        // 尾部裸字母也可能是下一个音节的首键；没有辅助码命中时，完整前缀行不能被较短的整段回退行挤到后面。
        append_helpcode_rows(&mut result, whole, unmatched);
        if no_helpcode_match {
            result.sort_by_key(|item| {
                std::cmp::Reverse(item.pinyin.bytes().filter(|&byte| byte != b'\'').count())
            });
        }
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
        // 末尾辅码的答案同样受逐音节约束影响，约束签名拼到缓存键尾；底下的整句答案按触点计价，`raw` 的触点签名也拼上。
        let cache_key = self.signed_key(&cache_key, raw).into_owned();
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
        let first_limited = candidates
            .iter()
            .position(is_limited)
            .expect("limited count checked above");
        candidates.reserve_exact(expanded.len() - limited);
        let mut removed = 0;
        candidates.retain(|item| {
            if is_limited(item) {
                removed += 1;
                false
            } else {
                true
            }
        });
        debug_assert_eq!(removed, limited);
        candidates.splice(first_limited..first_limited, expanded.iter().cloned());
        if !series_key.is_empty() {
            let key = self.sentence_cache_key(&self.signed_rows_key(series_key));
            self.series_cache.insert(key, candidates.clone());
        }
        true
    }

    /// 将在线行写入 `raw` 的系列缓存，命中时直接修改原页。
    pub fn insert_word_to_series_cache(
        &mut self,
        raw: &str,
        words: &[String],
        source: CandidateSource,
    ) -> bool {
        if raw.is_empty() {
            return false;
        }
        // 键与 `generate_series` 一致：带上当前请求的逐音节约束签名。
        let key = self.sentence_cache_key(&self.signed_rows_key(raw));
        if let Some(list) = self.series_cache.get_mut(&key) {
            return replace_cached_online_candidate_batch(list, raw, words, source);
        }
        // 缺槽时从空列表开始，下次查询 `raw` 即可读取在线行（SD:1373-1384）。
        let mut list = Vec::new();
        if !replace_online_candidate_batch(&mut list, raw, words, source) {
            return false;
        }
        self.series_cache.insert(key, list);
        true
    }

    /// 将在线行写入 `raw` 与 `help_codes` 对应的已有辅助码缓存。
    pub fn insert_word_to_active_helpcode_cache(
        &mut self,
        raw: &str,
        words: &[String],
        source: CandidateSource,
        help_codes: &str,
    ) -> bool {
        // 只有已有答案接收在线行；缺槽仍返回失败（SD:1386-1408）。
        let insert = |cache: &mut FifoCache<String, Vec<WordItem>>, key: &String| {
            let Some(list) = cache.get_mut(key) else {
                return false;
            };
            replace_cached_online_candidate_batch(list, raw, words, source)
        };
        // 键与 `generate_with_helpcodes` 一致：约束签名和 `raw` 的触点签名（`signed_key`）；两者都为空时原键直接用，不复制。
        let sign = |key: String| {
            let signed = match self.signed_key(&key, raw) {
                std::borrow::Cow::Owned(signed) => Some(signed),
                std::borrow::Cow::Borrowed(_) => None,
            };
            signed.unwrap_or(key)
        };
        if !help_codes.is_empty() {
            let key = sign(self.sentence_cache_key(&double_helpcode_cache_key(raw, help_codes)));
            return insert(&mut self.double_helpcode_cache, &key);
        }
        let key = sign(self.sentence_cache_key(raw));
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
        // 模型路径为空或不可载时回退 `None`：与 `msime-bigram.bin` 缺失的既有降级一致。
        self.collocation_db = if options.collocation_model.as_os_str().is_empty() {
            None
        } else {
            crate::lattice::octagram::GramDb::shared(&options.collocation_model)
        };
        self.sentence_association = options.clone();
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

struct SingleHelpcodeMatches {
    values: [SingleHelpcodeMatch; SMALL_SINGLE_HELP_CODE_BATCH],
    len: usize,
    heap: Option<Vec<SingleHelpcodeMatch>>,
}

impl SingleHelpcodeMatches {
    fn as_slice(&self) -> &[SingleHelpcodeMatch] {
        self.heap.as_deref().unwrap_or(&self.values[..self.len])
    }
}

fn single_helpcode_matches(
    candidates: &[WordItem],
    normalized: &str,
    keymap: &HelpcodeKeymap,
) -> SingleHelpcodeMatches {
    let mut values = [SingleHelpcodeMatch::None; SMALL_SINGLE_HELP_CODE_BATCH];
    if candidates.len() <= SMALL_SINGLE_HELP_CODE_BATCH {
        for (slot, candidate) in values.iter_mut().zip(candidates) {
            *slot = match_single_helpcode(&candidate.word, normalized, keymap);
        }
        return SingleHelpcodeMatches {
            values,
            len: candidates.len(),
            heap: None,
        };
    }
    let mut heap = Vec::with_capacity(candidates.len());
    heap.extend(
        candidates
            .iter()
            .map(|candidate| match_single_helpcode(&candidate.word, normalized, keymap)),
    );
    SingleHelpcodeMatches {
        values,
        len: 0,
        heap: Some(heap),
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
struct TypoCodes<'a> {
    values: [&'a str; INLINE_TYPO_CODES],
    len: usize,
    heap: Option<Vec<&'a str>>,
}

impl<'a> TypoCodes<'a> {
    fn empty() -> Self {
        Self {
            values: [""; INLINE_TYPO_CODES],
            len: 0,
            heap: None,
        }
    }

    fn as_slice(&self) -> &[&'a str] {
        self.heap.as_deref().unwrap_or(&self.values[..self.len])
    }

    fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }
}

fn typo_codes<'a>(
    types: u32,
    segments: &[String],
    segmentation: &'a str,
    rows_key: &str,
) -> TypoCodes<'a> {
    if types & SHUANGPIN_TYPO_TYPES == 0 || rows_key.contains('\'') {
        return TypoCodes::empty();
    }
    let mut inline = [""; INLINE_TYPO_CODES];
    let mut len = 0;
    let mut heap: Option<Vec<&'a str>> = None;
    for code in segmentation.split('\'') {
        if code.len() != 2 {
            return TypoCodes::empty();
        }
        if let Some(values) = &mut heap {
            values.push(code);
        } else if len < INLINE_TYPO_CODES {
            inline[len] = code;
            len += 1;
        } else {
            let mut values = Vec::with_capacity(segments.len());
            values.extend_from_slice(&inline[..len]);
            values.push(code);
            heap = Some(values);
        }
    }
    match heap {
        Some(values) if values.len() == segments.len() => TypoCodes {
            values: inline,
            len,
            heap: Some(values),
        },
        Some(_) => TypoCodes::empty(),
        None if len == segments.len() => TypoCodes {
            values: inline,
            len,
            heap: None,
        },
        None => TypoCodes::empty(),
    }
}

/// `touched` 是带触点的那串输入 `input` 的前缀、且这一段至少有一个触点时，返回这一段的触点。
fn touch_slice<'a>(
    touches: &'a [Option<KeyTouch>],
    input: &str,
    touched: &str,
) -> Option<&'a [Option<KeyTouch>]> {
    if touches.is_empty() || !input.starts_with(touched) {
        return None;
    }
    let touches = &touches[..touched.len()];
    touches.iter().any(Option::is_some).then_some(touches)
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
    if candidates.len() <= SMALL_SINGLE_HELP_CODE_BATCH {
        let mut order = [0usize; SMALL_SINGLE_HELP_CODE_BATCH];
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
    #[test]
    fn active_dictionary_reuses_ngram_tables_after_shared_eviction() {
        for bigram in [false, true] {
            for trigram in [false, true] {
                let fixture = crate::lattice::active_tables_tests::fixture(bigram, trigram);
                let selected = profile(crate::types::ShuangpinProfileKind::Xiaohe).unwrap();
                let mut dictionary = ShuangpinDictionary::new(selected, &fixture.paths);
                let segments = ["ni", "hao", "ma"].map(str::to_owned);
                crate::lattice::active_tables_tests::assert_query_stable(&fixture.paths, || {
                    let mut result = Vec::new();
                    dictionary.merge_sentences(
                        &mut result,
                        &segments,
                        "ni'hc'ma",
                        "nihcma",
                        "ni'hc'ma",
                    );
                    result
                });
            }
        }
    }

    /// 请求允许首选时（`lead_allowed`，而且 `lead_margin` 不是无穷大），触点签名多一个标记：允许和不允许时的答案各存各的缓存，不允许的请求（末尾辅助码的基础部分）读不到坐了首选的整句答案。不带触点时没有签名，键与以前相同。
    #[test]
    fn the_first_seat_rule_keys_its_own_cached_answers() {
        use crate::touch::{KeyGeometry, KeyTouch, TouchTuning};
        use crate::types::ShuangpinProfileKind;
        use std::sync::Arc;
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().to_path_buf();
        let paths = RuntimePaths {
            resources: directory.clone(),
            user_data: directory.clone(),
            cache: directory.clone(),
            dictionaries: directory,
        };
        let selected = profile(ShuangpinProfileKind::Xiaohe).unwrap();
        let mut dictionary = ShuangpinDictionary::new(selected, &paths);
        let mut keys = [[0.0f32; 4]; 26];
        for (index, key) in keys.iter_mut().enumerate() {
            *key = [index as f32 * 10.0, 0.0, 10.0, 10.0];
        }
        let geometry = Arc::new(KeyGeometry::new(7, &keys, None, &[]).unwrap());
        let touches = [Some(KeyTouch {
            dx: 0,
            dy: 0,
            copy: 0,
        }); 4];
        let leading = TouchTuning {
            lead_margin: 0.0,
            ..TouchTuning::default()
        };
        let mut signature = |tuning: TouchTuning, allowed: bool| {
            dictionary.set_key_touches(&touches, "mwgf", Some(&geometry), tuning, allowed);
            dictionary.touch_signature("mwgf")
        };
        let ruled = signature(leading, true).unwrap();
        let refused = signature(leading, false).unwrap();
        assert_ne!(ruled, refused);
        assert_eq!(signature(TouchTuning::default(), true).unwrap(), refused);
        assert_eq!(signature(TouchTuning::default(), false).unwrap(), refused);
        dictionary.set_key_touches(&[], "mwgf", None, leading, true);
        assert_eq!(dictionary.touch_signature("mwgf"), None);
    }

    /// 学习结果换了代次，按旧结果计价的整句和辅助码答案在下一次带触点的查询前清掉；不带触点的查询不看学习结果，缓存留着（桌面不变）。
    #[test]
    fn a_new_typo_generation_clears_touched_answers_only_on_the_touch_path() {
        use crate::touch::{KeyGeometry, KeyTouch, TouchTuning};
        use crate::types::ShuangpinProfileKind;
        use crate::user_dictionary::typo_profile::ShuangpinTypoOutcome;
        use std::sync::Arc;
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().to_path_buf();
        let paths = RuntimePaths {
            resources: directory.clone(),
            user_data: directory.clone(),
            cache: directory.clone(),
            dictionaries: directory,
        };
        let selected = profile(ShuangpinProfileKind::Xiaohe).unwrap();
        let mut dictionary = ShuangpinDictionary::new(selected, &paths);
        let mut keys = [[0.0f32; 4]; 26];
        for (index, key) in keys.iter_mut().enumerate() {
            *key = [index as f32 * 10.0, 0.0, 10.0, 10.0];
        }
        let geometry = Arc::new(KeyGeometry::new(7, &keys, None, &[]).unwrap());
        let touches = [Some(KeyTouch {
            dx: 0,
            dy: 0,
            copy: 0,
        }); 4];
        let cached = |dictionary: &mut ShuangpinDictionary| {
            dictionary
                .series_cache
                .insert("mwgf".to_owned(), vec![row("没跟")]);
            dictionary
                .single_helpcode_cache
                .insert("mwgf".to_owned(), vec![row("没跟")]);
        };
        let learn = || {
            dictionary_profile(&paths)
                .record_shuangpin(
                    ShuangpinProfileKind::Xiaohe,
                    &[(*b"gf", *b"gr")],
                    ShuangpinTypoOutcome::Accepted,
                )
                .unwrap();
        };

        dictionary.set_key_touches(
            &touches,
            "mwgf",
            Some(&geometry),
            TouchTuning::default(),
            false,
        );
        cached(&mut dictionary);
        // 代次没变：带触点的查询也不清。
        dictionary.set_key_touches(
            &touches,
            "mwgf",
            Some(&geometry),
            TouchTuning::default(),
            false,
        );
        assert!(dictionary.series_cache.contains(&"mwgf".to_owned()));

        learn();
        // 不带触点：不看学习结果，缓存留着。
        dictionary.set_key_touches(&[], "mwgf", None, TouchTuning::default(), false);
        assert!(dictionary.series_cache.contains(&"mwgf".to_owned()));
        assert!(dictionary
            .single_helpcode_cache
            .contains(&"mwgf".to_owned()));
        // 带触点：代次变了，整句和辅助码答案都清掉。
        dictionary.set_key_touches(
            &touches,
            "mwgf",
            Some(&geometry),
            TouchTuning::default(),
            false,
        );
        assert!(!dictionary.series_cache.contains(&"mwgf".to_owned()));
        assert!(!dictionary
            .single_helpcode_cache
            .contains(&"mwgf".to_owned()));
    }

    fn dictionary_profile(
        paths: &RuntimePaths,
    ) -> std::sync::Arc<crate::user_dictionary::typo_profile::PersonalTypoProfile> {
        crate::user_dictionary::typo_profile::PersonalTypoProfile::shared(
            &paths.user(crate::assets::USER_JOURNAL),
        )
    }

    use super::append_helpcode_rows;
    use super::append_prefix_rows;
    use super::double_helpcode_cache_key;
    use super::prefix_group_count;
    use super::reorder_single_helpcode_rows;
    use super::single_helpcode_matches;
    use super::typo_codes;
    use super::typo_sentence_seat;
    use super::ShuangpinDictionary;
    use super::SingleHelpcodeMatch;
    use super::INLINE_TYPO_CODES;
    use super::SHUANGPIN_TYPO_TYPES;
    use super::SMALL_SINGLE_HELP_CODE_BATCH;
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
    fn short_single_helpcode_matching_does_not_allocate_a_heap_vector() {
        let candidates = [row("甲"), row("乙"), row("丙")];
        let keymap = HelpcodeKeymap::from_codes(std::collections::HashMap::from([
            ("甲".to_owned(), "ab".to_owned()),
            ("乙".to_owned(), "cd".to_owned()),
            ("丙".to_owned(), "ef".to_owned()),
        ]));

        let (matches, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            single_helpcode_matches(&candidates, "a", &keymap)
        });

        assert_eq!(
            matches.as_slice(),
            [
                SingleHelpcodeMatch::First,
                SingleHelpcodeMatch::None,
                SingleHelpcodeMatch::None,
            ]
        );
        assert_eq!(allocations, 0, "短辅助码匹配不应申请临时堆向量");
    }

    #[test]
    fn long_single_helpcode_matching_keeps_the_heap_fallback() {
        let candidates = vec![row("甲"); SMALL_SINGLE_HELP_CODE_BATCH + 1];
        let matches = single_helpcode_matches(&candidates, "a", &HelpcodeKeymap::default());

        assert_eq!(matches.as_slice().len(), candidates.len());
        assert!(matches
            .as_slice()
            .iter()
            .all(|matched| *matched == SingleHelpcodeMatch::None));
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

    #[test]
    fn typical_typo_code_projection_does_not_allocate_a_heap_vector() {
        let segments: Vec<String> = ["mei", "gen", "xi"].map(str::to_owned).into();
        let (codes, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            typo_codes(SHUANGPIN_TYPO_TYPES, &segments, "mw'gf'xi", "mwgfxi")
        });

        assert_eq!(codes.as_slice(), ["mw", "gf", "xi"]);
        assert_eq!(allocations, 0, "双拼纠错码投影不应申请临时堆向量");
    }

    #[test]
    fn typo_code_projection_keeps_invalid_and_long_input_semantics() {
        let segments: Vec<String> = ["mei", "gen"].map(str::to_owned).into();
        assert!(typo_codes(SHUANGPIN_TYPO_TYPES, &segments, "mw'gf", "mw'gf").is_empty());
        assert!(typo_codes(SHUANGPIN_TYPO_TYPES, &segments, "mwgf", "mwgf").is_empty());
        assert!(typo_codes(0, &segments, "mw'gf", "mwgf").is_empty());

        let long_segments = vec!["aa".to_owned(); INLINE_TYPO_CODES + 1];
        let long_segmentation = long_segments.join("'");
        let codes = typo_codes(
            SHUANGPIN_TYPO_TYPES,
            &long_segments,
            &long_segmentation,
            &long_segmentation.replace('\'', ""),
        );
        assert_eq!(codes.as_slice().len(), INLINE_TYPO_CODES + 1);
    }
}

#[cfg(test)]
mod online_cache_update_tests;
