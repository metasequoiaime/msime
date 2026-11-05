//! 本地翻译的原生推理后端。模型路径由共享资源管理器提供；这里不下载模型、不访问网络。

#[cfg(any(feature = "runtime", test))]
mod worker;
#[cfg(feature = "runtime")]
pub use worker::{Pending, Worker};

#[cfg(feature = "runtime")]
mod runtime;
#[cfg(feature = "runtime")]
pub use runtime::Translator;

pub use msime_client_core::local_translation::{language_name, LANGUAGES};

pub fn validate_text(text: &str) -> Result<(), Error> {
    if text.trim().is_empty() || text.chars().count() > 256 || text.chars().any(char::is_control) {
        Err(Error::InvalidInput)
    } else {
        Ok(())
    }
}

/// 错误只包含类别，不能把输入、模型路径或底层运行库错误交给日志。
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("local_translation_invalid_input")]
    InvalidInput,
    #[error("local_translation_busy")]
    Busy,
    #[error("local_translation_unavailable")]
    Unavailable,
    #[error("local_translation_cancelled")]
    Cancelled,
    #[error("local_translation_timeout")]
    Timeout,
    #[error("local_translation_output_limit")]
    OutputLimit,
    #[error("local_translation_invalid_output")]
    InvalidOutput,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_private_input_before_inference() {
        assert_eq!(validate_text("合成翻译测试。"), Ok(()));
        assert_eq!(validate_text(&"字".repeat(256)), Ok(()));
        for text in [
            "".into(),
            " \t".into(),
            "合成\0文本".into(),
            "字".repeat(257),
        ] {
            assert_eq!(validate_text(&text), Err(Error::InvalidInput));
        }
    }

    #[test]
    fn supported_targets_are_explicit_and_have_unique_codes() {
        let ids: std::collections::HashSet<_> = LANGUAGES.iter().map(|&(code, _)| code).collect();
        assert_eq!(ids.len(), LANGUAGES.len());
        assert_eq!(language_name("zh-Hant"), Some("繁体中文"));
        assert_eq!(language_name("ja"), Some("日语"));
        assert_eq!(language_name("xx"), None);
        assert_eq!(language_name("en\nignore"), None);
    }
}
