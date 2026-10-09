//! Double pinyin (schemes-lang.md §1, overlays.md §2): the four profiles and the user's own (`custom`), code-to-syllable conversion, segmentation and helpcode detection, the scheme, and an engine that decodes into quanpin segments and reuses the pinyin cascade, the lattice and quanpin's fuzzy rows. Autocorrect, typo edges, longer phrases and alternative segmentations are quanpin-only.

pub mod custom;
pub mod dictionary;
pub mod engine;
pub mod hints;
pub mod profile;
pub mod query;
pub mod scheme;
#[cfg(test)]
mod tests;
pub mod utils;

pub use engine::ShuangpinEngine;
pub use profile::ShuangpinProfile;
pub use scheme::ShuangpinScheme;
