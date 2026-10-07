//! Neural reranking of the lattice's n-best (overlays.md §1.6.2-§1.6.3) on the `chinese-ime-lm` crate. The model never generates sentences; it only reorders lattice paths and contributes one row. Only the keyboard model runs here, synchronously on the session's own `Reranker` on every keystroke, whose prefix cache keeps it cheap. The desktop model, whose p95 of 153 ms does not fit a keystroke, is not the engine's: the input runtime runs it as its settled reranker once typing pauses.

use std::io::Read;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use chinese_ime_lm::{Reranker, SentenceModel};
use lru::LruCache;

use super::decode::SentencePath;
use crate::ordering::apply_order;
use crate::text::last_characters;
use crate::types::CandidateSource;

pub const RERANK_LAMBDA: f64 = 0.5;
pub const MAX_RERANK_PATHS: usize = 12;
pub const CONTEXT_CHARACTERS: usize = 64;
/// The shipped keyboard model is about 4.5 MiB; the bound leaves room for a larger compatible model without letting a stray file make a session allocate without bound.
pub const MAX_MODEL_BYTES: u64 = 64 * 1024 * 1024;
const MODEL_CACHE_CAPACITY: usize = 8;

/// The lattice score is converted as log10 the way the C++ did (patch:869-871); the arithmetic is ported unchanged so the two models' blend keeps the weight it was tuned with.
const LOG10_TO_NATS: f64 = std::f64::consts::LN_10;

static MODELS: OnceLock<Mutex<LruCache<PathBuf, Option<Arc<SentenceModel>>>>> = OnceLock::new();

/// A shipped model for tests: from `MSIME_EVAL_RESOURCES` (the dictionary resource set), else from `MSIME_NEURAL_MODEL_DIR` (where `scripts/fetch_neural_model.py` puts both models, `target/neural-model` by default); the error is the reason a test skips.
#[cfg(test)]
pub(crate) fn test_model_path(name: &str) -> Result<PathBuf, String> {
    let directories: Vec<PathBuf> = ["MSIME_EVAL_RESOURCES", "MSIME_NEURAL_MODEL_DIR"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect();
    if directories.is_empty() {
        return Err("neither MSIME_EVAL_RESOURCES nor MSIME_NEURAL_MODEL_DIR is set".to_owned());
    }
    directories
        .iter()
        .map(|directory| directory.join(name))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            format!("{name} is in neither MSIME_EVAL_RESOURCES nor MSIME_NEURAL_MODEL_DIR")
        })
}

/// Cache recent models by path; a load failure is remembered as `None`, so a missing file is not re-read on every keystroke (patch:902-912).
pub fn shared_sentence_model(path: &Path) -> Option<Arc<SentenceModel>> {
    let mut models = MODELS
        .get_or_init(|| {
            Mutex::new(LruCache::new(
                NonZeroUsize::new(MODEL_CACHE_CAPACITY).unwrap(),
            ))
        })
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if let Some(cached) = models.get(path) {
        return cached.clone();
    }
    let model = load_model(path);
    models.put(path.to_path_buf(), model.clone());
    model
}

/// A missing, oversized or malformed model means no neural rows, never a failed session: the lattice still answers.
fn load_model(path: &Path) -> Option<Arc<SentenceModel>> {
    if !std::fs::symlink_metadata(path).ok()?.file_type().is_file() {
        return None;
    }
    let file = crate::paths::open_file_no_follow(path).ok()?;
    let file_size = file.metadata().ok()?.len();
    if file_size > MAX_MODEL_BYTES {
        return None;
    }
    let mut bytes = Vec::with_capacity(file_size as usize);
    // Bounded again while reading, in case the file grew after the size check.
    file.take(MAX_MODEL_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_MODEL_BYTES {
        return None;
    }
    SentenceModel::load(&bytes).ok().map(Arc::new)
}

/// The first `min(n, 12)` indices stable-sorted by `static * ln 10 + lambda * (neural - static * ln 10)` descending, the rest in order. `None` for fewer than two paths or mismatched lengths (patch:914-944). `neural` holds per-sentence summed natural-log probabilities, `static_log10` the lattice scores.
///
/// `neural` scores the same paths as `static_log10`, in order; only its first `min(n, 12)` entries are read, so a caller may score only those and pass exactly that many.
pub fn rerank_order(neural: &[f64], static_log10: &[f64]) -> Option<Vec<usize>> {
    let n = static_log10.len();
    let scored = n.min(MAX_RERANK_PATHS);
    if n < 2 || (neural.len() != n && neural.len() != scored) {
        return None;
    }
    let combined: Vec<f64> = (0..scored)
        .map(|index| {
            let statik = static_log10[index] * LOG10_TO_NATS;
            statik + RERANK_LAMBDA * (neural[index] - statik)
        })
        .collect();
    let mut order: Vec<usize> = (0..n).collect();
    // Stable so that sentences the two models score identically keep the lattice's order, and the unscored tail past the cap stays as it was.
    order[..scored].sort_by(|&a, &b| combined[b].total_cmp(&combined[a]));
    Some(order)
}

pub struct NeuralReranker {
    pub source: CandidateSource,
    reranker: Reranker,
}

impl NeuralReranker {
    /// `source` is `NeuralKeyboard`, the one sentence-model row the engine emits.
    pub fn new(source: CandidateSource, model: Arc<SentenceModel>) -> Self {
        Self {
            source,
            reranker: Reranker::new(model),
        }
    }

    /// Reorder `paths` by `rerank_order` conditioned on the last 64 characters of `context`; false leaves them untouched. The crate returns mean per-character log-probabilities; the sum is that times the character count.
    pub fn rerank(&mut self, paths: &mut [SentencePath], context: &str) -> bool {
        if paths.len() < 2 {
            return false;
        }
        let scored = paths.len().min(MAX_RERANK_PATHS);
        let texts: Vec<&str> = paths[..scored]
            .iter()
            .map(|path| path.sentence.as_str())
            .collect();
        let means = self
            .reranker
            .log_probabilities(last_characters(context, CONTEXT_CHARACTERS), &texts);
        // A model that answered for some sentences only is inconsistent; half a reranking is worse than none, so the lattice order stays.
        if means.len() != scored {
            return false;
        }
        let neural: Vec<f64> = means
            .iter()
            .zip(&texts)
            .map(|(mean, text)| f64::from(*mean) * text.chars().count() as f64)
            .collect();
        let statik: Vec<f64> = paths.iter().map(|path| path.log_prob).collect();
        let Some(order) = rerank_order(&neural, &statik) else {
            return false;
        };
        apply_order(paths, &order);
        true
    }
}

#[cfg(test)]
fn reorder_paths(paths: &mut [SentencePath], order: &[usize]) {
    apply_order(paths, order);
}

#[cfg(test)]
mod tests {
    use super::super::decode::tests::{lookup, syllables, table};
    use super::super::decode::{LatticeOptions, TypoEdge};
    use super::super::merge::merge_lattice_candidates;
    use super::*;
    use crate::assets;
    use crate::types::WordItem;

    fn path(sentence: &str, log_prob: f64) -> SentencePath {
        SentencePath {
            sentence: sentence.into(),
            key: String::new(),
            log_prob,
            words: vec![sentence.into()],
            typo_edges: 0,
        }
    }

    #[test]
    fn rerank_order_blends_the_two_scores() {
        // Static in log10, neural in nats: path 1 is worse on the lattice but the model prefers it enough.
        let statik = [-1.0, -1.2];
        let neural = [-10.0, -2.0];
        let combined0 = -LOG10_TO_NATS + 0.5 * (-10.0 + LOG10_TO_NATS);
        let combined1 = -1.2 * LOG10_TO_NATS + 0.5 * (-2.0 + 1.2 * LOG10_TO_NATS);
        assert!(combined1 > combined0);
        assert_eq!(rerank_order(&neural, &statik), Some(vec![1, 0]));
        // The model agreeing with the lattice leaves the order.
        assert_eq!(rerank_order(&[-2.0, -10.0], &statik), Some(vec![0, 1]));
    }

    #[test]
    fn rerank_order_is_stable_on_ties() {
        assert_eq!(
            rerank_order(&[-1.0, -1.0, -1.0], &[-1.0, -1.0, -1.0]),
            Some(vec![0, 1, 2])
        );
    }

    #[test]
    fn rerank_order_declines_what_it_cannot_order() {
        assert_eq!(rerank_order(&[], &[]), None);
        assert_eq!(
            rerank_order(&[-1.0], &[-1.0]),
            None,
            "one path has nothing to reorder"
        );
        assert_eq!(rerank_order(&[-1.0], &[-1.0, -2.0]), None);
        assert_eq!(rerank_order(&[-1.0, -2.0, -3.0], &[-1.0, -2.0]), None);
    }

    #[test]
    fn rerank_order_leaves_the_tail_past_twelve() {
        let n = 15;
        let statik: Vec<f64> = (0..n).map(|index| -(index as f64)).collect();
        // The model reverses the scored twelve and would reverse the tail too if it were read.
        let neural: Vec<f64> = (0..n).map(|index| index as f64 * 100.0).collect();
        let order = rerank_order(&neural, &statik).unwrap();
        assert_eq!(&order[..12], &(0..12).rev().collect::<Vec<_>>()[..]);
        assert_eq!(&order[12..], &[12, 13, 14]);
        // Scoring only the first twelve is the same answer.
        assert_eq!(rerank_order(&neural[..12], &statik), Some(order));
    }

    #[test]
    fn rerank_order_constants() {
        assert_eq!(RERANK_LAMBDA, 0.5);
        assert_eq!(MAX_RERANK_PATHS, 12);
        assert_eq!(CONTEXT_CHARACTERS, 64);
        assert_eq!(last_characters("a你好", 2), "你好");
        assert_eq!(last_characters("a你好", 0), "");
    }

    #[test]
    fn reranking_paths_reuses_the_input_storage() {
        let mut paths = vec![path("甲", -1.0), path("乙", -2.0), path("丙", -3.0)];
        let capacity = paths.capacity();
        let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            reorder_paths(&mut paths, &[2, 0, 1]);
        });
        assert_eq!(
            paths
                .iter()
                .map(|path| path.sentence.as_str())
                .collect::<Vec<_>>(),
            ["丙", "甲", "乙"]
        );
        assert_eq!(paths.capacity(), capacity);
        assert_eq!(allocations, 0);
    }

    #[test]
    fn a_missing_model_is_remembered() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(assets::NEURAL_MODEL_KEYBOARD);
        assert!(shared_sentence_model(&path).is_none());
        std::fs::write(&path, b"not a model").unwrap();
        assert!(shared_sentence_model(&path).is_none());
        let garbage = directory.path().join("garbage.safetensors");
        std::fs::write(&garbage, b"not a model").unwrap();
        assert!(
            shared_sentence_model(&garbage).is_none(),
            "a malformed model loads as nothing"
        );
    }

    #[test]
    fn the_model_cache_is_bounded_across_paths() {
        let mut directories = Vec::new();
        for index in 0..=MODEL_CACHE_CAPACITY {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join(format!("model-{index}.safetensors"));
            assert!(shared_sentence_model(&path).is_none());
            directories.push(directory);
        }
        let cache = MODELS.get().unwrap().lock().unwrap();
        assert!(cache.len() <= MODEL_CACHE_CAPACITY);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_model_is_not_loaded() {
        use std::os::unix::fs::symlink;

        let Ok(model) = test_model_path(assets::NEURAL_MODEL_KEYBOARD) else {
            return;
        };
        assert!(
            load_model(&model).is_some(),
            "fixture must be a valid model"
        );
        let directory = tempfile::tempdir().unwrap();
        let linked = directory.path().join(assets::NEURAL_MODEL_KEYBOARD);
        symlink(&model, &linked).unwrap();

        assert!(load_model(&linked).is_none());
    }

    /// A shipped model (`test_model_path`), or the reason the test cannot run.
    fn resource_model(name: &str) -> Result<Arc<SentenceModel>, String> {
        let path = test_model_path(name)?;
        shared_sentence_model(&path).ok_or_else(|| format!("{} did not load", path.display()))
    }

    #[test]
    fn a_real_model_scores_and_reorders() {
        let model = match resource_model(assets::NEURAL_MODEL_KEYBOARD) {
            Ok(model) => model,
            Err(reason) => {
                eprintln!("skipping a_real_model_scores_and_reorders: {reason}");
                return;
            }
        };
        let mut reranker = NeuralReranker::new(CandidateSource::NeuralKeyboard, model.clone());
        let mut paths = vec![path("输入发", -5.0), path("输入法", -5.1)];
        assert!(reranker.rerank(&mut paths, "我在用一个新的"));
        assert_eq!(
            paths[0].sentence, "输入法",
            "the model knows the common word"
        );
        let mut single = vec![path("输入法", -5.0)];
        assert!(!reranker.rerank(&mut single, ""));

        // The sum the reranker uses is the crate's mean times the length.
        let means = Reranker::new(model).log_probabilities("", &["输入法"]);
        assert_eq!(means.len(), 1);
        assert!(means[0] < 0.0);
    }

    /// 904bd0976: with a reranker running, the typo decode still runs and still starts from the unreranked best, so the typo sentence answers the literal reading rather than whatever the model preferred.
    #[test]
    fn the_typo_decode_reads_the_unreranked_best_while_a_model_runs() {
        let model = match resource_model(assets::NEURAL_MODEL_KEYBOARD) {
            Ok(model) => model,
            Err(reason) => {
                eprintln!(
                    "skipping the_typo_decode_reads_the_unreranked_best_while_a_model_runs: {reason}"
                );
                return;
            }
        };
        let mut rerankers = vec![NeuralReranker::new(CandidateSource::NeuralKeyboard, model)];
        let options = LatticeOptions {
            nbest: MAX_RERANK_PATHS,
            show_next_on_duplicate: true,
            ..LatticeOptions::default()
        };

        // The model moves 输入法 ahead of the lattice's 输入发; the typo source still sees 输入发.
        let rows = table(&[
            ("shu'ru", &[("输入", 20000)]),
            ("fa", &[("法", 800000), ("发", 900000), ("罚", 100000)]),
        ]);
        let mut seen = Vec::new();
        let mut source = |best: &SentencePath| {
            seen.push(best.sentence.clone());
            Vec::new()
        };
        let mut candidates: Vec<WordItem> = Vec::new();
        let typo = merge_lattice_candidates(
            &mut candidates,
            &syllables("shu'ru'fa"),
            &mut lookup(&rows),
            "shurufa",
            &options,
            Some(&mut source),
            &mut rerankers,
            "我在用一个新的",
        );
        assert!(typo.is_none());
        assert_eq!(seen, ["输入发"]);
        let row = |source: CandidateSource| {
            candidates
                .iter()
                .find(|item| item.source == source)
                .map(|item| item.word.as_str())
        };
        assert_eq!(row(CandidateSource::Generated), Some("输入发"));
        assert_eq!(row(CandidateSource::NeuralKeyboard), Some("输入法"));

        // And a planned typo edge still comes back as the typo sentence.
        let rows = table(&[
            ("ta", &[("他", 900000)]),
            ("shi", &[("是", 900000)]),
            ("jian", &[("见", 500000)]),
            ("shi'jian", &[("时间", 30000)]),
        ]);
        let mut seen = Vec::new();
        let mut source = |best: &SentencePath| {
            seen.push(best.sentence.clone());
            vec![TypoEdge {
                start: 1,
                end: 3,
                key: "shi'jian".into(),
                value: "事件".into(),
                weight: 90000,
                penalty: 0.5,
            }]
        };
        let mut candidates: Vec<WordItem> = Vec::new();
        let typo = merge_lattice_candidates(
            &mut candidates,
            &syllables("ta'shi'jian"),
            &mut lookup(&rows),
            "tashijian",
            &options,
            Some(&mut source),
            &mut rerankers,
            "",
        )
        .expect("a typo sentence beside the reranked rows");
        assert_eq!(seen, ["他时间"]);
        assert_eq!(typo.sentence, "他事件");
        assert!(candidates.iter().all(|item| item.word != "他事件"));
    }

    /// The keyboard-model rerank on the real dictionary, keystroke by keystroke, as a mobile host with the switch on runs it: every prefix of a nine-syllable reading is answered and the full reading carries the NeuralKeyboard row. This pins that the rerank is on the key path; its latency is measured by `crates/input-runtime/examples/rerank_latency.rs`, not here, because a debug `cargo test` timing would only be noise. Needs the dict-v2.0.1 resources with the keyboard model in `MSIME_EVAL_RESOURCES`.
    #[test]
    fn the_keyboard_rerank_answers_every_keystroke_on_the_real_dictionary() {
        use crate::paths::RuntimePaths;
        use crate::quanpin::dictionary::QuanpinDictionary;
        use crate::types::{FuzzyPinyinOptions, SentenceAssociationOptions};

        let Some(resources) = std::env::var_os("MSIME_EVAL_RESOURCES").map(PathBuf::from) else {
            eprintln!(
                "skipped: MSIME_EVAL_RESOURCES is not set to the dict-v2.0.1 resource directory"
            );
            return;
        };
        if !resources.join(assets::NEURAL_MODEL_KEYBOARD).is_file() {
            eprintln!(
                "skipped: {} is not in MSIME_EVAL_RESOURCES",
                assets::NEURAL_MODEL_KEYBOARD
            );
            return;
        }
        let user = tempfile::tempdir().expect("user directory");
        let paths = RuntimePaths {
            resources: resources.clone(),
            user_data: user.path().to_path_buf(),
            cache: user.path().to_path_buf(),
            dictionaries: resources,
        };
        let mut dictionary = QuanpinDictionary::new(&paths);
        dictionary.set_sentence_association(SentenceAssociationOptions {
            word_lattice: true,
            neural_keyboard: true,
            show_next_on_duplicate: true,
        });
        dictionary.set_rescoring_context("今天晚上");
        let key = "womenyiqiquchifan";
        let mut last = Vec::new();
        for end in 1..=key.len() {
            last = dictionary.query(&key[..end], "", 0, FuzzyPinyinOptions::default());
            assert!(!last.is_empty(), "{}", &key[..end]);
        }
        let neural: Vec<&str> = last
            .iter()
            .filter(|item| item.source == CandidateSource::NeuralKeyboard)
            .map(|item| item.word.as_str())
            .collect();
        assert_eq!(
            neural.len(),
            1,
            "{:?}",
            last.iter()
                .map(|item| item.word.as_str())
                .collect::<Vec<_>>()
        );
    }
}
