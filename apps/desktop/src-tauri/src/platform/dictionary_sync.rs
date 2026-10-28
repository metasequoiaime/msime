//! 「我的词条」经 WebDAV 网盘同步（`docs/dictionary-webdav-sync.md`）的设置应用一侧，macOS 与 Linux 的 Tauri 设置应用用。请求经 `msime_host_api::dictionary_webdav::request_json_announcing`，与 C 入口 `msime_client_dictionary_sync` 是同一个实现；这里只补上 HostOptions 和本机状态的目录，由调用方挪到阻塞线程上。
//!
//! 目录是账号目录下的 `dictionary-sync/`。账号目录是设置应用和输入法共用的那一个：macOS 是 `~/Library/Application Support/<设置应用的 bundle id>`（`macos_launch::native_locator_root`，输入法的 `BackendDesktopSessionFile.standardDirectory` 也是它），Linux 是 IBus、Fcitx5 和设置应用共用的状态目录（`LinuxStateDirectory`，通常是 `~/.config/msime-client`）。不直接用账号目录本身：云剪贴板的中继在那里放着 `cloud-clipboard-relay-keystore` 和 `cloud-clipboard-webdav-state`，词库的密钥串和它的网盘缓存用的是同样的文件名，放在一起会互相覆盖。
//!
//! 同步遇到词库正忙、要请输入法让出时，macOS 与设置页改词一样经分布式通知立刻叫醒 IMK（[`wake_input_method`]）。
//!
//! 什么时候调由共享设置页决定（`packages/ui/src/settings/use-dictionary-sync.ts`）：设置打开时 `opened`，之后按 `next_check_ms` 定时 `periodic`，经设置改了词之后 `local_change`，「立即同步」是 `manual`。

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// 账号目录下放词库同步状态、词库密钥串和锁文件的子目录名。
pub(crate) const DIRECTORY: &str = "dictionary-sync";

/// 本机词库同步状态的目录。
pub(crate) fn directory(account_directory: &Path) -> PathBuf {
    account_directory.join(DIRECTORY)
}

/// 交给 Rust 入口的请求文档 `{options, directory, action}`。页面只给 `action`，HostOptions 和目录都由宿主决定；设备名不给，Rust 用与云剪贴板相同的本机名称。目录不是 UTF-8 时为 `None`。
pub(crate) fn request_document(options: Value, directory: &Path, action: Value) -> Option<Value> {
    Some(json!({
        "options": options,
        "directory": directory.to_str()?,
        "action": action,
    }))
}

/// 头文件列出的错误码和词库密钥串驱动会给的码。`CommandError` 的码是 `'static`，认不出的一律按 `dictionary_sync_unavailable` 交给页面。
const CODES: &[&str] = &[
    "invalid_dictionary_sync",
    "dictionary_sync_not_configured",
    "dictionary_sync_unauthorized",
    "dictionary_sync_unavailable",
    "dictionary_sync_rate_limited",
    "dictionary_sync_too_large",
    "dictionary_sync_rollback",
    "dictionary_sync_unsupported",
    "dictionary_sync_corrupt",
    "dictionary_sync_storage",
    "clipboard_device_pending",
    "clipboard_device_limit",
    "clipboard_key_conflict",
    "clipboard_keystore_conflict",
    "clipboard_identity_missing",
    "not_found",
];

pub(crate) fn error_code(code: &str) -> &'static str {
    CODES
        .iter()
        .find(|known| **known == code)
        .copied()
        .unwrap_or("dictionary_sync_unavailable")
}

/// 同步要请输入法让出词库（手动同步、打开设置和设备操作时词库正忙）的租约一挂上就叫醒输入法：macOS 经分布式通知叫醒 IMK，与设置页改词时 `dictionary_request` 的做法相同，不让它等每秒一次的偏好轮询；Linux 的 IBus、Fcitx5 没有这样的通道，只靠它们的定时器看租约。
fn wake_input_method() {
    #[cfg(target_os = "macos")]
    msime_host_macos::quiesce_input_sessions();
}

/// 一个词库同步动作。除 `status` 外都会联网，只在阻塞线程上调用。
pub(crate) fn request(
    options: Value,
    directory: &Path,
    action: Value,
) -> Result<Value, crate::CommandError> {
    let invalid = crate::CommandError {
        code: "invalid_dictionary_sync",
    };
    let document = request_document(options, directory, action).ok_or(invalid)?;
    let body = serde_json::to_vec(&document).map_err(|_| crate::CommandError {
        code: "invalid_dictionary_sync",
    })?;
    msime_host_api::dictionary_webdav::request_json_announcing(&body, wake_input_method).map_err(
        |code| crate::CommandError {
            code: error_code(&code),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_lives_beside_the_relay_keystore_not_on_top_of_it() {
        let account = Path::new("/synthetic/account");
        let directory = directory(account);
        assert_eq!(directory, Path::new("/synthetic/account/dictionary-sync"));
        // 云剪贴板中继的密钥串在账号目录本身里；词库的同名文件落在子目录里，两份不会是同一个文件。
        assert_ne!(
            directory.join("cloud-clipboard-relay-keystore"),
            account.join("cloud-clipboard-relay-keystore")
        );
    }

    #[test]
    fn the_page_only_supplies_the_action() {
        let document = request_document(
            json!({"api_version": 1}),
            Path::new("/synthetic/account/dictionary-sync"),
            json!({"operation": "status"}),
        )
        .unwrap();
        assert_eq!(
            document,
            json!({
                "options": {"api_version": 1},
                "directory": "/synthetic/account/dictionary-sync",
                "action": {"operation": "status"},
            })
        );
    }

    #[test]
    fn codes_pass_through_and_unknown_ones_become_unavailable() {
        assert_eq!(
            error_code("dictionary_sync_rollback"),
            "dictionary_sync_rollback"
        );
        assert_eq!(error_code("not_found"), "not_found");
        assert_eq!(error_code("something_new"), "dictionary_sync_unavailable");
    }

    #[test]
    fn an_unconfigured_drive_is_reported_without_touching_the_network() {
        let root = tempfile::tempdir().unwrap();
        let preferences = root.path().join("preferences");
        std::fs::create_dir_all(&preferences).unwrap();
        let options = json!({
            "api_version": 1,
            "resources": root.path().join("resources"),
            "user_data": root.path().join("user"),
            "cache": root.path().join("cache"),
            "dictionaries": root.path().join("dictionaries"),
            "preferences": msime_client_core::preferences::Preferences::default(),
            "preferences_directory": preferences,
        });
        let state = directory(root.path());
        let error = request(options.clone(), &state, json!({"operation": "status"})).unwrap_err();
        assert_eq!(error.code, "dictionary_sync_not_configured");
        let error = request(options, &state, json!({"operation": "pull"})).unwrap_err();
        assert_eq!(error.code, "invalid_dictionary_sync");
    }
}
