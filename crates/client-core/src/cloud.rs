//! Contracts for the account-backed cloud services.
//!
//! Every module here defines data and validation only; the network call and the
//! credentials that authorise it are injected by the platform host, so nothing
//! in this crate ever opens a socket.

pub mod candidates;
pub mod clipboard_e2e;
pub mod clipboard_log;
pub mod dictionary;
pub mod dictionary_sync;
pub mod learning_e2e;
pub mod snapshot_queue;
pub mod snapshot_validation;
pub mod transport;

/// 端到端加密驱动的 JSON 入口（C ABI `msime_client_cloud_clipboard_e2e`，JNI 和 NAPI 经它调用）。请求顶层可以带 `"domain"`：没有或为 `"clipboard"` 时交给云剪贴板驱动，为 `"learning"` 时交给输入习惯同步驱动（`docs/learning-sync.md`），其他值是错误。不带 `domain` 的请求原样交给云剪贴板驱动，已有宿主的行为逐字节不变。
pub fn e2e_drive_json(input: &[u8]) -> Result<serde_json::Value, String> {
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(input) else {
        return clipboard_e2e::drive_json(input);
    };
    let Some(domain) = value
        .as_object_mut()
        .and_then(|object| object.remove("domain"))
    else {
        return clipboard_e2e::drive_json(input);
    };
    match domain.as_str() {
        Some("clipboard") => clipboard_e2e::drive_value(value),
        Some("learning") => learning_e2e::drive_value(value),
        _ => Err("invalid e2e domain".to_owned()),
    }
}
