//! Double pinyin (schemes-lang.md §1, overlays.md §2): the four profiles, code-to-syllable conversion, segmentation and helpcode detection, the scheme, and an engine that decodes into quanpin segments and reuses the pinyin cascade, the lattice and quanpin's fuzzy rows. 纠错只有纠错整句一条路（`typo_edges`，按当前方案的两键编码生成邻键和对调变体）；按非法拼写纠错、更长词组和备选切分仍只属于全拼。

pub mod dictionary;
pub mod engine;
pub mod hints;
pub mod profile;
pub mod query;
pub mod scheme;
#[cfg(test)]
mod tests;
pub mod typo_edges;
pub mod utils;

pub use engine::ShuangpinEngine;
pub use profile::ShuangpinProfile;
pub use scheme::ShuangpinScheme;
