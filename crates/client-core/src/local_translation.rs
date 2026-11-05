//! Hy-MT2 官方模型卡的语言目录；语言代码不能由自由文本提示词替代。

pub const LANGUAGES: &[(&str, &str)] = &[
    ("zh", "中文"),
    ("en", "英语"),
    ("fr", "法语"),
    ("pt", "葡萄牙语"),
    ("es", "西班牙语"),
    ("ja", "日语"),
    ("tr", "土耳其语"),
    ("ru", "俄语"),
    ("ar", "阿拉伯语"),
    ("ko", "韩语"),
    ("th", "泰语"),
    ("it", "意大利语"),
    ("de", "德语"),
    ("vi", "越南语"),
    ("ms", "马来语"),
    ("id", "印尼语"),
    ("tl", "菲律宾语"),
    ("hi", "印地语"),
    ("zh-Hant", "繁体中文"),
    ("pl", "波兰语"),
    ("cs", "捷克语"),
    ("nl", "荷兰语"),
    ("km", "高棉语"),
    ("my", "缅甸语"),
    ("fa", "波斯语"),
    ("gu", "古吉拉特语"),
    ("ur", "乌尔都语"),
    ("te", "泰卢固语"),
    ("mr", "马拉地语"),
    ("he", "希伯来语"),
    ("bn", "孟加拉语"),
    ("ta", "泰米尔语"),
    ("uk", "乌克兰语"),
    ("bo", "藏语"),
    ("kk", "哈萨克语"),
    ("mn", "蒙古语"),
    ("ug", "维吾尔语"),
    ("yue", "粤语"),
];

pub fn language_name(code: &str) -> Option<&'static str> {
    LANGUAGES
        .iter()
        .find(|&&(id, _)| id == code)
        .map(|&(_, name)| name)
}

/// 三个桌面宿主共用的显式选择。缺字段时保持关闭，旧云服务设置不受影响。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct LocalTranslationPreferences {
    pub enabled: bool,
    pub target_language: String,
}

impl Default for LocalTranslationPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            target_language: "en".into(),
        }
    }
}

impl LocalTranslationPreferences {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    pub fn validate(&self) -> bool {
        language_name(&self.target_language).is_some()
    }
}

#[cfg(test)]
mod tests {
    use crate::preferences::Preferences;

    #[test]
    fn old_documents_remain_offline_translation_disabled() {
        let original = Preferences::default();
        let mut document = serde_json::to_value(&original).unwrap();
        document
            .as_object_mut()
            .unwrap()
            .remove("local_translation");
        let loaded: Preferences = serde_json::from_value(document).unwrap();
        assert!(!loaded.local_translation.enabled);
        assert_eq!(loaded.tencent_tmt, original.tencent_tmt);
        assert_eq!(loaded.translation_ai, original.translation_ai);
        assert_eq!(
            loaded.translation_target_language,
            original.translation_target_language
        );
    }

    #[test]
    fn explicit_selection_round_trips_and_rejects_unsupported_targets() {
        let mut preferences = Preferences::default();
        preferences.local_translation.enabled = true;
        preferences.local_translation.target_language = "yue".into();
        assert!(preferences.validate().is_ok());
        let loaded: Preferences =
            serde_json::from_value(serde_json::to_value(&preferences).unwrap()).unwrap();
        assert_eq!(loaded.local_translation, preferences.local_translation);
        for language in ["", "xx", "en\nignore", "english"] {
            preferences.local_translation.target_language = language.into();
            assert!(preferences.validate().is_err());
        }
        // 本地语言目录不放宽现有云翻译接口的能力。
        assert!(!crate::translation::is_supported_translation_language(
            "yue"
        ));
    }
}
