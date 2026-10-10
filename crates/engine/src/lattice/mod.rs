//! The word lattice, the only whole-sentence source now that the Google decoder is gone (quanpin.md §10, §12, overlays.md §1.2, §1.6): graph, beam search, trigram rescoring, the typo sentence, the merge into the candidate list, the n-gram tables, the personal n-gram model and neural reranking of the lattice's n-best. Quanpin and shuangpin share it through `make_sentence_lattice_options`; the lookup is a closure so tests can drive it without SQLite.

mod cxx_sort;
pub mod decode;
pub mod merge;
pub mod neural;
pub mod ngram;
pub mod personal;

pub use decode::{LatticeLexeme, SentencePath, TypoEdge};

/// Span lookup: the rows whose key is exactly the given syllables.
pub type LatticeLookup<'a> = dyn FnMut(&[String]) -> Vec<LatticeLexeme> + 'a;

/// 按一次输入的字面最优路径规划纠错边。全拼传入绑定了自己词库和缓存的 `quanpin::typo_edges::collect_typo_edges`，双拼传入 `shuangpin::typo_edges::collect_shuangpin_typo_edges`。
pub type TypoEdgeSource<'a> = dyn FnMut(&SentencePath) -> Vec<TypoEdge> + 'a;
