//! 新操作的测试：假的后端记下每次调用用的是哪个令牌，会话在内存里。不联网。

use super::*;
use msime_client_services::account::{
    AccountChallenge, AccountChatModel, AccountChatModels, AccountDataExport, AccountDataSection,
    AccountDataSummary, AccountDevice, AccountDevices, AccountFeedback, AccountFeedbackReceipt,
    AccountProfile, AccountTokens, AccountTranslations, AccountUser, SavedAccountSession,
};
use msime_client_services::diagnostics::upload::{
    DiagnosticState, DiagnosticUpload, DiagnosticUploadReceipt,
};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const ACCOUNT_USER: &str = "synthetic-user";
const ANONYMOUS_USER: &str = "synthetic-anonymous";

fn token(character: char) -> String {
    character.to_string().repeat(64)
}

fn user(id: &str) -> AccountUser {
    AccountUser {
        id: id.into(),
        display_name: "合成用户".into(),
        created_at: "2026-01-01T00:00:00Z".into(),
        email: None,
        avatar_url: None,
    }
}

fn tokens(access: char, refresh: char, user_id: &str) -> AccountTokens {
    AccountTokens {
        access_token: token(access),
        refresh_token: token(refresh),
        token_type: "Bearer".into(),
        expires_in: 3600,
        user: user(user_id),
    }
}

fn saved(access: char, refresh: char, user_id: &str) -> SavedAccountSession {
    SavedAccountSession {
        tokens: tokens(access, refresh, user_id),
        expires_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 3_600_000,
        session_id: Some(msime_client_core::uuid::Uuid::new_v4()),
    }
}

/// 假的后端：每次调用记下 `<操作> <令牌首字符>`。令牌 `a`（登录账号）和 `1`（匿名账号）在 `stale` 里时以 401 拒绝；刷新令牌 `b` 换成 `c`/`d`，`2` 换成 `3`/`4`。`failures` 里有的操作返回那个失败。
#[derive(Clone, Default)]
struct FakeApi {
    calls: Arc<Mutex<Vec<String>>>,
    stale: Arc<Mutex<Vec<String>>>,
    failures: Arc<Mutex<HashMap<&'static str, AccountFailure>>>,
    current_device: Arc<Mutex<Option<String>>>,
}

impl FakeApi {
    fn call(&self, operation: &'static str, access_token: &str) -> Result<(), AccountFailure> {
        self.calls.lock().unwrap().push(format!(
            "{operation} {}",
            access_token.chars().next().unwrap_or('-')
        ));
        if self
            .stale
            .lock()
            .unwrap()
            .iter()
            .any(|stale| stale == access_token)
        {
            return Err(AccountError::Unauthorized.into());
        }
        match self.failures.lock().unwrap().get(operation) {
            Some(failure) => Err(failure.clone()),
            None => Ok(()),
        }
    }

    fn calls(&self) -> Vec<String> {
        std::mem::take(&mut *self.calls.lock().unwrap())
    }
}

impl AccountApi for FakeApi {
    fn providers(&self) -> Result<HashMap<String, bool>, AccountError> {
        unreachable!("providers")
    }
    fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
        unreachable!("challenge")
    }
    fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
        unreachable!("login")
    }
    fn refresh(&self, refresh_token: &str) -> Result<AccountTokens, AccountError> {
        self.calls.lock().unwrap().push(format!(
            "refresh {}",
            refresh_token.chars().next().unwrap_or('-')
        ));
        if refresh_token == token('b') {
            Ok(tokens('c', 'd', ACCOUNT_USER))
        } else if refresh_token == token('2') {
            Ok(tokens('3', '4', ANONYMOUS_USER))
        } else {
            Err(AccountError::Unauthorized)
        }
    }
    fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
        unreachable!("profile")
    }
    fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
        unreachable!("rename")
    }
    fn logout(&self, access_token: &str, all: bool) -> Result<(), AccountError> {
        assert!(!all);
        self.call("logout", access_token)
            .map_err(|failure| failure.error)
    }
    fn delete_account(&self, _: &str) -> Result<(), AccountError> {
        unreachable!("delete_account")
    }
    fn chat_models(&self, access_token: &str) -> Result<AccountChatModels, AccountError> {
        self.call("chat_models", access_token)
            .map_err(|failure| failure.error)?;
        Ok(AccountChatModels {
            data: vec![AccountChatModel {
                id: "synthetic-model".into(),
            }],
            default_model: "synthetic-model".into(),
        })
    }
    fn chat(
        &self,
        messages: &[AccountChatMessage],
        _: &str,
        access_token: &str,
    ) -> Result<String, AccountError> {
        self.call("chat", access_token)
            .map_err(|failure| failure.error)?;
        Ok(format!("回答：{}", messages[0].content))
    }
    fn translate(
        &self,
        request: &AccountTranslateRequest,
        timeout: Duration,
        access_token: &str,
    ) -> Result<AccountTranslations, AccountFailure> {
        assert_eq!(timeout, Duration::from_millis(10_000));
        self.call("translate", access_token)?;
        Ok(AccountTranslations {
            translations: request
                .texts
                .iter()
                .map(|text| Some(format!("{text}-en")))
                .collect(),
        })
    }
    fn phrases(&self, access_token: &str) -> Result<AccountPhrases, AccountFailure> {
        self.call("phrases", access_token)?;
        Ok(AccountPhrases {
            revision: 2,
            phrases: vec![AccountPhrase {
                id: "p1".into(),
                text: "合成常用语".into(),
                group: String::new(),
                position: 0,
            }],
        })
    }
    fn put_phrases(
        &self,
        phrases: &AccountPhrases,
        access_token: &str,
    ) -> Result<AccountPhrases, AccountFailure> {
        self.call("put_phrases", access_token)?;
        Ok(AccountPhrases {
            revision: phrases.revision + 1,
            phrases: phrases.phrases.clone(),
        })
    }
    fn devices(&self, access_token: &str) -> Result<AccountDevices, AccountFailure> {
        self.call("devices", access_token)?;
        let current = self.current_device.lock().unwrap().clone();
        Ok(AccountDevices {
            sessions: current
                .into_iter()
                .map(|id| AccountDevice {
                    id,
                    platform: "web".into(),
                    name: String::new(),
                    app_version: String::new(),
                    created_at: "2026-10-01T00:00:00Z".into(),
                    last_active: "2026-10-16T00:00:00Z".into(),
                    current: true,
                })
                .collect(),
        })
    }
    fn revoke_device(&self, _: &str, access_token: &str) -> Result<(), AccountFailure> {
        self.call("revoke_device", access_token)
    }
    fn data_summary(&self, access_token: &str) -> Result<AccountDataSummary, AccountFailure> {
        self.call("data_summary", access_token)?;
        Ok(AccountDataSummary {
            bytes: 12,
            sections: vec![AccountDataSection {
                id: "phrases".into(),
                bytes: 12,
                items: 1,
            }],
        })
    }
    fn data_export(
        &self,
        destination: &Path,
        access_token: &str,
    ) -> Result<AccountDataExport, AccountFailure> {
        self.call("data_export", access_token)?;
        Ok(AccountDataExport {
            path: destination.to_string_lossy().into_owned(),
            bytes: 22,
            filename: "msime-data-2026-10-16.zip".into(),
        })
    }
    fn data_delete(&self, _: &[String], access_token: &str) -> Result<(), AccountFailure> {
        self.call("data_delete", access_token)
    }
    fn feedback(
        &self,
        feedback: &AccountFeedback,
        access_token: &str,
    ) -> Result<AccountFeedbackReceipt, AccountFailure> {
        self.call("feedback", access_token)?;
        let payload: Value = serde_json::from_slice(feedback.payload()).unwrap();
        assert_eq!(payload["text"], "合成反馈");
        Ok(AccountFeedbackReceipt {
            id: "6f9619ff-8b86-d011-b42d-00c04fc964ff".into(),
            status: "received".into(),
        })
    }
    fn diagnostics_upload(
        &self,
        upload: &DiagnosticUpload,
        access_token: &str,
    ) -> Result<DiagnosticUploadReceipt, AccountFailure> {
        self.call("diagnostics_upload", access_token)?;
        let body: Value = serde_json::from_slice(upload.body()).unwrap();
        assert_eq!(body["sections"], json!({ "input_events": [] }));
        Ok(DiagnosticUploadReceipt {
            id: "0".repeat(24),
            mcp_url: format!("https://api.msime.app/mcp/s/{}", "0".repeat(24)),
            token: "msk_synthetic".into(),
            expires_at: "2026-10-17T00:00:00Z".into(),
        })
    }
    fn diagnostics_state(&self, access_token: &str) -> Result<DiagnosticState, AccountFailure> {
        self.call("diagnostics_state", access_token)?;
        Ok(DiagnosticState {
            snapshot: None,
            accesses: Vec::new(),
        })
    }
    fn diagnostics_regenerate_token(&self, access_token: &str) -> Result<String, AccountFailure> {
        self.call("diagnostics_regenerate_token", access_token)?;
        Ok("msk_replacement".into())
    }
    fn diagnostics_delete(&self, access_token: &str) -> Result<(), AccountFailure> {
        self.call("diagnostics_delete", access_token)
    }
}

#[derive(Clone, Default)]
struct MemoryStorage(Arc<Mutex<Option<SavedAccountSession>>>);

impl AccountSessionStorage for MemoryStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        *self.0.lock().unwrap() = Some(session.clone());
        Ok(())
    }
    fn clear(&self) -> Result<(), AccountError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

type Session = BackendAccountSession<FakeApi, MemoryStorage>;

struct Fixture {
    api: FakeApi,
    account_storage: MemoryStorage,
    account: Session,
    anonymous: Arc<Session>,
}

impl Fixture {
    fn new(account: Option<SavedAccountSession>, anonymous: Option<SavedAccountSession>) -> Self {
        let api = FakeApi::default();
        let account_storage = MemoryStorage(Arc::new(Mutex::new(account)));
        let anonymous_storage = MemoryStorage(Arc::new(Mutex::new(anonymous)));
        Self {
            account: BackendAccountSession::new(api.clone(), account_storage.clone()),
            anonymous: Arc::new(BackendAccountSession::new(api.clone(), anonymous_storage)),
            api,
            account_storage,
        }
    }

    fn signed_in() -> Self {
        Self::new(
            Some(saved('a', 'b', ACCOUNT_USER)),
            Some(saved('1', '2', ANONYMOUS_USER)),
        )
    }

    fn run(&self, operation: Value) -> Result<Value, OperationFailure> {
        let operation: Operation = serde_json::from_value(operation).unwrap();
        let anonymous = Arc::clone(&self.anonymous);
        run(&self.account, move || Ok(anonymous), operation)
    }

    fn code(&self, operation: Value) -> &'static str {
        self.run(operation).unwrap_err().code
    }
}

#[test]
fn signed_in_operations_reach_their_calls_and_answer_the_server_shapes() {
    let fixture = Fixture::signed_in();
    assert_eq!(
        fixture
            .run(json!({ "operation": "chat", "messages": [{ "role": "user", "content": "你好" }], "model": "synthetic-model" }))
            .unwrap(),
        json!({ "content": "回答：你好" })
    );
    assert_eq!(
        fixture.run(json!({ "operation": "chat_models" })).unwrap(),
        json!({ "data": [{ "id": "synthetic-model" }], "defaultModel": "synthetic-model" })
    );
    assert_eq!(
        fixture.run(json!({ "operation": "phrases_load" })).unwrap(),
        json!({ "revision": 2, "phrases": [{ "id": "p1", "text": "合成常用语", "group": "", "position": 0 }] })
    );
    assert_eq!(
        fixture
            .run(json!({ "operation": "phrases_save", "revision": 2, "phrases": [{ "id": "p2", "text": "  ", "group": "组", "position": 3 }] }))
            .unwrap()["revision"],
        3
    );
    assert_eq!(
        fixture
            .run(json!({ "operation": "sessions_list" }))
            .unwrap(),
        json!({ "sessions": [] })
    );
    assert_eq!(
        fixture.run(json!({ "operation": "data_summary" })).unwrap(),
        json!({ "bytes": 12, "sections": [{ "id": "phrases", "bytes": 12, "items": 1 }] })
    );
    let destination = std::env::temp_dir().join("synthetic-export.zip");
    assert_eq!(
        fixture
            .run(json!({ "operation": "data_export", "destination": destination }))
            .unwrap(),
        json!({ "path": destination, "bytes": 22, "filename": "msime-data-2026-10-16.zip" })
    );
    assert_eq!(
        fixture
            .run(json!({ "operation": "data_delete", "sections": ["phrases", "voice"] }))
            .unwrap(),
        Value::Null
    );
    // 都用登录账号的令牌。
    assert_eq!(
        fixture.api.calls(),
        [
            "chat a",
            "chat_models a",
            "phrases a",
            "put_phrases a",
            "devices a",
            "data_summary a",
            "data_export a",
            "data_delete a"
        ]
    );
}

#[test]
fn signed_in_only_operations_do_not_fall_back_to_the_anonymous_account() {
    let fixture = Fixture::new(None, Some(saved('1', '2', ANONYMOUS_USER)));
    for operation in [
        json!({ "operation": "chat_models" }),
        json!({ "operation": "phrases_load" }),
        json!({ "operation": "sessions_list" }),
        json!({ "operation": "data_summary" }),
    ] {
        assert_eq!(
            fixture.code(operation.clone()),
            "account_unauthorized",
            "{operation}"
        );
    }
    assert!(fixture.api.calls().is_empty());
}

#[test]
fn translate_feedback_and_upload_prefer_the_signed_in_account() {
    let translate = json!({ "operation": "translate", "texts": ["水杉"], "source_lang": "zh", "target_lang": "en" });
    let fixture = Fixture::signed_in();
    assert_eq!(
        fixture.run(translate.clone()).unwrap(),
        json!({ "translations": ["水杉-en"] })
    );
    assert_eq!(fixture.api.calls(), ["translate a"]);

    let anonymous_only = Fixture::new(None, Some(saved('1', '2', ANONYMOUS_USER)));
    assert!(anonymous_only.run(translate.clone()).is_ok());
    let feedback = json!({
        "operation": "feedback",
        "type": "suggestion",
        "text": " 合成反馈 ",
        "platform": "linux",
        "app_version": "0.9.0",
        "edition": "",
        "diagnostics": { "os": "合成系统" },
    });
    assert_eq!(
        anonymous_only.run(feedback.clone()).unwrap(),
        json!({ "id": "6f9619ff-8b86-d011-b42d-00c04fc964ff", "status": "received" })
    );
    let state_root = tempfile::tempdir().unwrap();
    let upload = json!({
        "operation": "diagnostics_upload",
        "platform": "linux",
        "app_version": "0.9.0",
        "ttl": "one_hour",
        "bundle": { "state_root": state_root.path(), "include": { "input_events": true } },
    });
    let uploaded = anonymous_only.run(upload).unwrap();
    assert_eq!(uploaded["user_id"], ANONYMOUS_USER);
    assert_eq!(uploaded["anonymous"], true);
    assert_eq!(uploaded["token"], "msk_synthetic");
    assert_eq!(
        uploaded["counts"]["input_events"],
        json!({ "kept": 0, "dropped": 0, "truncated": 0 })
    );
    assert_eq!(
        anonymous_only.api.calls(),
        ["translate 1", "feedback 1", "diagnostics_upload 1"]
    );

    // 两个都没有时不发请求。
    let neither = Fixture::new(None, None);
    assert_eq!(neither.code(translate), "account_unauthorized");
    assert_eq!(neither.code(feedback), "account_unauthorized");
    assert!(neither.api.calls().is_empty());
}

#[test]
fn a_rejected_token_is_refreshed_once_within_the_same_account() {
    let fixture = Fixture::signed_in();
    fixture.api.stale.lock().unwrap().push(token('a'));
    let translate = json!({ "operation": "translate", "texts": ["水杉"], "source_lang": "zh", "target_lang": "en" });
    assert!(fixture.run(translate).is_ok());
    assert_eq!(
        fixture.api.calls(),
        ["translate a", "refresh b", "translate c"]
    );
    assert_eq!(
        fixture
            .account_storage
            .load()
            .unwrap()
            .unwrap()
            .tokens
            .access_token,
        token('c')
    );

    // 只经登录会话的操作同样重试一次。
    let fixture = Fixture::signed_in();
    fixture.api.stale.lock().unwrap().push(token('a'));
    assert!(fixture.run(json!({ "operation": "phrases_load" })).is_ok());
    assert_eq!(fixture.api.calls(), ["phrases a", "refresh b", "phrases c"]);
}

#[test]
fn diagnostics_operations_stay_with_the_uploading_account() {
    let fixture = Fixture::signed_in();
    assert_eq!(
        fixture
            .run(json!({ "operation": "diagnostics_state", "user_id": ANONYMOUS_USER }))
            .unwrap(),
        json!({ "snapshot": null, "accesses": [] })
    );
    assert_eq!(
        fixture
            .run(json!({ "operation": "diagnostics_regenerate_token", "user_id": ACCOUNT_USER }))
            .unwrap(),
        json!({ "token": "msk_replacement" })
    );
    assert_eq!(
        fixture
            .run(json!({ "operation": "diagnostics_delete", "user_id": ANONYMOUS_USER }))
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        fixture.api.calls(),
        [
            "diagnostics_state 1",
            "diagnostics_regenerate_token a",
            "diagnostics_delete 1"
        ]
    );
    // 上传时的账号已经不在了：报错，不去删另一个账号的快照。
    assert_eq!(
        fixture.code(json!({ "operation": "diagnostics_delete", "user_id": "someone-else" })),
        "account_identity_changed"
    );
    assert_eq!(
        fixture.code(json!({ "operation": "diagnostics_state", "user_id": "" })),
        "account_invalid"
    );
    assert!(fixture.api.calls().is_empty());

    // 401 后的刷新也只在同一个账号里。
    fixture.api.stale.lock().unwrap().push(token('1'));
    assert!(fixture
        .run(json!({ "operation": "diagnostics_delete", "user_id": ANONYMOUS_USER }))
        .is_ok());
    assert_eq!(
        fixture.api.calls(),
        ["diagnostics_delete 1", "refresh 2", "diagnostics_delete 3"]
    );
}

#[test]
fn revoking_the_current_session_signs_out_here() {
    let fixture = Fixture::signed_in();
    let other = "0".repeat(64);
    assert_eq!(
        fixture
            .run(json!({ "operation": "session_revoke", "id": other }))
            .unwrap(),
        json!({ "signed_out": false })
    );
    assert_eq!(fixture.api.calls(), ["devices a", "revoke_device a"]);

    let current = "1".repeat(64);
    *fixture.api.current_device.lock().unwrap() = Some(current.clone());
    assert_eq!(
        fixture
            .run(json!({ "operation": "session_revoke", "id": current }))
            .unwrap(),
        json!({ "signed_out": true })
    );
    assert_eq!(fixture.api.calls(), ["devices a", "logout a"]);
    assert!(fixture.account_storage.load().unwrap().is_none());

    // 列不出来时直接 DELETE。
    let fixture = Fixture::signed_in();
    fixture
        .api
        .failures
        .lock()
        .unwrap()
        .insert("devices", AccountError::Unavailable.into());
    assert_eq!(
        fixture
            .run(json!({ "operation": "session_revoke", "id": "2".repeat(64) }))
            .unwrap(),
        json!({ "signed_out": false })
    );
    assert_eq!(fixture.api.calls(), ["devices a", "revoke_device a"]);
    assert_eq!(
        fixture.code(
            json!({ "operation": "session_revoke", "id": "6f9619ff-8b86-d011-b42d-00c04fc964ff" })
        ),
        "account_invalid"
    );
}

#[test]
fn server_codes_and_retry_after_come_through() {
    let fixture = Fixture::signed_in();
    let failure = |status: u16, code: &str, retry: Option<&str>| {
        AccountFailure::from_http_response(
            status,
            json!({ "error": { "code": code, "message": code } })
                .to_string()
                .as_bytes(),
            retry,
        )
    };
    let mut failures = fixture.api.failures.lock().unwrap();
    failures.insert(
        "data_export",
        failure(429, "rate_limit_exceeded", Some("3600")),
    );
    failures.insert("data_delete", failure(403, "recent_login_required", None));
    failures.insert("revoke_device", failure(404, "session_not_found", None));
    failures.insert("put_phrases", failure(409, "revision_conflict", None));
    failures.insert("feedback", failure(413, "feedback_too_large", None));
    failures.insert(
        "diagnostics_regenerate_token",
        failure(404, "diagnostics_not_found", None),
    );
    drop(failures);
    let destination = std::env::temp_dir().join("synthetic-export.zip");
    assert_eq!(
        fixture
            .run(json!({ "operation": "data_export", "destination": destination }))
            .unwrap_err(),
        OperationFailure {
            code: "account_rate_limited",
            retry_after_seconds: Some(3600)
        }
    );
    assert_eq!(
        fixture.code(json!({ "operation": "data_delete", "sections": ["dictionary"] })),
        "account_recent_login_required"
    );
    assert_eq!(
        fixture.code(json!({ "operation": "session_revoke", "id": "0".repeat(64) })),
        "account_not_found"
    );
    assert_eq!(
        fixture.code(json!({ "operation": "phrases_save", "revision": 0, "phrases": [] })),
        "account_conflict"
    );
    assert_eq!(
        fixture.code(json!({ "operation": "feedback", "type": "bug", "text": "合成反馈", "platform": "ios", "app_version": "1", "edition": "full" })),
        "account_too_large"
    );
    assert_eq!(
        fixture
            .code(json!({ "operation": "diagnostics_regenerate_token", "user_id": ACCOUNT_USER })),
        "account_not_found"
    );
}

#[test]
fn bad_requests_fail_before_any_backend_call() {
    let fixture = Fixture::signed_in();
    let long = "长".repeat(2731);
    let messages: Vec<Value> = (0..8)
        .map(|_| json!({ "role": "user", "content": long }))
        .collect();
    assert_eq!(
        fixture
            .code(json!({ "operation": "chat", "messages": messages, "model": "synthetic-model" })),
        "account_too_large"
    );
    assert_eq!(
        fixture.code(json!({ "operation": "chat", "messages": [], "model": "synthetic-model" })),
        "account_invalid"
    );
    let full = "a".repeat(8192);
    assert_eq!(
        fixture.code(json!({ "operation": "translate", "texts": vec![full; 32], "source_lang": "zh", "target_lang": "en" })),
        "account_too_large"
    );
    for timeout_ms in [999, 30_001] {
        assert_eq!(
            fixture.code(json!({ "operation": "translate", "texts": ["词"], "source_lang": "zh", "target_lang": "en", "timeout_ms": timeout_ms })),
            "account_invalid"
        );
    }
    assert_eq!(
        fixture.code(json!({ "operation": "phrases_save", "revision": 0, "phrases": [{ "id": "a", "text": "", "group": "", "position": 0 }] })),
        "account_invalid"
    );
    for sections in [
        json!([]),
        json!(["phrases", "phrases"]),
        json!(["community"]),
    ] {
        assert_eq!(
            fixture.code(json!({ "operation": "data_delete", "sections": sections })),
            "account_invalid"
        );
    }
    assert_eq!(
        fixture.code(json!({ "operation": "data_export", "destination": "relative.zip" })),
        "account_invalid"
    );
    assert_eq!(
        fixture.code(json!({ "operation": "feedback", "type": "bug", "text": "回车\r换行", "platform": "ios", "app_version": "1", "edition": "full" })),
        "account_invalid"
    );
    assert_eq!(
        fixture.code(json!({ "operation": "feedback", "type": "bug", "text": "合成", "platform": "ios", "app_version": "1", "edition": "full", "screenshots": ["relative.png"] })),
        "account_invalid"
    );
    let state_root = tempfile::tempdir().unwrap();
    for (platform, ttl, include) in [
        ("linux", "one_hour", json!({})),
        ("linux", "forever", json!({ "input_events": true })),
        ("linux os", "one_hour", json!({ "input_events": true })),
    ] {
        assert_eq!(
            fixture.code(json!({
                "operation": "diagnostics_upload",
                "platform": platform,
                "app_version": "0.9.0",
                "ttl": ttl,
                "bundle": { "state_root": state_root.path(), "include": include },
            })),
            "account_invalid"
        );
    }
    assert_eq!(
        fixture.code(json!({
            "operation": "diagnostics_upload",
            "platform": "linux",
            "app_version": "0.9.0",
            "ttl": "one_hour",
            "bundle": { "state_root": state_root.path(), "include": { "input_events": true }, "sources": { "input_events": "relative.log" } },
        })),
        "diagnostics_invalid"
    );
    assert!(fixture.api.calls().is_empty());
}
