//! 候选重排的原生模型后端：chinese-ime-lm 的单图按键模型（`lm_step`）跑在 ONNX Runtime 上，实现 [`msime_engine::ordering::CandidateScorer`]。
//!
//! 模型把一次上屏后的上文编码进缓存，之后每次按键只算缓存里还没有的候选前缀：候选共享的前缀只算一次，候选的最后一个字不用算（没有谁读它的输出），上一次按键算过的前缀直接复用。这份簿记在 [`prefix`] 里，不依赖原生库，单独测试；跑模型的部分在 `runtime` 特性后面，要链接按模型算子清单精简构建的 ONNX Runtime。
//!
//! 引擎本身保持纯 Rust：这个 crate 在引擎之外，由宿主挂到 `Runtime` 上，见 `.agents/notes/implemented/architecture/2026-11-01-candidate-scorer-boundary.md`。

pub mod prefix;

#[cfg(feature = "runtime")]
mod scorer;
#[cfg(feature = "runtime")]
pub use scorer::{LoadError, OnnxScorer};

use serde::Deserialize;

/// 模型文件旁的 `model.json`：图本身说不出、运行时又要知道的事实，由 chinese-ime-lm 的 `neural/choice/export_step.py` 写出。
#[derive(Clone, Debug, Deserialize)]
pub struct ModelInfo {
    pub layers: usize,
    pub heads: usize,
    pub head_dim: usize,
    pub width: usize,
    /// 位置表的长度：上文（含开头的 BOS）加最长的候选不能超过它。
    pub max_positions: usize,
    /// 训练时看的上文字数，更长的上文只取最后这么多字。
    pub context_chars: usize,
    pub bos: u32,
    pub unk: u32,
}

/// 字到编号：字表里单个字的条目，编号是它在表里的位置；不在表里的字是 `unk`。
#[derive(Clone, Debug)]
pub struct Vocabulary {
    ids: std::collections::HashMap<char, u32>,
    unk: u32,
}

impl Vocabulary {
    /// `tokens` 是 `vocab.json` 的 `tokens` 列表。
    pub fn new(tokens: &[String], unk: u32) -> Self {
        let ids = tokens
            .iter()
            .enumerate()
            .filter_map(|(id, token)| {
                let mut chars = token.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => Some((c, id as u32)),
                    _ => None,
                }
            })
            .collect();
        Self { ids, unk }
    }

    /// 字表里的字，按编号从小到大。
    pub fn characters(&self) -> impl Iterator<Item = char> + '_ {
        let mut chars: Vec<(u32, char)> = self.ids.iter().map(|(&c, &id)| (id, c)).collect();
        chars.sort_unstable();
        chars.into_iter().map(|(_, c)| c)
    }

    pub fn encode(&self, text: &str) -> Vec<u32> {
        text.chars()
            .map(|c| self.ids.get(&c).copied().unwrap_or(self.unk))
            .collect()
    }
}
