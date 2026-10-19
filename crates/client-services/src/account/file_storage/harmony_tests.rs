//! 鸿蒙的会话文件（#8201 的 P1、P2）：ArkTS 写的文件带属组位，登录会话还是扁平布局。扁平布局的字节和读出的结果在共享用例 `shared/contracts/account-session-layouts/cases.json` 里（`harmony_flat`、`harmony_flat_expired`），解码规则由 `layout_cases_tests` 逐条核对。

use super::layout_cases_tests::{case_document, layout_case};
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(unix)]
#[test]
fn harmony_reads_group_writable_session_and_keystore_files_and_other_hosts_do_not() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let storage = FileAccountSessionStorage::new(root.path(), AccountSessionFileLayout::Native);
    storage.create_directory().unwrap();
    let session = case_document(&layout_case("harmony_flat"));
    for (name, contents) in [
        (ACCOUNT_SESSION_FILE, session.as_slice()),
        (CLOUD_CLIPBOARD_KEYSTORE_FILE, b"v1.synthetic".as_slice()),
    ] {
        let path = root.path().join(name);
        std::fs::write(&path, contents).unwrap();
        // ArkTS 没有 chmod，按应用 umask 写出 0660：鸿蒙上照样读得出，其他平台仍然拒绝。
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660)).unwrap();
        assert_eq!(
            storage.read_private_with(name, MAX_SESSION_BYTES, true, false),
            Ok(Some(contents.to_vec())),
            "{name}"
        );
        assert_eq!(
            storage.read_private_with(name, MAX_SESSION_BYTES, false, false),
            Err(AccountError::Storage),
            "{name}"
        );
        // 其他用户可读的文件在哪里都不认。
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o604)).unwrap();
        for ohos in [true, false] {
            assert_eq!(
                storage.read_private_with(name, MAX_SESSION_BYTES, ohos, false),
                Err(AccountError::Storage),
                "{name} ohos={ohos}"
            );
        }
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        for ohos in [true, false] {
            assert_eq!(
                storage.read_private_with(name, MAX_SESSION_BYTES, ohos, false),
                Ok(Some(contents.to_vec())),
                "{name} ohos={ohos}"
            );
        }
    }
}

/// 刷新时轮换令牌，记下收到的刷新令牌。
#[derive(Clone, Default)]
struct RefreshingApi {
    refreshes: Arc<AtomicUsize>,
    presented: Arc<Mutex<Vec<String>>>,
}

impl AccountApi for RefreshingApi {
    fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn refresh(&self, refresh_token: &str) -> Result<AccountTokens, AccountError> {
        self.refreshes.fetch_add(1, Ordering::SeqCst);
        self.presented
            .lock()
            .unwrap()
            .push(refresh_token.to_owned());
        Ok(AccountTokens {
            access_token: "3".repeat(64),
            refresh_token: "4".repeat(64),
            token_type: "Bearer".into(),
            expires_in: 900,
            user: AccountUser {
                id: "synthetic-harmony".into(),
                display_name: "合成用户".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                email: None,
                avatar_url: None,
            },
        })
    }
    fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
    fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
    fn delete_account(&self, _: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
}

#[test]
fn an_upgraded_flat_session_with_an_expired_access_token_refreshes_and_is_saved_native() {
    // 升级时多数用户的状态：ArkTS 存的扁平会话，访问令牌早就过期了，刷新令牌还有效。
    let expired = layout_case("harmony_flat_expired");
    let root = tempfile::tempdir().unwrap();
    let storage = FileAccountSessionStorage::new(root.path(), AccountSessionFileLayout::Native);
    storage.create_directory().unwrap();
    std::fs::write(
        root.path().join(ACCOUNT_SESSION_FILE),
        case_document(&expired),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            root.path().join(ACCOUNT_SESSION_FILE),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    let api = RefreshingApi::default();
    let session = BackendAccountSession::new(api.clone(), storage);
    assert_eq!(session.status().unwrap().unwrap().id, "synthetic-harmony");

    let (user, token) = session.credentials(None, None).unwrap();
    assert_eq!(user, "synthetic-harmony");
    assert_eq!(token, "3".repeat(64));
    assert_eq!(
        *api.presented.lock().unwrap(),
        [expired["session"]["tokens"]["refresh_token"]
            .as_str()
            .unwrap()]
    );

    // 写回的是 Native 布局，带上了补出的登录 ID，再读一次不用再刷新。
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.path().join(ACCOUNT_SESSION_FILE)).unwrap())
            .unwrap();
    assert!(written.get("access_token").is_none());
    assert!(written.get("expires_at").is_none());
    assert_eq!(written["tokens"]["refresh_token"], "4".repeat(64));
    assert_eq!(written["tokens"]["expires_in"], 900);
    assert!(written["expires_at_unix_ms"].as_u64().unwrap() > now_ms());
    assert!(written["session_id"].as_str().is_some());
    assert_eq!(session.credentials(None, None).unwrap().1, "3".repeat(64));
    assert_eq!(api.refreshes.load(Ordering::SeqCst), 1);
}

#[test]
fn a_published_session_is_read_with_the_same_rules_in_any_layout() {
    let root = tempfile::tempdir().unwrap();
    let published = root.path().join("group").join("settings");
    // 发布方的目录还不存在时与文件不存在相同。
    assert!(
        read_published_session(&published.join(ACCOUNT_SESSION_FILE))
            .unwrap()
            .is_none()
    );
    FileAccountSessionStorage::new(&published, AccountSessionFileLayout::Native)
        .create_directory()
        .unwrap();
    let path = published.join(ACCOUNT_SESSION_FILE);
    assert!(read_published_session(&path).unwrap().is_none());
    let flat = layout_case("harmony_flat");
    std::fs::write(&path, case_document(&flat)).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let saved = read_published_session(&path).unwrap().unwrap();
    assert_eq!(
        saved.tokens.refresh_token,
        flat["session"]["tokens"]["refresh_token"].as_str().unwrap()
    );
    std::fs::write(&path, b"{}").unwrap();
    assert_eq!(
        read_published_session(&path).err(),
        Some(AccountError::Storage)
    );
    #[cfg(unix)]
    {
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(root.path().join("elsewhere"), &path).unwrap();
        assert_eq!(
            read_published_session(&path).err(),
            Some(AccountError::Storage)
        );
    }
}

/// 鸿蒙上发布文件的属主规则：属主是本进程的有效 uid 就认，不看所在目录的属主；其他平台仍要求与目录同一个属主。data-group 共享目录的属主在真机上没有核对过，这里用 root 的 `/tmp`（macOS 上是 `/private/tmp`）造出「目录不是自己的、文件是自己写的」这种发布目录。以 root 运行时两者相同，造不出来，什么也不查；没有 `/tmp` 的系统（Android 15 以前，API 28 的设备单测就是这样）同样造不出来，也什么都不查。
#[cfg(unix)]
#[test]
fn harmony_reads_a_published_session_it_owns_in_a_directory_owned_by_someone_else() {
    use std::io::Write as _;
    use std::os::unix::fs::MetadataExt;

    let shared = if cfg!(target_vendor = "apple") {
        Path::new("/private/tmp")
    } else {
        Path::new("/tmp")
    };
    if !shared.is_dir() {
        return;
    }
    let mut file = tempfile::Builder::new()
        .prefix("msime-synthetic-published-")
        .tempfile_in(shared)
        .unwrap();
    if file.as_file().metadata().unwrap().uid() == std::fs::metadata(shared).unwrap().uid() {
        return;
    }
    let flat = layout_case("harmony_flat");
    file.write_all(&case_document(&flat)).unwrap();
    file.as_file().sync_all().unwrap();
    let saved = read_published_session_with(file.path(), true)
        .unwrap()
        .unwrap();
    assert_eq!(
        saved.tokens.refresh_token,
        flat["session"]["tokens"]["refresh_token"].as_str().unwrap()
    );
    // 其他平台的规则不变：文件属主与所在目录不同就拒收。
    assert_eq!(
        read_published_session_with(file.path(), false).err(),
        Some(AccountError::Storage)
    );
    // 其他用户位在鸿蒙上照样拒收。
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o604)).unwrap();
    assert_eq!(
        read_published_session_with(file.path(), true).err(),
        Some(AccountError::Storage)
    );
}
