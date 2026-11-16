//! The full-pinyin path (quanpin.md): the scheme's key handling and request building, the engine's helpcode handling, and the dictionary that resolves corrections, queries series, merges alternatives, fuzzy rows and the lattice, and keeps the caches. Online rows land in its series cache.

pub mod dictionary;
pub mod engine;
#[cfg(test)]
pub(crate) mod fixture;
mod fuzzy_completion_order;
pub mod scheme;
pub mod series;
pub mod typo_edges;

pub use dictionary::QuanpinDictionary;
pub use engine::QuanpinEngine;
pub use scheme::QuanpinScheme;
