//! 语音识别语言的稳定标识和各宿主展示值；不读取偏好、凭据或网络。

use serde::Serialize;

/// 一种语言在云端识别、系统识别和移动端选择器中的公开投影。
#[derive(Debug, Serialize)]
pub struct VoiceLanguagePreset {
    pub id: &'static str,
    pub provider_value: &'static str,
    pub provider_title: &'static str,
    pub system_value: &'static str,
    pub system_title: &'static str,
    pub mobile_value: &'static str,
    pub mobile_title: &'static str,
}

/// 顺序也是设置页和选择器的稳定顺序。没有对应系统语言的条目保持空值，宿主不会显示它。
pub const LANGUAGES: &[VoiceLanguagePreset] = &[
    VoiceLanguagePreset {
        id: "zh-cn",
        provider_value: "zh-cn",
        provider_title: "中文（普通话）",
        system_value: "zh-CN",
        system_title: "中文（普通话）",
        mobile_value: "zh-cn",
        mobile_title: "普通话",
    },
    VoiceLanguagePreset {
        id: "yue",
        provider_value: "",
        provider_title: "",
        system_value: "",
        system_title: "",
        mobile_value: "yue",
        mobile_title: "粤语",
    },
    VoiceLanguagePreset {
        id: "en",
        provider_value: "en",
        provider_title: "English",
        system_value: "en-US",
        system_title: "English",
        mobile_value: "en",
        mobile_title: "英语",
    },
    VoiceLanguagePreset {
        id: "ja",
        provider_value: "ja",
        provider_title: "日本語",
        system_value: "ja-JP",
        system_title: "日本語",
        mobile_value: "",
        mobile_title: "日语",
    },
    VoiceLanguagePreset {
        id: "auto",
        provider_value: "auto",
        provider_title: "自动识别",
        system_value: "",
        system_title: "",
        mobile_value: "auto",
        mobile_title: "普通话 + 英语",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_catalog_has_stable_ids_and_nonempty_projections() {
        assert_eq!(
            LANGUAGES.iter().map(|item| item.id).collect::<Vec<_>>(),
            ["zh-cn", "yue", "en", "ja", "auto"]
        );
        assert!(LANGUAGES.iter().all(|item| !item.mobile_title.is_empty()));
        assert_eq!(
            LANGUAGES
                .iter()
                .filter(|item| !item.mobile_value.is_empty())
                .count(),
            4
        );
        assert_eq!(
            LANGUAGES
                .iter()
                .filter(|item| !item.provider_value.is_empty())
                .count(),
            4
        );
        assert_eq!(
            LANGUAGES
                .iter()
                .filter(|item| !item.system_value.is_empty())
                .count(),
            3
        );
    }
}
