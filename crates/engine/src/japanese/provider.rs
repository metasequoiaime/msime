//! The Japanese candidate provider (schemes-lang.md §5.5), without the dropped `japanese_lexicon` step. Display order is insertion order; nothing is re-sorted by weight.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::decoder::JapaneseDictionary;
use super::matrix::search_converted;
use super::romaji::{
    convert_romaji_into, hiragana_to_katakana_into, is_single_kana_conversion,
    kana_for_romaji_prefix, RomajiConversion,
};
use crate::cache::FifoCache;
use crate::types::{CandidateSource, QueryRequest, SchemeType, WordItem};

pub const DYNAMIC_CACHE_CAPACITY: usize = 128;

const KANA_WEIGHT: i64 = 1_000_000;
/// 读音完整时平假名所在的最晚位置（从 0 数），首位留给最可能的转换。
const KANA_SLOT: usize = 1;
const KATAKANA_WEIGHT: i64 = 999_999;
const PREFIX_LEMMA_BASE: i64 = 980_000;
const SENTENCE_BASE: i64 = 900_000;
const PENDING_PREFIX_LEMMAS: usize = 24;
const READING_PREFIX_LEMMAS: usize = 16;
const SENTENCE_LIMIT: usize = 12;
/// Two kana: a one-kana reading is already answered by the kana rows and the sentence search.
const MIN_PREFIX_READING_BYTES: usize = 6;

fn join_reading(prefix: &str, suffix: &str) -> String {
    let mut reading = String::with_capacity(prefix.len() + suffix.len());
    reading.push_str(prefix);
    reading.push_str(suffix);
    reading
}

pub struct JapaneseProvider {
    model: PathBuf,
    /// `None` until the first query, then the shared model or `Some(None)` when it is missing or invalid; a failed load is not retried by this provider, as in the reference.
    dictionary: Option<Option<Arc<JapaneseDictionary>>>,
    dynamic: FifoCache<String, Vec<WordItem>>,
    conversion: RomajiConversion,
    katakana: String,
}

/// 按插入顺序保留唯一词面；`used` 之后是上次查询留下的可复用行。
struct Rows<'a> {
    items: Vec<WordItem>,
    code: &'a str,
    used: usize,
}

impl Rows<'_> {
    fn reserve(&mut self, additional: usize) {
        self.items.reserve(additional);
    }

    fn contains_word(&self, word: &str) -> bool {
        self.items[..self.used].iter().any(|item| item.word == word)
    }

    fn push(&mut self, word: &str, weight: i64, source: CandidateSource) {
        // 只扫描本次已写入的行，尾部旧行不参与去重。
        if word.is_empty() || self.contains_word(word) {
            return;
        }
        if let Some(item) = self.items.get_mut(self.used) {
            item.pinyin.clear();
            item.pinyin.push_str(self.code);
            item.canonical_pinyin.clear();
            item.canonical_pinyin.push_str(self.code);
            item.word.clear();
            item.word.push_str(word);
            item.weight = weight;
            item.source = source;
            item.scheme = SchemeType::Quanpin;
            item.fixed_position = 0;
            item.fuzzy = false;
            item.corrected_from.clear();
            item.sentence_association = false;
            item.sentence_words.clear();
        } else {
            self.items
                .push(WordItem::new(self.code, word, weight, source, self.code));
        }
        self.used += 1;
    }

    fn push_kana(&mut self, hiragana: &str, katakana: &str) {
        self.push(hiragana, KANA_WEIGHT, CandidateSource::Generated);
        self.push(katakana, KATAKANA_WEIGHT, CandidateSource::Generated);
    }

    /// 把已在列表里的 `word` 挪到不晚于 `slot` 的位置，其余行保持相对顺序。
    fn promote(&mut self, word: &str, slot: usize) {
        let Some(index) = self.items[..self.used]
            .iter()
            .position(|item| item.word == word)
        else {
            return;
        };
        if index > slot {
            self.items[slot..=index].rotate_right(1);
        }
    }

    fn finish_into(mut self, destination: &mut Vec<WordItem>) {
        self.items.truncate(self.used);
        *destination = self.items;
    }
}

impl JapaneseProvider {
    /// Shares the model at `model` on the first query, so sessions that never type Japanese never read the 66 MB file; a missing model leaves only the kana rows.
    pub fn new(model: &Path) -> Self {
        Self {
            model: model.to_path_buf(),
            dictionary: None,
            dynamic: FifoCache::new(DYNAMIC_CACHE_CAPACITY),
            conversion: RomajiConversion::default(),
            katakana: String::new(),
        }
    }

    pub fn query(&mut self, request: &QueryRequest) -> Vec<WordItem> {
        let mut destination = Vec::new();
        self.query_into(request, &mut destination);
        destination
    }

    /// 将日文查询直接写入已有候选行，保留列表和字符串容量。
    pub(crate) fn query_into(&mut self, request: &QueryRequest, destination: &mut Vec<WordItem>) {
        if !request.valid || request.scheme != SchemeType::JapaneseRomaji {
            destination.clear();
            return;
        }
        let mut rows = Rows {
            items: std::mem::take(destination),
            code: &request.raw_input_with_cases,
            used: 0,
        };
        // A bare minus opens a composition whose first choice is the long-vowel mark, with the plain hyphen kept as the alternative.
        if request.raw_input == "-" {
            rows.push("ー", KANA_WEIGHT, CandidateSource::Generated);
            rows.push("-", KATAKANA_WEIGHT, CandidateSource::Generated);
            rows.finish_into(destination);
            return;
        }
        convert_romaji_into(&request.raw_input, &mut self.conversion);
        let dictionary = self.dictionary();
        let conversion = &self.conversion;
        hiragana_to_katakana_into(&conversion.hiragana, &mut self.katakana);
        let kana_first = is_single_kana_conversion(conversion);
        if kana_first {
            rows.push_kana(&conversion.hiragana, &self.katakana);
        }

        if let Some(dictionary) = dictionary {
            // With letters still pending, the lemmas the letters can go on to spell lead: the sentence search can convert only the finished kana. With the reading complete, the lemmas whose reading only starts with it (predictions) go after the conversions of the reading itself: listed first, the cheapest longer readings fill the page and push the word the reading spells off it (にじ listed 二重, 二条 and 二次創作 ahead of 虹).
            let mut predictions = Vec::new();
            if !conversion.hiragana.is_empty() && !conversion.pending.is_empty() {
                // `kana_for_romaji_prefix` already limits the kana to spellings that start with the pending letters. Re-deriving romaji from each lemma's reading to check the prefix again would drop correct lemmas: a reading has several valid spellings and `hiragana_to_romaji` picks one, so しし reads `shishi` and fails `sis`.
                let pending_kana = kana_for_romaji_prefix(&conversion.pending);
                rows.reserve(
                    pending_kana
                        .len()
                        .saturating_mul(PENDING_PREFIX_LEMMAS)
                        .saturating_add(SENTENCE_LIMIT + 1),
                );
                for kana in pending_kana {
                    let prefix = join_reading(&conversion.hiragana, kana);
                    for lemma in dictionary.prefix_lemmas(&prefix, PENDING_PREFIX_LEMMAS) {
                        rows.push(
                            &lemma.surface,
                            PREFIX_LEMMA_BASE - i64::from(lemma.word_cost),
                            CandidateSource::Database,
                        );
                    }
                }
            } else if conversion.pending.is_empty()
                && conversion.hiragana.len() >= MIN_PREFIX_READING_BYTES
            {
                predictions = dictionary.prefix_lemmas(&conversion.hiragana, READING_PREFIX_LEMMAS);
            }
            rows.reserve(predictions.len() + SENTENCE_LIMIT + 1);
            for sentence in search_converted(&dictionary, conversion, SENTENCE_LIMIT) {
                rows.push(
                    &sentence.text,
                    SENTENCE_BASE - sentence.cost,
                    CandidateSource::Database,
                );
            }
            for lemma in predictions {
                rows.push(
                    &lemma.surface,
                    PREFIX_LEMMA_BASE - i64::from(lemma.word_cost),
                    CandidateSource::Database,
                );
            }
        }

        if !conversion.hiragana.is_empty() && !kana_first {
            rows.push_kana(&conversion.hiragana, &self.katakana);
            // 读音完整时，平假名本身最晚排在第二位。词库里没有这个假名词条时，它原本跟在全部汉字和联想后面：`tyou`/`chou` 的ちょう排在第 23 个，用户以为打不出来；有词条的きょう、にほん本来就在第二位。
            if conversion.pending.is_empty() {
                rows.promote(&conversion.hiragana, KANA_SLOT);
            }
        }

        if let Some(dynamic) = self.dynamic.get_ref(&request.raw_input) {
            let mut insertion = rows.used.min(if kana_first { 2 } else { 1 });
            for item in dynamic {
                // Dynamic rows are bounded by the cache quota; keep the word index in sync while inserting them.
                if rows.contains_word(&item.word) {
                    continue;
                }
                if let Some(target) = rows.items.get_mut(rows.used) {
                    target.pinyin.clone_from(&item.pinyin);
                    target.canonical_pinyin.clone_from(&item.canonical_pinyin);
                    target.word.clone_from(&item.word);
                    target.weight = item.weight;
                    target.source = item.source;
                    target.scheme = item.scheme;
                    target.fixed_position = item.fixed_position;
                    target.fuzzy = item.fuzzy;
                    target.corrected_from.clone_from(&item.corrected_from);
                    target.sentence_association = item.sentence_association;
                    target.sentence_words.clone_from(&item.sentence_words);
                } else {
                    rows.items.push(item.clone());
                }
                rows.used += 1;
                rows.items[insertion..rows.used].rotate_right(1);
                insertion += 1;
            }
        }
        rows.finish_into(destination);
    }

    /// Only a non-empty cloud word; replaces the previous cloud row for that code.
    pub fn cache_dynamic_candidate(
        &mut self,
        code: &str,
        word: &str,
        source: CandidateSource,
    ) -> bool {
        if code.is_empty() || word.is_empty() || source != CandidateSource::CloudSuggestion {
            return false;
        }
        let mut items = self.dynamic.get_ref_by(code).cloned().unwrap_or_default();
        items.retain(|item| item.source != source);
        items.push(WordItem::new(code, word, 1, source, code));
        self.dynamic.insert(code.to_owned(), items);
        true
    }

    /// Remove one provider's rows from cached Japanese readings.
    pub fn clear_online_candidates(&mut self, source: CandidateSource) {
        self.dynamic.retain_mut(|_, rows| {
            rows.retain(|item| item.source != source);
            !rows.is_empty()
        });
    }

    /// Clears the dynamic rows and keeps the model: it is immutable and shared process-wide.
    pub fn reset_cache(&mut self) {
        self.dynamic.clear();
    }

    fn dictionary(&mut self) -> Option<Arc<JapaneseDictionary>> {
        self.dictionary
            .get_or_insert_with(|| JapaneseDictionary::shared(&self.model))
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::super::decoder::test_model;
    use super::*;

    fn request(raw: &str) -> QueryRequest {
        QueryRequest {
            scheme: SchemeType::JapaneseRomaji,
            raw_input: raw.to_ascii_lowercase(),
            raw_input_with_cases: raw.to_owned(),
            valid: true,
            ..QueryRequest::default()
        }
    }

    fn words(items: &[WordItem]) -> Vec<&str> {
        items.iter().map(|item| item.word.as_str()).collect()
    }

    #[test]
    fn join_reading_allocates_only_result_bytes() {
        let reading = join_reading("か", "き");
        assert_eq!(reading, "かき");
        assert_eq!(reading.capacity(), reading.len());
    }

    fn provider_with(model: Option<Vec<u8>>) -> (tempfile::TempDir, JapaneseProvider) {
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("dict_japanese_test.dat");
        if let Some(bytes) = model {
            std::fs::write(&path, bytes).expect("write model");
        }
        let provider = JapaneseProvider::new(&path);
        (root, provider)
    }

    #[test]
    fn rows_scan_owned_words_when_deduplicating() {
        let mut rows = Rows {
            items: Vec::new(),
            code: "ka",
            used: 0,
        };
        assert!(!rows.contains_word("かな"));
        rows.push("かな", KANA_WEIGHT, CandidateSource::Generated);
        assert!(rows.contains_word("かな"));
        rows.push("かな", KATAKANA_WEIGHT, CandidateSource::Generated);
        assert_eq!(words(&rows.items), vec!["かな"]);
    }

    #[test]
    fn query_into_reuses_existing_candidate_rows() {
        let (_root, mut provider) = provider_with(None);
        let request = request("ka");
        let mut destination = provider.query(&request);
        let word_pointer = destination[0].word.as_ptr();

        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            provider.query_into(&request, &mut destination);
        });

        eprintln!("日文单假名 provider 热查询分配：{allocations}");
        assert_eq!(allocations, 0, "已有候选行与转换缓冲应复用");
        assert_eq!(words(&destination), ["か", "カ"]);
        assert_eq!(destination[0].word.as_ptr(), word_pointer);
    }

    fn assert_provider_reuses_conversion_strings(raw: &str) {
        let (_root, mut provider) = provider_with(None);
        let request = request(raw);
        let mut destination = provider.query(&request);
        let expected = destination.clone();
        let pointer = destination.as_ptr();
        for row in &mut destination {
            row.source = CandidateSource::CloudSuggestion;
            row.fixed_position = 7;
            row.fuzzy = true;
            row.corrected_from.push_str("synthetic");
            row.sentence_association = true;
            row.sentence_words.push("synthetic".to_owned());
        }
        for _ in 0..3 {
            let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
                provider.query_into(&request, &mut destination);
            });
            assert_eq!(destination, expected);
            assert_eq!(destination.as_ptr(), pointer);
            eprintln!(
                "日文 provider 合成输入长度 {} 热查询分配：{allocations}",
                raw.len()
            );
            assert_eq!(allocations, 0, "热查询不应重建转换字符串");
        }
    }

    #[test]
    fn complete_provider_queries_reuse_conversion_strings() {
        for raw in ["nihongo", "Sinnyou", "n'a", "xtsu", "ko-hi-"] {
            assert_provider_reuses_conversion_strings(raw);
        }
    }

    #[test]
    fn long_provider_queries_reuse_conversion_strings() {
        assert_provider_reuses_conversion_strings(&"ka".repeat(32));
    }

    #[test]
    fn pending_provider_queries_reuse_conversion_strings() {
        for raw in ["NiHoNg", "kak", "k", "ka漢字", ""] {
            assert_provider_reuses_conversion_strings(raw);
        }
    }

    #[test]
    fn bare_minus_query_reuses_rows_without_allocating() {
        let (_root, mut provider) = provider_with(None);
        let request = request("-");
        let mut destination = provider.query(&request);
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            provider.query_into(&request, &mut destination);
        });
        assert_eq!(allocations, 0);
        assert_eq!(words(&destination), ["ー", "-"]);
    }

    #[test]
    fn model_load_stays_lazy_and_missing_model_is_not_retried() {
        let (_root, mut provider) = provider_with(None);
        let mut invalid = request("ka");
        invalid.valid = false;
        assert!(provider.query(&invalid).is_empty());
        let mut wrong_scheme = request("ka");
        wrong_scheme.scheme = SchemeType::Quanpin;
        assert!(provider.query(&wrong_scheme).is_empty());
        assert_eq!(words(&provider.query(&request("-"))), ["ー", "-"]);
        assert!(provider.dictionary.is_none());
        assert_eq!(words(&provider.query(&request("ka"))), ["か", "カ"]);
        assert!(matches!(provider.dictionary, Some(None)));
        std::fs::write(&provider.model, test_model::smoke()).expect("write synthetic model");
        assert_eq!(
            words(&provider.query(&request("kanji"))),
            ["かんじ", "カンジ"]
        );
        assert!(matches!(provider.dictionary, Some(None)));
    }

    #[test]
    fn reused_rows_keep_query_order_and_dynamic_fields_across_edits() {
        let (_root, mut provider) = provider_with(Some(test_model::smoke()));
        let (_expected_root, mut expected_provider) = provider_with(Some(test_model::smoke()));
        for provider in [&mut provider, &mut expected_provider] {
            assert!(provider.cache_dynamic_candidate(
                "kanji",
                "雲候補",
                CandidateSource::CloudSuggestion
            ));
        }
        let mut destination = Vec::new();
        for text in ["sis", "kanji", "ka", "k", "-", "Sis", "kanji"] {
            let request = request(text);
            let expected = expected_provider.query(&request);
            provider.query_into(&request, &mut destination);
            assert_eq!(destination, expected, "{text}");
        }
        provider.clear_online_candidates(CandidateSource::CloudSuggestion);
        expected_provider.clear_online_candidates(CandidateSource::CloudSuggestion);
        let request = request("kanji");
        provider.query_into(&request, &mut destination);
        assert_eq!(destination, expected_provider.query(&request));
        provider.query_into(&QueryRequest::default(), &mut destination);
        assert!(destination.is_empty());
    }

    // test_engine_smoke.cpp:410-455, on the two-lemma synthetic model.
    #[test]
    fn prefix_lemmas_lead_whatever_spelling_the_reading_romanises_to() {
        let (_root, mut provider) = provider_with(Some(test_model::smoke()));

        let shishi = provider.query(&request("sis"));
        assert_eq!(shishi[0].word, "四肢", "{:?}", words(&shishi));
        assert_eq!(shishi[0].source, CandidateSource::Database);
        assert!(words(&shishi).contains(&"し") && words(&shishi).contains(&"シ"));
        assert!(shishi.iter().all(|item| !item.word.is_empty()));

        let bare_long_vowel = provider.query(&request("-"));
        assert_eq!(words(&bare_long_vowel), vec!["ー", "-"]);

        let kanji = provider.query(&request("kanj"));
        assert_eq!(kanji.first().map(|item| item.word.as_str()), Some("漢字"));
    }

    #[test]
    fn weights_sources_and_codes() {
        let (_root, mut provider) = provider_with(Some(test_model::smoke()));
        let items = provider.query(&request("Sis"));
        let shishi = &items[0];
        assert_eq!(shishi.weight, 980_000 - 1_200);
        assert_eq!(
            (shishi.pinyin.as_str(), shishi.canonical_pinyin.as_str()),
            ("Sis", "Sis")
        );
        assert!(items.iter().all(|item| item.scheme == SchemeType::Quanpin));
        // し has no lemma, so the sentence search reaches it first as an unknown-kana path and that row is the one kept; the katakana row has no such twin.
        let kana = items
            .iter()
            .find(|item| item.word == "し")
            .expect("kana row");
        assert_eq!(
            (kana.weight, kana.source),
            (900_000 - 12_000, CandidateSource::Database)
        );
        let katakana = items
            .iter()
            .find(|item| item.word == "シ")
            .expect("katakana row");
        assert_eq!(
            (katakana.weight, katakana.source),
            (999_999, CandidateSource::Generated)
        );

        // A whole-reading sentence: 漢字 is both the best path and a prefix lemma, listed once, as the conversion that comes first.
        let kanji = provider.query(&request("kanji"));
        assert_eq!(words(&kanji), vec!["漢字", "かんじ", "カンジ"]);
        assert_eq!(kanji[0].weight, 900_000 - 1_000);
    }

    #[test]
    fn a_complete_reading_lists_its_own_words_before_longer_ones() {
        // 二重 is far more common, but its reading only starts with にじ: typing にじ asks for 虹.
        let model = test_model::bytes(
            &[("にじ", "虹", 0, 0, 5_000), ("にじゅう", "二重", 0, 0, 100)],
            1,
            &[0],
        );
        let (_root, mut provider) = provider_with(Some(model));
        let niji = provider.query(&request("niji"));
        let at = |word: &str| words(&niji).iter().position(|w| *w == word);
        assert_eq!(niji[0].word, "虹", "{:?}", words(&niji));
        assert!(at("二重").is_some_and(|predicted| predicted > 0));
        // While a letter is still pending, the lemmas it can go on to spell lead.
        let nijy = provider.query(&request("nijy"));
        assert_eq!(nijy[0].word, "二重", "{:?}", words(&nijy));
    }

    /// 用户反馈 `tyou` 打不出ちょう：词库里只有ちょう的汉字词条、没有假名词条时，平假名原本排在全部转换之后。
    #[test]
    fn a_complete_reading_keeps_its_hiragana_on_the_first_page() {
        let model = test_model::bytes(
            &[
                ("ちょう", "超", 0, 0, 100),
                ("ちょう", "長", 0, 0, 200),
                ("ちょう", "町", 0, 0, 300),
                ("ちょう", "朝", 0, 0, 400),
                ("ちょう", "帳", 0, 0, 500),
                ("ちょう", "庁", 0, 0, 600),
            ],
            1,
            &[0],
        );
        let (_root, mut provider) = provider_with(Some(model));
        for raw in ["tyou", "chou"] {
            let rows = provider.query(&request(raw));
            assert_eq!(words(&rows)[1], "ちょう", "{raw}: {:?}", words(&rows));
            assert_eq!(rows[0].word, "超", "{raw}: {:?}", words(&rows));
        }
        // 还挂着半截字母时不挪：先给它能拼成的词。
        let pending = provider.query(&request("tyouk"));
        assert_ne!(
            words(&pending).get(1),
            Some(&"ちょう"),
            "{:?}",
            words(&pending)
        );
    }

    #[test]
    fn single_kana_leads_and_without_a_model_only_kana_remain() {
        let (_root, mut provider) = provider_with(None);
        assert_eq!(words(&provider.query(&request("ka"))), vec!["か", "カ"]);
        assert_eq!(
            words(&provider.query(&request("kanji"))),
            vec!["かんじ", "カンジ"]
        );
        assert!(provider.query(&request("k")).is_empty());

        let (_root, mut provider) = provider_with(Some(test_model::smoke()));
        let items = provider.query(&request("si"));
        assert_eq!(words(&items)[..2], ["し", "シ"]);
    }

    #[test]
    fn invalid_or_foreign_requests_are_empty() {
        let (_root, mut provider) = provider_with(Some(test_model::smoke()));
        let mut invalid = request("ka");
        invalid.valid = false;
        assert!(provider.query(&invalid).is_empty());
        let mut quanpin = request("ka");
        quanpin.scheme = SchemeType::Quanpin;
        assert!(provider.query(&quanpin).is_empty());
    }

    #[test]
    fn cloud_rows_follow_the_leading_kana_and_replace_each_other() {
        let (_root, mut provider) = provider_with(None);
        assert!(!provider.cache_dynamic_candidate("ka", "蚊", CandidateSource::AiSuggestion));
        assert!(!provider.cache_dynamic_candidate("", "蚊", CandidateSource::CloudSuggestion));
        assert!(!provider.cache_dynamic_candidate("ka", "", CandidateSource::CloudSuggestion));

        assert!(provider.cache_dynamic_candidate("ka", "蚊", CandidateSource::CloudSuggestion));
        let single_kana = provider.query(&request("ka"));
        assert_eq!(words(&single_kana), vec!["か", "カ", "蚊"]);
        assert_eq!(single_kana[2].source, CandidateSource::CloudSuggestion);
        assert_eq!(single_kana[2].weight, 1);

        assert!(provider.cache_dynamic_candidate("ka", "課", CandidateSource::CloudSuggestion));
        assert_eq!(
            words(&provider.query(&request("ka"))),
            vec!["か", "カ", "課"]
        );

        // Not a single kana: the cloud row goes to the second seat, and a word already listed is not repeated.
        assert!(provider.cache_dynamic_candidate(
            "kanji",
            "感じ",
            CandidateSource::CloudSuggestion
        ));
        assert_eq!(
            words(&provider.query(&request("kanji"))),
            vec!["かんじ", "感じ", "カンジ"]
        );
        assert!(provider.cache_dynamic_candidate(
            "kanji",
            "カンジ",
            CandidateSource::CloudSuggestion
        ));
        assert_eq!(
            words(&provider.query(&request("kanji"))),
            vec!["かんじ", "カンジ"]
        );

        provider.reset_cache();
        assert_eq!(words(&provider.query(&request("ka"))), vec!["か", "カ"]);
    }

    /// The shipped model through the provider: a sentence reading and a pending tail.
    #[test]
    fn real_model_answers_common_readings() {
        let Some(resources) = std::env::var_os("MSIME_EVAL_RESOURCES") else {
            eprintln!(
                "skipped: MSIME_EVAL_RESOURCES is not set to the dict-v2.0.1 resource directory"
            );
            return;
        };
        let mut provider =
            JapaneseProvider::new(&Path::new(&resources).join(crate::assets::JAPANESE_MODEL));
        let nihon = provider.query(&request("nihon"));
        assert!(words(&nihon)[..5].contains(&"日本"), "{:?}", words(&nihon));
        let kanj = provider.query(&request("kanj"));
        // Each pending kana contributes its own 24 cheapest prefix lemmas in kana order, so 漢字 follows the cheaper 感じ family.
        assert!(words(&kanj)[..24].contains(&"漢字"), "{:?}", words(&kanj));
        assert_eq!(words(&kanj)[0], "感じ");
        let sentence = provider.query(&request("watashihagakuseidesu"));
        assert!(sentence.len() > 2, "{:?}", words(&sentence));
    }
}
