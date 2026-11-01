//! 双拼（schemes-lang.md §1，overlays.md §2）：内置及自定义方案的编码转换、切分、辅助码与拼音查询。纠错整句和触屏短词复用 `typo_edges` 按当前方案生成的邻键、对调变体；没有触点时保持原有整句路径。

pub mod custom;
pub mod dictionary;
pub mod engine;
pub mod hints;
pub mod mid_sentence;
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
