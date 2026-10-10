//! 双拼（schemes-lang.md §1，overlays.md §2）：四个内置方案和用户自己的方案（`custom`），编码到音节的转换，切分和辅助码识别，方案本身，以及一个把输入解成全拼片段、复用拼音级联查询、词格和全拼模糊行的引擎。自动纠错、错键边、更长的词组和备选切分只有全拼有。

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
