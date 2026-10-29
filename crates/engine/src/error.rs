//! The crate's one error type. Display strings are part of the contract: hosts surface them, and the host tests and goldens compare the exact C++ wording, so constructors take the message from `diagnostics` rather than composing new text.

pub type Result<T, E = EngineError> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// A caller-supplied value was rejected (the C++ `std::invalid_argument`).
    #[error("{0}")]
    InvalidArgument(String),
    /// An operation could not complete (the C++ `std::runtime_error`).
    #[error("{0}")]
    Failed(String),
    /// 别的进程或本进程的另一处正占着这份数据，操作什么也没做，稍后重试即可。目前只有词库维护：拿不到 `DictionaryAccess` 的独占租约（还有会话或另一次维护开着）。
    #[error("{0}")]
    Busy(String),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// [`EngineError`] 的稳定数字码，供宿主和 C ABI 按数字分支，不必解析说明文字。编号与引擎库 C ABI 草案的返回码共用一个空间：0 是成功，4（不支持）、5（revision 不符）、7（会话已毒化）、8（空指针）留给 C ABI 自己的情形，引擎不产生它们。已发布的编号只追加、不改、不复用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ErrorCode {
    InvalidArgument = 1,
    Failed = 2,
    /// SQLite 或文件系统出错（[`EngineError::Sqlite`]、[`EngineError::Io`]）。
    Storage = 3,
    Busy = 6,
}

impl ErrorCode {
    pub const fn value(self) -> u32 {
        self as u32
    }
}

impl EngineError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidArgument(message.into())
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed(message.into())
    }

    pub fn busy(message: impl Into<String>) -> Self {
        Self::Busy(message.into())
    }

    /// 这条错误的稳定数字码；说明文字仍是 `Display`。
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidArgument(_) => ErrorCode::InvalidArgument,
            Self::Failed(_) => ErrorCode::Failed,
            Self::Busy(_) => ErrorCode::Busy,
            Self::Sqlite(_) | Self::Io(_) => ErrorCode::Storage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 编号一经发布就冻结：这张表改了，C ABI 的调用方会按错的数字分支。
    #[test]
    fn error_codes_are_frozen() {
        let cases = [
            (EngineError::invalid("x"), 1),
            (EngineError::failed("x"), 2),
            (EngineError::Sqlite(rusqlite::Error::InvalidQuery), 3),
            (EngineError::Io(std::io::Error::other("x")), 3),
            (EngineError::busy("x"), 6),
        ];
        for (error, code) in cases {
            assert_eq!(error.code().value(), code, "{error:?}");
        }
    }

    #[test]
    fn codes_keep_the_message() {
        let error = EngineError::busy(crate::diagnostics::DICTIONARY_MAINTENANCE_BUSY);
        assert_eq!(error.to_string(), "Dictionary maintenance busy");
        assert_eq!(error.code(), ErrorCode::Busy);
    }
}
