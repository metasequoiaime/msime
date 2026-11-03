//! 共用一个 [`OnnxScorer`] 的那几份：会话挂在 runtime 上的逐键打分器，以及交给引擎、在拼整句时给前缀打分的那一份。它们锁的是同一个模型，前缀缓存也是同一份：引擎在束搜索里算过的前缀，逐键重排比较整池时直接复用。

use crate::OnnxScorer;
use msime_engine::ordering::{CandidateScorer, SentenceScorer};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

fn lock(scorer: &Mutex<OnnxScorer>) -> MutexGuard<'_, OnnxScorer> {
    scorer.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 共用重排器在一个会话里的那一份：每次打分时加锁。同一时刻通常只有一个会话在打字，锁几乎不会等。
///
/// 上屏后的新上文交给这个会话自己的后台线程编码（[`OnnxScorer::prime_context`]），按键线程不等这一步；线程在第一次上屏时才起，会话的这一份放开时随之结束。用户在编码完之前就按下一个键时，按键在锁上等它做完，与没有这个线程时按键自己编码花的时间相同。
pub struct SharedOnnxScorer {
    scorer: Arc<Mutex<OnnxScorer>>,
    sentence: Arc<dyn SentenceScorer>,
    primer: Option<std::sync::mpsc::Sender<String>>,
}

impl SharedOnnxScorer {
    pub fn new(scorer: Arc<Mutex<OnnxScorer>>) -> Self {
        Self {
            sentence: Arc::new(SharedSentenceScorer(Arc::clone(&scorer))),
            scorer,
            primer: None,
        }
    }

    fn lock(&self) -> MutexGuard<'_, OnnxScorer> {
        lock(&self.scorer)
    }
}

/// 交给引擎的那一份（[`SentenceScorer`]）：同一个模型，每次打分时加锁。
struct SharedSentenceScorer(Arc<Mutex<OnnxScorer>>);

impl SentenceScorer for SharedSentenceScorer {
    fn log_probabilities(&self, context: &str, candidates: &[&str]) -> Vec<f32> {
        lock(&self.0).log_probabilities(context, candidates)
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
        self.lock().log_probabilities(context, candidates)
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
