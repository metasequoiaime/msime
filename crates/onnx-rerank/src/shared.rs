//! 共用一个 [`OnnxScorer`] 的那几份：会话挂在 runtime 上的逐键打分器，以及交给引擎、在拼整句时给前缀打分的那一份。它们锁的是同一个模型，前缀缓存也是同一份：引擎在束搜索里算过的前缀，逐键重排比较整池时直接复用。

use crate::OnnxScorer;
use msime_engine::ordering::{CandidateScorer, SentenceScorer};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 备忘里最多存几条文字的分，超过就清空重来。一次按键束搜索加逐键比较不过百来条，一段上文打下来也就几千条。
const SCORE_MEMO_LIMIT: usize = 8192;

/// 同一段上文下各条文字的分，几份共用：引擎在束搜索最后一列给整句打过的分，逐键比较整池时直接取，不再跑一遍模型。按模型实际读的上文（[`OnnxScorer::context_tail`]）作键，上文变了就清空。
#[derive(Default)]
struct ScoreMemo {
    context: String,
    by_text: HashMap<String, f32>,
}

/// 先查备忘，没打过分的文字一次交给模型，结果记进备忘；模型打不了分时返回空表，同 [`OnnxScorer`] 的 `log_probabilities`。锁的次序固定是先备忘后模型。
fn memoized(
    scorer: &Mutex<OnnxScorer>,
    memo: &Mutex<ScoreMemo>,
    context: &str,
    candidates: &[&str],
) -> Vec<f32> {
    if candidates.is_empty() || candidates.iter().any(|candidate| candidate.is_empty()) {
        return Vec::new();
    }
    let mut memo = lock(memo);
    let mut scorer = lock(scorer);
    let tail = scorer.context_tail(context);
    if memo.context != tail || memo.by_text.len() > SCORE_MEMO_LIMIT {
        memo.context = tail;
        memo.by_text.clear();
    }
    let missing: Vec<&str> = {
        let mut seen = HashSet::new();
        candidates
            .iter()
            .copied()
            .filter(|text| !memo.by_text.contains_key(*text) && seen.insert(*text))
            .collect()
    };
    if !missing.is_empty() {
        let scores = scorer.log_probabilities(context, &missing);
        if scores.len() != missing.len() {
            return Vec::new();
        }
        for (text, score) in missing.into_iter().zip(scores) {
            memo.by_text.insert(text.to_owned(), score);
        }
    }
    candidates.iter().map(|text| memo.by_text[*text]).collect()
}

/// 共用重排器在一个会话里的那一份：每次打分时加锁。同一时刻通常只有一个会话在打字，锁几乎不会等。
///
/// 上屏后的新上文交给这个会话自己的后台线程编码（[`OnnxScorer::prime_context`]），按键线程不等这一步；线程在第一次上屏时才起，会话的这一份放开时随之结束。用户在编码完之前就按下一个键时，按键在锁上等它做完，与没有这个线程时按键自己编码花的时间相同。
pub struct SharedOnnxScorer {
    scorer: Arc<Mutex<OnnxScorer>>,
    memo: Arc<Mutex<ScoreMemo>>,
    sentence: Arc<dyn SentenceScorer>,
    primer: Option<std::sync::mpsc::Sender<String>>,
}

impl SharedOnnxScorer {
    pub fn new(scorer: Arc<Mutex<OnnxScorer>>) -> Self {
        let memo = Arc::new(Mutex::new(ScoreMemo::default()));
        Self {
            sentence: Arc::new(SharedSentenceScorer {
                scorer: Arc::clone(&scorer),
                memo: Arc::clone(&memo),
            }),
            scorer,
            memo,
            primer: None,
        }
    }

    fn lock(&self) -> MutexGuard<'_, OnnxScorer> {
        lock(&self.scorer)
    }
}

/// 交给引擎的那一份（[`SentenceScorer`]）：同一个模型和分数备忘，每次打分时加锁。
struct SharedSentenceScorer {
    scorer: Arc<Mutex<OnnxScorer>>,
    memo: Arc<Mutex<ScoreMemo>>,
}

impl SentenceScorer for SharedSentenceScorer {
    fn log_probabilities(&self, context: &str, candidates: &[&str]) -> Vec<f32> {
        memoized(&self.scorer, &self.memo, context, candidates)
    }
}

/// 起一个编码上文的后台线程：每次只编码排队里最新的那段上文，连着几次上屏时中间的不必编码。发送端放开时线程结束；起不了线程时返回 `None`，按键照旧自己编码。
fn spawn_context_primer(scorer: Arc<Mutex<OnnxScorer>>) -> Option<std::sync::mpsc::Sender<String>> {
    let (sender, receiver) = std::sync::mpsc::channel::<String>();
    std::thread::Builder::new()
        .name("msime-rerank-context".into())
        .spawn(move || {
            while let Ok(mut context) = receiver.recv() {
                while let Ok(newer) = receiver.try_recv() {
                    context = newer;
                }
                lock(&scorer).prime_context(&context);
            }
        })
        .ok()?;
    Some(sender)
}

impl CandidateScorer for SharedOnnxScorer {
    fn context_length(&self) -> usize {
        self.lock().context_length()
    }

    fn scores_prediction_pool_within_keystroke_budget(&self) -> bool {
        self.lock().scores_prediction_pool_within_keystroke_budget()
    }

    fn log_probabilities(&mut self, context: &str, candidates: &[&str]) -> Vec<f32> {
        memoized(&self.scorer, &self.memo, context, candidates)
    }

    // 不转发就落到默认的 0，挂上的 ONNX 后端从来用不上整句池。
    fn sentence_pool(&self) -> usize {
        self.lock().sentence_pool()
    }

    fn static_evidence_weight(&self) -> f64 {
        self.lock().static_evidence_weight()
    }

    // 不转发就落到默认的 `false`，挂上 ONNX 后端时停顿后仍会让随包的 model-v1 再排一遍。
    fn supersedes_settled_rerank(&self) -> bool {
        self.lock().supersedes_settled_rerank()
    }

    fn sentence_scorer(&self) -> Option<Arc<dyn SentenceScorer>> {
        Some(Arc::clone(&self.sentence))
    }

    fn context_changed(&mut self, context: &str) {
        if self.primer.is_none() {
            self.primer = spawn_context_primer(Arc::clone(&self.scorer));
        }
        let sent = self
            .primer
            .as_ref()
            .is_some_and(|primer| primer.send(context.to_owned()).is_ok());
        if !sent {
            self.primer = None;
        }
    }
}
