//! 云词库面板的 C 入口：账号云词库的列表、目录、增删改、导入导出、候选调频和固定排位，以及完整快照（下载预览、应用到本机、备份与恢复）。分发与 Windows 设置应用相同（[`crate::cloud_dictionary::account`]、[`crate::cloud_dictionary::full_backup`]），会话与 `msime_client_account` 是同一个对象。
//!
//! 整份快照可能有 512 MiB，放不进一次响应，所以 C 上的备份与恢复走宿主给的文件路径（`snapshot_export_file`、`snapshot_restore_preview_file`、`snapshot_restore_file`），文本版的三条报 `snapshot_unavailable`。单个云词库的导出最多 384 MiB，除了文本版的 `export`，也可以用 `export_file` 写进宿主给的路径，中转同样经 `snapshot_directory`。
//!
//! 属于 C ABI 的一部分，这些薄封装保证什么见父模块。

use super::account::session_for;
use super::session_store::{session_source, Source};
use super::with_bounded_bytes;
use crate::cloud_dictionary::full_backup::{self, CloudDictionaryState};
use crate::cloud_dictionary::{account as dictionary_account, CloudDictionaryRequest};
use crate::*;

/// 请求的上限：导入的词表最多 `MAX_IMPORT_BYTES`（1 MiB），加上 HostOptions 文档，4 MiB 足够；目录各最多 16 KiB。
const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_DIRECTORY_BYTES: usize = 16 * 1024;
const INVALID: &str = "invalid_cloud_dictionary";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CloudDictionaryCall {
    /// 共享应用写 `account-session.json` 的目录；与 `session_store` 恰好给一个。
    #[serde(default)]
    directory: Option<String>,
    /// 旧版本放会话的目录，与 `msime_client_account` 相同；只能与 `directory` 一起给。
    #[serde(default)]
    legacy_directory: Option<String>,
    /// `msime_client_session_store_register` 给的 id；与 `directory` 恰好给一个。
    #[serde(default)]
    session_store: Option<u64>,
    /// 本机的 HostOptions 文档，`snapshot_preview`、`snapshot_enqueue`、`snapshot_status`、`snapshot_cancel` 用它找到词库和状态目录。
    #[serde(default)]
    options: Option<Value>,
    /// 本用户私有的临时目录（绝对路径），放预览下载和中转文件；所有 `snapshot_*` 都要带。
    #[serde(default)]
    snapshot_directory: Option<String>,
    action: Action,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Action {
    File(FileSnapshotAction),
    Shared(CloudDictionaryRequest),
}

/// 只有 C 有的四条：快照和单个云词库的导出经宿主给的文件路径进出，不放进请求和响应。
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum FileSnapshotAction {
    #[serde(rename = "snapshot_export_file")]
    Export { path: String },
    /// 一个云词库按 `format` 导出到 `path`（#8201 的 P7），与文本版的 `export` 相同的请求，正文不进响应。
    #[serde(rename = "export_file")]
    DictionaryExport {
        kind: String,
        format: String,
        path: String,
    },
    #[serde(rename = "snapshot_restore_preview_file")]
    RestorePreview { path: String },
    #[serde(rename = "snapshot_restore_file")]
    Restore {
        path: String,
        expected_sha256: String,
        revision: i64,
    },
}

/// 每个临时目录一份预览表，第一次用到时建立（那时清掉上次运行留下的中转文件）。只在取对象时持锁。
static STATES: OnceLock<Mutex<HashMap<PathBuf, CloudDictionaryState>>> = OnceLock::new();

fn state_for(directory: &Path) -> Result<CloudDictionaryState, String> {
    let mut states = STATES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "snapshot_unavailable".to_owned())?;
    Ok(states
        .entry(directory.to_path_buf())
        .or_insert_with(|| CloudDictionaryState::new(directory.to_path_buf()))
        .clone())
}

fn absolute(value: &str, invalid: &'static str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.len() > MAX_DIRECTORY_BYTES || !path.is_absolute() {
        return Err(invalid.to_owned());
    }
    Ok(path.to_path_buf())
}

fn validate_file_action(action: &FileSnapshotAction) -> Result<(), String> {
    let path = match action {
        FileSnapshotAction::Export { path } | FileSnapshotAction::RestorePreview { path } => path,
        FileSnapshotAction::DictionaryExport { kind, format, path } => {
            crate::cloud_dictionary::validate_cloud_request(&CloudDictionaryRequest::Export {
                kind: kind.clone(),
                format: format.clone(),
            })
            .map_err(|_| INVALID.to_owned())?;
            path
        }
        FileSnapshotAction::Restore {
            path,
            expected_sha256,
            revision,
        } => {
            if !crate::valid_sha256(expected_sha256) || *revision < 0 {
                return Err(INVALID.to_owned());
            }
            path
        }
    };
    absolute(path, INVALID).map(|_| ())
}

/// 一次云词库请求。请求、各操作的返回值和错误码见头文件。请求本身的问题（操作、路径、`options`、`snapshot_directory`）都在取会话之前发现，不留下任何文件。
fn run(call: CloudDictionaryCall) -> Result<Value, String> {
    let invalid = || INVALID.to_owned();
    // 走账号分发的、在 C 上不可用的，都不需要临时目录和 HostOptions。
    let mut needs_snapshot_directory = true;
    let mut needs_options = false;
    match &call.action {
        Action::File(action) => validate_file_action(action)?,
        Action::Shared(request) => {
            // 文本版的备份与恢复在 C 上不可用，用带 `_file` 的那三条；参数合不合法都一样，所以在校验之前就答。
            if matches!(
                request,
                CloudDictionaryRequest::SnapshotExport
                    | CloudDictionaryRequest::SnapshotRestorePreview { .. }
                    | CloudDictionaryRequest::SnapshotRestore { .. }
            ) {
                return Err("snapshot_unavailable".to_owned());
            }
            crate::cloud_dictionary::validate_cloud_request(request).map_err(|_| invalid())?;
            if dictionary_account::handled_by_account(request) {
                needs_snapshot_directory = false;
            }
            needs_options = matches!(
                request,
                CloudDictionaryRequest::SnapshotPreview
                    | CloudDictionaryRequest::SnapshotEnqueue { .. }
                    | CloudDictionaryRequest::SnapshotStatus
                    | CloudDictionaryRequest::SnapshotCancel
            );
        }
    }
    let options = match call.options {
        Some(options) if options.is_object() => options,
        Some(_) => return Err(invalid()),
        None if needs_options => return Err(invalid()),
        None => Value::Null,
    };
    let snapshot_directory = match call.snapshot_directory.as_deref() {
        Some(directory) => Some(absolute(directory, INVALID)?),
        None if needs_snapshot_directory => return Err(invalid()),
        None => None,
    };
    let source = session_source(
        call.directory.as_deref(),
        call.legacy_directory.is_some(),
        call.session_store,
    )
    .ok_or_else(invalid)?;
    let session = match source {
        Source::Directory(directory) => {
            let directory = absolute(directory, INVALID)?;
            let legacy = call
                .legacy_directory
                .as_deref()
                .map(|legacy| absolute(legacy, INVALID))
                .transpose()?;
            session_for(&directory, legacy.as_deref()).map_err(|error| error.code().to_owned())?
        }
        Source::Store(entry) => Arc::clone(&entry.account),
    };
    if let Action::Shared(request) = &call.action {
        if let Some(result) = dictionary_account::account_request(session.as_ref(), request) {
            return result.map_err(str::to_owned);
        }
    }
    let state = state_for(&snapshot_directory.ok_or_else(invalid)?)?;
    match call.action {
        Action::File(FileSnapshotAction::Export { path }) => {
            full_backup::export_to_file(session.as_ref(), &state, Path::new(&path))
        }
        Action::File(FileSnapshotAction::DictionaryExport { kind, format, path }) => {
            dictionary_account::dictionary_kind(&kind).and_then(|kind| {
                full_backup::dictionary_export_to_file(
                    session.as_ref(),
                    &state,
                    kind,
                    &format,
                    Path::new(&path),
                )
            })
        }
        Action::File(FileSnapshotAction::RestorePreview { path }) => {
            full_backup::restore_preview_from_file(session.as_ref(), &state, Path::new(&path))
        }
        Action::File(FileSnapshotAction::Restore {
            path,
            expected_sha256,
            revision,
        }) => full_backup::restore_from_file(
            session.as_ref(),
            &state,
            Path::new(&path),
            &expected_sha256,
            revision,
        ),
        Action::Shared(request) => full_backup::request(session.as_ref(), &state, options, request),
    }
    .map_err(str::to_owned)
}

/// 云词库面板的一次请求。请求、各操作的返回值和错误码见头文件。会联网、读写文件，宿主在工作线程调用。
/// # Safety
/// `request` 指向 `length` 个可读的 UTF-8 JSON 字节；NULL 被拒绝。返回的响应要用 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_cloud_dictionary(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: 由调用方契约保证。
        let call: CloudDictionaryCall = unsafe {
            with_bounded_bytes(request, length, MAX_REQUEST_BYTES, INVALID, |bytes| {
                serde_json::from_slice(bytes).map_err(|_| INVALID.to_owned())
            })?
        };
        run(call)
    })
}

#[cfg(test)]
mod export_file_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn call(request: Value) -> Value {
        let bytes = request.to_string();
        // SAFETY: 缓冲区在本次调用期间有效，返回的字符串在下面释放。
        unsafe {
            let raw = msime_client_cloud_dictionary(bytes.as_ptr(), bytes.len());
            let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
            msime_client_string_free(raw);
            serde_json::from_str(&text).unwrap()
        }
    }

    fn error(value: Value) -> String {
        assert_eq!(value["ok"], false, "{value}");
        value["error"].as_str().unwrap().to_owned()
    }

    #[test]
    fn malformed_requests_are_rejected_before_any_session_is_used() {
        let directory = tempfile::tempdir().unwrap();
        let account = directory.path().join("account");
        let snapshots = directory.path().join("snapshots");
        for request in [
            json!({ "action": { "operation": "list", "kind": "pinyin", "offset": 0, "search": "" } }),
            json!({ "directory": "relative", "action": { "operation": "snapshot_status" } }),
            json!({ "directory": account, "session_store": 1, "action": { "operation": "snapshot_status" } }),
            json!({ "session_store": 1, "legacy_directory": account, "action": { "operation": "snapshot_status" } }),
            json!({ "session_store": u64::MAX, "action": { "operation": "snapshot_status" } }),
            json!({ "directory": account, "action": { "operation": "list", "kind": "klingon", "offset": 0, "search": "" } }),
            json!({ "directory": account, "action": { "operation": "snapshot_unknown" } }),
            json!({ "directory": account, "unknown": 1, "action": { "operation": "snapshot_status" } }),
            json!({ "directory": account, "snapshot_directory": snapshots, "action": { "operation": "snapshot_export_file", "path": "relative.ndjson" } }),
            json!({ "directory": account, "snapshot_directory": snapshots, "action": { "operation": "snapshot_restore_file", "path": "/backup.ndjson", "expected_sha256": "bad", "revision": 1 } }),
            json!({ "directory": account, "snapshot_directory": snapshots, "action": { "operation": "snapshot_export_file", "path": "/backup.ndjson", "extra": true } }),
            // 快照操作没有临时目录，或临时目录不是绝对路径。
            json!({ "directory": account, "options": {}, "action": { "operation": "snapshot_status" } }),
            json!({ "directory": account, "options": {}, "snapshot_directory": "snapshots", "action": { "operation": "snapshot_status" } }),
            // 要找本机词库的四条没有 HostOptions，或它不是对象。
            json!({ "directory": account, "snapshot_directory": snapshots, "action": { "operation": "snapshot_status" } }),
            json!({ "directory": account, "snapshot_directory": snapshots, "options": [], "action": { "operation": "snapshot_cancel" } }),
        ] {
            assert_eq!(error(call(request.clone())), INVALID, "{request}");
        }
        assert!(!snapshots.exists());
    }

    #[test]
    fn whole_snapshot_text_does_not_cross_the_c_boundary() {
        let directory = tempfile::tempdir().unwrap();
        let account = directory.path().join("account");
        let snapshots = directory.path().join("snapshots");
        for action in [
            json!({ "operation": "snapshot_export" }),
            json!({ "operation": "snapshot_restore_preview", "text": "{\"type\":\"header\"}\n" }),
            json!({ "operation": "snapshot_restore", "text": "{\"type\":\"header\"}\n", "expected_sha256": "a".repeat(64), "revision": 0 }),
            // 参数不合法时也一样：这三条在 C 上本来就不可用。
            json!({ "operation": "snapshot_restore_preview", "text": "" }),
            json!({ "operation": "snapshot_restore", "text": "\u{0}", "expected_sha256": "x", "revision": -1 }),
        ] {
            assert_eq!(
                error(call(
                    json!({ "directory": account, "snapshot_directory": snapshots, "action": action })
                )),
                "snapshot_unavailable",
                "{action}"
            );
        }
        // 宿主选文件、保存文件的那一套是 macOS 的，与 Windows 设置应用一样不可用。
        for action in [
            json!({ "operation": "snapshot_choose_restore" }),
            json!({ "operation": "snapshot_restore_cancel" }),
            json!({ "operation": "snapshot_restore_native", "token": "native-token" }),
        ] {
            assert_eq!(
                error(call(
                    json!({ "directory": account, "snapshot_directory": snapshots, "action": action })
                )),
                "snapshot_unavailable",
                "{action}"
            );
        }
    }

    #[test]
    fn signed_out_account_operations_report_the_account_code() {
        let directory = tempfile::tempdir().unwrap();
        let account = directory.path().join("account");
        std::fs::create_dir_all(&account).unwrap();
        let signed_out = call(json!({
            "directory": account,
            "action": { "operation": "list", "kind": "pinyin", "offset": 0, "search": "" },
        }));
        assert_eq!(error(signed_out), "account_unauthorized");
        // 没登录时取消排队的快照也报未登录，不碰队列；临时目录照样建好。
        let snapshots = directory.path().join("snapshots");
        let cancelled = call(json!({
            "directory": account,
            "snapshot_directory": snapshots,
            "options": { "user_data": directory.path().join("state").join("user") },
            "action": { "operation": "snapshot_cancel" },
        }));
        assert_eq!(error(cancelled), "account_unauthorized");
        assert!(snapshots.is_dir());
        // 没登录时导出备份失败，宿主选的位置上已有的那份备份原样保留。
        let backup = directory.path().join("msime-dictionary-snapshot.ndjson");
        std::fs::write(&backup, "上一份合成备份\n").unwrap();
        let exported = call(json!({
            "directory": account,
            "snapshot_directory": snapshots,
            "action": { "operation": "snapshot_export_file", "path": backup },
        }));
        assert_eq!(error(exported), "account_unauthorized");
        assert_eq!(
            std::fs::read_to_string(&backup).unwrap(),
            "上一份合成备份\n"
        );
    }
}
