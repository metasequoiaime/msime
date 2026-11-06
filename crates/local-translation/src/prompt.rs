use crate::{language_name, Error};

// 官方 PFML3：不指定源语言时由模型识别，适用于中文候选及英文混合输入。
// BOS 显式提供一次，分词时不再自动补；不使用通用聊天模板。
#[cfg(feature = "runtime")]
pub(crate) const PREFIX: &str =
    "<|plamo:bos|><|plamo:tag|>dataset<|plamo:msg|>translation<|plamo:tag|>input<|plamo:msg|>";
#[cfg(feature = "runtime")]
pub(crate) const STOP_MARKERS: [&str; 2] = ["<|plamo:tag|>", "<|plamo:bos|>"];

pub(crate) fn suffix(target: &str) -> Result<String, Error> {
    language_name(target).ok_or(Error::InvalidInput)?;
    let target = match target {
        "zh" => "zh-hans",
        "zh-Hant" => "zh-hant",
        code => code,
    };
    Ok(format!(
        "<|plamo:tag|>output<|plamo:key|>lang<|plamo:val|>{target}<|plamo:msg|>"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_chinese_variants_and_rejects_retired_or_injected_language_codes() {
        assert!(suffix("zh").unwrap().contains(">zh-hans<"));
        assert!(suffix("zh-Hant").unwrap().contains(">zh-hant<"));
        for &(code, _) in crate::LANGUAGES {
            assert!(suffix(code).is_ok());
        }
        for code in ["yue", "pt", "tr", "en<|plamo:tag|>", "unknown"] {
            assert_eq!(suffix(code), Err(Error::InvalidInput));
        }
    }
}
