//! 设置文件：把共享偏好导出成用户自己保存的 JSON 文件，或者从这样的文件导入，用于换电脑、重装之前留一份设置。不经过账号，也不需要登录。
//!
//! 文件是 `{"format": "app.msime.client.preferences", "version": 1, "preferences": {...}}`，`preferences` 是共享偏好文档里的 `preferences`，去掉只属于本机的部分（[`LOCAL_SECTIONS`]）。macOS 原生设置窗口导出的是另一种文件（`app.msime.client.settings`，内容是 Apple 云同步的键值），两边的内容对不上，所以格式名也分开，导入时认出它并单独说明。
//!
//! 导入不直接写盘，只把文件换算成一份完整的偏好交给调用方，由调用方按修订号比较并交换保存（`PreferencesStore::save`），与设置窗口里任何一次修改走同一条路。

use crate::edition::Edition;
use crate::preferences::{ChineseScheme, InputScheme, Preferences};
use serde_json::{Map, Value};

/// 设置文件的格式名。
pub const FORMAT: &str = "app.msime.client.preferences";
/// 当前写出的版本。读得懂的最高版本也是它：更新的版本可能带着这里不认识的字段，按文件来自更新版本的水杉输入法拒绝。
pub const VERSION: u64 = 1;
/// 设置文件的大小上限，与偏好文档的上限（自定义皮肤照片也在里面）相同。
pub const MAX_DOCUMENT_BYTES: usize = 1 << 20;
/// macOS 原生设置窗口「导出设置…」写出的格式名，内容是 Apple 云同步的键值，不是共享偏好。
const MACOS_NATIVE_FORMAT: &str = "app.msime.client.settings";

/// 只属于本机、不随设置文件走的偏好：语音、AI 辅助和翻译服务的整组配置（里面有服务密钥，地址、模型和密钥要成对才有意义，本地语音模型的路径也只在这台电脑上成立）、诊断日志开关、匿名使用统计的同意与否，以及剪贴板历史开关。导出时不写，导入时保留本机的值。
///
/// 剪贴板历史开关跟着本机走有两层原因：记不记录这台电脑复制过的内容是本机的隐私选择；它默认关闭，大多数文件里都是关的，如果跟着文件走，导入时会把本机开着的历史关掉，而关掉就会清空已存的历史，这些记录找不回来。
pub const LOCAL_SECTIONS: [&str; 8] = [
    "voice_input",
    "ai_assistant",
    "custom_translation",
    "tencent_tmt",
    "niutrans",
    "diagnostic_log",
    "usage_reporting",
    "clipboard_history",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsDocumentError {
    /// 不是 JSON，或者不是水杉输入法的设置文件。
    NotSettingsDocument,
    /// macOS 原生设置窗口导出的文件。
    MacosNativeDocument,
    /// 是设置文件，但版本更新，或者里面的设置这一版读不懂、取值不合规。
    Unsupported,
}

impl SettingsDocumentError {
    /// 宿主之间传递的错误码。
    pub fn code(self) -> &'static str {
        match self {
            Self::NotSettingsDocument => "settings_document_invalid",
            Self::MacosNativeDocument => "settings_document_macos",
            Self::Unsupported => "settings_document_unsupported",
        }
    }
}

/// 把 `preferences` 写成设置文件的内容：缩进写出，用户打开或拿两台电脑的文件比对时读得懂。
pub fn export_document(preferences: &Preferences) -> Result<String, serde_json::Error> {
    let mut value = serde_json::to_value(preferences)?;
    if let Some(object) = value.as_object_mut() {
        for section in LOCAL_SECTIONS {
            object.remove(section);
        }
    }
    let mut document = Map::new();
    document.insert("format".into(), Value::String(FORMAT.into()));
    document.insert("version".into(), Value::from(VERSION));
    document.insert("preferences".into(), value);
    serde_json::to_string_pretty(&Value::Object(document))
}

/// 把设置文件换算成要保存的偏好：文件里的设置覆盖 `current`，[`LOCAL_SECTIONS`] 保留 `current` 的值，`fuzzy_pinyin.seeded` 只会从假变真（本机种过、文件里标着种过，或文件里模糊音开着）；文件里的输入方案本版本不提供时保留本机的方案（与账号同步下载时的规则一样，见 `edition::filter_downloaded_account_settings`），只有一个方案的版本根本不从文件里取方案。
pub fn import_document(
    bytes: &[u8],
    current: &Preferences,
    edition: &Edition,
) -> Result<Preferences, SettingsDocumentError> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(SettingsDocumentError::NotSettingsDocument);
    }
    // 用户可能用记事本打开看过再存回去，「带 BOM 的 UTF-8」开头的字节序标记不算文件内容。
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let mut document: Value =
        serde_json::from_slice(bytes).map_err(|_| SettingsDocumentError::NotSettingsDocument)?;
    let format = document.get("format").and_then(Value::as_str);
    if format == Some(MACOS_NATIVE_FORMAT) {
        return Err(SettingsDocumentError::MacosNativeDocument);
    }
    if format != Some(FORMAT) {
        return Err(SettingsDocumentError::NotSettingsDocument);
    }
    match document.get("version").and_then(Value::as_u64) {
        Some(version) if (1..=VERSION).contains(&version) => {}
        _ => return Err(SettingsDocumentError::Unsupported),
    }
    let Some(Value::Object(mut imported)) = document.get_mut("preferences").map(Value::take) else {
        return Err(SettingsDocumentError::Unsupported);
    };
    let local = serde_json::to_value(current).map_err(|_| SettingsDocumentError::Unsupported)?;
    for section in LOCAL_SECTIONS {
        imported.remove(section);
        if let Some(value) = local.get(section) {
            imported.insert(section.to_owned(), value.clone());
        }
    }
    let mut preferences: Preferences = serde_json::from_value(Value::Object(imported))
        .map_err(|_| SettingsDocumentError::Unsupported)?;
    // 记录一次性种子已经做过；文件里的值不能让本机重新种下用户关掉的规则。文件里标着种过（导出时随文件写出），或者模糊音开着，规则就是用户在另一台电脑上定下的（包括有意清空），标成已种过，本机以后第一次打开模糊音时不再被盖成全部规则（PreferencesStore::save、Windows 设置窗口和共享设置页的首次打开种子都看这个标记）。
    preferences.fuzzy_pinyin.seeded = current.fuzzy_pinyin.seeded
        || preferences.fuzzy_pinyin.seeded
        || preferences.fuzzy_pinyin.enabled;
    narrow_to_edition(&mut preferences, current, edition);
    preferences
        .validate()
        .map_err(|_| SettingsDocumentError::Unsupported)?;
    Ok(preferences)
}

fn narrow_to_edition(preferences: &mut Preferences, current: &Preferences, edition: &Edition) {
    let single_scheme = edition.input_schemes.len() <= 1;
    if single_scheme || !edition.offers(preferences.scheme) {
        preferences.scheme = current.scheme;
        preferences.last_chinese_scheme = current.last_chinese_scheme;
    }
    if preferences
        .last_chinese_scheme
        .is_some_and(|scheme| !edition.offers(InputScheme::from(scheme)))
    {
        preferences.last_chinese_scheme = current
            .last_chinese_scheme
            .filter(|scheme| edition.offers(InputScheme::from(*scheme)))
            .or_else(|| ChineseScheme::of(edition.default_scheme));
    }
    // 触屏键盘的方案列表按版本收窄过，非 full 的版本不从别的版本的文件里取。
    if !edition.is_full() {
        preferences.touch_keyboard_schemes = current.touch_keyboard_schemes.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::CredentialSlot;

    fn with_every_credential(mut preferences: Preferences) -> Preferences {
        for (index, (_, slot)) in preferences.credential_slots().into_iter().enumerate() {
            let secret = format!("synthetic-secret-{index}");
            match slot {
                CredentialSlot::Text(text) => *text = secret,
                CredentialSlot::Map(map) => {
                    map.insert("synthetic".into(), secret);
                }
            }
        }
        preferences
    }

    #[test]
    fn export_never_writes_a_credential_or_a_local_section() {
        let preferences = with_every_credential(Preferences::default());
        let text = export_document(&preferences).unwrap();
        assert!(!text.contains("synthetic-secret"), "{text}");
        let document: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(document["format"], FORMAT);
        assert_eq!(document["version"], VERSION);
        for section in LOCAL_SECTIONS {
            assert!(document["preferences"].get(section).is_none(), "{section}");
        }
    }

    #[test]
    fn a_round_trip_keeps_shared_settings_and_this_machine_s_local_ones() {
        let mut exported = Preferences::default();
        exported.candidate_page_size = 9;
        exported.chinese_punctuation = !exported.chinese_punctuation;
        exported.scheme = InputScheme::Shuangpin;
        exported.usage_reporting = false;
        exported.clipboard_history = false;
        exported.voice_input.asr_model_path = "/elsewhere/model".into();
        let text = export_document(&exported).unwrap();

        let mut current = Preferences::default();
        current.voice_input.asr_token = "synthetic-local-token".into();
        current.ai_assistant.token = "synthetic-local-token".into();
        current.usage_reporting = true;
        current.clipboard_history = true;
        current.voice_input.asr_model_path = "/here/model".into();
        current.fuzzy_pinyin.seeded = true;
        let imported = import_document(text.as_bytes(), &current, Edition::full()).unwrap();

        assert_eq!(imported.candidate_page_size, 9);
        assert_eq!(imported.chinese_punctuation, exported.chinese_punctuation);
        assert_eq!(imported.scheme, InputScheme::Shuangpin);
        assert!(imported.usage_reporting);
        assert!(imported.clipboard_history);
        assert_eq!(imported.voice_input, current.voice_input);
        assert_eq!(imported.ai_assistant, current.ai_assistant);
        assert_eq!(imported.custom_translation, current.custom_translation);
        assert_eq!(imported.tencent_tmt, current.tencent_tmt);
        assert_eq!(imported.niutrans, current.niutrans);
        assert_eq!(imported.diagnostic_log, current.diagnostic_log);
        assert!(imported.fuzzy_pinyin.seeded);
    }

    #[test]
    fn a_local_section_written_into_the_file_by_hand_is_ignored() {
        let mut document: Value =
            serde_json::from_str(&export_document(&Preferences::default()).unwrap()).unwrap();
        document["preferences"]["usage_reporting"] = Value::Bool(false);
        document["preferences"]["ai_assistant"] = serde_json::json!({ "token": "planted" });
        let current = Preferences::default();
        let imported =
            import_document(document.to_string().as_bytes(), &current, Edition::full()).unwrap();
        assert_eq!(imported.usage_reporting, current.usage_reporting);
        assert_eq!(imported.ai_assistant, current.ai_assistant);
    }

    #[test]
    fn files_that_are_not_ours_are_told_apart() {
        let current = Preferences::default();
        let full = Edition::full();
        assert_eq!(
            import_document(b"not json", &current, full),
            Err(SettingsDocumentError::NotSettingsDocument)
        );
        assert_eq!(
            import_document(
                br#"{"format":"something.else","version":1}"#,
                &current,
                full
            ),
            Err(SettingsDocumentError::NotSettingsDocument)
        );
        assert_eq!(
            import_document(
                br#"{"format":"app.msime.client.settings","version":1,"settings":{}}"#,
                &current,
                full
            ),
            Err(SettingsDocumentError::MacosNativeDocument)
        );
        let mut with_bom = b"\xEF\xBB\xBF".to_vec();
        with_bom.extend_from_slice(export_document(&current).unwrap().as_bytes());
        assert_eq!(
            import_document(&with_bom, &current, full),
            Ok(current.clone())
        );
        let oversized = vec![b' '; MAX_DOCUMENT_BYTES + 1];
        assert_eq!(
            import_document(&oversized, &current, full),
            Err(SettingsDocumentError::NotSettingsDocument)
        );
    }

    #[test]
    fn a_newer_or_damaged_document_is_refused_rather_than_partly_applied() {
        let current = Preferences::default();
        let full = Edition::full();
        let mut document: Value =
            serde_json::from_str(&export_document(&Preferences::default()).unwrap()).unwrap();
        document["version"] = Value::from(VERSION + 1);
        assert_eq!(
            import_document(document.to_string().as_bytes(), &current, full),
            Err(SettingsDocumentError::Unsupported)
        );
        document["version"] = Value::from(VERSION);
        document["preferences"]["a_setting_from_the_future"] = Value::Bool(true);
        assert_eq!(
            import_document(document.to_string().as_bytes(), &current, full),
            Err(SettingsDocumentError::Unsupported)
        );
        document["preferences"]
            .as_object_mut()
            .unwrap()
            .remove("a_setting_from_the_future");
        document["preferences"]["candidate_page_size"] = Value::from(200);
        assert_eq!(
            import_document(document.to_string().as_bytes(), &current, full),
            Err(SettingsDocumentError::Unsupported)
        );
    }

    // 在一台从没开过模糊音的电脑上导入开着模糊音的文件，保存后仍是文件里那几条规则，不被首次打开的种子盖成全部规则。
    #[test]
    fn imported_fuzzy_rules_survive_the_first_enable_seed() {
        use crate::preferences::{FuzzyPinyinRule, PreferencesStore};
        let mut exported = Preferences::default();
        exported.fuzzy_pinyin.enabled = true;
        exported.fuzzy_pinyin.rules = [FuzzyPinyinRule::ZZh].into_iter().collect();
        let text = export_document(&exported).unwrap();

        let directory = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(directory.path());
        let current = store.save(0, Preferences::default()).unwrap();
        assert!(!current.preferences.fuzzy_pinyin.seeded);
        let imported =
            import_document(text.as_bytes(), &current.preferences, Edition::full()).unwrap();
        let saved = store.save(current.revision, imported).unwrap();
        assert!(saved.preferences.fuzzy_pinyin.enabled);
        assert!(saved.preferences.fuzzy_pinyin.seeded);
        assert_eq!(
            saved.preferences.fuzzy_pinyin.rules,
            [FuzzyPinyinRule::ZZh].into_iter().collect()
        );

        // 文件来自一台从没种过的电脑、模糊音关着时不替本机标记，本机以后第一次打开仍按原来的规则种一次。
        let mut disabled = exported.clone();
        disabled.fuzzy_pinyin.enabled = false;
        let text = export_document(&disabled).unwrap();
        let imported =
            import_document(text.as_bytes(), &Preferences::default(), Edition::full()).unwrap();
        assert!(!imported.fuzzy_pinyin.seeded);

        // 另一台电脑上种过、挑过规则（包括全部清空）再关掉模糊音，导出的文件带着种过的标记；导入后第一次打开模糊音，仍是文件里挑的规则。
        for rules in [
            [FuzzyPinyinRule::ZZh].into_iter().collect(),
            std::collections::BTreeSet::new(),
        ] {
            let mut chosen = Preferences::default();
            chosen.fuzzy_pinyin.seeded = true;
            chosen.fuzzy_pinyin.rules = rules;
            let text = export_document(&chosen).unwrap();

            let directory = tempfile::tempdir().unwrap();
            let store = PreferencesStore::new(directory.path());
            let current = store.save(0, Preferences::default()).unwrap();
            let imported =
                import_document(text.as_bytes(), &current.preferences, Edition::full()).unwrap();
            assert!(imported.fuzzy_pinyin.seeded);
            let saved = store.save(current.revision, imported).unwrap();
            let mut enabled = saved.preferences.clone();
            enabled.fuzzy_pinyin.enabled = true;
            let enabled = store.save(saved.revision, enabled).unwrap();
            assert_eq!(
                enabled.preferences.fuzzy_pinyin.rules,
                chosen.fuzzy_pinyin.rules
            );
        }
    }

    #[test]
    fn a_scheme_this_edition_does_not_offer_keeps_the_local_one() {
        let wubi = Edition::by_id("wubi").unwrap();
        let exported = Preferences {
            scheme: InputScheme::Quanpin,
            last_chinese_scheme: Some(ChineseScheme::Quanpin),
            candidate_page_size: 7,
            ..Preferences::default()
        };
        let text = export_document(&exported).unwrap();
        let current = Preferences::for_edition(wubi);
        let imported = import_document(text.as_bytes(), &current, wubi).unwrap();
        assert_eq!(imported.scheme, current.scheme);
        assert_eq!(imported.last_chinese_scheme, current.last_chinese_scheme);
        assert_eq!(
            imported.touch_keyboard_schemes,
            current.touch_keyboard_schemes
        );
        assert_eq!(imported.candidate_page_size, 7);
    }
}
