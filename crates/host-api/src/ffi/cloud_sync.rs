//! `msime_client_cloud_sync`：多分类自动云同步的 C 入口，一个按 `operation` 分发的 JSON 请求。请求、各操作的返回值和错误码见头文件；驱动在 crate 的 `cloud_sync`。
//!
//! 属于 C ABI 的一部分，这些薄封装保证什么见父模块。`run` 和 `commit` 要联网、读写文件，宿主在工作线程调用；`mark_dirty` 只读写状态文件、`dictionary_reset` 只读写状态文件和快照激活队列、`peek` 只读状态文件，键盘扩展和 Android 的 `:ime` 进程也能调；其余操作要会话来源，只有持有登录的进程能调（`status`、`set_enabled` 和 `run` 会按会话对账，看不到登录就清空共享的进度）。

use super::account::session_for;
use super::session_store::{session_source, Source};
use super::with_bounded_bytes;
use crate::cloud_sync::{
    self, AndroidLegacy, Capabilities, HostInputs, HostLocal, RunRequest, INVALID,
};
use crate::*;
use msime_client_core::ios_keyboard::IosKeyboardPreferences;
use msime_client_services::account::cloud_sync::machine::Trigger;
use msime_client_services::account::cloud_sync::policy::{Answer, Section};
use msime_client_services::account::cloud_sync::state::SyncStateStore;
use std::time::{SystemTime, UNIX_EPOCH};

/// 请求的上限：`host_inputs` 里可能有 iOS 最多 800 000 字节的自定义键盘皮肤，或 Android 最多 768 KiB 的皮肤库。
const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_PATH_BYTES: usize = 16 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CloudSyncRequest {
    #[serde(default)]
    directory: Option<String>,
    #[serde(default)]
    legacy_directory: Option<String>,
    #[serde(default)]
    session_store: Option<u64>,
    /// 同步状态放在它下面的 `cloud-sync/`；iOS 是 App Group 的根目录，键盘扩展打标记时也传它。
    state_directory: String,
    /// 共享偏好和常用语所在的偏好目录；`run` 和 `commit` 要带。
    #[serde(default)]
    preferences_directory: Option<String>,
    /// Engine 的宿主选项（与 `msime_client_dictionary` 的 `options` 相同）；同步词库时要带。
    #[serde(default)]
    options: Option<Value>,
    /// 词库快照激活队列的目录（Android 是 `files/bootstrap/state/dictionary-snapshots`）；同步词库时要带，`sign_out` 带上时取消旧账号排队中的快照。
    #[serde(default)]
    snapshot_queue_directory: Option<String>,
    action: Action,
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct HostResults {
    saved_native: Option<IosKeyboardPreferences>,
}

/// 没有参数的操作也写成空的结构体变体：内部标签的单元变体不检查多余的字段。
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Status {},
    Peek {},
    SignOut {},
    SetEnabled {
        enabled: bool,
    },
    MarkDirty {
        section: Section,
    },
    /// 本机清除学习记录以后：取消等激活的云端快照并给词库打待上传标记（`cloud_sync::dictionary_reset`）。
    DictionaryReset {},
    Run {
        trigger: Trigger,
        /// `merge`、`use_cloud`，或者只回答「云端被清空」的 `disable`（要和 `trigger:"choice"` 一起给）。
        #[serde(default)]
        choice: Option<Answer>,
        capabilities: Capabilities,
        #[serde(default)]
        host_inputs: HostInputs,
    },
    Commit {
        run_id: String,
        #[serde(default)]
        host_results: HostResults,
    },
    ImportLegacy {
        #[serde(default)]
        ios_v1: Option<Value>,
        #[serde(default)]
        android: Option<AndroidLegacy>,
    },
}

fn invalid() -> String {
    INVALID.to_owned()
}

fn absolute(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.len() > MAX_PATH_BYTES || !path.is_absolute() {
        return Err(invalid());
    }
    Ok(path.to_path_buf())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

fn perform(request: CloudSyncRequest) -> Result<Value, String> {
    let store = SyncStateStore::new(&absolute(&request.state_directory)?);
    let now = now_ms();
    // 只碰状态文件的几个操作不需要会话，也从不按会话对账：不持有登录的进程只能调它们。
    let action = match request.action {
        Action::Peek {} => return cloud_sync::peek(&store, now),
        Action::MarkDirty { section } => return cloud_sync::mark_dirty(&store, section),
        Action::DictionaryReset {} => {
            let local = HostLocal {
                preferences: PathBuf::new(),
                options: None,
                snapshot_queue: request
                    .snapshot_queue_directory
                    .as_deref()
                    .map(absolute)
                    .transpose()?,
                inputs: HostInputs::default(),
            };
            return cloud_sync::dictionary_reset(&store, &local);
        }
        Action::ImportLegacy { ios_v1, android } => {
            let snapshot_queue = request
                .snapshot_queue_directory
                .as_deref()
                .map(absolute)
                .transpose()?;
            return cloud_sync::import_legacy(
                &store,
                ios_v1,
                android,
                snapshot_queue.as_deref(),
                now,
            );
        }
        action => action,
    };
    let preferences = request
        .preferences_directory
        .as_deref()
        .map(absolute)
        .transpose()?;
    let snapshot_queue = request
        .snapshot_queue_directory
        .as_deref()
        .map(absolute)
        .transpose()?;
    let source = session_source(
        request.directory.as_deref(),
        request.legacy_directory.is_some(),
        request.session_store,
    )
    .ok_or_else(invalid)?;
    let session = match source {
        Source::Directory(directory) => {
            let legacy = request
                .legacy_directory
                .as_deref()
                .map(absolute)
                .transpose()?;
            session_for(&absolute(directory)?, legacy.as_deref())
                .map_err(|error| error.code().to_owned())?
        }
        Source::Store(entry) => Arc::clone(&entry.account),
    };
    let local = |inputs: HostInputs| -> Result<HostLocal, String> {
        Ok(HostLocal {
            preferences: preferences.clone().ok_or_else(invalid)?,
            options: request.options.clone(),
            snapshot_queue: snapshot_queue.clone(),
            inputs,
        })
    };
    match action {
        Action::Status {} => cloud_sync::status(&session, &store, now),
        Action::SignOut {} => {
            let local = HostLocal {
                preferences: preferences.clone().unwrap_or_default(),
                options: None,
                snapshot_queue: snapshot_queue.clone(),
                inputs: HostInputs::default(),
            };
            cloud_sync::sign_out(&store, &local, now)
        }
        Action::SetEnabled { enabled } => cloud_sync::set_enabled(&session, &store, enabled, now),
        Action::Run {
            trigger,
            choice,
            capabilities,
            mut host_inputs,
        } => {
            if trigger == Trigger::Choice && choice.is_none()
                // 「关闭同步」只是对提示的回答，不能跟着回到前台、打开开关或手动同步一起来。
                || choice == Some(Answer::Disable) && trigger != Trigger::Choice
                || capabilities.dictionary
                    && (request.options.is_none() || snapshot_queue.is_none())
                || capabilities.skins_library && !capabilities.settings
                // 鸿蒙的映射没有 iOS 的键盘设置、Android 的本地设置文件和皮肤库这几样。
                || host_inputs.harmony
                    && (host_inputs.ios_native.is_some()
                        || host_inputs.android_local.is_some()
                        || host_inputs.custom_keyboard_skins.is_some()
                        || capabilities.skins_library)
            {
                return Err(invalid());
            }
            // 不联网、不读写本机内容，所以不要 `preferences_directory`；其余字段照样校验，宿主用同一份请求带三种回答。
            if choice == Some(Answer::Disable) {
                return cloud_sync::disable(&session, &store, now);
            }
            let choice = choice.and_then(Answer::choice);
            let settings_locked = host_inputs.settings_locked;
            let excluded = std::mem::take(&mut host_inputs.excluded_phrase_texts);
            let local = local(host_inputs)?;
            cloud_sync::run(
                &session,
                &store,
                &local,
                RunRequest {
                    trigger,
                    choice,
                    capabilities,
                    settings_locked,
                    excluded_phrase_texts: excluded,
                },
                now,
            )
        }
        Action::Commit {
            run_id,
            host_results,
        } => {
            let local = local(HostInputs::default())?;
            cloud_sync::commit(
                &session,
                &store,
                &local,
                &run_id,
                host_results.saved_native.as_ref(),
                now,
            )
        }
        Action::Peek {}
        | Action::MarkDirty { .. }
        | Action::DictionaryReset {}
        | Action::ImportLegacy { .. } => Err(invalid()),
    }
}

/// 多分类自动云同步。请求、各操作的返回值和错误码见头文件。
/// # Safety
/// `request` 指向 `length` 个可读字节；NULL 被拒绝。返回的响应要用 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_cloud_sync(request: *const u8, length: usize) -> *mut c_char {
    response(|| {
        // SAFETY: 由上面的调用方契约保证。
        unsafe {
            with_bounded_bytes(request, length, MAX_REQUEST_BYTES, INVALID, |bytes| {
                let request: CloudSyncRequest =
                    serde_json::from_slice(bytes).map_err(|_| invalid())?;
                perform(request)
            })
        }
    })
}

#[cfg(test)]
mod tests;
