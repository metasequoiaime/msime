//! 原生宿主的 WebDAV 设置同步 JSON 入口。状态查询不联网，其余操作由宿主工作线程调用。
//! 网盘草稿由宿主安全存储持有，不返回凭据，不借用现有词库或云剪贴板的配置。

use msime_client_core::preferences::PreferencesStore;
use msime_client_services::settings_webdav::{SettingsSyncTrigger, SettingsWebDav};
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

mod configuration;

pub(crate) const INVALID: &str = "invalid_settings_sync";
pub(crate) const MAX_REQUEST_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    url: String,
    username: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Configuration {},
    Configure {
        expected_revision: u64,
        #[serde(default)]
        keep_password: bool,
    },
    Status {},
    ScheduleStatus {},
    Run {},
    Scheduled {
        trigger: ScheduleTrigger,
    },
    Devices {},
    Approve {
        device_id: String,
    },
    Reject {
        device_id: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ScheduleTrigger {
    LocalChange,
    Opened,
    Periodic,
    Manual,
}

impl From<ScheduleTrigger> for SettingsSyncTrigger {
    fn from(trigger: ScheduleTrigger) -> Self {
        match trigger {
            ScheduleTrigger::LocalChange => Self::LocalChange,
            ScheduleTrigger::Opened => Self::Opened,
            ScheduleTrigger::Periodic => Self::Periodic,
            ScheduleTrigger::Manual => Self::Manual,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    preferences_directory: String,
    #[serde(default)]
    config: Option<Config>,
    #[serde(default)]
    device_name: String,
    /// 只有设置应用在用户配置同步时显式允许生成身份；输入法进程保持默认的 false。
    #[serde(default)]
    create_identity: bool,
    action: Action,
}

pub fn request_json(bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(INVALID.into());
    }
    let request: Request = serde_json::from_slice(bytes).map_err(|_| INVALID.to_owned())?;
    let path = Path::new(&request.preferences_directory);
    if !path.is_absolute()
        || request.preferences_directory.len() > 16384
        || request.preferences_directory.contains('\0')
        || request.device_name.len() > 256
    {
        return Err(INVALID.into());
    }
    if let Action::Approve { device_id } | Action::Reject { device_id } = &request.action {
        if device_id.len() != 32
            || !device_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(INVALID.into());
        }
    }
    let store = PreferencesStore::new(path);
    if matches!(
        request.action,
        Action::Configuration {} | Action::Configure { .. }
    ) {
        return configuration::request(&store, &request);
    }
    let config = match request.config {
        Some(config) => config,
        None => {
            let saved = configuration::load(&store)?
                .preferences
                .settings_sync_webdav;
            if !saved.is_configured() {
                return Err("settings_sync_not_configured".into());
            }
            Config {
                url: saved.url,
                username: saved.username,
                password: saved.password,
            }
        }
    };
    let service = SettingsWebDav::new(
        &config.url,
        &config.username,
        &config.password,
        path.join("settings-sync"),
        &request.device_name,
    )
    .map_err(|error| error.code().to_owned())?;
    let service = if request.create_identity {
        service
    } else {
        service.existing_identity_only()
    };
    let serialize =
        |value| serde_json::to_value(value).map_err(|_| "settings_sync_protocol".to_owned());
    match request.action {
        Action::Configuration {} | Action::Configure { .. } => unreachable!(),
        Action::Status {} => serialize(service.status().map_err(|error| error.code().to_owned())?),
        Action::ScheduleStatus {} => serde_json::to_value(
            service
                .schedule_status()
                .map_err(|error| error.code().to_owned())?,
        )
        .map_err(|_| "settings_sync_protocol".into()),
        Action::Run {} => serde_json::to_value(
            service
                .run(&PreferencesStore::new(path))
                .map_err(|error| error.code().to_owned())?,
        )
        .map_err(|_| "settings_sync_protocol".into()),
        Action::Scheduled { trigger } => serde_json::to_value(
            service
                .run_scheduled(&PreferencesStore::new(path), trigger.into())
                .map_err(|error| error.code().to_owned())?,
        )
        .map_err(|_| "settings_sync_protocol".into()),
        Action::Devices {} => {
            serde_json::to_value(service.devices().map_err(|error| error.code().to_owned())?)
                .map_err(|_| "settings_sync_protocol".into())
        }
        Action::Approve { device_id } => serde_json::to_value(
            service
                .approve(&device_id)
                .map_err(|error| error.code().to_owned())?,
        )
        .map_err(|_| "settings_sync_protocol".into()),
        Action::Reject { device_id } => serde_json::to_value(
            service
                .reject(&device_id)
                .map_err(|error| error.code().to_owned())?,
        )
        .map_err(|_| "settings_sync_protocol".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn request(directory: &Path, action: Value) -> Vec<u8> {
        serde_json::to_vec(&json!({"preferences_directory": directory, "config": {"url":"https://dav.example.test/dav", "username":"synthetic-user", "password":"synthetic-secret"}, "action": action})).unwrap()
    }

    #[test]
    fn status_and_default_identity_policy_are_offline_without_creating_directories() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("not-created");
        let state = request_json(&request(&directory, json!({"operation":"status"}))).unwrap();
        assert_eq!(state["joined"], false);
        assert_eq!(state["device_id"], Value::Null);
        assert_eq!(state["pending_write"], false);
        assert!(!state.to_string().contains("synthetic-secret"));
        let schedule =
            request_json(&request(&directory, json!({"operation":"schedule_status"}))).unwrap();
        assert_eq!(schedule["pending_changes"], false);
        assert_eq!(schedule["last_success_ms"], Value::Null);
        assert!(!schedule.to_string().contains("synthetic-secret"));
        for trigger in ["local_change", "opened", "periodic", "manual"] {
            assert_eq!(
                request_json(&request(
                    &directory,
                    json!({"operation":"scheduled", "trigger":trigger})
                ))
                .unwrap_err(),
                "clipboard_identity_missing"
            );
        }
        assert_eq!(
            request_json(&request(&directory, json!({"operation":"run"}))).unwrap_err(),
            "clipboard_identity_missing"
        );
        assert!(!directory.exists());
    }

    #[test]
    fn invalid_actions_paths_and_device_ids_are_rejected_before_network_access() {
        let root = tempfile::tempdir().unwrap();
        for action in [
            json!({"operation":"unknown"}),
            json!({"operation":"approve", "device_id":"bad"}),
            json!({"operation":"status", "unused":true}),
            json!({"operation":"scheduled"}),
            json!({"operation":"scheduled", "trigger":"unknown"}),
        ] {
            assert_eq!(
                request_json(&request(root.path(), action)).unwrap_err(),
                INVALID
            );
        }
        assert_eq!(
            request_json(&request(Path::new("relative"), json!({"operation":"run"}))).unwrap_err(),
            INVALID
        );
        assert_eq!(
            request_json(&vec![0; MAX_REQUEST_BYTES + 1]).unwrap_err(),
            INVALID
        );
    }

    #[test]
    fn c_abi_rejects_null_and_oversized_buffers_without_dereferencing() {
        unsafe {
            for (pointer, size) in [
                (std::ptr::null(), 1),
                (std::ptr::dangling(), MAX_REQUEST_BYTES + 1),
            ] {
                let response = crate::msime_client_settings_sync(pointer, size);
                let text = std::ffi::CStr::from_ptr(response)
                    .to_str()
                    .unwrap()
                    .to_owned();
                crate::msime_client_string_free(response);
                assert!(text.contains(INVALID));
            }
        }
    }
}
