//! 注音输入：大千（Dachen）键盘，按 libchewing 的语义；或者注音九键（`nine_key`），数字键拼音节、声调键结束音节。两者都对照 `msime-zhuyin.db`（`language_dictionary`）转换，输出词库里存的繁体，不学习。

pub mod conversion;
pub mod layout;
pub mod nine_key;
pub mod scheme;
pub mod syllable;
