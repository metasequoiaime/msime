//! The Japanese candidate provider (schemes-lang.md §5.5), without the dropped `japanese_lexicon` step. Display order is insertion order; nothing is re-sorted by weight.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::decoder::JapaneseDictionary;
use super::matrix::search_converted;
use super::romaji::{
    convert_romaji, hiragana_to_katakana, is_single_kana_conversion, kana_for_romaji_prefix,
};
use crate::cache::FifoCache;
use crate::types::{CandidateSource, QueryRequest, SchemeType, WordItem};

pub const DYNAMIC_CACHE_CAPACITY: usize = 128;

const KANA_WEIGHT: i64 = 1_000_000;
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
}

/// Rows unique by word, in insertion order.
struct Rows {
    items: Vec<WordItem>,
    code: String,
}

impl Rows {
    fn reserve(&mut self, additional: usize) {
        self.items.reserve(additional);
    }

    fn contains_word(&self, word: &str) -> bool {
        self.items.iter().any(|item| item.word == word)
    }

    fn push(&mut self, word: &str, weight: i64, source: CandidateSource) {
        if word.is_empty() || self.contains_word(word) {
            return;
        }
        self.items.push(WordItem::new(
            self.code.as_str(),
            word,
            weight,
            source,
            self.code.as_str(),
        ));
    }

    fn push_kana(&mut self, hiragana: &str) {
        self.push(hiragana, KANA_WEIGHT, CandidateSource::Generated);
        self.push(
            &hiragana_to_katakana(hiragana),
            KATAKANA_WEIGHT,
            CandidateSource::Generated,
        );
    }
}

impl JapaneseProvider {
    /// Shares the model at `model` on the first query, so sessions that never type Japanese never read the 66 MB file; a missing model leaves only the kana rows.
    pub fn new(model: &Path) -> Self {
        Self {
            model: model.to_path_buf(),
            dictionary: None,
            dynamic: FifoCache::new(DYNAMIC_CACHE_CAPACITY),
        }
    }

    pub fn query(&mut self, request: &QueryRequest) -> Vec<WordItem> {
        if !request.valid || request.scheme != SchemeType::JapaneseRomaji {
            return Vec::new();
        }
        let mut rows = Rows {
            items: Vec::with_capacity(2),
            code: request.raw_input_with_cases.clone(),
        };
        // A bare minus opens a composition whose first choice is the long-vowel mark, with the plain hyphen kept as the alternative.
        if request.raw_input == "-" {
            rows.push("ー", KANA_WEIGHT, CandidateSource::Generated);
            rows.push("-", KATAKANA_WEIGHT, CandidateSource::Generated);
            return rows.items;
        }
        let conversion = convert_romaji(&request.raw_input);
        let kana_first = is_single_kana_conversion(&conversion);
        if kana_first {
            rows.push_kana(&conversion.hiragana);
        }

        if let Some(dictionary) = self.dictionary() {
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
                rows.reserve(READING_PREFIX_LEMMAS + SENTENCE_LIMIT + 1);
                for lemma in dictionary.prefix_lemmas(&conversion.hiragana, READING_PREFIX_LEMMAS) {
                    rows.push(
                        &lemma.surface,
                        PREFIX_LEMMA_BASE - i64::from(lemma.word_cost),
                        CandidateSource::Database,
                    );
                }
            } else {
                rows.reserve(SENTENCE_LIMIT + 1);
            }
            for sentence in search_converted(&dictionary, &conversion, SENTENCE_LIMIT) {
                rows.push(
                    &sentence.text,
                    SENTENCE_BASE - sentence.cost,
                    CandidateSource::Database,
                );
            }
        }

        if !conversion.hiragana.is_empty() && !kana_first {
            rows.push_kana(&conversion.hiragana);
        }

        if let Some(dynamic) = self.dynamic.get_ref(&request.raw_input) {
            let mut insertion = rows.items.len().min(if kana_first { 2 } else { 1 });
            for item in dynamic {
                // Dynamic rows are bounded by the cache quota; scan the already-owned words to avoid cloning a second key into `seen`.
                if rows.contains_word(&item.word) {
                    continue;
                }
                rows.items.insert(insertion, item.clone());
                insertion += 1;
            }
        }
        rows.items
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
            code: "ka".to_owned(),
        };
        assert!(!rows.contains_word("かな"));
        rows.push("かな", KANA_WEIGHT, CandidateSource::Generated);
        assert!(rows.contains_word("かな"));
        rows.push("かな", KATAKANA_WEIGHT, CandidateSource::Generated);
        assert_eq!(words(&rows.items), vec!["かな"]);
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

        // A whole-reading sentence: 漢字 is both the prefix lemma and the best path, listed once.
        let kanji = provider.query(&request("kanji"));
        assert_eq!(words(&kanji), vec!["漢字", "かんじ", "カンジ"]);
        assert_eq!(kanji[0].weight, 980_000 - 1_000);
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
