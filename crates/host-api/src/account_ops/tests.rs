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

/// 假的后端：每次调用记下 `<操作> <令牌首字符>`。令牌 `a`（登录账号）和 `1`（匿名账号）在 `stale` 里时以 401 拒绝；刷新令牌 `b` 换成 `c`/`d`，`2` 换成 `3`/`4`。`failures` 里有的操作返回那个失败，`token_failures` 里有的令牌做任何操作都返回那个失败。`data_summary` 对匿名账号的令牌只回语音样本。匿名账号重新登录时换来令牌 `5`；`data_delete` 对 `recent_login` 里的令牌回 403 `recent_login_required`。`during_request` 在对话和模型目录的请求进行中调用一次。
#[derive(Clone, Default)]
struct FakeApi {
    calls: Arc<Mutex<Vec<String>>>,
    stale: Arc<Mutex<Vec<String>>>,
    failures: Arc<Mutex<HashMap<&'static str, AccountFailure>>>,
    token_failures: Arc<Mutex<HashMap<String, AccountFailure>>>,
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
        if let Some(failure) = self.token_failures.lock().unwrap().get(access_token) {
            return Err(failure.clone());
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
    /// 分两段交出回答，两段之间做 `during_request` 那件事、再看一次取消。
    fn chat_stream_detailed(
        &self,
        messages: &[AccountChatMessage],
        _: &str,
        access_token: &str,
        cancel: &ChatStreamCancel,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<String, AccountFailure> {
        self.call("chat_stream", access_token)?;
        cancel.check()?;
        on_delta("回答：");
        self.interrupt();
        cancel.check()?;
        on_delta(&messages[0].content);
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
        let section = |id: &str, bytes: i64, items: i64| AccountDataSection {
            id: id.into(),
            bytes,
            items,
        };
        // 匿名账号名下只有语音样本；登录账号的 voice 分区是空的（语音贡献只用匿名账号），服务端总会列出它。
        if [token('1'), token('3'), token('5')].contains(&access_token.to_owned()) {
            return Ok(AccountDataSummary {
                bytes: 300,
                sections: vec![section("voice", 300, 2)],
            });
        }
        Ok(AccountDataSummary {
            bytes: 12,
            sections: vec![section("phrases", 12, 1), section("voice", 0, 0)],
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
    fn put_typing_statistics(
        &self,
        upload: &msime_client_services::typing_statistics::upload::TypingStatisticsUpload,
        access_token: &str,
    ) -> Result<(), AccountFailure> {
        self.call("put_typing_statistics", access_token)?;
        assert_eq!(upload.platform, "windows");
        assert_eq!(upload.device.len(), 32);
        Ok(())
    }
    fn delete_typing_statistics(&self, access_token: &str) -> Result<(), AccountFailure> {
        self.call("delete_typing_statistics", access_token)
    }
    fn qq_links(
        &self,
        access_token: &str,
    ) -> Result<msime_client_services::account::AccountQqLinks, AccountFailure> {
        self.call("qq_links", access_token)?;
        Ok(msime_client_services::account::AccountQqLinks {
            links: vec![msime_client_services::account::AccountQqLink {
                id: "12".into(),
                created_at: "2026-10-17T08:00:00Z".into(),
            }],
        })
    }
    fn redeem_qq_link(
        &self,
        code: &str,
        access_token: &str,
    ) -> Result<msime_client_services::account::AccountQqLink, AccountFailure> {
        self.call("redeem_qq_link", access_token)?;
        assert_eq!(code, "12345678");
        Ok(msime_client_services::account::AccountQqLink {
            id: "13".into(),
            created_at: "2026-10-17T08:01:00Z".into(),
        })
    }
    fn delete_qq_link(&self, id: &str, access_token: &str) -> Result<(), AccountFailure> {
        self.call("delete_qq_link", access_token)?;
        assert_eq!(id, "13");
        Ok(())
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
    // 匿名账号的语音样本合进 voice 分区和总字节数。
    assert_eq!(
        fixture.run(json!({ "operation": "data_summary" })).unwrap(),
        json!({
            "bytes": 312,
            "sections": [
                { "id": "phrases", "bytes": 12, "items": 1 },
                { "id": "voice", "bytes": 300, "items": 2 }
            ],
            "anonymous_voice": "included"
        })
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
        json!({ "voice_samples": { "ok": true } })
    );
    // 都用登录账号的令牌，只有汇总和删除的语音样本另用匿名账号。
    assert_eq!(
        fixture.api.calls(),
        [
            "chat a",
            "chat_models a",
            "phrases a",
            "put_phrases a",
            "devices a",
            "data_summary a",
            "data_summary 1",
            "data_export a",
            // 删 voice 分区时先删登录账号的，再删匿名账号名下的语音样本。
            "data_delete a",
            "data_delete 1"
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
    assert_eq!(
        fixture.run(delete.clone()).unwrap(),
        json!({ "voice_samples": { "ok": true } })
    );
    assert!(fixture.account_storage.0.lock().unwrap().is_none());
    // 只用登录账号：没有登录时不退回匿名账号。
    assert_eq!(fixture.code(delete), "account_unauthorized");
    // 需要最近登录时没注销成功，也就不碰匿名账号；注销成功以后才删匿名账号的语音样本。
    assert_eq!(
        fixture.api.calls(),
        ["delete_account a", "delete_account a", "data_delete 1"]
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

fn rate_limited() -> AccountFailure {
    AccountFailure::from_http_response(
        429,
        br#"{"error":{"code":"rate_limit_exceeded","message":"rate limit exceeded"}}"#,
        Some("3600"),
    )
}

#[test]
fn deleting_voice_data_also_deletes_the_anonymous_samples() {
    // 匿名会话是新的：两个账号各删一次，登录账号的在前，匿名的只删 voice。
    let fixture = Fixture::signed_in();
    assert_eq!(
        fixture
            .run(json!({ "operation": "data_delete", "sections": ["voice", "phrases"] }))
            .unwrap(),
        json!({ "voice_samples": { "ok": true } })
    );
    assert_eq!(fixture.api.calls(), ["data_delete a", "data_delete 1"]);

    // 不删 voice 时不碰匿名账号，value 仍是 null。
    assert_eq!(
        fixture
            .run(json!({ "operation": "data_delete", "sections": ["phrases"] }))
            .unwrap(),
        Value::Null
    );
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
            "data_delete a",
            "data_delete 1",
            "challenge",
            "login",
            "data_delete 5"
        ]
    );

    // 本机没有匿名账号时只删登录账号的。
    let fixture = Fixture {
        identity: IdentityStore(None),
        ..Fixture::new(Some(saved('a', 'b', ACCOUNT_USER)), None)
    };
    assert_eq!(
        fixture
            .run(json!({ "operation": "data_delete", "sections": ["voice"] }))
            .unwrap(),
        json!({ "voice_samples": { "ok": true } })
    );
    assert_eq!(fixture.api.calls(), ["data_delete a"]);
}

#[test]
fn a_signed_in_recent_login_failure_does_not_spend_an_anonymous_re_login() {
    // 两个会话都早就建立了：登录账号那一步先失败，匿名账号不重新登录、什么也不删（PR #8350 上负责人定的顺序）。
    let fixture = Fixture::signed_in();
    fixture
        .api
        .recent_login
        .lock()
        .unwrap()
        .extend([token('a'), token('1')]);
    assert_eq!(
        fixture.code(json!({ "operation": "data_delete", "sections": ["voice", "dictionary"] })),
        "account_recent_login_required"
    );
    assert_eq!(fixture.api.calls(), ["data_delete a"]);
}

#[test]
fn a_failed_anonymous_step_after_the_signed_in_sections_is_reported_apart() {
    // 登录账号的分区已经删掉：匿名那一步失败不让操作失败，宿主据 voice_samples 用 voice_delete 重试。
    let fixture = Fixture::signed_in();
    fixture.api.recent_login.lock().unwrap().push(token('1'));
    let fixture = Fixture {
        identity: IdentityStore(None),
        ..fixture
    };
    assert_eq!(
        fixture
            .run(json!({ "operation": "data_delete", "sections": ["voice", "dictionary"] }))
            .unwrap(),
        json!({ "voice_samples": { "ok": false, "error": "account_recent_login_required" } })
    );
    assert_eq!(fixture.api.calls(), ["data_delete a", "data_delete 1"]);

    // 限流带着秒数。
    let fixture = Fixture::signed_in();
    fixture
        .api
        .token_failures
        .lock()
        .unwrap()
        .insert(token('1'), rate_limited());
    assert_eq!(
        fixture
            .run(json!({ "operation": "data_delete", "sections": ["voice"] }))
            .unwrap(),
        json!({ "voice_samples": { "ok": false, "error": "account_rate_limited", "retry_after_seconds": 3600 } })
    );
}

#[test]
fn deleting_the_account_then_deletes_the_anonymous_voice_samples_best_effort() {
    let delete = json!({ "operation": "account_delete" });
    // 匿名会话要重新登录：注销以后照样换新会话再删。
    let fixture = Fixture::signed_in();
    fixture.api.recent_login.lock().unwrap().push(token('1'));
    assert_eq!(
        fixture.run(delete.clone()).unwrap(),
        json!({ "voice_samples": { "ok": true } })
    );
    assert_eq!(
        fixture.api.calls(),
        [
            "delete_account a",
            "data_delete 1",
            "challenge",
            "login",
            "data_delete 5"
        ]
    );

    // 删语音样本失败：注销照样成功、本地会话照样清掉，失败只在 voice_samples 里。
    let fixture = Fixture::signed_in();
    fixture
        .api
        .token_failures
        .lock()
        .unwrap()
        .insert(token('1'), rate_limited());
    assert_eq!(
        fixture.run(delete.clone()).unwrap(),
        json!({ "voice_samples": { "ok": false, "error": "account_rate_limited", "retry_after_seconds": 3600 } })
    );
    assert!(fixture.account_storage.0.lock().unwrap().is_none());
    assert_eq!(fixture.api.calls(), ["delete_account a", "data_delete 1"]);

    // 读不了匿名会话也一样只是 voice_samples 失败。
    let fixture = Fixture::signed_in();
    assert_eq!(
        account_deleted(
            || Err::<Arc<Session>, _>(AccountError::Storage),
            AnonymousIdentitySource::Store(&fixture.identity),
        ),
        json!({ "voice_samples": { "ok": false, "error": "account_storage" } })
    );

    // 本机没有匿名账号：没有样本可删，算成功。
    let fixture = Fixture {
        identity: IdentityStore(None),
        ..Fixture::new(Some(saved('a', 'b', ACCOUNT_USER)), None)
    };
    assert_eq!(
        fixture.run(delete).unwrap(),
        json!({ "voice_samples": { "ok": true } })
    );
    assert_eq!(fixture.api.calls(), ["delete_account a"]);
}

#[test]
fn voice_samples_can_be_deleted_without_signing_in() {
    let voice_delete = json!({ "operation": "voice_delete" });
    // 从没登录过：只用匿名账号。
    let fixture = Fixture::new(None, Some(saved('1', '2', ANONYMOUS_USER)));
    assert_eq!(fixture.run(voice_delete.clone()).unwrap(), Value::Null);
    assert_eq!(fixture.api.calls(), ["data_delete 1"]);

    // 登录了也只删匿名账号的，不碰登录账号的 voice 分区。
    let fixture = Fixture::signed_in();
    assert_eq!(fixture.run(voice_delete.clone()).unwrap(), Value::Null);
    assert_eq!(fixture.api.calls(), ["data_delete 1"]);

    // 匿名会话要重新登录时用保存的身份。
    let fixture = Fixture::new(None, Some(saved('1', '2', ANONYMOUS_USER)));
    fixture.api.recent_login.lock().unwrap().push(token('1'));
    assert_eq!(fixture.run(voice_delete.clone()).unwrap(), Value::Null);
    assert_eq!(
        fixture.api.calls(),
        ["data_delete 1", "challenge", "login", "data_delete 5"]
    );

    // 有会话却换不了新会话：交回那次失败。
    let fixture = Fixture {
        identity: IdentityStore(None),
        ..Fixture::new(None, Some(saved('1', '2', ANONYMOUS_USER)))
    };
    fixture.api.recent_login.lock().unwrap().push(token('1'));
    assert_eq!(
        fixture.code(voice_delete.clone()),
        "account_recent_login_required"
    );

    // 本机既没有匿名会话也没有身份：没有样本可删，不联网。
    let fixture = Fixture {
        identity: IdentityStore(None),
        ..Fixture::new(None, None)
    };
    assert_eq!(fixture.run(voice_delete).unwrap(), Value::Null);
    assert!(fixture.api.calls().is_empty());
}

#[test]
fn the_data_summary_counts_the_anonymous_voice_samples_or_says_it_could_not() {
    let summary = json!({ "operation": "data_summary" });
    let own = json!({
        "bytes": 12,
        "sections": [
            { "id": "phrases", "bytes": 12, "items": 1 },
            { "id": "voice", "bytes": 0, "items": 0 }
        ],
    });
    let with = |anonymous_voice: &str| {
        let mut value = own.clone();
        value["anonymous_voice"] = json!(anonymous_voice);
        value
    };

    // 本机既没有匿名会话也没有匿名身份：没有可合的，只发一次请求。
    let fixture = Fixture {
        identity: IdentityStore(None),
        ..Fixture::new(Some(saved('a', 'b', ACCOUNT_USER)), None)
    };
    assert_eq!(fixture.run(summary.clone()).unwrap(), with("none"));
    assert_eq!(fixture.api.calls(), ["data_summary a"]);

    // 身份还在、会话已经没了（服务端的会话 30 天后过期，刷新被拒时本地清掉）：名下可能还有样本，不能说没有；汇总不为此重新登录，标成 unknown。
    let fixture = Fixture::new(Some(saved('a', 'b', ACCOUNT_USER)), None);
    assert_eq!(fixture.run(summary.clone()).unwrap(), with("unknown"));
    assert_eq!(fixture.api.calls(), ["data_summary a"]);

    // 身份读不出来也是 unknown。
    let fixture = Fixture {
        identity: IdentityStore(Some(b"not json".to_vec())),
        ..Fixture::new(Some(saved('a', 'b', ACCOUNT_USER)), None)
    };
    assert_eq!(fixture.run(summary.clone()).unwrap(), with("unknown"));

    // 匿名那一次失败：汇总照样成功，标成 unknown，voice 只是登录账号的。
    let fixture = Fixture::signed_in();
    fixture
        .api
        .token_failures
        .lock()
        .unwrap()
        .insert(token('1'), AccountFailure::network());
    assert_eq!(fixture.run(summary.clone()).unwrap(), with("unknown"));
    assert_eq!(fixture.api.calls(), ["data_summary a", "data_summary 1"]);

    // 读不了匿名会话也是 unknown。
    let fixture = Fixture::signed_in();
    assert_eq!(
        data_summary(
            &fixture.account,
            || Err::<Arc<Session>, _>(AccountError::Storage),
            AnonymousIdentitySource::Store(&fixture.identity),
        )
        .unwrap(),
        with("unknown")
    );

    // 匿名令牌被拒时在匿名账号里刷新一次再取。
    let fixture = Fixture::signed_in();
    fixture.api.stale.lock().unwrap().push(token('1'));
    assert_eq!(
        fixture.run(summary.clone()).unwrap()["anonymous_voice"],
        "included"
    );
    assert_eq!(
        fixture.api.calls(),
        [
            "data_summary a",
            "data_summary 1",
            "refresh 2",
            "data_summary 3"
        ]
    );

    // 登录账号的汇总失败时整个失败，不再去问匿名账号。
    let fixture = Fixture::signed_in();
    fixture
        .api
        .token_failures
        .lock()
        .unwrap()
        .insert(token('a'), rate_limited());
    assert_eq!(fixture.code(summary), "account_rate_limited");
    assert_eq!(fixture.api.calls(), ["data_summary a"]);
}

#[test]
fn merging_adds_a_missing_voice_section() {
    let mut summary = AccountDataSummary {
        bytes: 5,
        sections: vec![AccountDataSection {
            id: "phrases".into(),
            bytes: 5,
            items: 1,
        }],
    };
    let anonymous = AccountDataSummary {
        bytes: i64::MAX,
        sections: vec![AccountDataSection {
            id: "voice".into(),
            bytes: i64::MAX,
            items: 3,
        }],
    };
    merge_anonymous_voice(&mut summary, &anonymous);
    // 加不下时停在上限，不溢出。
    assert_eq!(summary.bytes, i64::MAX);
    assert_eq!(summary.sections[1].id, "voice");
    assert_eq!(summary.sections[1].items, 3);
}

/// 走 `chat_stream`，返回结果和按顺序收到的增量；`during` 在每段增量交出后调用。
fn chat_stream_with(
    fixture: &Fixture,
    messages: Value,
    cancel: &ChatStreamCancel,
    mut during: impl FnMut(&str),
) -> (Result<Value, OperationFailure>, Vec<String>) {
    let messages: Vec<AccountChatMessage> = serde_json::from_value(messages).unwrap();
    let anonymous = Arc::clone(&fixture.anonymous);
    let mut deltas = Vec::new();
    let result = chat_stream(
        &fixture.account,
        move || Ok(anonymous),
        &messages,
        "synthetic-model",
        cancel,
        &mut |delta| {
            deltas.push(delta.to_owned());
            during(delta);
        },
    );
    (result, deltas)
}

fn question() -> Value {
    json!([{ "role": "user", "content": "你好" }])
}

#[test]
fn chat_stream_hands_over_deltas_and_prefers_the_signed_in_account() {
    let signed_in = Fixture::signed_in();
    let (result, deltas) =
        chat_stream_with(&signed_in, question(), &ChatStreamCancel::default(), |_| {});
    assert_eq!(result.unwrap(), json!({ "content": "回答：你好" }));
    assert_eq!(deltas, ["回答：", "你好"]);
    assert_eq!(signed_in.api.calls(), ["chat_stream a"]);

    // 没有登录时用匿名账号；匿名令牌被拒时只在匿名账号里刷新重试，被拒的那次没有交出增量。
    let anonymous_only = Fixture::new(None, Some(saved('1', '2', ANONYMOUS_USER)));
    anonymous_only.api.stale.lock().unwrap().push(token('1'));
    let (result, deltas) = chat_stream_with(
        &anonymous_only,
        question(),
        &ChatStreamCancel::default(),
        |_| {},
    );
    assert!(result.is_ok());
    assert_eq!(deltas, ["回答：", "你好"]);
    assert_eq!(
        anonymous_only.api.calls(),
        ["chat_stream 1", "refresh 2", "chat_stream 3"]
    );

    let neither = Fixture::new(None, None);
    let (result, deltas) =
        chat_stream_with(&neither, question(), &ChatStreamCancel::default(), |_| {});
    assert_eq!(result.unwrap_err().code, "account_unauthorized");
    assert!(deltas.is_empty());
    assert!(neither.api.calls().is_empty());
}

#[test]
fn chat_stream_checks_the_request_before_taking_a_token() {
    let fixture = Fixture::signed_in();
    for (messages, code) in [
        (json!([]), "account_invalid"),
        (
            json!([{ "role": "tool", "content": "问题" }]),
            "account_invalid",
        ),
        (
            json!((0..5)
                .map(|_| json!({ "role": "user", "content": "字".repeat(5000) }))
                .collect::<Vec<_>>()),
            "account_too_large",
        ),
    ] {
        let (result, deltas) =
            chat_stream_with(&fixture, messages, &ChatStreamCancel::default(), |_| {});
        assert_eq!(result.unwrap_err().code, code);
        assert!(deltas.is_empty());
    }
    assert!(fixture.api.calls().is_empty());
}

#[test]
fn chat_stream_cancelled_in_the_callback_stops_and_reports_cancelled() {
    let fixture = Fixture::signed_in();
    let cancel = ChatStreamCancel::default();
    let inner = cancel.clone();
    let (result, deltas) = chat_stream_with(&fixture, question(), &cancel, move |_| inner.cancel());
    assert_eq!(result.unwrap_err().code, "account_cancelled");
    assert_eq!(deltas, ["回答："]);

    // 开始之前就取消的不发请求。
    let (result, deltas) = chat_stream_with(&fixture, question(), &cancel, |_| {});
    assert_eq!(result.unwrap_err().code, "account_cancelled");
    assert!(deltas.is_empty());
    assert_eq!(fixture.api.calls(), ["chat_stream a", "chat_stream a"]);
}

#[test]
fn chat_stream_reply_is_cancelled_when_its_account_signs_out_midway() {
    for signed_in in [false, true] {
        let fixture = if signed_in {
            Fixture::signed_in()
        } else {
            Fixture::new(None, Some(saved('1', '2', ANONYMOUS_USER)))
        };
        let serving = Arc::downgrade(if signed_in {
            &fixture.account
        } else {
            &fixture.anonymous
        });
        *fixture.api.during_request.lock().unwrap() = Some(Box::new(move || {
            serving.upgrade().unwrap().forget().unwrap()
        }));
        let (result, deltas) =
            chat_stream_with(&fixture, question(), &ChatStreamCancel::default(), |_| {});
        assert_eq!(result.unwrap_err().code, "account_cancelled");
        assert_eq!(deltas, ["回答：", "你好"]);
    }
}

/// 一个打开了统计、记过一天字数的统计目录。
fn statistics_directory() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let store = TypingStatisticsStore::new(directory.path());
    store.set_enabled(true).unwrap();
    store
        .record(
            "合成",
            msime_client_services::typing_statistics::TypingSource::Quanpin,
            "2026-10-17",
            None,
        )
        .unwrap();
    directory
}

#[test]
fn typing_upload_turns_on_uploads_throttles_and_turns_off_with_a_delete() {
    let fixture = Fixture::signed_in();
    let directory = statistics_directory();
    let dir = directory.path().to_str().unwrap();
    let status = json!({ "operation": "typing_upload_status", "statistics_directory": dir });
    assert_eq!(
        fixture.run(status.clone()).unwrap(),
        json!({ "enabled": false, "last_upload_ms": null, "signed_in": true })
    );
    // 没打开时自动上传什么也不发。
    let upload = json!({ "operation": "typing_upload", "statistics_directory": dir, "day": "2026-10-17", "platform": "windows" });
    assert_eq!(fixture.run(upload.clone()).unwrap()["enabled"], false);
    assert!(fixture.api.calls().is_empty());
    // 打开时立刻上传一次。
    let on = fixture
        .run(json!({ "operation": "typing_upload_set", "statistics_directory": dir, "enabled": true, "day": "2026-10-17", "platform": "windows" }))
        .unwrap();
    assert_eq!(
        (on["enabled"].clone(), on["signed_in"].clone()),
        (json!(true), json!(true))
    );
    assert!(on["last_upload_ms"].as_u64().is_some_and(|ms| ms > 0));
    assert_eq!(fixture.api.calls(), ["put_typing_statistics a"]);
    // 十分钟内的自动上传不发请求，force 时照发。
    fixture.run(upload.clone()).unwrap();
    assert!(fixture.api.calls().is_empty());
    let mut forced = upload.clone();
    forced["force"] = json!(true);
    fixture.run(forced).unwrap();
    assert_eq!(fixture.api.calls(), ["put_typing_statistics a"]);
    // 关掉时先在本地关掉，再删云端副本；删除失败也保持关闭，错误交给宿主。
    fixture.api.failures.lock().unwrap().insert(
        "delete_typing_statistics",
        AccountFailure::from(AccountError::Unavailable),
    );
    let off = json!({ "operation": "typing_upload_set", "statistics_directory": dir, "enabled": false, "day": "2026-10-17", "platform": "windows" });
    assert_eq!(fixture.code(off.clone()), "account_unavailable");
    assert_eq!(fixture.run(status.clone()).unwrap()["enabled"], false);
    fixture.api.failures.lock().unwrap().clear();
    assert_eq!(
        fixture.run(off).unwrap(),
        json!({ "enabled": false, "last_upload_ms": null, "signed_in": true })
    );
    assert_eq!(
        fixture.api.calls(),
        ["delete_typing_statistics a", "delete_typing_statistics a"]
    );
}

#[test]
fn typing_upload_needs_the_signed_in_account_and_a_valid_request() {
    let fixture = Fixture::new(None, Some(saved('1', '2', ANONYMOUS_USER)));
    let directory = statistics_directory();
    let dir = directory.path().to_str().unwrap();
    // 只认登录账号，不退回匿名账号；没登录时打开是 account_unauthorized，开关不动。
    assert_eq!(
        fixture.code(json!({ "operation": "typing_upload_set", "statistics_directory": dir, "enabled": true, "day": "2026-10-17", "platform": "windows" })),
        "account_unauthorized"
    );
    assert_eq!(
        fixture
            .run(json!({ "operation": "typing_upload_status", "statistics_directory": dir }))
            .unwrap(),
        json!({ "enabled": false, "last_upload_ms": null, "signed_in": false })
    );
    // 已经打开、后来退出登录时，自动上传不发请求也不报错。
    TypingStatisticsStore::new(directory.path())
        .set_upload_enabled(true)
        .unwrap();
    assert_eq!(
        fixture
            .run(json!({ "operation": "typing_upload", "statistics_directory": dir, "day": "2026-10-17", "platform": "windows" }))
            .unwrap(),
        json!({ "enabled": true, "last_upload_ms": null, "signed_in": false })
    );
    assert!(fixture.api.calls().is_empty());
    for bad in [
        json!({ "operation": "typing_upload", "statistics_directory": "relative", "day": "2026-10-17", "platform": "windows" }),
        json!({ "operation": "typing_upload", "statistics_directory": dir, "day": "2026/10/17", "platform": "windows" }),
        json!({ "operation": "typing_upload", "statistics_directory": dir, "day": "2026-10-17", "platform": "beos" }),
        json!({ "operation": "typing_upload_set", "statistics_directory": dir, "enabled": true, "day": "", "platform": "windows" }),
    ] {
        assert_eq!(fixture.code(bad), "account_invalid");
    }
}

#[test]
fn qq_links_list_redeem_and_delete_with_the_signed_in_account() {
    let fixture = Fixture::signed_in();
    assert_eq!(
        fixture.run(json!({ "operation": "qq_links" })).unwrap(),
        json!({ "links": [{ "id": "12", "created_at": "2026-10-17T08:00:00Z" }] })
    );
    // 码前后的空白和中间的空格、连字符先去掉。
    assert_eq!(
        fixture
            .run(json!({ "operation": "qq_link_redeem", "code": " 1234-5678 " }))
            .unwrap(),
        json!({ "id": "13", "created_at": "2026-10-17T08:01:00Z" })
    );
    assert_eq!(
        fixture
            .run(json!({ "operation": "qq_link_delete", "id": "13" }))
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        fixture.api.calls(),
        ["qq_links a", "redeem_qq_link a", "delete_qq_link a"]
    );
    // 格式不对的码和编号在本地就拒绝，不发请求。
    assert_eq!(
        fixture.code(json!({ "operation": "qq_link_redeem", "code": "1234" })),
        "account_invalid"
    );
    assert_eq!(
        fixture.code(json!({ "operation": "qq_link_delete", "id": "abc" })),
        "account_invalid"
    );
    assert!(fixture.api.calls().is_empty());
    // 服务端的码原样交给宿主。
    fixture.api.failures.lock().unwrap().insert(
        "redeem_qq_link",
        AccountFailure {
            error: AccountError::NotFound,
            status: Some(404),
            server_code: Some("qq_link_code_not_found".into()),
            retry_after_seconds: None,
            network_failure: false,
        },
    );
    assert_eq!(
        fixture.code(json!({ "operation": "qq_link_redeem", "code": "12345678" })),
        "account_qq_link_code_not_found"
    );
    // 没有登录时不退回匿名账号。
    let anonymous_only = Fixture::new(None, Some(saved('1', '2', ANONYMOUS_USER)));
    assert_eq!(
        anonymous_only.code(json!({ "operation": "qq_links" })),
        "account_unauthorized"
    );
    assert!(anonymous_only.api.calls().is_empty());
}
