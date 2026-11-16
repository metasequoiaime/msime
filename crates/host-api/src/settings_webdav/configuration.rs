//! 原生设置页的连接读写；响应只含摘要，保存不联网、不生成同步身份。

use super::{Action, Request, INVALID};
use msime_client_core::preferences::{
    PreferencesError, PreferencesSnapshot, PreferencesStore, SettingsSyncWebDavPreferences,
};
use serde_json::{json, Value};

fn code(error: PreferencesError) -> String {
    match error {
        PreferencesError::Conflict => "conflict",
        PreferencesError::InvalidSettingsSyncWebDav => INVALID,
        _ => "settings_sync_storage",
    }
    .into()
}

pub(super) fn load(store: &PreferencesStore) -> Result<PreferencesSnapshot, String> {
    // 只读预检不会为了查看尚未配置的连接而创建目录或锁文件。
    store.preview_patch(&json!({})).map_err(code)
}

fn summary(snapshot: &PreferencesSnapshot) -> Value {
    let config = &snapshot.preferences.settings_sync_webdav;
    json!({
        "revision": snapshot.revision,
        "configured": config.is_configured(),
        "url": config.url,
        "username": config.username,
        "password_saved": !config.password.is_empty(),
    })
}

pub(super) fn request(store: &PreferencesStore, request: &Request) -> Result<Value, String> {
    let Action::Configure {
        expected_revision,
        keep_password,
    } = request.action
    else {
        return Ok(summary(&load(store)?));
    };
    let draft = request.config.as_ref().ok_or(INVALID)?;
    let current = load(store)?;
    if current.revision != expected_revision {
        return Err("conflict".into());
    }
    let saved = &current.preferences.settings_sync_webdav;
    let password = if keep_password {
        if !draft.password.is_empty()
            || draft.url != saved.url
            || draft.username != saved.username
            || !saved.is_configured()
        {
            return Err(INVALID.into());
        }
        saved.password.clone()
    } else {
        draft.password.clone()
    };
    let next = SettingsSyncWebDavPreferences {
        url: draft.url.clone(),
        username: draft.username.clone(),
        password,
    };
    if !next.validate() {
        return Err(INVALID.into());
    }
    // CAS 在偏好锁内再次比较；并发编辑其他设置也不能被这个旧表单盖掉。
    let snapshot = store
        .apply_patch(expected_revision, &json!({"settings_sync_webdav":next}))
        .map_err(code)?;
    Ok(summary(&snapshot))
}

#[cfg(test)]
mod tests {
    use super::super::request_json;
    use super::*;

    fn call(
        directory: &std::path::Path,
        action: Value,
        config: Option<Value>,
    ) -> Result<Value, String> {
        let mut request = json!({"preferences_directory": directory, "action": action});
        if let Some(config) = config {
            request["config"] = config;
        }
        request_json(&serde_json::to_vec(&request).unwrap())
    }

    fn config() -> Value {
        json!({"url":"https://dav.example.test/synthetic", "username":"synthetic-user", "password":"synthetic-app-password"})
    }

    #[test]
    fn configuration_is_offline_and_saved_credentials_are_never_returned() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("not-created");
        let initial = call(&directory, json!({"operation":"configuration"}), None).unwrap();
        assert_eq!(initial["configured"], false);
        assert_eq!(initial["revision"], 0);
        assert!(!directory.exists());
        assert_eq!(
            call(
                &directory,
                json!({"operation":"scheduled", "trigger":"opened"}),
                None
            )
            .unwrap_err(),
            "settings_sync_not_configured"
        );
        assert!(!directory.exists());
        let saved = call(
            &directory,
            json!({"operation":"configure", "expected_revision":0}),
            Some(config()),
        )
        .unwrap();
        assert_eq!(saved["configured"], true);
        assert_eq!(saved["password_saved"], true);
        assert!(!saved.to_string().contains("synthetic-app-password"));
        assert_eq!(
            call(&directory, json!({"operation":"configuration"}), None).unwrap(),
            saved
        );
        let status = call(&directory, json!({"operation":"status"}), None).unwrap();
        assert_eq!(status["device_id"], Value::Null);
        assert!(!directory.join("settings-sync").exists());
    }

    #[test]
    fn password_reuse_requires_the_same_connection_and_cas_preserves_concurrent_edits() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path();
        let saved = call(
            directory,
            json!({"operation":"configure", "expected_revision":0}),
            Some(config()),
        )
        .unwrap();
        let revision = saved["revision"].as_u64().unwrap();
        let action =
            json!({"operation":"configure", "expected_revision":revision, "keep_password":true});
        let mut draft = config();
        draft["password"] = json!("");
        let reused = call(directory, action.clone(), Some(draft.clone())).unwrap();
        let revision = reused["revision"].as_u64().unwrap();
        let action =
            json!({"operation":"configure", "expected_revision":revision, "keep_password":true});
        for field in ["url", "username", "password"] {
            let mut changed = draft.clone();
            changed[field] = json!("synthetic-changed");
            assert_eq!(
                call(directory, action.clone(), Some(changed)).unwrap_err(),
                INVALID
            );
        }
        let store = PreferencesStore::new(directory);
        let changed = store
            .apply_patch(revision, &json!({"candidate_page_size":7}))
            .unwrap();
        assert_eq!(
            call(directory, action, Some(draft)).unwrap_err(),
            "conflict"
        );
        assert_eq!(store.load().unwrap().preferences.candidate_page_size, 7);
        assert_eq!(
            store
                .load()
                .unwrap()
                .preferences
                .settings_sync_webdav
                .password,
            "synthetic-app-password"
        );
        let stopped = call(
            directory,
            json!({"operation":"configure", "expected_revision":changed.revision}),
            Some(json!({"url":"", "username":"", "password":""})),
        )
        .unwrap();
        assert_eq!(stopped["configured"], false);
        assert_eq!(stopped["password_saved"], false);
        assert_eq!(store.load().unwrap().preferences.candidate_page_size, 7);
    }
}
