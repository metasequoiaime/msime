//! 鸿蒙的会话文件（#8201 的 P1、P2）：ArkTS 写的文件带属组位，登录会话还是扁平布局。

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

const ACCESS: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const REFRESH: &str = "2222222222222222222222222222222222222222222222222222222222222222";

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// `AccountCloudBridge.ts` 保存的扁平会话，`expires_at` 是 Unix 毫秒。
fn flat_session(expires_at: u64) -> String {
    format!(
        r#"{{"access_token":"{ACCESS}","refresh_token":"{REFRESH}","token_type":"Bearer","expires_at":{expires_at},"user":{{"id":"synthetic-harmony","display_name":"合成用户","created_at":"2026-01-01T00:00:00Z"}}}}"#
    )
}

#[cfg(unix)]
#[test]
fn harmony_reads_group_writable_session_and_keystore_files_and_other_hosts_do_not() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let storage = FileAccountSessionStorage::new(root.path(), AccountSessionFileLayout::Native);
    storage.create_directory().unwrap();
    let session = flat_session(now_ms() + 600_000);
    for (name, contents) in [
        (ACCOUNT_SESSION_FILE, session.as_bytes()),
        (CLOUD_CLIPBOARD_KEYSTORE_FILE, b"v1.synthetic".as_slice()),
    ] {
        let path = root.path().join(name);
        std::fs::write(&path, contents).unwrap();
        // ArkTS 没有 chmod，按应用 umask 写出 0660：鸿蒙上照样读得出，其他平台仍然拒绝。
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660)).unwrap();
        assert_eq!(
            storage.read_private_with(name, MAX_SESSION_BYTES, true),
            Ok(Some(contents.to_vec())),
            "{name}"
        );
        assert_eq!(
            storage.read_private_with(name, MAX_SESSION_BYTES, false),
            Err(AccountError::Storage),
            "{name}"
        );
        // 其他用户可读的文件在哪里都不认。
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o604)).unwrap();
        for ohos in [true, false] {
            assert_eq!(
                storage.read_private_with(name, MAX_SESSION_BYTES, ohos),
                Err(AccountError::Storage),
                "{name} ohos={ohos}"
            );
        }
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        for ohos in [true, false] {
            assert_eq!(
                storage.read_private_with(name, MAX_SESSION_BYTES, ohos),
                Ok(Some(contents.to_vec())),
                "{name} ohos={ohos}"
            );
        }
    }
}

#[test]
fn the_flat_harmony_layout_decodes_with_the_remaining_lifetime_as_expires_in() {
    let now = 1_800_000_000_000;
    let saved = decode_at(flat_session(now + 899_600).as_bytes(), now).unwrap();
    assert_eq!(saved.tokens.access_token, ACCESS);
    assert_eq!(saved.tokens.refresh_token, REFRESH);
    assert_eq!(saved.tokens.token_type, "Bearer");
    assert_eq!(saved.tokens.user.id, "synthetic-harmony");
    assert_eq!(saved.tokens.user.display_name, "合成用户");
    assert_eq!(saved.expires_at_unix_ms, now + 899_600);
    assert_eq!(saved.session_id, None);
    // 与 `anonymousSessionForStorage` 相同：剩余毫秒数除以 1000 四舍五入。
    assert_eq!(saved.tokens.expires_in, 900);
    assert_eq!(
        decode_at(flat_session(now + 899_400).as_bytes(), now)
            .unwrap()
            .tokens
            .expires_in,
        899
    );
    // 已经过期或只剩不到半秒时至少是 1，否则 `validate_tokens` 会把整个会话当成坏的。
    for expires_at in [now - 3_600_000, now, now + 400] {
        let saved = decode_at(flat_session(expires_at).as_bytes(), now).unwrap();
        assert_eq!(saved.tokens.expires_in, 1);
        assert_eq!(saved.expires_at_unix_ms, expires_at);
        super::super::validate::validate_tokens(&saved.tokens).unwrap();
    }
    // 少了字段的扁平文档不是会话。
    assert_eq!(
        decode_at(
            format!(r#"{{"access_token":"{ACCESS}","expires_at":1}}"#).as_bytes(),
            now
        )
        .err(),
        Some(AccountError::Storage)
    );
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
    let root = tempfile::tempdir().unwrap();
    let storage = FileAccountSessionStorage::new(root.path(), AccountSessionFileLayout::Native);
    storage.create_directory().unwrap();
    std::fs::write(
        root.path().join(ACCOUNT_SESSION_FILE),
        flat_session(now_ms() - 3_600_000),
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
    assert_eq!(*api.presented.lock().unwrap(), [REFRESH]);

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
    std::fs::write(&path, flat_session(now_ms() + 600_000)).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let saved = read_published_session(&path).unwrap().unwrap();
    assert_eq!(saved.tokens.refresh_token, REFRESH);
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
