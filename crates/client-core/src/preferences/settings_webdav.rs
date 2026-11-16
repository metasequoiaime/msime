//! 设置同步网盘是本安装的连接配置，不随任何同步或设置导出迁移。

use super::{valid_relay_url, valid_webdav_password, valid_webdav_username};
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "preferences-schema", derive(schemars::JsonSchema))]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SettingsSyncWebDavPreferences {
    pub url: String,
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for SettingsSyncWebDavPreferences {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SettingsSyncWebDavPreferences")
            .field("configured", &self.is_configured())
            .finish_non_exhaustive()
    }
}

impl SettingsSyncWebDavPreferences {
    pub(super) fn is_default(&self) -> bool {
        self.url.is_empty() && self.username.is_empty() && self.password.is_empty()
    }

    /// 地址为空时不触发自动同步；只有三项均合规的连接才能保存。
    pub fn is_configured(&self) -> bool {
        !self.url.is_empty()
    }

    pub fn validate(&self) -> bool {
        self.is_default()
            || (valid_relay_url(&self.url)
                && valid_webdav_username(&self.username)
                && valid_webdav_password(&self.password))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::{Preferences, PreferencesError, PreferencesStore, REDACTED};
    use crate::settings_document::{export_document, import_document};
    use crate::settings_sync::SettingsSyncDocument;
    use serde_json::{json, Value};

    fn configured() -> SettingsSyncWebDavPreferences {
        SettingsSyncWebDavPreferences {
            url: "https://dav.example.test/synthetic".into(),
            username: "synthetic-user".into(),
            password: "synthetic-app-password".into(),
        }
    }

    #[test]
    fn settings_webdav_defaults_are_omitted_and_invalid_connections_cannot_be_saved() {
        let defaults = Preferences::default();
        assert!(!defaults.settings_sync_webdav.is_configured());
        assert!(serde_json::to_value(defaults)
            .unwrap()
            .get("settings_sync_webdav")
            .is_none());
        let root = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(root.path());
        for patch in [
            json!({"url":"https://dav.example.test"}),
            json!({"username":"synthetic-user", "password":"synthetic-app-password"}),
            json!({"url":"http://dav.example.test", "username":"synthetic-user", "password":"synthetic-app-password"}),
            json!({"url":"https://dav.example.test/?token=synthetic", "username":"synthetic-user", "password":"synthetic-app-password"}),
            json!({"url":"https://dav.example.test/", "username":"invalid:user", "password":"synthetic-app-password"}),
        ] {
            assert!(matches!(
                store.apply_patch(0, &json!({"settings_sync_webdav":patch})),
                Err(PreferencesError::InvalidSettingsSyncWebDav)
            ));
            assert_eq!(store.load().unwrap().revision, 0);
        }
    }

    #[test]
    fn settings_webdav_round_trips_locally_but_never_enters_portable_documents() {
        let preferences = Preferences {
            settings_sync_webdav: configured(),
            ..Preferences::default()
        };
        let root = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(root.path());
        store.save(0, preferences.clone()).unwrap();
        assert_eq!(
            store.load().unwrap().preferences.settings_sync_webdav,
            configured()
        );
        let exported = export_document(&preferences).unwrap();
        let document = SettingsSyncDocument::from_preferences(&preferences)
            .unwrap()
            .to_json()
            .unwrap();
        for text in [&exported, &document] {
            assert!(!text.contains("settings_sync_webdav"));
            assert!(!text.contains("synthetic-app-password"));
            assert!(!text.contains("dav.example.test"));
        }
        let mut imported: Value = serde_json::from_str(&exported).unwrap();
        imported["preferences"]["settings_sync_webdav"] = json!({"password":"synthetic-injected"});
        let imported = import_document(
            &serde_json::to_vec(&imported).unwrap(),
            &preferences,
            crate::edition::Edition::full(),
        )
        .unwrap();
        assert_eq!(imported.settings_sync_webdav, configured());
        assert_eq!(
            preferences.restored_to_defaults().settings_sync_webdav,
            configured()
        );
    }

    #[test]
    fn settings_webdav_credentials_and_account_are_redacted() {
        let mut preferences = Preferences {
            settings_sync_webdav: configured(),
            ..Preferences::default()
        };
        assert!(preferences
            .credential_slots()
            .iter()
            .any(|(path, _)| *path == "settings_sync_webdav.password"));
        let redacted = preferences.redacted_for_diagnostics();
        assert_eq!(redacted["settings_sync_webdav"]["password"], REDACTED);
        assert_eq!(redacted["settings_sync_webdav"]["username"], REDACTED);
        let debug = format!("{:?}", preferences.settings_sync_webdav);
        assert!(!debug.contains("synthetic"));
        preferences.settings_sync_webdav.url =
            "https://user:synthetic@dav.example.test/path?token=synthetic#synthetic".into();
        assert_eq!(
            preferences.redacted_for_diagnostics()["settings_sync_webdav"]["url"],
            "https://dav.example.test/path"
        );
    }
}
