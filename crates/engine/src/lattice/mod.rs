//! 词网格是移除 Google 解码器后的整句来源（quanpin.md §10、§12，overlays.md §1.2、§1.6）：包括图构造、束搜索、三元重排、纠错整句、候选合并、n 元表、个人模型及神经重排。全拼和双拼通过 `SentenceNgramTables` 共用已加载的表和整句配置；查词用闭包注入，测试无需依赖 SQLite。

mod cxx_sort;
pub mod decode;
pub mod merge;
pub mod neural;
pub mod ngram;
pub mod octagram;

#[cfg(test)]
pub(crate) mod active_tables_tests;
pub mod personal;

pub use decode::{LatticeLexeme, SentencePath, TypoEdge};

/// Span lookup: the rows whose key is exactly the given syllables.
pub type LatticeLookup<'a> = dyn FnMut(&[String]) -> Vec<LatticeLexeme> + 'a;

/// 按一次输入的字面最优路径规划纠错边。全拼传入绑定了自己词库和缓存的 `quanpin::typo_edges::collect_typo_edges`，双拼传入 `shuangpin::typo_edges::collect_shuangpin_typo_edges`。
pub type TypoEdgeSource<'a> = dyn FnMut(&SentencePath) -> Vec<TypoEdge> + 'a;
