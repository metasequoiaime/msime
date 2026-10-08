//! Scheme-neutral pinyin: the syllable tables, segmentation, the syllable graph, jianpin predicates, fuzzy expansion and autocorrect. Quanpin and shuangpin both build on this (quanpin.md §16), and the host facade's `normalize_full_pinyin` lives here. Nothing in this module touches SQLite.

pub mod active_helpcode;
pub mod autocorrect;
pub mod fuzzy;
pub mod glide;
pub mod graph;
pub mod jianpin;
pub mod normalize;
pub mod segment;
pub mod syllables;
pub mod typos;
