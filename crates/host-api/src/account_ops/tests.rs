//! 新操作的测试：假的后端记下每次调用用的是哪个令牌，会话在内存里。不联网。

use super::*;
use msime_client_services::account::{
    AccountChallenge, AccountChatModel, AccountChatModels, AccountDataExport, AccountDataSection,
    AccountDataSummary, AccountDevice, AccountDevices, AccountDownloadLink, AccountFeedback,
    AccountFeedbackReceipt, AccountProfile, AccountTokens, AccountTranslations, AccountUser,
    AccountVoiceContribution, AccountVoiceContributionReceipt, SavedAccountSession,
    SessionStoreBackend, SessionStoreItem, WriteOutcome,
};
use msime_client_services::diagnostics::upload::{
    DiagnosticState, DiagnosticUpload, DiagnosticUploadReceipt,
};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const ACCOUNT_USER: &str = "synthetic-user";
const ANONYMOUS_USER: &str = "synthetic-anonymous";
/// 匿名账号保存的身份，形状与 Rust 生成的相同。
const ANONYMOUS_SUBJECT: &str = "msime-0123456789abcdef";
const ANONYMOUS_SECRET: &str = "abcdefghijklmnopqrstuvwxyz0123456789abcdefghijkl";

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

/// 假的后端：每次调用记下 `<操作> <令牌首字符>`。令牌 `a`（登录账号）和 `1`（匿名账号）在 `stale` 里时以 401 拒绝；刷新令牌 `b` 换成 `c`/`d`，`2` 换成 `3`/`4`。`failures` 里有的操作返回那个失败。匿名账号重新登录时换来令牌 `5`；`data_delete` 对 `recent_login` 里的令牌回 403 `recent_login_required`。`during_request` 在对话和模型目录的请求进行中调用一次。
#[derive(Clone, Default)]
struct FakeApi {
    calls: Arc<Mutex<Vec<String>>>,
    stale: Arc<Mutex<Vec<String>>>,
    failures: Arc<Mutex<HashMap<&'static str, AccountFailure>>>,
    current_device: Arc<Mutex<Option<String>>>,
    recent_login: Arc<Mutex<Vec<String>>>,
    during_request: Arc<Mutex<Option<Interruption>>>,
}

/// 请求进行中要做的一件事，例如让会话退出登录。
type Interruption = Box<dyn FnOnce() + Send>;

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

    fn interrupt(&self) {
        let during = self.during_request.lock().unwrap().take();
        if let Some(during) = during {
            during();
        }
    }
}

impl AccountApi for FakeApi {
    fn providers(&self) -> Result<HashMap<String, bool>, AccountError> {
        unreachable!("providers")
    }
    fn challenge(&self, provider: &str, target: &str) -> Result<AccountChallenge, AccountError> {
        assert_eq!((provider, target), ("anonymous", ANONYMOUS_SUBJECT));
        self.calls.lock().unwrap().push("challenge".into());
        Ok(AccountChallenge {
            challenge_id: "synthetic-challenge".into(),
            expires_in: 300,
            nonce: None,
            authorization_url: None,
        })
    }
    fn login(&self, _: &str, credential: &str) -> Result<AccountTokens, AccountError> {
        assert_eq!(credential, ANONYMOUS_SECRET);
        self.calls.lock().unwrap().push("login".into());
        Ok(tokens('5', '6', ANONYMOUS_USER))
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
    fn delete_account_detailed(&self, access_token: &str) -> Result<(), AccountFailure> {
        self.call("delete_account", access_token)
    }
    fn chat_models_detailed(
        &self,
        access_token: &str,
    ) -> Result<AccountChatModels, AccountFailure> {
        self.call("chat_models", access_token)?;
        self.interrupt();
        Ok(AccountChatModels {
            data: vec![AccountChatModel {
                id: "synthetic-model".into(),
            }],
            default_model: "synthetic-model".into(),
        })
    }
    fn chat_detailed(
        &self,
        messages: &[AccountChatMessage],
        _: &str,
        access_token: &str,
    ) -> Result<String, AccountFailure> {
        self.call("chat", access_token)?;
        self.interrupt();
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
    fn data_delete(&self, sections: &[String], access_token: &str) -> Result<(), AccountFailure> {
        self.call("data_delete", access_token)?;
        // 匿名账号那一次只删 voice 分区。
        if access_token == token('1') || access_token == token('5') {
            assert_eq!(sections, ["voice"]);
        }
        if self
            .recent_login
            .lock()
            .unwrap()
            .iter()
            .any(|recent| recent == access_token)
        {
            return Err(AccountFailure::from_http_response(
                403,
                br#"{"error":{"code":"recent_login_required","message":"recent login required"}}"#,
                None,
            ));
        }
        Ok(())
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
    fn download_link(
        &self,
        platform: &str,
        access_token: &str,
    ) -> Result<AccountDownloadLink, AccountFailure> {
        self.call("download_link", access_token)?;
        assert_eq!(platform, "windows");
        Ok(AccountDownloadLink {
            sent_to: Some("s***@example.test".into()),
        })
    }
    fn voice_contribution(
        &self,
        contribution: &AccountVoiceContribution,
        access_token: &str,
    ) -> Result<AccountVoiceContributionReceipt, AccountFailure> {
        self.call("voice_contribution", access_token)?;
        let payload: Value = serde_json::from_slice(contribution.payload()).unwrap();
        assert_eq!(payload["transcript"], "合成识别文本");
        Ok(AccountVoiceContributionReceipt {
            id: "0123456789abcdef".repeat(4),
        })
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

/// 宿主存储里只有匿名账号的身份一项，`run` 只会读它。
struct IdentityStore(Option<Vec<u8>>);

impl IdentityStore {
    fn saved() -> Self {
        Self(Some(
            serde_json::to_vec(
                &json!({ "subject": ANONYMOUS_SUBJECT, "secret": ANONYMOUS_SECRET }),
            )
            .unwrap(),
        ))
    }
}

impl SessionStoreBackend for IdentityStore {
    fn read(&self, item: SessionStoreItem, _: usize) -> Result<Option<Vec<u8>>, AccountError> {
        assert_eq!(item, SessionStoreItem::AnonymousIdentity);
        Ok(self.0.clone())
    }
    fn write(&self, _: SessionStoreItem, _: &[u8]) -> Result<WriteOutcome, AccountError> {
        unreachable!("write")
    }
    fn remove(&self, _: SessionStoreItem) -> Result<(), AccountError> {
        unreachable!("remove")
    }
    fn lock(&self, _: SessionStoreItem, _: Duration) -> Result<u64, AccountError> {
        unreachable!("lock")
    }
    fn unlock(&self, _: SessionStoreItem, _: u64) {
        unreachable!("unlock")
    }
    fn shared_across_processes(&self) -> bool {
        false
    }
    fn layout(&self) -> msime_client_services::account::AccountSessionFileLayout {
        msime_client_services::account::AccountSessionFileLayout::Native
    }
}

struct Fixture {
    api: FakeApi,
    account_storage: MemoryStorage,
    account: Arc<Session>,
    anonymous: Arc<Session>,
    identity: IdentityStore,
}

impl Fixture {
    fn new(account: Option<SavedAccountSession>, anonymous: Option<SavedAccountSession>) -> Self {
        let api = FakeApi::default();
        let account_storage = MemoryStorage(Arc::new(Mutex::new(account)));
        let anonymous_storage = MemoryStorage(Arc::new(Mutex::new(anonymous)));
        Self {
            account: Arc::new(BackendAccountSession::new(
                api.clone(),
                account_storage.clone(),
            )),
            anonymous: Arc::new(BackendAccountSession::new(api.clone(), anonymous_storage)),
            api,
            account_storage,
            identity: IdentityStore::saved(),
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
        run(
            &self.account,
            move || Ok(anonymous),
            AnonymousIdentitySource::Store(&self.identity),
            operation,
        )
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
            // 删 voice 分区时先删匿名账号名下的语音样本。
            "data_delete 1",
            "data_delete a"
        ]
    );
}

#[test]
fn signed_in_only_operations_do_not_fall_back_to_the_anonymous_account() {
    let fixture = Fixture::new(None, Some(saved('1', '2', ANONYMOUS_USER)));
    for operation in [
        json!({ "operation": "phrases_load" }),
        json!({ "operation": "sessions_list" }),
        json!({ "operation": "data_summary" }),
        // 删除云端数据要登录，没有登录时连匿名账号的语音样本也不碰。
        json!({ "operation": "data_delete", "sections": ["voice"] }),
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

/// 16 kHz 单声道 16 位 PCM 的 WAV（全零），`millis` 毫秒长，编成标准 base64。
fn wav_base64(millis: u32) -> String {
    let data = millis * 32;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    for field in [16_u32, 0x0001_0001, 16_000, 32_000, 0x0010_0002] {
        wav.extend_from_slice(&field.to_le_bytes());
    }
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data.to_le_bytes());
    wav.resize(wav.len() + data as usize, 0);
    base64::engine::general_purpose::STANDARD.encode(wav)
}

fn voice_contribution(audio: String) -> Value {
    json!({
        "operation": "voice_contribution",
        "language": "zh-CN",
        "provider": "system",
        "transcript": " 合成识别文本 ",
        "app_version": "0.9.0",
        "audio": audio,
    })
}

#[test]
fn the_download_link_goes_only_to_the_signed_in_account() {
    let fixture = Fixture::signed_in();
    assert_eq!(
        fixture
            .run(json!({ "operation": "download_link", "platform": "windows" }))
            .unwrap(),
        json!({ "sent_to": "s***@example.test" })
    );
    assert_eq!(fixture.api.calls(), ["download_link a"]);
    assert_eq!(
        fixture.code(json!({ "operation": "download_link", "platform": "web" })),
        "account_invalid"
    );
    assert!(fixture.api.calls().is_empty());
    fixture.api.failures.lock().unwrap().insert(
        "download_link",
        AccountFailure::from_http_response(
            409,
            br#"{"error":{"code":"no_verified_email","message":"no verified email"}}"#,
            None,
        ),
    );
    assert_eq!(
        fixture.code(json!({ "operation": "download_link", "platform": "windows" })),
        "account_no_verified_email"
    );

    // 匿名账号没有邮箱，不退回它。
    let anonymous_only = Fixture::new(None, Some(saved('1', '2', ANONYMOUS_USER)));
    assert_eq!(
        anonymous_only.code(json!({ "operation": "download_link", "platform": "windows" })),
        "account_unauthorized"
    );
    assert!(anonymous_only.api.calls().is_empty());
}

#[test]
fn a_voice_contribution_goes_only_to_the_anonymous_account() {
    // 登录了也用匿名账号。
    let fixture = Fixture::signed_in();
    assert_eq!(
        fixture.run(voice_contribution(wav_base64(1500))).unwrap(),
        json!({ "id": "0123456789abcdef".repeat(4), "duration_ms": 1500 })
    );
    assert_eq!(fixture.api.calls(), ["voice_contribution 1"]);

    let anonymous_only = Fixture::new(None, Some(saved('1', '2', ANONYMOUS_USER)));
    assert!(anonymous_only
        .run(voice_contribution(wav_base64(800)))
        .is_ok());
    assert_eq!(anonymous_only.api.calls(), ["voice_contribution 1"]);

    // 没有匿名会话时不退回登录账号。
    for fixture in [
        Fixture::new(Some(saved('a', 'b', ACCOUNT_USER)), None),
        Fixture::new(None, None),
    ] {
        assert_eq!(
            fixture.code(voice_contribution(wav_base64(800))),
            "account_unauthorized"
        );
        assert!(fixture.api.calls().is_empty());
    }
}

#[test]
fn a_bad_voice_contribution_is_not_sent() {
    let fixture = Fixture::signed_in();
    // 不是 base64、不是 WAV、超过 60 秒。
    for audio in [
        "不是 base64".to_owned(),
        base64::engine::general_purpose::STANDARD.encode(b"OggS synthetic"),
        wav_base64(60_001),
    ] {
        assert_eq!(fixture.code(voice_contribution(audio)), "account_invalid");
    }
    // 解码前按长度挡掉一定超过 2 MiB 的音频：同样不是 base64 的字符，超长的是 account_too_large，不超长的才去解码、报 account_invalid。
    let mut large = "!".repeat(MAX_VOICE_AUDIO_BASE64_BYTES + 4);
    assert_eq!(
        fixture.code(voice_contribution(large.clone())),
        "account_too_large"
    );
    large.truncate(MAX_VOICE_AUDIO_BASE64_BYTES);
    assert_eq!(fixture.code(voice_contribution(large)), "account_invalid");
    let mut blank = voice_contribution(wav_base64(800));
    blank["transcript"] = json!("  ");
    assert_eq!(fixture.code(blank), "account_invalid");
    assert!(fixture.api.calls().is_empty());
}

#[test]
fn chat_and_models_report_retry_after_503_and_network_apart() {
    let fixture = Fixture::signed_in();
    let chat = json!({ "operation": "chat", "messages": [{ "role": "user", "content": "你好" }], "model": "synthetic-model" });
    let failure = |status: u16, code: &str, retry: Option<&str>| {
        AccountFailure::from_http_response(
            status,
            json!({ "error": { "code": code, "message": code } })
                .to_string()
                .as_bytes(),
            retry,
        )
    };
    let set = |operation: &'static str, failure: AccountFailure| {
        fixture
            .api
            .failures
            .lock()
            .unwrap()
            .insert(operation, failure);
    };
    set("chat", failure(429, "rate_limit_exceeded", Some("60")));
    set(
        "chat_models",
        failure(429, "rate_limit_exceeded", Some("3600")),
    );
    assert_eq!(
        fixture.run(chat.clone()).unwrap_err(),
        OperationFailure {
            code: "account_rate_limited",
            retry_after_seconds: Some(60)
        }
    );
    assert_eq!(
        fixture
            .run(json!({ "operation": "chat_models" }))
            .unwrap_err(),
        OperationFailure {
            code: "account_rate_limited",
            retry_after_seconds: Some(3600)
        }
    );
    set("chat", failure(503, "upstream_unavailable", None));
    set("chat_models", AccountFailure::network());
    assert_eq!(fixture.code(chat.clone()), "account_unavailable");
    assert_eq!(
        fixture.code(json!({ "operation": "chat_models" })),
        "account_network"
    );
    set("chat", AccountFailure::network());
    assert_eq!(fixture.code(chat), "account_network");
    assert_eq!(
        fixture.api.calls(),
        [
            "chat a",
            "chat_models a",
            "chat a",
            "chat_models a",
            "chat a"
        ]
    );
}

#[test]
fn account_delete_reports_recent_login_and_signs_out_only_on_success() {
    let fixture = Fixture::signed_in();
    let delete = json!({ "operation": "account_delete" });
    fixture.api.failures.lock().unwrap().insert(
        "delete_account",
        AccountFailure::from_http_response(
            403,
            br#"{"error":{"code":"recent_login_required","message":"recent_login_required"}}"#,
            None,
        ),
    );
    assert_eq!(
        fixture.code(delete.clone()),
        "account_recent_login_required"
    );
    // 需要重新登录时本地会话留着，宿主让用户重新登录后再注销。
    assert!(fixture.account_storage.0.lock().unwrap().is_some());
    fixture.api.failures.lock().unwrap().clear();
    assert_eq!(fixture.run(delete.clone()).unwrap(), Value::Null);
    assert!(fixture.account_storage.0.lock().unwrap().is_none());
    // 只用登录账号：没有登录时不退回匿名账号。
    assert_eq!(fixture.code(delete), "account_unauthorized");
    assert_eq!(
        fixture.api.calls(),
        ["delete_account a", "delete_account a"]
    );
}

#[test]
fn chat_and_models_fall_back_to_the_anonymous_account() {
    let chat = json!({ "operation": "chat", "messages": [{ "role": "user", "content": "你好" }], "model": "synthetic-model" });
    let anonymous_only = Fixture::new(None, Some(saved('1', '2', ANONYMOUS_USER)));
    assert_eq!(
        anonymous_only.run(chat.clone()).unwrap(),
        json!({ "content": "回答：你好" })
    );
    assert!(anonymous_only
        .run(json!({ "operation": "chat_models" }))
        .is_ok());
    assert_eq!(anonymous_only.api.calls(), ["chat 1", "chat_models 1"]);

    // 匿名令牌被拒时只在匿名账号里刷新重试。
    anonymous_only.api.stale.lock().unwrap().push(token('1'));
    assert!(anonymous_only.run(chat.clone()).is_ok());
    assert_eq!(
        anonymous_only.api.calls(),
        ["chat 1", "refresh 2", "chat 3"]
    );

    let neither = Fixture::new(None, None);
    assert_eq!(neither.code(chat), "account_unauthorized");
    assert_eq!(
        neither.code(json!({ "operation": "chat_models" })),
        "account_unauthorized"
    );
    assert!(neither.api.calls().is_empty());
}

#[test]
fn a_reply_is_dropped_when_its_account_signs_out_during_the_request() {
    let chat = json!({ "operation": "chat", "messages": [{ "role": "user", "content": "你好" }], "model": "synthetic-model" });
    for signed_in in [false, true] {
        for operation in [chat.clone(), json!({ "operation": "chat_models" })] {
            let fixture = if signed_in {
                Fixture::signed_in()
            } else {
                Fixture::new(None, Some(saved('1', '2', ANONYMOUS_USER)))
            };
            // 请求进行中，给出令牌的那个会话退出了登录。
            let serving = Arc::downgrade(if signed_in {
                &fixture.account
            } else {
                &fixture.anonymous
            });
            *fixture.api.during_request.lock().unwrap() = Some(Box::new(move || {
                serving.upgrade().unwrap().forget().unwrap()
            }));
            assert_eq!(
                fixture.code(operation.clone()),
                "account_cancelled",
                "{operation}"
            );
        }
    }
}

#[test]
fn deleting_voice_data_also_deletes_the_anonymous_samples() {
    // 匿名会话是新的：两个账号各删一次，匿名的在前、只删 voice。
    let fixture = Fixture::signed_in();
    assert!(fixture
        .run(json!({ "operation": "data_delete", "sections": ["voice", "phrases"] }))
        .is_ok());
    assert_eq!(fixture.api.calls(), ["data_delete 1", "data_delete a"]);

    // 不删 voice 时不碰匿名账号。
    assert!(fixture
        .run(json!({ "operation": "data_delete", "sections": ["phrases"] }))
        .is_ok());
    assert_eq!(fixture.api.calls(), ["data_delete a"]);

    // 匿名会话不是 10 分钟内建立的：用保存的身份重新登录匿名账号再删。
    let fixture = Fixture::signed_in();
    fixture.api.recent_login.lock().unwrap().push(token('1'));
    assert!(fixture
        .run(json!({ "operation": "data_delete", "sections": ["voice"] }))
        .is_ok());
    assert_eq!(
        fixture.api.calls(),
        [
            "data_delete 1",
            "challenge",
            "login",
            "data_delete 5",
            "data_delete a"
        ]
    );

    // 匿名那一步失败时登录账号的分区不动。
    let fixture = Fixture::signed_in();
    fixture.api.recent_login.lock().unwrap().push(token('1'));
    let fixture = Fixture {
        identity: IdentityStore(None),
        ..fixture
    };
    assert_eq!(
        fixture.code(json!({ "operation": "data_delete", "sections": ["voice", "dictionary"] })),
        "account_recent_login_required"
    );
    assert_eq!(fixture.api.calls(), ["data_delete 1"]);

    // 本机没有匿名账号时只删登录账号的。
    let fixture = Fixture {
        identity: IdentityStore(None),
        ..Fixture::new(Some(saved('a', 'b', ACCOUNT_USER)), None)
    };
    assert!(fixture
        .run(json!({ "operation": "data_delete", "sections": ["voice"] }))
        .is_ok());
    assert_eq!(fixture.api.calls(), ["data_delete a"]);
}
