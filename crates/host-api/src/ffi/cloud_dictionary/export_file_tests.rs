//! `export_file`（#8201 的 P7）：单个云词库按格式导出到宿主给的路径，预检、中转和清理与 `snapshot_export_file` 相同。

use super::*;
use msime_client_core::cloud::dictionary::DictionaryKind;
use msime_client_services::account::{
    AccountApi, AccountChallenge, AccountError, AccountProfile, AccountSessionStorage,
    AccountTokens, AccountUser, BackendAccountSession, SavedAccountSession,
};
use std::ffi::CStr;

const INVALID_CODE: &str = "invalid_cloud_dictionary";

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

fn names(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn malformed_export_file_requests_are_rejected_before_any_session_is_used() {
    let directory = tempfile::tempdir().unwrap();
    let account = directory.path().join("account");
    let snapshots = directory.path().join("snapshots");
    let path = directory.path().join("dictionary-pinyin.tsv");
    let action = |kind: &str, format: &str, path: Value| json!({ "operation": "export_file", "kind": kind, "format": format, "path": path });
    for request in [
        json!({ "directory": account, "snapshot_directory": snapshots, "action": action("klingon", "standard", json!(path)) }),
        json!({ "directory": account, "snapshot_directory": snapshots, "action": action("pinyin", "csv", json!(path)) }),
        json!({ "directory": account, "snapshot_directory": snapshots, "action": action("pinyin", "standard", json!("relative.tsv")) }),
        json!({ "directory": account, "snapshot_directory": snapshots, "action": { "operation": "export_file", "kind": "pinyin", "format": "standard" } }),
        json!({ "directory": account, "snapshot_directory": snapshots, "action": { "operation": "export_file", "kind": "pinyin", "format": "standard", "path": path, "extra": true } }),
        // 与 snapshot_export_file 一样要临时目录。
        json!({ "directory": account, "action": action("pinyin", "standard", json!(path)) }),
    ] {
        assert_eq!(error(call(request.clone())), INVALID_CODE, "{request}");
    }
    assert!(!snapshots.exists());
    assert!(!path.exists());
}

#[test]
fn a_signed_out_export_leaves_the_chosen_file_and_no_scratch_file() {
    let directory = tempfile::tempdir().unwrap();
    let account = directory.path().join("account");
    std::fs::create_dir_all(&account).unwrap();
    let snapshots = directory.path().join("snapshots");
    let path = directory.path().join("dictionary-wubi.tsv");
    std::fs::write(&path, "上一份合成导出\n").unwrap();
    let exported = call(json!({
        "directory": account,
        "snapshot_directory": snapshots,
        "action": { "operation": "export_file", "kind": "wubi", "format": "windows", "path": path },
    }));
    assert_eq!(error(exported), "account_unauthorized");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "上一份合成导出\n");
    assert!(names(&snapshots).is_empty());
    // 保存位置的上级目录不存在时在取会话、下载之前就拒绝。
    let missing = directory.path().join("missing").join("dictionary-wubi.tsv");
    let refused = call(json!({
        "directory": account,
        "snapshot_directory": snapshots,
        "action": { "operation": "export_file", "kind": "wubi", "format": "windows", "path": missing },
    }));
    assert_eq!(error(refused), "account_invalid");
}

const ACCESS_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// 只答导出的后端：把 `body` 写进给它的路径；`fail` 时写一半再报 `Unavailable`，模拟下载到一半断开。记下收到的类别、格式和写入的路径。
struct ExportApi {
    body: &'static str,
    fail: bool,
    calls: Calls,
}

type Calls = Arc<Mutex<Vec<(DictionaryKind, String, PathBuf)>>>;

impl AccountApi for ExportApi {
    fn providers(&self) -> Result<HashMap<String, bool>, AccountError> {
        unreachable!("providers")
    }
    fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
        unreachable!("challenge")
    }
    fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
        unreachable!("login")
    }
    fn refresh(&self, _: &str) -> Result<AccountTokens, AccountError> {
        unreachable!("refresh")
    }
    fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
        unreachable!("profile")
    }
    fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
        unreachable!("rename")
    }
    fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
        unreachable!("logout")
    }
    fn delete_account(&self, _: &str) -> Result<(), AccountError> {
        unreachable!("delete_account")
    }
    fn export_dictionary_to_file(
        &self,
        kind: DictionaryKind,
        format: &str,
        destination: &Path,
        access_token: &str,
    ) -> Result<u64, AccountError> {
        assert_eq!(access_token, ACCESS_TOKEN);
        self.calls
            .lock()
            .unwrap()
            .push((kind, format.to_owned(), destination.to_path_buf()));
        if self.fail {
            std::fs::write(destination, &self.body[..3]).unwrap();
            return Err(AccountError::Unavailable);
        }
        std::fs::write(destination, self.body).unwrap();
        Ok(self.body.len() as u64)
    }
}

struct SignedIn;

impl AccountSessionStorage for SignedIn {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        Ok(Some(SavedAccountSession {
            tokens: AccountTokens {
                access_token: ACCESS_TOKEN.into(),
                refresh_token: "b".repeat(64),
                token_type: "Bearer".into(),
                expires_in: 3600,
                user: AccountUser {
                    id: "synthetic-user".into(),
                    display_name: "合成用户".into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    email: None,
                    avatar_url: None,
                },
            },
            expires_at_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                + 3_600_000,
            session_id: Some(msime_client_core::uuid::Uuid::new_v4()),
        }))
    }
    fn save(&self, _: &SavedAccountSession) -> Result<(), AccountError> {
        Ok(())
    }
    fn clear(&self) -> Result<(), AccountError> {
        Ok(())
    }
}

fn session(fail: bool) -> (BackendAccountSession<ExportApi, SignedIn>, Calls) {
    let calls = Calls::default();
    let session = BackendAccountSession::new(
        ExportApi {
            body: "ni\t你\t10000\n",
            fail,
            calls: Arc::clone(&calls),
        },
        SignedIn,
    );
    (session, calls)
}

#[test]
fn the_export_goes_through_a_scratch_file_into_the_chosen_path() {
    let directory = tempfile::tempdir().unwrap();
    let scratch = directory.path().join("snapshots");
    let state = CloudDictionaryState::new(scratch.clone());
    let saved = directory.path().join("saved");
    std::fs::create_dir(&saved).unwrap();
    let destination = saved.join("我的五笔.tsv");
    let (session, calls) = session(false);
    let value = full_backup::dictionary_export_to_file(
        &session,
        &state,
        DictionaryKind::Wubi98,
        "windows",
        &destination,
    )
    .unwrap();
    assert_eq!(
        value,
        json!({ "path": destination, "filename": "dictionary-wubi98.tsv", "bytes": "ni\t你\t10000\n".len() })
    );
    assert_eq!(
        std::fs::read_to_string(&destination).unwrap(),
        "ni\t你\t10000\n"
    );
    // 下载先进临时目录，换入以后删掉；保存位置旁边也不留临时文件。
    let calls = calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        (calls[0].0, calls[0].1.as_str()),
        (DictionaryKind::Wubi98, "windows")
    );
    assert_eq!(calls[0].2.parent(), Some(scratch.as_path()));
    let scratch_name = calls[0]
        .2
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(
        scratch_name.starts_with("export-") && scratch_name.ends_with(".tsv"),
        "{scratch_name}"
    );
    assert!(names(&scratch).is_empty());
    assert_eq!(names(&saved), ["我的五笔.tsv"]);
}

#[test]
fn a_failed_download_keeps_the_previous_file_and_removes_the_scratch_file() {
    let directory = tempfile::tempdir().unwrap();
    let scratch = directory.path().join("snapshots");
    let state = CloudDictionaryState::new(scratch.clone());
    let destination = directory.path().join("dictionary-pinyin.tsv");
    std::fs::write(&destination, "上一份合成导出\n").unwrap();
    assert_eq!(
        full_backup::dictionary_export_to_file(
            &session(true).0,
            &state,
            DictionaryKind::Pinyin,
            "standard",
            &destination,
        ),
        Err("account_unavailable")
    );
    assert_eq!(
        std::fs::read_to_string(&destination).unwrap(),
        "上一份合成导出\n"
    );
    assert!(names(&scratch).is_empty());
}

#[test]
fn scratch_exports_left_by_a_killed_process_are_cleaned_up() {
    let directory = tempfile::tempdir().unwrap();
    let scratch = directory.path().join("snapshots");
    std::fs::create_dir(&scratch).unwrap();
    for name in ["export-1.tsv", "export-2.ndjson", "keep.tsv"] {
        std::fs::write(scratch.join(name), "合成\n").unwrap();
    }
    let _state = CloudDictionaryState::new(scratch.clone());
    assert_eq!(names(&scratch), ["keep.tsv"]);
}
