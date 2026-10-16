//! 客户端各 crate 共用的底层：私有文件的存储与加锁、有上限的读取、文本校验谓词、日历日期运算和 OpenCC 简繁转换。
//!
//! 这些模块不引用客户端里的任何业务模块，所以单独成一个 crate，不联网、不带 reqwest 也能链接（例如 msime-backend-engine 只用简繁转换）。`msime-client-core` 在自己的根上把它们原样接回，宿主照旧写 `msime_client_core::file_lock` 这类路径。

#[doc(hidden)]
pub mod bounded_io;
pub mod calendar;
pub mod chinese_conversion;
pub mod file_lock;
#[doc(hidden)]
pub mod storage;
pub mod text;
