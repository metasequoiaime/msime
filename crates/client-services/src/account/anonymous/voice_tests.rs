//! `delete_anonymous_voice_samples`：先用现有的匿名会话删，服务端要求最近登录或令牌已失效时用保存的身份重新登录再删；本机没有匿名身份时什么也不做。不联网。

use super::super::host_storage::tests::MemoryBackend;
use super::*;

/// 登录第 n 次给出令牌 `logins[n]`；`data_delete` 对 `recent` 里的令牌回 403 `recent_login_required`，对 `failing` 里的回 503。每次调用记下 `<操作> <令牌首字符>`。
#[derive(Clone)]
struct FakeApi {
    calls: Arc<Mutex<Vec<String>>>,
    logins: Arc<Mutex<Vec<char>>>,
    recent: Arc<Mutex<Vec<String>>>,
    failing: Arc<Mutex<Vec<String>>>,
}

impl FakeApi {
    fn new(logins: &[char]) -> Self {
        Self {
            calls: Arc::default(),
            logins: Arc::new(Mutex::new(logins.iter().rev().copied().collect())),
            recent: Arc::default(),
            failing: Arc::default(),
        }
    }

    fn calls(&self) -> Vec<String> {
        std::mem::take(&mut *self.calls.lock().unwrap())
    }
}

fn token(character: char) -> String {
    character.to_string().repeat(64)
}

fn tokens(access: char) -> AccountTokens {
    AccountTokens {
        access_token: token(access),
        refresh_token: token('f'),
        token_type: "Bearer".into(),
        expires_in: 3600,
        user: AccountUser {
            id: "fixture-anonymous".into(),
            display_name: String::new(),
            created_at: "2026-01-01T00:00:00Z".into(),
            email: None,
            avatar_url: None,
        },
    }
}

impl AccountApi for FakeApi {
    fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
        unreachable!("providers")
    }
    fn challenge(&self, provider: &str, target: &str) -> Result<AccountChallenge, AccountError> {
        assert_eq!(provider, "anonymous");
        assert!(valid_anonymous_subject(target));
        self.calls.lock().unwrap().push("challenge".into());
        Ok(AccountChallenge {
            challenge_id: "fixture-challenge".into(),
            expires_in: 300,
            nonce: None,
            authorization_url: None,
        })
    }
    fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
        let access = self.logins.lock().unwrap().pop().expect("unexpected login");
        self.calls.lock().unwrap().push(format!("login {access}"));
        Ok(tokens(access))
    }
    fn refresh(&self, _: &str) -> Result<AccountTokens, AccountError> {
        self.calls.lock().unwrap().push("refresh".into());
        Err(AccountError::Unauthorized)
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
    fn data_delete(&self, sections: &[String], access_token: &str) -> Result<(), AccountFailure> {
        assert_eq!(sections, ["voice"]);
        self.calls.lock().unwrap().push(format!(
            "data_delete {}",
            access_token.chars().next().unwrap()
        ));
        let listed =
            |list: &Mutex<Vec<String>>| list.lock().unwrap().iter().any(|t| t == access_token);
        if listed(&self.recent) {
            return Err(AccountFailure::from_http_response(
                403,
                br#"{"error":{"code":"recent_login_required","message":"recent_login_required"}}"#,
                None,
            ));
        }
        if listed(&self.failing) {
            return Err(AccountFailure::from_http_response(
                503,
                br#"{"error":{"code":"auth_unavailable","message":"auth_unavailable"}}"#,
                None,
            ));
        }
        Ok(())
    }
}

/// 注入存储上的匿名会话：先按 `ensure_anonymous_account_in` 注册一次（身份和会话都在 `backend` 里），令牌是 `first`。
fn registered(
    backend: &Arc<MemoryBackend>,
    api: &FakeApi,
) -> BackendAccountSession<FakeApi, HostSessionStorage> {
    let dynamic: Arc<dyn SessionStoreBackend> = backend.clone();
    let anonymous = BackendAccountSession::new(
        api.clone(),
        HostSessionStorage::new(
            dynamic,
            HostSessionSlot::Anonymous,
            Arc::new(HostStoreLocks::new()),
        ),
    );
    ensure_anonymous_account_in(&anonymous, backend.as_ref()).unwrap();
    api.calls();
    anonymous
}

#[test]
fn a_fresh_anonymous_session_deletes_directly() {
    let backend = Arc::new(MemoryBackend::default());
    let api = FakeApi::new(&['1']);
    let anonymous = registered(&backend, &api);
    delete_anonymous_voice_samples(&anonymous, AnonymousIdentitySource::Store(backend.as_ref()))
        .unwrap();
    assert_eq!(api.calls(), ["data_delete 1"]);
}

#[test]
fn an_old_anonymous_session_signs_in_again_with_the_saved_identity() {
    let backend = Arc::new(MemoryBackend::default());
    let api = FakeApi::new(&['1', '5']);
    let anonymous = registered(&backend, &api);
    api.recent.lock().unwrap().push(token('1'));
    delete_anonymous_voice_samples(&anonymous, AnonymousIdentitySource::Store(backend.as_ref()))
        .unwrap();
    assert_eq!(
        api.calls(),
        ["data_delete 1", "challenge", "login 5", "data_delete 5"]
    );
    // 新会话替换了存储里的旧会话。
    assert_eq!(
        anonymous.credentials(None, None).unwrap(),
        ("fixture-anonymous".to_owned(), token('5'))
    );
}

#[test]
fn a_revoked_anonymous_session_signs_in_again() {
    let backend = Arc::new(MemoryBackend::default());
    let api = FakeApi::new(&['1', '5']);
    drop(registered(&backend, &api));
    // 存储里换成一个已过期的会话，刷新被服务端拒绝：会话清掉以后用身份重新登录。
    backend.items.lock().unwrap().insert(
        SessionStoreItem::Anonymous,
        serde_json::to_vec(&SavedAccountSession {
            tokens: tokens('9'),
            expires_at_unix_ms: 1,
            session_id: Some(uuid::Uuid::new_v4()),
        })
        .unwrap(),
    );
    let dynamic: Arc<dyn SessionStoreBackend> = backend.clone();
    let anonymous = BackendAccountSession::new(
        api.clone(),
        HostSessionStorage::new(
            dynamic,
            HostSessionSlot::Anonymous,
            Arc::new(HostStoreLocks::new()),
        ),
    );
    delete_anonymous_voice_samples(&anonymous, AnonymousIdentitySource::Store(backend.as_ref()))
        .unwrap();
    assert_eq!(
        api.calls(),
        ["refresh", "challenge", "login 5", "data_delete 5"]
    );
}

#[test]
fn other_failures_are_returned_without_signing_in_again() {
    let backend = Arc::new(MemoryBackend::default());
    let api = FakeApi::new(&['1']);
    let anonymous = registered(&backend, &api);
    api.failing.lock().unwrap().push(token('1'));
    let failure = delete_anonymous_voice_samples(
        &anonymous,
        AnonymousIdentitySource::Store(backend.as_ref()),
    )
    .unwrap_err();
    assert_eq!(failure.code(), "account_unavailable");
    assert_eq!(api.calls(), ["data_delete 1"]);
}

#[test]
fn a_session_without_its_identity_reports_the_recent_login_failure() {
    let backend = Arc::new(MemoryBackend::default());
    let api = FakeApi::new(&['1']);
    let anonymous = registered(&backend, &api);
    backend
        .items
        .lock()
        .unwrap()
        .remove(&SessionStoreItem::AnonymousIdentity);
    api.recent.lock().unwrap().push(token('1'));
    let failure = delete_anonymous_voice_samples(
        &anonymous,
        AnonymousIdentitySource::Store(backend.as_ref()),
    )
    .unwrap_err();
    assert_eq!(failure.code(), "account_recent_login_required");
    assert_eq!(api.calls(), ["data_delete 1"]);
}

#[test]
fn a_device_that_never_had_an_anonymous_account_has_nothing_to_delete() {
    let api = FakeApi::new(&[]);
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("account");
    // 目录还不存在时匿名会话是「没有」，不是读不了。
    assert!(matches!(
        AnonymousSessionStorage::new(&directory).load(),
        Ok(None)
    ));
    let anonymous =
        BackendAccountSession::new(api.clone(), AnonymousSessionStorage::new(&directory));
    delete_anonymous_voice_samples(&anonymous, AnonymousIdentitySource::Directory(&directory))
        .unwrap();
    let backend = MemoryBackend::default();
    delete_anonymous_voice_samples(&anonymous, AnonymousIdentitySource::Store(&backend)).unwrap();
    assert!(api.calls().is_empty());
    assert!(!directory.exists());
}

#[test]
fn the_identity_file_in_the_directory_is_used_without_a_session() {
    let api = FakeApi::new(&['5']);
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("account");
    prepare_directory(&directory).unwrap();
    let identity = anonymous_identity(&directory).unwrap();
    let anonymous =
        BackendAccountSession::new(api.clone(), AnonymousSessionStorage::new(&directory));
    delete_anonymous_voice_samples(&anonymous, AnonymousIdentitySource::Directory(&directory))
        .unwrap();
    assert_eq!(api.calls(), ["challenge", "login 5", "data_delete 5"]);
    // 身份没有换，会话写进了 anonymous-session.json。
    assert_eq!(
        saved_anonymous_identity(AnonymousIdentitySource::Directory(&directory))
            .unwrap()
            .unwrap()
            .subject,
        identity.subject
    );
    assert!(directory.join(ANONYMOUS_SESSION_FILE).exists());
}
