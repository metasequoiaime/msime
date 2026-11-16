//! WebDAV 设置同步的无网络文档投影。
//!
//! 这里只负责把已经通过 `Preferences` 校验的本机偏好分成可同步的共享设置、主题
//! 和本机设置，并提供三方合并、事件链及密文信封。网络、设备批准和宿主触发留给服务层；输入法宿主不能
//! 直接把完整偏好文件交给 WebDAV。

use crate::edition::Edition;
use crate::preferences::Preferences;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

/// WebDAV 设置域的文档格式名，与设置文件和其他同步域分开。
pub const FORMAT: &str = "app.msime.settings-sync";
/// 当前文档版本。
pub const VERSION: u64 = 1;
/// 单个同步文档的上限；图片主题已经在 `TouchKeyboardSkinDesign` 中受更小的限制。
pub const MAX_DOCUMENT_BYTES: usize = 1 << 20;
/// 事件信封的格式名；事件信封与其他 WebDAV 日志域分开，便于独立轮换密钥。
pub const EVENT_FORMAT: &str = "app.msime.settings-sync-event";
pub const EVENT_VERSION: u64 = 1;
pub const MAX_EVENT_BYTES: usize = MAX_DOCUMENT_BYTES + 4096;
/// 从已批准的设备密钥派生设置同步专用密钥，不能复用云剪贴板或词库的标签。
pub const KEY_INFO: &[u8] = b"msime-settings-v1-key";
mod crypto;
mod event;
mod fields;
pub use crypto::{SealedSettingsEvent, SettingsSyncCipher, MAX_SEALED_EVENT_BYTES};
pub use event::SettingsSyncEvent;

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

/// 未审核的字段一律留在本机；设置文件导出与网盘自动上传的边界不同。
pub fn classify_field(name: &str) -> SyncFieldClass {
    if THEME_FIELDS.contains(&name) {
        SyncFieldClass::Theme
    } else if fields::SHARED_FIELDS.contains(&name) {
        SyncFieldClass::Shared
    } else {
        SyncFieldClass::Local
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsSyncDocument {
    pub format: String,
    pub version: u64,
    /// 跨平台输入与显示选择，不包含 `platform.*` 宿主设置和本机凭据。
    pub shared: Map<String, Value>,
    /// 可移植主题选择和自定义定义。
    pub theme: Map<String, Value>,
    /// 明确删除的顶层字段。缺字段本身只表示旧文档没有携带该字段，不能表达删除。
    #[serde(default)]
    pub deleted_shared: BTreeSet<String>,
    #[serde(default)]
    pub deleted_theme: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSyncError {
    Crypto,
    RandomUnavailable,
    Serialization,
    InvalidPreferences,
    InvalidDocument,
    TooLarge,
    InvalidEvent,
    ChainMismatch,
}

/// 两台设备同时修改同一个字段时交给宿主显示的冲突。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsSyncConflict {
    pub class: SyncFieldClass,
    pub field: String,
}

/// 三方合并结果。没有冲突的字段已经合入；冲突字段保留本地值，调用方必须提示用户
/// 后再决定是否采用远端值，不能在后台静默覆盖用户刚保存的设置。
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsSyncMerge {
    pub document: SettingsSyncDocument,
    pub conflicts: Vec<SettingsSyncConflict>,
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
            deleted_shared: fields::SHARED_FIELDS
                .iter()
                .filter(|field| !object.contains_key(**field))
                .map(|field| (*field).to_owned())
                .collect(),
            deleted_theme: THEME_FIELDS
                .iter()
                .filter(|field| !object.contains_key(**field))
                .map(|field| (*field).to_owned())
                .collect(),
        };
        let bytes = serde_json::to_vec(&document).map_err(|_| SettingsSyncError::Serialization)?;
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(SettingsSyncError::TooLarge);
        }
        Ok(document)
    }

    /// 读取 WebDAV 事件中的明文文档。加密、设备批准和事件链由上层驱动负责；这里
    /// 仍然严格检查格式、版本、域边界和主题可移植性。
    pub fn from_json(bytes: &[u8]) -> Result<Self, SettingsSyncError> {
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(SettingsSyncError::TooLarge);
        }
        let document: Self =
            serde_json::from_slice(bytes).map_err(|_| SettingsSyncError::InvalidDocument)?;
        document.validate()
    }

    fn validate(self) -> Result<Self, SettingsSyncError> {
        let encoded = serde_json::to_vec(&self).map_err(|_| SettingsSyncError::Serialization)?;
        if encoded.len() > MAX_DOCUMENT_BYTES {
            return Err(SettingsSyncError::TooLarge);
        }
        if self.format != FORMAT || self.version != VERSION {
            return Err(SettingsSyncError::InvalidDocument);
        }
        for field in self.shared.keys() {
            if classify_field(field) != SyncFieldClass::Shared {
                return Err(SettingsSyncError::InvalidDocument);
            }
        }
        for field in self.theme.keys() {
            if classify_field(field) != SyncFieldClass::Theme {
                return Err(SettingsSyncError::InvalidDocument);
            }
        }
        if !valid_tombstones(&self.deleted_shared, SyncFieldClass::Shared, &self.shared)
            || !valid_tombstones(&self.deleted_theme, SyncFieldClass::Theme, &self.theme)
        {
            return Err(SettingsSyncError::InvalidDocument);
        }
        if let Some(custom) = self.theme.get("custom_theme") {
            let Some(custom) = custom.as_object() else {
                return Err(SettingsSyncError::InvalidDocument);
            };
            if custom.contains_key("candidate_skin") || custom.contains_key("candidate_skin_dark") {
                return Err(SettingsSyncError::InvalidDocument);
            }
        }
        let mut values = serde_json::to_value(Preferences::default())
            .map_err(|_| SettingsSyncError::Serialization)?;
        let object = values
            .as_object_mut()
            .ok_or(SettingsSyncError::Serialization)?;
        object.extend(self.shared.clone());
        object.extend(self.theme.clone());
        let (typed, unknown) =
            crate::preferences::deserialize_keeping_unknown::<Preferences>(&values, &[])
                .map_err(|_| SettingsSyncError::InvalidDocument)?;
        if !unknown.is_empty() || !crate::preferences::newer_list_values(&values).is_empty() {
            return Err(SettingsSyncError::InvalidDocument);
        }
        typed
            .validate()
            .map_err(|_| SettingsSyncError::InvalidDocument)?;
        Ok(self)
    }

    /// 按基线做字段级三方合并。显式墓碑参与比较；旧文档未提供的字段沿用基线。
    pub fn merge(
        base: &Self,
        local: &Self,
        remote: &Self,
    ) -> Result<SettingsSyncMerge, SettingsSyncError> {
        base.clone().validate()?;
        local.clone().validate()?;
        remote.clone().validate()?;
        if base.format != local.format
            || base.format != remote.format
            || base.version != local.version
            || base.version != remote.version
        {
            return Err(SettingsSyncError::InvalidDocument);
        }
        let mut conflicts = Vec::new();
        let (shared, deleted_shared) = merge_map(
            (&base.shared, &base.deleted_shared),
            (&local.shared, &local.deleted_shared),
            (&remote.shared, &remote.deleted_shared),
            SyncFieldClass::Shared,
            &mut conflicts,
        );
        let (theme, deleted_theme) = merge_map(
            (&base.theme, &base.deleted_theme),
            (&local.theme, &local.deleted_theme),
            (&remote.theme, &remote.deleted_theme),
            SyncFieldClass::Theme,
            &mut conflicts,
        );
        let document = Self {
            format: FORMAT.to_owned(),
            version: VERSION,
            shared,
            theme,
            deleted_shared,
            deleted_theme,
        }
        .validate()?;
        Ok(SettingsSyncMerge {
            document,
            conflicts,
        })
    }

    pub fn to_json(&self) -> Result<String, SettingsSyncError> {
        let validated = self.clone().validate()?;
        let bytes = serde_json::to_vec(&validated).map_err(|_| SettingsSyncError::Serialization)?;
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(SettingsSyncError::TooLarge);
        }
        String::from_utf8(bytes).map_err(|_| SettingsSyncError::Serialization)
    }

    /// 将同步文档应用到当前偏好。同步文档只覆盖共享字段和可移植主题字段，
    /// 本机字段（凭据、路径、代理和宿主能力相关设置）始终沿用 `current`。
    ///
    /// 复用设置文件导入的版本、未知字段和版本裁剪规则，避免 WebDAV 下载和
    /// 本地设置文件导入各自维护一套不同的偏好安全边界。
    pub fn apply_to_preferences(
        &self,
        current: &Preferences,
        edition: &Edition,
    ) -> Result<Preferences, SettingsSyncError> {
        self.clone().validate()?;
        let mut preferences = serde_json::to_value(current)
            .map_err(|_| SettingsSyncError::Serialization)?
            .as_object()
            .cloned()
            .ok_or(SettingsSyncError::Serialization)?;
        let defaults = serde_json::to_value(Preferences::default())
            .map_err(|_| SettingsSyncError::Serialization)?
            .as_object()
            .cloned()
            .ok_or(SettingsSyncError::Serialization)?;
        for field in self.deleted_shared.iter().chain(self.deleted_theme.iter()) {
            if field == "custom_theme" {
                let skin = preferences.get(field).and_then(Value::as_object);
                let mut reset = defaults
                    .get(field)
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                for key in ["candidate_skin", "candidate_skin_dark"] {
                    if let Some(value) = skin.and_then(|skin| skin.get(key)) {
                        reset.insert(key.to_owned(), value.clone());
                    }
                }
                preferences.insert(field.clone(), Value::Object(reset));
            } else if let Some(value) = defaults.get(field) {
                preferences.insert(field.clone(), value.clone());
            } else {
                preferences.remove(field);
            }
        }
        for (field, value) in self.shared.iter().chain(self.theme.iter()) {
            if *field == "custom_theme" {
                let current_theme = preferences
                    .get(field)
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                let Some(remote_theme) = value.as_object() else {
                    return Err(SettingsSyncError::InvalidDocument);
                };
                let mut merged_theme = remote_theme.clone();
                for key in ["candidate_skin", "candidate_skin_dark"] {
                    if let Some(value) = current_theme.get(key) {
                        merged_theme.insert(key.to_owned(), value.clone());
                    }
                }
                preferences.insert(field.clone(), Value::Object(merged_theme));
            } else {
                preferences.insert(field.clone(), value.clone());
            }
        }
        let mut document = Map::new();
        document.insert(
            "format".into(),
            Value::String(crate::settings_document::FORMAT.into()),
        );
        document.insert(
            "version".into(),
            Value::from(crate::settings_document::VERSION),
        );
        document.insert("preferences".into(), Value::Object(preferences));
        let bytes = serde_json::to_vec(&Value::Object(document))
            .map_err(|_| SettingsSyncError::Serialization)?;
        crate::settings_document::import_document(&bytes, current, edition)
            .map_err(|_| SettingsSyncError::InvalidDocument)
    }
}

fn merge_map(
    base: (&Map<String, Value>, &BTreeSet<String>),
    local: (&Map<String, Value>, &BTreeSet<String>),
    remote: (&Map<String, Value>, &BTreeSet<String>),
    class: SyncFieldClass,
    conflicts: &mut Vec<SettingsSyncConflict>,
) -> (Map<String, Value>, BTreeSet<String>) {
    let (base, base_deleted) = base;
    let (local, local_deleted) = local;
    let (remote, remote_deleted) = remote;
    let fields: BTreeSet<_> = base
        .keys()
        .chain(local.keys())
        .chain(remote.keys())
        .chain(base_deleted)
        .chain(local_deleted)
        .chain(remote_deleted)
        .cloned()
        .collect();
    let mut merged = Map::new();
    let mut deleted = BTreeSet::new();
    for field in fields {
        let baseline = entry(base, base_deleted, &field);
        let local_value = match entry(local, local_deleted, &field) {
            Entry::Missing => baseline,
            value => value,
        };
        let remote_value = match entry(remote, remote_deleted, &field) {
            Entry::Missing => baseline,
            value => value,
        };
        let local_changed = local_value != baseline;
        let remote_changed = remote_value != baseline;
        let selected = if !local_changed {
            remote_value
        } else if !remote_changed || local_value == remote_value {
            local_value
        } else {
            conflicts.push(SettingsSyncConflict {
                class,
                field: field.clone(),
            });
            local_value
        };
        match selected {
            Entry::Value(value) => {
                merged.insert(field, value.clone());
            }
            Entry::Deleted => {
                deleted.insert(field);
            }
            Entry::Missing => {}
        }
    }
    (merged, deleted)
}

#[derive(Clone, Copy, PartialEq)]
enum Entry<'a> {
    Missing,
    Deleted,
    Value(&'a Value),
}

fn entry<'a>(map: &'a Map<String, Value>, deleted: &BTreeSet<String>, field: &str) -> Entry<'a> {
    map.get(field)
        .map(Entry::Value)
        .or_else(|| deleted.contains(field).then_some(Entry::Deleted))
        .unwrap_or(Entry::Missing)
}

fn valid_tombstones(
    deleted: &BTreeSet<String>,
    class: SyncFieldClass,
    values: &Map<String, Value>,
) -> bool {
    deleted.iter().all(|field| {
        classify_field(field) == class && !values.contains_key(field) && !field.is_empty()
    })
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
        assert_eq!(classify_field("unreviewed_secret"), SyncFieldClass::Local);
        assert_eq!(
            classify_field("android_keyboard_skin_light"),
            SyncFieldClass::Local
        );
        assert_eq!(classify_field("keybindings"), SyncFieldClass::Local);
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
        for (path, _) in preferences.credential_slots() {
            assert_eq!(
                classify_field(path.split('.').next().unwrap()),
                SyncFieldClass::Local,
                "{path}"
            );
        }
        preferences.voice_input.asr_token = "synthetic-secret".into();
        preferences.ai_assistant.token = "synthetic-secret".into();
        let text = SettingsSyncDocument::from_preferences(&preferences)
            .unwrap()
            .to_json()
            .unwrap();
        assert!(!text.contains("synthetic-secret"), "{text}");
    }

    #[test]
    fn json_round_trip_rejects_local_sections_and_skin_references() {
        let preferences = Preferences::default();
        let document = SettingsSyncDocument::from_preferences(&preferences).unwrap();
        let text = document.to_json().unwrap();
        assert_eq!(
            SettingsSyncDocument::from_json(text.as_bytes()).unwrap(),
            document
        );

        let mut leaked = document.clone();
        leaked
            .shared
            .insert("voice_input".into(), Value::Object(Map::new()));
        assert_eq!(
            SettingsSyncDocument::from_json(&serde_json::to_vec(&leaked).unwrap()),
            Err(SettingsSyncError::InvalidDocument)
        );

        let mut custom = Map::new();
        custom.insert("candidate_skin".into(), Value::String("local".into()));
        let mut skin_leak = document;
        skin_leak
            .theme
            .insert("custom_theme".into(), Value::Object(custom));
        assert_eq!(
            SettingsSyncDocument::from_json(&serde_json::to_vec(&skin_leak).unwrap()),
            Err(SettingsSyncError::InvalidDocument)
        );
    }

    #[test]
    fn three_way_merge_keeps_independent_changes_and_reports_same_field_conflicts() {
        let base = SettingsSyncDocument {
            format: FORMAT.into(),
            version: VERSION,
            shared: Map::from_iter([
                ("scheme".into(), Value::String("quanpin".into())),
                ("candidate_page_size".into(), Value::from(5)),
            ]),
            theme: Map::new(),
            deleted_shared: BTreeSet::new(),
            deleted_theme: BTreeSet::new(),
        };
        let mut local = base.clone();
        local
            .shared
            .insert("scheme".into(), Value::String("shuangpin".into()));
        let mut remote = base.clone();
        remote
            .shared
            .insert("candidate_page_size".into(), Value::from(8));
        let merged = SettingsSyncDocument::merge(&base, &local, &remote).unwrap();
        assert!(merged.conflicts.is_empty());
        assert_eq!(merged.document.shared["scheme"], "shuangpin");
        assert_eq!(merged.document.shared["candidate_page_size"], 8);

        remote
            .shared
            .insert("scheme".into(), Value::String("wubi".into()));
        let conflict = SettingsSyncDocument::merge(&base, &local, &remote).unwrap();
        assert_eq!(
            conflict.conflicts,
            vec![SettingsSyncConflict {
                class: SyncFieldClass::Shared,
                field: "scheme".into(),
            }]
        );
        assert_eq!(conflict.document.shared["scheme"], "shuangpin");
    }

    #[test]
    fn applying_a_remote_document_preserves_local_fields_and_skin_references() {
        let mut current = Preferences::default();
        current.voice_input.asr_token = "synthetic-local-token".into();
        current.network.proxy_server = "http://synthetic.invalid:8080".into();
        current.custom_theme.candidate_skin = Some("installed-skin".into());

        let mut remote = SettingsSyncDocument::from_preferences(&current).unwrap();
        remote
            .shared
            .insert("candidate_page_size".into(), Value::from(9));
        remote.theme.insert(
            "custom_theme".into(),
            serde_json::json!({"keyboard": {"background": 0x123456}}),
        );

        let applied = remote
            .apply_to_preferences(&current, Edition::full())
            .unwrap();
        assert_eq!(applied.candidate_page_size, 9);
        assert_eq!(applied.voice_input.asr_token, "synthetic-local-token");
        assert_eq!(
            applied.network.proxy_server,
            "http://synthetic.invalid:8080"
        );
        assert_eq!(
            applied.custom_theme.candidate_skin.as_deref(),
            Some("installed-skin")
        );
        assert!(applied.custom_theme.keyboard.is_some());
    }

    #[test]
    fn event_chain_rejects_wrong_parent_and_round_trips() {
        let document = SettingsSyncDocument::from_preferences(&Preferences::default()).unwrap();
        let first = SettingsSyncEvent::new(0, "a".repeat(32), "", document.clone()).unwrap();
        let second =
            SettingsSyncEvent::new(1, "b".repeat(32), first.head().unwrap(), document).unwrap();
        second.validate_successor(&first).unwrap();
        let encoded = second.to_json().unwrap();
        assert_eq!(SettingsSyncEvent::from_json(&encoded).unwrap(), second);

        let wrong =
            SettingsSyncEvent::new(1, "b".repeat(32), "0".repeat(64), second.document).unwrap();
        assert_eq!(
            wrong.validate_successor(&first),
            Err(SettingsSyncError::ChainMismatch)
        );
    }

    #[test]
    fn event_validation_rejects_bad_device_ids_and_oversized_payloads() {
        let document = SettingsSyncDocument::from_preferences(&Preferences::default()).unwrap();
        assert_eq!(
            SettingsSyncEvent::new(0, "not-a-device", "", document.clone()),
            Err(SettingsSyncError::InvalidEvent)
        );
        let mut oversized = document;
        oversized
            .shared
            .insert("future".into(), Value::String("x".repeat(MAX_EVENT_BYTES)));
        assert_eq!(
            SettingsSyncEvent::new(0, "a".repeat(32), "", oversized),
            Err(SettingsSyncError::TooLarge)
        );
    }

    #[test]
    fn tombstones_survive_three_way_merge_and_reset_the_deleted_field() {
        let mut current = Preferences::default();
        current.candidate_page_size = 9;
        let base = SettingsSyncDocument::from_preferences(&current).unwrap();
        let mut local = base.clone();
        local.shared.remove("candidate_page_size");
        local.deleted_shared.insert("candidate_page_size".into());
        let merged = SettingsSyncDocument::merge(&base, &local, &base).unwrap();
        assert!(merged
            .document
            .deleted_shared
            .contains("candidate_page_size"));
        let applied = merged
            .document
            .apply_to_preferences(&current, Edition::full())
            .unwrap();
        assert_eq!(
            applied.candidate_page_size,
            Preferences::default().candidate_page_size
        );
    }
    #[test]
    fn omitted_default_values_are_tombstones_and_theme_removal_keeps_only_local_references() {
        let mut old = Preferences::default();
        old.touch_number_row = true;
        old.custom_theme.keyboard = Some(crate::preferences::TouchKeyboardSkinDesign::default());
        old.custom_theme.candidate_skin = Some("local-skin".into());
        let base = SettingsSyncDocument::from_preferences(&old).unwrap();
        let reset = SettingsSyncDocument::from_preferences(&Preferences::default()).unwrap();
        assert!(reset.deleted_shared.contains("touch_number_row"));
        let merged = SettingsSyncDocument::merge(&base, &base, &reset).unwrap();
        let applied = merged
            .document
            .apply_to_preferences(&old, Edition::full())
            .unwrap();
        assert!(!applied.touch_number_row);
        assert!(applied.custom_theme.keyboard.is_none());
        assert_eq!(
            applied.custom_theme.candidate_skin.as_deref(),
            Some("local-skin")
        );
    }

    #[test]
    fn external_documents_reject_unknown_fields_types_and_nested_theme_keys() {
        let base = SettingsSyncDocument::from_preferences(&Preferences::default()).unwrap();
        for (key, value) in [
            ("unreviewed_secret", serde_json::json!("synthetic")),
            ("candidate_page_size", serde_json::json!("bad")),
        ] {
            let mut bad = base.clone();
            bad.shared.insert(key.into(), value);
            assert_eq!(
                SettingsSyncDocument::from_json(&serde_json::to_vec(&bad).unwrap()),
                Err(SettingsSyncError::InvalidDocument)
            );
        }
        let mut bad = base;
        bad.deleted_theme.remove("custom_theme");
        bad.theme.insert(
            "custom_theme".into(),
            serde_json::json!({"file_path": "/synthetic/private"}),
        );
        assert_eq!(
            SettingsSyncDocument::from_json(&serde_json::to_vec(&bad).unwrap()),
            Err(SettingsSyncError::InvalidDocument)
        );
    }
}
