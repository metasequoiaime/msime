//! 藏文输入：在拉丁键盘上按 EWTS（扩展威利转写）拼写，组字里存的是当前音节串的威利原文，显示和上屏的是 `ewts` crate 转换出的藏文。没有候选，也不学习任何东西。

pub mod scheme;

pub use scheme::TibetanScheme;

/// 音节点（tsheg），空格结束组字时跟在藏文后面。
pub const TSHEG: char = '\u{0F0B}';

/// 垂符（shad），`/` 结束组字时跟在藏文后面；没有组字时 `/` 单独输出它。
pub const SHAD: char = '\u{0F0D}';
