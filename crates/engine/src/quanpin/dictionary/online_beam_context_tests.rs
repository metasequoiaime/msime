use super::*;
use crate::ordering::{SentenceScorer, SentenceScorerHandle};
use crate::quanpin::fixture::Fixture;
use std::sync::Arc;

struct FlatSentenceScorer;

impl SentenceScorer for FlatSentenceScorer {
    fn log_probabilities(&self, _context: &str, candidates: &[&str]) -> Vec<f32> {
        vec![-1.0; candidates.len()]
    }
}

#[test]
fn online_rows_survive_refresh_with_beam_scorer_and_stay_in_their_context() {
    let fixture = Fixture::new();
    fixture.insert("ni'hao", "你好", 1000);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    dictionary.set_sentence_scorer(&SentenceScorerHandle(Some(Arc::new(FlatSentenceScorer))));
    for context in ["", "合成上文"] {
        for source in [
            CandidateSource::AiSuggestion,
            CandidateSource::CloudSuggestion,
        ] {
            dictionary.set_rescoring_context(context);
            let before = dictionary.query("nihao", "ni'hao", 0, FuzzyPinyinOptions::default());
            assert!(!before.is_empty());
            assert!(dictionary.insert_online_words(
                "nihao",
                "ni'hao",
                0,
                &["合成答案".into()],
                source
            ));
            let rows = dictionary.query("nihao", "ni'hao", 0, FuzzyPinyinOptions::default());
            assert!(
                rows.iter()
                    .any(|row| row.word == "合成答案" && row.source == source),
                "模型开启时在线候选必须能在刷新后读回"
            );
            dictionary.set_rescoring_context("另一段合成上文");
            let rows = dictionary.query("nihao", "ni'hao", 0, FuzzyPinyinOptions::default());
            assert!(!rows.iter().any(|row| row.word == "合成答案"));
            dictionary.set_rescoring_context(context);
            let rows = dictionary.query("nihao", "ni'hao", 0, FuzzyPinyinOptions::default());
            assert!(rows
                .iter()
                .any(|row| row.word == "合成答案" && row.source == source));
            dictionary.clear_online_candidates(source);
        }
    }
}
