//! `QuanpinEngine` (`R/quanpin/engine.cpp`): helpcode filtering and reordering on top of the dictionary (quanpin.md §8), and the initial-candidate expansion gate (§11.5).

use std::collections::HashMap;

use crate::dictionary::DictRow;
use crate::error::Result;
use crate::helpcode::{
    filter_candidates_with_double_helpcodes, reorder_candidates_with_single_helpcode,
    HelpcodeKeymap,
};
use crate::lattice::decode::PinnedSpan;
use crate::paths::RuntimePaths;
use crate::pinyin::active_helpcode::{detect_active_helpcode_length, strip_active_helpcodes};
use crate::pinyin::segment::{cut_pinyin_by_mode, join_segments, split_segments, CutMode};
use crate::types::{request_autocorrect_mask, CandidateSource, QueryRequest, SchemeType, WordItem};

use super::QuanpinDictionary;

pub struct QuanpinEngine {
    dictionary: QuanpinDictionary,
}

impl QuanpinEngine {
    pub fn new(paths: &RuntimePaths) -> Self {
        Self {
            dictionary: QuanpinDictionary::new(paths),
        }
    }

    /// Double helpcode: query the base and keep matches; single: query the base and reorder; otherwise query the raw input with its segmentation, mask and fuzzy options (QE:39-81). Empty for an invalid request.
    ///
    /// Without a keymap every helpcode matches nothing: a double helpcode leaves no rows and a single one leaves the order alone. The reference fell back to a process-wide default table here, which the port does not keep; the session always passes the keymap it loaded.
    pub fn query(
        &mut self,
        request: &QueryRequest,
        keymap: Option<&HelpcodeKeymap>,
    ) -> Vec<WordItem> {
        if !request.valid {
            return Vec::new();
        }
        // Each setter is a no-op when the value is unchanged, so this costs nothing per keystroke. The association options and the context select the series slot rather than clearing it; only a change of the sentence-alternatives answer clears the cached lists.
        self.dictionary
            .set_sentence_alternatives(request.sentence_alternatives);
        self.dictionary
            .set_sentence_association(request.sentence_association);
        self.dictionary
            .set_rescoring_context(&request.rescoring_context);

        let types = request_autocorrect_mask(
            request.enable_quanpin_autocorrect_transposition,
            request.enable_quanpin_autocorrect_neighbor,
        );
        let helpcode_length = if request.enable_quanpin_helpcode {
            detect_active_helpcode_length(&request.raw_input, &request.raw_input_with_cases)
        } else {
            0
        };
        if helpcode_length == 0 {
            return self.dictionary.query(
                &request.raw_input,
                &request.segmentation,
                types,
                request.fuzzy_pinyin,
            );
        }

        let base = strip_active_helpcodes(&request.raw_input, &request.raw_input_with_cases);
        // An active helpcode follows complete pinyin, so the base always has a cut.
        let base_segmentation = cut_pinyin_by_mode(&base, CutMode::Correction)
            .first()
            .map(|segments| join_segments(segments))
            .unwrap_or_default();
        let codes = &request.raw_input[request.raw_input.len() - helpcode_length..];
        let rows = self
            .dictionary
            .query(&base, &base_segmentation, types, request.fuzzy_pinyin);
        let empty = HelpcodeKeymap::default();
        let keymap = keymap.unwrap_or(&empty);
        if helpcode_length == 2 {
            filter_candidates_with_double_helpcodes(rows, codes, keymap)
        } else {
            reorder_candidates_with_single_helpcode(rows, codes, keymap)
        }
    }

    /// Only for a quanpin request with no active helpcode whose first segment is one letter (QE:83-98).
    pub fn expand_initial_candidates(
        &mut self,
        request: &QueryRequest,
        candidates: &mut Vec<WordItem>,
    ) -> bool {
        if request.scheme != SchemeType::Quanpin
            || (request.enable_quanpin_helpcode
                && detect_active_helpcode_length(&request.raw_input, &request.raw_input_with_cases)
                    > 0)
        {
            return false;
        }
        let segments = split_segments(&request.segmentation);
        match segments.first() {
            Some(first) if first.len() == 1 => self.dictionary.expand_initial_candidates(
                &request.raw_input,
                &request.segmentation,
                request_autocorrect_mask(
                    request.enable_quanpin_autocorrect_transposition,
                    request.enable_quanpin_autocorrect_neighbor,
                ),
                first,
                candidates,
            ),
            _ => false,
        }
    }

    /// Put online rows into the series cache slot the request's query used, so they show on the next refresh.
    pub fn insert_online_words(
        &mut self,
        request: &QueryRequest,
        words: &[String],
        source: CandidateSource,
    ) -> bool {
        self.dictionary.insert_online_words(
            &request.raw_input,
            &request.segmentation,
            request_autocorrect_mask(
                request.enable_quanpin_autocorrect_transposition,
                request.enable_quanpin_autocorrect_neighbor,
            ),
            words,
            source,
        )
    }

    pub fn clear_online_candidates(&mut self, source: CandidateSource) {
        self.dictionary.clear_online_candidates(source);
    }

    pub fn find_candidate(&self, key: &str, value: &str) -> Option<WordItem> {
        self.dictionary.find_candidate(key, value)
    }

    pub fn knows_han_char(&self, han: &str) -> bool {
        self.dictionary.knows_han_char(han)
    }

    pub fn best_weights(&self, keys: &[String]) -> HashMap<String, i64> {
        self.dictionary.best_weights(keys)
    }

    pub fn create_word_from_canonical_pinyin(&mut self, pinyin: &str, word: &str) -> Result<()> {
        self.dictionary
            .create_word_from_canonical_pinyin(pinyin, word)
    }

    pub fn conversion_rows(&mut self, span: &[String], limit: usize) -> Vec<DictRow> {
        self.dictionary.conversion_rows(span, limit)
    }

    pub fn convert_pinned(&mut self, syllables: &[String], pins: &[PinnedSpan]) -> Vec<PinnedSpan> {
        self.dictionary.convert_pinned(syllables, pins)
    }

    pub fn reset_cache(&mut self) {
        self.dictionary.reset_cache()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::quanpin::fixture::Fixture;
    use crate::quanpin::QuanpinScheme;
    use crate::types::SchemeKey;

    fn request(text: &str) -> QueryRequest {
        let mut scheme = QuanpinScheme::new();
        for byte in text.bytes() {
            scheme.handle_key(SchemeKey::Letter(byte));
        }
        let mut request = scheme.build_request();
        request.enable_quanpin_helpcode = true;
        request.sentence_alternatives = true;
        request
    }

    fn words(items: &[WordItem]) -> Vec<&str> {
        items.iter().map(|item| item.word.as_str()).collect()
    }

    fn keymap() -> HelpcodeKeymap {
        HelpcodeKeymap::from_codes(HashMap::from([
            ("你".to_string(), "rx".to_string()),
            ("拟".to_string(), "sy".to_string()),
            ("好".to_string(), "nz".to_string()),
        ]))
    }

    fn fixture() -> Fixture {
        let fixture = Fixture::new();
        fixture
            .insert("ni", "你", 8000)
            .insert("ni", "拟", 7000)
            .insert("ni'hao", "你好", 10_000)
            .insert("ni'hao", "拟好", 30);
        fixture
    }

    #[test]
    fn invalid_request_answers_nothing() {
        let fixture = fixture();
        let mut engine = QuanpinEngine::new(&fixture.paths);
        assert!(engine.query(&QueryRequest::default(), None).is_empty());
    }

    #[test]
    fn double_helpcode_filters_the_base_query() {
        let fixture = fixture();
        let mut engine = QuanpinEngine::new(&fixture.paths);
        let keymap = keymap();
        // 你好: first code r, last code n.
        let rows = engine.query(&request("nihaoRN"), Some(&keymap));
        assert_eq!(words(&rows), ["你好"]);
        assert!(engine.query(&request("nihaoRN"), None).is_empty());
    }

    #[test]
    fn single_helpcode_reorders_the_base_query() {
        let fixture = fixture();
        let mut engine = QuanpinEngine::new(&fixture.paths);
        let keymap = keymap();
        let rows = engine.query(&request("niS"), Some(&keymap));
        assert_eq!(words(&rows)[0], "拟");
        let plain = engine.query(&request("niS"), None);
        assert_eq!(words(&plain)[0], "你");
    }

    #[test]
    fn helpcode_switch_off_queries_the_letters_as_typed() {
        let fixture = fixture();
        let mut engine = QuanpinEngine::new(&fixture.paths);
        let keymap = keymap();
        let mut disabled = request("nihaoRN");
        disabled.enable_quanpin_helpcode = false;
        let rows = engine.query(&disabled, Some(&keymap));
        // The scheme stripped the helpcode letters from the segmentation either way.
        assert_eq!(disabled.segmentation, "ni'hao");
        assert!(words(&rows).contains(&"拟好"));
    }

    #[test]
    fn expansion_gate_needs_a_quanpin_single_letter_without_helpcode() {
        let fixture = Fixture::new();
        for i in 0..30 {
            fixture.insert("ni", &format!("n{i}"), 1_000 - i);
        }
        let mut engine = QuanpinEngine::new(&fixture.paths);
        let single = request("n");
        let mut rows = engine.query(&single, None);
        assert_eq!(rows.len(), 24);

        let mut shuangpin = single.clone();
        shuangpin.scheme = SchemeType::Shuangpin;
        assert!(!engine.expand_initial_candidates(&shuangpin, &mut rows));
        assert!(!engine.expand_initial_candidates(&request("ni"), &mut rows));
        assert!(engine.expand_initial_candidates(&single, &mut rows));
        assert_eq!(rows.len(), 30);
    }

    /// With the keyboard model loaded the trimmed context is part of the series slot: an online row is read back only under the context it was stored with, and a store lands in the slot of the last query's context. Fails if `query` stops passing the request's context to the dictionary. Needs the keyboard model in `MSIME_EVAL_RESOURCES`.
    #[test]
    fn online_rows_follow_the_reranker_context_slot() {
        let model =
            match crate::lattice::neural::test_model_path(crate::assets::NEURAL_MODEL_KEYBOARD) {
                Ok(model) => model,
                Err(reason) => {
                    eprintln!("skipped: {reason}");
                    return;
                }
            };
        let fixture = fixture();
        std::fs::copy(
            &model,
            fixture.paths.resource(crate::assets::NEURAL_MODEL_KEYBOARD),
        )
        .expect("model copy");
        let mut engine = QuanpinEngine::new(&fixture.paths);
        let mut typed = request("nihao");
        typed.sentence_association = crate::types::SentenceAssociationOptions {
            neural_keyboard: true,
            ..crate::types::SentenceAssociationOptions::default()
        };
        let mut query = |engine: &mut QuanpinEngine, context: &str| {
            typed.rescoring_context = context.into();
            let rows = engine.query(&typed, None);
            (typed.clone(), rows)
        };

        let (asked, _) = query(&mut engine, "上文");
        assert!(engine.insert_online_words(
            &asked,
            &["甲".to_string()],
            CandidateSource::AiSuggestion
        ));
        assert!(words(&query(&mut engine, "上文").1).contains(&"甲"));
        assert!(!words(&query(&mut engine, "另一段上文").1).contains(&"甲"));
        assert!(words(&query(&mut engine, "上文").1).contains(&"甲"));

        let (asked, _) = query(&mut engine, "别处");
        assert!(engine.insert_online_words(
            &asked,
            &["乙".to_string()],
            CandidateSource::AiSuggestion
        ));
        assert!(words(&query(&mut engine, "别处").1).contains(&"乙"));
        assert!(!words(&query(&mut engine, "上文").1).contains(&"乙"));
    }
}
