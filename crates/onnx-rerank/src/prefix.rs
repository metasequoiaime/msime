//! 跨按键的前缀簿记：哪些前缀已经在模型缓存里、这次按键要新算哪些、每个候选字由哪个前缀预测。
//!
//! 缓存的排列是先上文、后候选前缀：上文是一条链（BOS 和上屏的字，每个字看见它之前的字），候选前缀挂在上文最后一个字后面，各自看见整段上文和自己的前缀。一个候选的第一个字由上文最后一个字预测，第 t 个字由它长 t 的前缀预测；所以长 t 的前缀只有在某个候选比 t 长时才需要算，候选本身（最长的那个前缀）从来不算。与 chinese-ime-lm `neural/choice/model.py` 的 `context_inputs` 和 `step_inputs` 逐项对应。

use std::collections::HashMap;

/// 一次图运行的输入，都是行优先展平的数组。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StepInputs {
    /// 这次要算的节点（字编号），`[N]`。
    pub nodes: Vec<i64>,
    /// 节点的位置，`[N]`。
    pub positions: Vec<i64>,
    /// 节点 i 看不看得见第 j 个节点（缓存里的在前，这次的在后），`[N, M + N]`。
    pub visible: Vec<bool>,
    /// `M + N`：`visible` 的列数。
    pub seen: usize,
    /// 候选的字编号，补零到最长的候选，`[B, W]`。
    pub candidates: Vec<i64>,
    /// 预测每个候选字的节点下标（指向 `M + N` 个节点），`[B, W]`。
    pub steps: Vec<i64>,
    /// 每个候选的字数，`[B]`。
    pub lengths: Vec<i64>,
    /// `W`：最长候选的字数。
    pub width: usize,
}

/// 上一次上屏以来，模型缓存里有哪些节点。
#[derive(Clone, Debug, Default)]
pub struct PrefixCache {
    /// 上文节点的个数（含 BOS）；0 表示还没有编码上文。
    context: usize,
    /// 缓存里的候选前缀，按缓存顺序。
    prefixes: Vec<Vec<u32>>,
    /// 候选前缀在缓存里的下标（已经加上了上文节点数）。
    index: HashMap<Vec<u32>, usize>,
}

impl PrefixCache {
    /// 缓存里一共有多少个节点。
    pub fn len(&self) -> usize {
        self.context + self.prefixes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 上文节点的个数（含 BOS）。
    pub fn context_len(&self) -> usize {
        self.context
    }

    /// 编码一段新的上文：清空缓存，返回把 `context`（以 BOS 开头的字编号）作为一条链跑进空缓存的输入。图要求至少一个候选，这里放一个占位候选，它的分数不用。
    pub fn encode_context(&mut self, context: &[u32]) -> StepInputs {
        assert!(!context.is_empty(), "the context starts with BOS");
        let n = context.len();
        self.context = n;
        self.prefixes.clear();
        self.index.clear();
        let mut visible = Vec::with_capacity(n * n);
        for i in 0..n {
            visible.extend((0..n).map(|j| j <= i));
        }
        StepInputs {
            nodes: context.iter().map(|&c| i64::from(c)).collect(),
            positions: (0..n as i64).collect(),
            visible,
            seen: n,
            candidates: vec![i64::from(context[0])],
            steps: vec![0],
            lengths: vec![1],
            width: 1,
        }
    }

    /// 一次按键：给 `bodies`（各候选的字编号，都不为空）打分要新算的节点，以及每个候选字由哪个节点预测。新算的前缀记进缓存，下一次按键复用。
    ///
    /// 返回的输入要在 [`PrefixCache::encode_context`] 之后、且图确实把这次的节点追加进了缓存时才成立；图运行失败时调用方应当 [`PrefixCache::clear`]。
    pub fn keystroke(&mut self, bodies: &[Vec<u32>]) -> StepInputs {
        assert!(self.context > 0, "encode the context first");
        assert!(bodies.iter().all(|b| !b.is_empty()), "empty candidate");
        let before = self.prefixes.len();
        for body in bodies {
            for t in 1..body.len() {
                if !self.index.contains_key(&body[..t]) {
                    let prefix = body[..t].to_vec();
                    self.index
                        .insert(prefix.clone(), self.context + self.prefixes.len());
                    self.prefixes.push(prefix);
                }
            }
        }
        let fresh = &self.prefixes[before..];
        let seen = self.len();
        let mut visible = Vec::with_capacity(fresh.len() * seen);
        for p in fresh {
            visible.extend(std::iter::repeat_n(true, self.context));
            visible.extend(
                self.prefixes
                    .iter()
                    .map(|q| q.len() <= p.len() && p[..q.len()] == q[..]),
            );
        }
        let width = bodies.iter().map(Vec::len).max().unwrap_or(0);
        let mut candidates = vec![0; bodies.len() * width];
        let mut steps = vec![0; bodies.len() * width];
        for (i, body) in bodies.iter().enumerate() {
            for (t, &c) in body.iter().enumerate() {
                candidates[i * width + t] = i64::from(c);
                steps[i * width + t] = if t == 0 {
                    (self.context - 1) as i64
                } else {
                    self.index[&body[..t]] as i64
                };
            }
        }
        StepInputs {
            nodes: fresh.iter().map(|p| i64::from(p[p.len() - 1])).collect(),
            positions: fresh
                .iter()
                .map(|p| (self.context + p.len() - 1) as i64)
                .collect(),
            visible,
            seen,
            candidates,
            steps,
            lengths: bodies.iter().map(|b| b.len() as i64).collect(),
            width,
        }
    }

    /// 丢掉一切，下一次按键要先重新编码上文。
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_context_is_a_chain() {
        let mut cache = PrefixCache::default();
        let step = cache.encode_context(&[2, 10, 11]);
        assert_eq!(step.nodes, [2, 10, 11]);
        assert_eq!(step.positions, [0, 1, 2]);
        #[rustfmt::skip]
        assert_eq!(step.visible, [
            true, false, false,
            true, true, false,
            true, true, true,
        ]);
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn shared_prefixes_run_once_and_last_characters_never() {
        let mut cache = PrefixCache::default();
        cache.encode_context(&[2, 10]);
        // 甲乙丙 / 甲乙丁 / 甲戊：要算的前缀只有「甲」「甲乙」。
        let step = cache.keystroke(&[vec![20, 21, 22], vec![20, 21, 23], vec![20, 24]]);
        assert_eq!(step.nodes, [20, 21]);
        assert_eq!(step.positions, [2, 3]);
        assert_eq!(step.seen, 4);
        #[rustfmt::skip]
        assert_eq!(step.visible, [
            true, true, true, false,
            true, true, true, true,
        ]);
        assert_eq!(step.width, 3);
        assert_eq!(step.candidates, [20, 21, 22, 20, 21, 23, 20, 24, 0]);
        // 第一个字由上文末字（下标 1）预测，第二个字由「甲」（2），第三个字由「甲乙」（3）。
        assert_eq!(step.steps, [1, 2, 3, 1, 2, 3, 1, 2, 0]);
        assert_eq!(step.lengths, [3, 3, 2]);
    }

    #[test]
    fn the_next_keystroke_runs_only_new_prefixes() {
        let mut cache = PrefixCache::default();
        cache.encode_context(&[2]);
        cache.keystroke(&[vec![20, 21], vec![30, 31]]);
        let step = cache.keystroke(&[vec![20, 21, 22], vec![30, 32, 33]]);
        // 「甲」「丙」上一次已算；这次新算「甲乙」「丙丁」。
        assert_eq!(step.nodes, [21, 32]);
        assert_eq!(step.positions, [2, 2]);
        assert_eq!(step.seen, 5);
        #[rustfmt::skip]
        assert_eq!(step.visible, [
            true, true, false, true, false,
            true, false, true, false, true,
        ]);
        assert_eq!(step.steps, [0, 1, 3, 0, 2, 4]);
    }

    #[test]
    fn single_character_pages_need_no_new_node() {
        let mut cache = PrefixCache::default();
        cache.encode_context(&[2]);
        let step = cache.keystroke(&[vec![20], vec![21]]);
        assert!(step.nodes.is_empty());
        assert!(step.visible.is_empty());
        assert_eq!(step.seen, 1);
        assert_eq!(step.steps, [0, 0]);
    }

    #[test]
    fn a_new_context_starts_over() {
        let mut cache = PrefixCache::default();
        cache.encode_context(&[2, 10]);
        cache.keystroke(&[vec![20, 21]]);
        cache.encode_context(&[2, 11, 12]);
        let step = cache.keystroke(&[vec![20, 21]]);
        assert_eq!(step.nodes, [20]);
        assert_eq!(step.positions, [3]);
    }
}
