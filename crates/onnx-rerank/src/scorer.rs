//! 跑模型的部分：ONNX Runtime 会话、模型缓存的张量，以及 [`CandidateScorer`] 的实现。

use crate::prefix::{PrefixCache, StepInputs};
use crate::{ModelInfo, Vocabulary};
use msime_engine::ordering::CandidateScorer;
use ort::session::Session;
use ort::value::{DynValue, Tensor};
use std::sync::OnceLock;

/// 上文留给候选的字数：[`CandidateScorer::context_length`] 报上文字数加这么多，`rerank_context` 据此在候选不太长时把上文完整交过来，又在候选很长时提前截短。
const CANDIDATE_ROOM: usize = 16;

/// 缓存里的候选前缀超过这么多就从上文重来：长时间不上屏、反复改拼音时缓存只增不减，每次按键的注意力也跟着变长。
const PREFIX_LIMIT: usize = 512;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("ONNX Runtime: {0}")]
    Runtime(#[from] ort::Error),
    #[error("model.json or vocab.json: {0}")]
    Metadata(#[from] serde_json::Error),
}

/// 进程里只建一个 ONNX Runtime 环境，所有会话共用。
fn environment() -> Result<&'static ort::environment::Environment, ort::Error> {
    static ENVIRONMENT: OnceLock<ort::environment::Environment> = OnceLock::new();
    if let Some(environment) = ENVIRONMENT.get() {
        return Ok(environment);
    }
    let built = ort::init()
        .with_name("msime-rerank")
        .with_telemetry(false)
        .build()?;
    Ok(ENVIRONMENT.get_or_init(|| built))
}

/// 模型缓存：每个节点各层的键和值、最后一层的隐状态和对数归一化常数。
struct Cache {
    k: DynValue,
    v: DynValue,
    h: DynValue,
    z: DynValue,
}

/// chinese-ime-lm 的单图按键模型。上文不变时跨按键复用前缀，上文一变（上屏）就重新编码。
pub struct OnnxScorer {
    session: Session,
    info: ModelInfo,
    vocabulary: Vocabulary,
    prefixes: PrefixCache,
    cache: Option<Cache>,
    /// 缓存是为哪段上文编码的。
    context: Option<String>,
}

impl OnnxScorer {
    /// `model` 是 ORT 格式的 `lm_step.ort`，`model_json` 与 `vocab_json` 是同一次导出的 `model.json` 和 `vocab.json`；`threads` 是一次图运行用几个线程。
    pub fn load(
        model: &[u8],
        model_json: &str,
        vocab_json: &str,
        threads: usize,
    ) -> Result<Self, LoadError> {
        let info: ModelInfo = serde_json::from_str(model_json)?;
        #[derive(serde::Deserialize)]
        struct Tokens {
            tokens: Vec<String>,
        }
        let tokens: Tokens = serde_json::from_str(vocab_json)?;
        let session = Session::builder(environment()?)?
            .with_intra_threads(threads.max(1))
            .map_err(ort::Error::from)?
            .commit_from_memory(model)?;
        Ok(Self {
            session,
            vocabulary: Vocabulary::new(&tokens.tokens, info.unk),
            info,
            prefixes: PrefixCache::default(),
            cache: None,
            context: None,
        })
    }

    fn empty_cache(&self) -> Result<Cache, ort::Error> {
        let (l, h, d, w) = (
            self.info.layers as i64,
            self.info.heads as i64,
            self.info.head_dim as i64,
            self.info.width as i64,
        );
        Ok(Cache {
            k: Tensor::from_array((vec![l, h, 0, d], Vec::<f32>::new()))?.into_dyn(),
            v: Tensor::from_array((vec![l, h, 0, d], Vec::<f32>::new()))?.into_dyn(),
            h: Tensor::from_array((vec![0, w], Vec::<f32>::new()))?.into_dyn(),
            z: Tensor::from_array((vec![0i64], Vec::<f32>::new()))?.into_dyn(),
        })
    }

    /// 跑一次图，把新节点追加进缓存，返回各候选的逐字平均对数概率。
    fn run(&mut self, cache: Cache, step: StepInputs) -> Result<Vec<f32>, ort::Error> {
        let n = step.nodes.len() as i64;
        let b = step.lengths.len() as i64;
        let w = step.width as i64;
        let mut outputs = self.session.run(ort::inputs![
            "cache_k" => cache.k,
            "cache_v" => cache.v,
            "cache_h" => cache.h,
            "cache_z" => cache.z,
            "nodes" => Tensor::from_array((vec![n], step.nodes))?,
            "positions" => Tensor::from_array((vec![n], step.positions))?,
            "visible" => Tensor::from_array((vec![n, step.seen as i64], step.visible))?,
            "candidates" => Tensor::from_array((vec![b, w], step.candidates))?,
            "steps" => Tensor::from_array((vec![b, w], step.steps))?,
            "lengths" => Tensor::from_array((vec![b], step.lengths))?,
        ])?;
        let scores = outputs["scores"].try_extract_tensor::<f32>()?.1.to_vec();
        let missing = || ort::Error::new("lm_step returned no cache");
        self.cache = Some(Cache {
            k: outputs.remove("next_k").ok_or_else(missing)?,
            v: outputs.remove("next_v").ok_or_else(missing)?,
            h: outputs.remove("next_h").ok_or_else(missing)?,
            z: outputs.remove("next_z").ok_or_else(missing)?,
        });
        Ok(scores)
    }

    fn score(&mut self, context: &str, candidates: &[&str]) -> Result<Vec<f32>, ort::Error> {
        let bodies: Vec<Vec<u32>> = candidates
            .iter()
            .map(|c| self.vocabulary.encode(c))
            .collect();
        let tail: String = {
            let chars: Vec<char> = context.chars().collect();
            chars[chars.len().saturating_sub(self.info.context_chars)..]
                .iter()
                .collect()
        };
        let longest = bodies.iter().map(Vec::len).max().unwrap_or(0);
        let fresh = self.context.as_deref() != Some(tail.as_str())
            || self.cache.is_none()
            || self.prefixes.len() > PREFIX_LIMIT;
        if fresh {
            let mut encoded = vec![self.info.bos];
            encoded.extend(self.vocabulary.encode(&tail));
            if encoded.len() + longest > self.info.max_positions {
                return Ok(Vec::new());
            }
            self.cache = None;
            self.context = None;
            let step = self.prefixes.encode_context(&encoded);
            let empty = self.empty_cache()?;
            self.run(empty, step)?;
            self.context = Some(tail);
        } else if self.prefixes.context_len() + longest > self.info.max_positions {
            return Ok(Vec::new());
        }
        let step = self.prefixes.keystroke(&bodies);
        let cache = self
            .cache
            .take()
            .ok_or_else(|| ort::Error::new("no model cache"))?;
        self.run(cache, step)
    }
}

impl CandidateScorer for OnnxScorer {
    fn context_length(&self) -> usize {
        self.info.context_chars + CANDIDATE_ROOM
    }

    /// 打不了分（候选为空、太长，或图运行失败）时返回空表，`rerank_pick` 见长度不符就不动候选；失败后缓存作废，下一次按键从上文重来。
    fn log_probabilities(&mut self, context: &str, candidates: &[&str]) -> Vec<f32> {
        if candidates.is_empty() || candidates.iter().any(|c| c.is_empty()) {
            return Vec::new();
        }
        match self.score(context, candidates) {
            Ok(scores) => scores,
            Err(_) => {
                self.prefixes.clear();
                self.cache = None;
                self.context = None;
                Vec::new()
            }
        }
    }
}
