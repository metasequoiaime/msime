//! 日文罗马字输入（schemes-lang.md §5）：带待定尾部的假名转换、假名变体、
//! `msime-japanese.dat`（MSJPDT1）词条与矩阵搜索、provider 及输入方案。
//! 不查询不存在于 `msime-pinyin.db` 的 `japanese_lexicon`；转换规则共用输入法专用扫描。

pub mod decoder;
pub mod matrix;
pub mod provider;
pub mod romaji;
pub mod scheme;

pub use provider::JapaneseProvider;
pub use scheme::JapaneseRomajiScheme;
