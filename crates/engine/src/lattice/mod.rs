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

/// Plans the typo edges for one input from its literal best path; quanpin passes `quanpin::typo_edges::collect_typo_edges` bound to its database and caches, shuangpin `shuangpin::typo_edges::collect_shuangpin_typo_edges`.
pub type TypoEdgeSource<'a> = dyn FnMut(&SentencePath) -> Vec<TypoEdge> + 'a;
