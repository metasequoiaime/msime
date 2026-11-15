//! WebDAV 设置同步的无网络文档投影。
//!
//! 这里只负责把已经通过 `Preferences` 校验的本机偏好分成可同步的共享设置、主题
//! 和本机设置。网络、设备批准、冲突合并和宿主触发留在后续切片；输入法宿主不能
//! 直接把完整偏好文件交给 WebDAV。

use crate::preferences::Preferences;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// WebDAV 设置域的文档格式名，与设置文件和其他同步域分开。
pub const FORMAT: &str = "app.msime.settings-sync";
/// 当前文档版本。
pub const VERSION: u64 = 1;
/// 单个同步文档的上限；图片主题已经在 `TouchKeyboardSkinDesign` 中受更小的限制。
pub const MAX_DOCUMENT_BYTES: usize = 1 << 20;

/// 偏好顶层字段在同步文档中的归属。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncFieldClass {
    Shared,
    Theme,
    Local,
}

/// 主题字段。候选皮肤是本机皮肤库的引用，另一台设备未必安装同一个包，暂不写入
/// WebDAV；可移植的自定义键盘设计和颜色仍随文档传输。
const THEME_FIELDS: &[&str] = &[
    "theme",
    "settings_theme",
    "candidate_theme",
    "toolbar_theme",
    "screen_keyboard_theme",
    "handwriting_theme",
    "voice_theme",
    "emoji_theme",
    "menu_theme",
    "global_theme",
    "custom_theme",
];

/// `settings_document` 已经为导出文件定义了本机边界。同步域复用它，避免账号、
/// WebDAV 凭据、模型路径和网络代理在两个导出入口出现不同的泄露规则。
fn is_local_field(name: &str) -> bool {
    crate::settings_document::LOCAL_SECTIONS.contains(&name)
}

/// 返回已知字段的同步归属；未知字段不会从已类型化的 `Preferences` 产生，但在
/// 宿主传入字段名时仍按本机处理，防止未来字段未经审查就出网。
pub fn classify_field(name: &str) -> SyncFieldClass {
    if is_local_field(name) {
        SyncFieldClass::Local
    } else if THEME_FIELDS.contains(&name) {
        SyncFieldClass::Theme
    } else {
        SyncFieldClass::Shared
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingsSyncDocument {
    pub format: String,
    pub version: u64,
    /// 跨平台输入与显示选择，不包含 `platform.*` 宿主设置和本机凭据。
    pub shared: Map<String, Value>,
    /// 可移植主题选择和自定义定义。
    pub theme: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSyncError {
    Serialization,
    InvalidPreferences,
    TooLarge,
}

impl SettingsSyncDocument {
    /// 从已校验偏好生成同步文档。这里不读取文件、不访问网络，也不携带未知字段。
    pub fn from_preferences(preferences: &Preferences) -> Result<Self, SettingsSyncError> {
        preferences
            .validate()
            .map_err(|_| SettingsSyncError::InvalidPreferences)?;
        let value =
            serde_json::to_value(preferences).map_err(|_| SettingsSyncError::Serialization)?;
        let object = value.as_object().ok_or(SettingsSyncError::Serialization)?;
        let mut shared = Map::new();
        let mut theme = Map::new();
        for (name, value) in object {
            match classify_field(name) {
                SyncFieldClass::Shared => {
                    shared.insert(name.clone(), value.clone());
                }
                SyncFieldClass::Theme => {
                    if name == "custom_theme" {
                        theme.insert(name.clone(), portable_custom_theme(value));
                    } else {
                        theme.insert(name.clone(), value.clone());
                    }
                }
                SyncFieldClass::Local => {}
            }
        }
        let document = Self {
            format: FORMAT.to_owned(),
            version: VERSION,
            shared,
            theme,
        };
        let bytes = serde_json::to_vec(&document).map_err(|_| SettingsSyncError::Serialization)?;
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(SettingsSyncError::TooLarge);
        }
        Ok(document)
    }

    pub fn to_json(&self) -> Result<String, SettingsSyncError> {
        let bytes = serde_json::to_vec(self).map_err(|_| SettingsSyncError::Serialization)?;
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(SettingsSyncError::TooLarge);
        }
        String::from_utf8(bytes).map_err(|_| SettingsSyncError::Serialization)
    }
}

fn portable_custom_theme(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return Value::Object(Map::new());
    };
    let mut portable = object.clone();
    // 这些是本机皮肤库的名字，不是主题定义；同步它们会让另一台设备得到悬空引用。
    portable.remove("candidate_skin");
    portable.remove("candidate_skin_dark");
    Value::Object(portable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::InputScheme;

    #[test]
    fn classifies_local_theme_and_shared_fields() {
        assert_eq!(classify_field("scheme"), SyncFieldClass::Shared);
        assert_eq!(classify_field("global_theme"), SyncFieldClass::Theme);
        assert_eq!(classify_field("voice_input"), SyncFieldClass::Local);
        assert_eq!(
            classify_field("cloud_clipboard_webdav"),
            SyncFieldClass::Local
        );
    }

    #[test]
    fn projection_keeps_shared_and_portable_theme_but_drops_local_data() {
        let mut preferences = Preferences::default();
        preferences.scheme = InputScheme::Shuangpin;
        preferences.voice_input.asr_token = "synthetic-token".into();
        preferences.network.proxy_server = "http://synthetic.invalid:8080".into();
        preferences.custom_theme.candidate_skin = Some("local-package".into());
        preferences.custom_theme.keyboard =
            Some(crate::preferences::TouchKeyboardSkinDesign::default());

        let document = SettingsSyncDocument::from_preferences(&preferences).unwrap();
        assert_eq!(document.shared["scheme"], Value::String("shuangpin".into()));
        assert!(!document.shared.contains_key("voice_input"));
        assert!(!document.shared.contains_key("network"));
        assert!(document.theme.contains_key("global_theme"));
        let custom = document.theme["custom_theme"].as_object().unwrap();
        assert!(!custom.contains_key("candidate_skin"));
        assert!(custom.contains_key("keyboard"));
        let text = document.to_json().unwrap();
        assert!(!text.contains("synthetic-token"));
        assert!(!text.contains("synthetic.invalid"));
    }

    #[test]
    fn credential_slots_never_cross_the_sync_boundary() {
        let mut preferences = Preferences::default();
        preferences.voice_input.asr_token = "synthetic-secret".into();
        preferences.ai_assistant.token = "synthetic-secret".into();
        let text = SettingsSyncDocument::from_preferences(&preferences)
            .unwrap()
            .to_json()
            .unwrap();
        assert!(!text.contains("synthetic-secret"), "{text}");
    }
}
