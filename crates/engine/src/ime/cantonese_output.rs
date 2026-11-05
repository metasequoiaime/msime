//! 粤语输出转换只改显示和上屏；学习仍使用词库原文，避免有损转换破坏调频和造词。

use std::collections::HashMap;

use crate::cantonese::CantoneseCandidate;
use crate::types::{SharedString, TextConversion};

#[derive(Default)]
pub(super) struct CantoneseOutputOrigins {
    words: HashMap<String, HashMap<String, SharedString>>,
}

impl CantoneseOutputOrigins {
    pub fn clear(&mut self) {
        self.words.clear();
    }

    pub fn convert(&mut self, candidates: &mut [CantoneseCandidate], conversion: TextConversion) {
        for candidate in candidates {
            let displayed = conversion.apply(&candidate.text);
            let original = std::mem::replace(&mut candidate.text, displayed);
            // 转换去重保留先到的行；原文映射必须选择同一行，不能反向转换猜字。
            self.words
                .entry(candidate.key.clone())
                .or_default()
                .entry(candidate.text.clone())
                .or_insert_with(|| original.into());
        }
    }

    pub fn word(&self, key: &str, displayed: &SharedString) -> SharedString {
        self.words
            .get(key)
            .and_then(|words| words.get(displayed.as_str()))
            .unwrap_or(displayed)
            .clone()
    }
}
