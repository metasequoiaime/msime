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
///
/// 建环境放在 `get_or_init` 里：先查后建的写法会让同时加载模型的两个线程各建一个环境，Linux x86_64 上并行跑测试时模型因此加载失败过。建失败也记下来，不反复重试。
fn environment() -> Result<&'static ort::environment::Environment, ort::Error> {
    static ENVIRONMENT: OnceLock<Result<ort::environment::Environment, String>> = OnceLock::new();
    ENVIRONMENT
        .get_or_init(|| {
            ort::init()
                .with_name("msime-rerank")
                .with_telemetry(false)
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|message| ort::Error::new(message.clone()))
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
    /// [`Self::fits_keystroke_budget`] 量过、且在预算之内；[`CandidateScorer::scores_prediction_pool_within_keystroke_budget`] 报的就是它。
    within_keystroke_budget: bool,
}

/// 建 ONNX Runtime 会话：`threads` 是一次图运行用几个线程；`memory_arena` 为假时关掉 CPU 内存池，每次运行完把张量的内存还给系统。
///
/// 内存池按一次运行用到的最大量留着内存、只增不减，上文和候选前缀的缓存每次按键长度不同，它攒下的比真正同时在用的多得多。2026-11-02 在 iOS 模拟器里用 `footprint` 示例量 phys_footprint（比加载前多出的部分）：8x448 模型开着内存池打字后是 80 MiB，关掉是 46 MiB；6x256 是 28 与 11 MiB。代价是每次运行重新向系统要内存，同一负载在 M 系列 Mac 的模拟器里每键 p50 从 1.9 到 2.7 ms、p95 从 9.3 到 11.8 ms，自测从 4.0 到 4.5 ms。
fn session_builder(
    threads: usize,
    memory_arena: bool,
) -> Result<ort::session::builder::SessionBuilder, ort::Error> {
    Session::builder(environment()?)?
        .with_intra_threads(threads.max(1))
        .map_err(ort::Error::<()>::from)?
        .with_execution_providers([ort::ep::CPU::default()
            .with_arena_allocator(memory_arena)
            .build()])
        .map_err(ort::Error::<()>::from)
}

/// 读 `model.json` 与 `vocab.json`。在建会话之前读，元数据坏了就不去加载模型。
fn metadata(model_json: &str, vocab_json: &str) -> Result<(ModelInfo, Vocabulary), LoadError> {
    let info: ModelInfo = serde_json::from_str(model_json)?;
    #[derive(serde::Deserialize)]
    struct Tokens {
        tokens: Vec<String>,
    }
    let tokens: Tokens = serde_json::from_str(vocab_json)?;
    let vocabulary = Vocabulary::new(&tokens.tokens, info.unk);
    Ok((info, vocabulary))
}

impl OnnxScorer {
    /// `model` 是 ORT 格式的 `lm_step.ort` 的内容，`model_json` 与 `vocab_json` 是同一次导出的 `model.json` 和 `vocab.json`；`threads` 与 `memory_arena` 见 `session_builder`。
    pub fn load(
        model: &[u8],
        model_json: &str,
        vocab_json: &str,
        threads: usize,
        memory_arena: bool,
    ) -> Result<Self, LoadError> {
        let (info, vocabulary) = metadata(model_json, vocab_json)?;
        let session = session_builder(threads, memory_arena)?.commit_from_memory(model)?;
        Ok(Self::with_session(session, info, vocabulary))
    }

    /// 同 [`OnnxScorer::load`]，但让 ONNX Runtime 自己读 `model` 这个文件。宿主用这一个：先把文件读进内存再交给运行时，加载的那一刻文件内容多一份，8x448 模型加载时的内存峰值因此高出约 30 MiB（iOS 模拟器里比加载前多 104 MiB，自己读是 74 MiB），而 iOS 键盘扩展有硬上限。
    pub fn load_file(
        model: &std::path::Path,
        model_json: &str,
        vocab_json: &str,
        threads: usize,
        memory_arena: bool,
    ) -> Result<Self, LoadError> {
        let (info, vocabulary) = metadata(model_json, vocab_json)?;
        let session = session_builder(threads, memory_arena)?.commit_from_file(model)?;
        Ok(Self::with_session(session, info, vocabulary))
    }

    fn with_session(session: Session, info: ModelInfo, vocabulary: Vocabulary) -> Self {
        Self {
            session,
            vocabulary,
            info,
            prefixes: PrefixCache::default(),
            cache: None,
            context: None,
            within_keystroke_budget: false,
        }
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

impl OnnxScorer {
    /// 这台机器上这个模型打一页重负载要多久（毫秒）：32 字的上文之后，9 个 6 字的候选只共享第一个字，一次要新算 37 个前缀，相当于长句打字时尾部的那种按键。先预热一次，再取 3 次的中位数；每次换一组候选，不让缓存帮忙。测完清空缓存。
    ///
    /// 宿主拿它在大小两个模型之间选：桌面和新手机上大模型这一页只要几毫秒，老手机上要二三十毫秒，按键预算挡不住。
    pub fn calibrate(&mut self) -> f64 {
        self.calibrated()
    }

    /// 用 [`Self::calibrate`] 量这台机器上打一页重负载要多久，不超过 `budget_ms` 时返回 `true`，并记下结论：之后 [`CandidateScorer::scores_prediction_pool_within_keystroke_budget`] 报的就是它，上屏后联想因此只交给过了预算的模型排。没量过（只装了一个模型、宿主没自测）或量了超预算的模型都不排。
    ///
    /// 宿主在大小两档之间选时调它（host-api 的 `onnx_reranker_within`）：过了预算的大模型（Mac、iPhone 17 上的 8x448）同时也排联想，Mac 上一次上屏多花 p95 约 5 ms；没过的机器退到 6x256，不排，Pixel 3 XL 上 6x256 排一池要多花 p95 13–14 ms，超过逐键预算。
    pub fn fits_keystroke_budget(&mut self, budget_ms: f64) -> bool {
        self.calibrate_for_keystroke_budget(budget_ms) <= budget_ms
    }

    /// 与 [`Self::fits_keystroke_budget`] 相同地量一次并记下结论，返回量出来的毫秒数而不是结论：宿主要把自测值写进诊断行（host-api 的 `OnnxRerankers::attach`），不能为此再量一遍。
    pub fn calibrate_for_keystroke_budget(&mut self, budget_ms: f64) -> f64 {
        let measured = self.calibrated();
        self.accept_calibration(measured, budget_ms);
        measured
    }

    /// 记下同一个模型文件在这个进程里之前量出的 `measured_ms`，结论与刚量过一样，不再量：宿主重新加载同一个模型时用它（host-api 的 `OnnxRerankers` 每个进程只自测一次）。
    pub fn accept_calibration(&mut self, measured_ms: f64, budget_ms: f64) {
        self.within_keystroke_budget = measured_ms <= budget_ms;
    }

    fn calibrated(&mut self) -> f64 {
        let ids: Vec<char> = self.vocabulary.characters().take(64).collect();
        if ids.len() < 64 {
            return f64::INFINITY;
        }
        let context: String = ids[..32].iter().collect();
        let mut times = Vec::new();
        for run in 0..4 {
            let page: Vec<String> = (0..9)
                .map(|c| {
                    let head = ids[32 + run];
                    let rest = (0..5).map(|t| ids[36 + (c * 3 + t + run * 7) % 28]);
                    std::iter::once(head).chain(rest).collect()
                })
                .collect();
            let texts: Vec<&str> = page.iter().map(String::as_str).collect();
            // 上文先编码好，只量按键这一步。
            self.log_probabilities(&context, &texts[..1]);
            let started = std::time::Instant::now();
            let scored = self.log_probabilities(&context, &texts);
            let elapsed = started.elapsed().as_secs_f64() * 1000.0;
            if scored.len() != texts.len() {
                return f64::INFINITY;
            }
            if run > 0 {
                times.push(elapsed);
            }
            self.prefixes.clear();
            self.cache = None;
            self.context = None;
        }
        times.sort_by(f64::total_cmp);
        times[times.len() / 2]
    }
}

impl CandidateScorer for OnnxScorer {
    fn context_length(&self) -> usize {
        self.info.context_chars + CANDIDATE_ROOM
    }

    /// 只有 [`OnnxScorer::fits_keystroke_budget`] 量过且在预算之内时报 `true`。
    fn scores_prediction_pool_within_keystroke_budget(&self) -> bool {
        self.within_keystroke_budget
    }

    /// 大小两档都要引擎解满一次比较的条数：多出的三条在 Pixel 3 XL（6x256，4 线程）上让整键 p95 从 9.0 ms 到 9.8 ms，仍在 10 ms 的逐键预算之内；桌面 8x448 的整键 p99 从 3.1 ms 到 3.6 ms。见 `.agents/notes/implemented/feature/2026-11-02-sentence-pool-for-reranker.md`。
    fn sentence_pool(&self) -> usize {
        msime_engine::ordering::SENTENCE_POOL
    }

    /// 8x448 和 6x256 比随包模型准得多，混合里词网格分只占三成。2026-11-02 在 dev（ctx-v1 分句与选词、typing-chat-v1）上从 0.2 扫到 0.5 定的，见 `.agents/notes/implemented/feature/2026-11-02-onnx-static-evidence-weight.md`。
    fn static_evidence_weight(&self) -> f64 {
        0.3
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
