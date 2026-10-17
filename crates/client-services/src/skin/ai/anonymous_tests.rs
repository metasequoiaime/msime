//! 带匿名会话的 AI 皮肤服务（`BackendAiSkinService::with_anonymous`）：登录账号优先，没有登录时整次生成和单个任务都用匿名账号的令牌，只用登录账号的 `new` 不变。不联网。

use super::tests::{artwork, job, response, token, MemoryStorage};
use super::*;
use crate::account::{
    AccountChallenge, AccountProfile, AccountTokens, AccountUser, SavedAccountSession,
};
use std::time::SystemTime;

/// 每次 AI 请求记下 `<操作> <令牌首字符>`，按合成数据走完一次生成。
#[derive(Clone, Default)]
struct RecordingApi {
    calls: Arc<Mutex<Vec<String>>>,
}

impl RecordingApi {
    fn record(&self, operation: &str, token: &str) {
        self.calls.lock().unwrap().push(format!(
            "{operation} {}",
            token.chars().next().unwrap_or('-')
        ));
    }

    /// 记下的调用去重后按首次出现的顺序：出图在三个线程上并发，次数和交错不固定。
    fn distinct_calls(&self) -> Vec<String> {
        let mut seen = Vec::new();
        for call in self.calls.lock().unwrap().drain(..) {
            if !seen.contains(&call) {
                seen.push(call);
            }
        }
        seen
    }
}

impl AccountApi for RecordingApi {
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
}

impl AiSkinApi for RecordingApi {
    fn ai_models(&self, token: &str) -> Result<AiModelCatalog, AccountFailure> {
        self.record("models", token);
        Ok(AiModelCatalog {
            data: vec![AiModel {
                id: "synthetic-model".into(),
            }],
            default_model: "synthetic-model".into(),
        })
    }
    fn ai_chat(
        &self,
        _: &[AiChatMessage<'_>],
        _: &str,
        token: &str,
    ) -> Result<String, AccountFailure> {
        self.record("chat", token);
        Ok(response())
    }
    fn create_skin_artwork(&self, prompt: &str, token: &str) -> Result<AiSkinJob, AccountFailure> {
        self.record("create", token);
        let seed = prompt
            .bytes()
            .fold(0_u8, |sum, byte| sum.wrapping_add(byte));
        Ok(job(&format!("{seed:02x}").repeat(24), "running", None))
    }
    fn get_skin_artwork(&self, id: &str, token: &str) -> Result<AiSkinJob, AccountFailure> {
        self.record("get", token);
        Ok(job(id, "succeeded", Some(artwork())))
    }
    fn delete_skin_artwork(&self, _: &str, token: &str) -> Result<(), AccountFailure> {
        self.record("delete", token);
        Ok(())
    }
}

fn storage(session: Option<(u8, &str)>) -> MemoryStorage {
    let saved = session.map(|(access, user)| SavedAccountSession {
        tokens: AccountTokens {
            access_token: token(access),
            refresh_token: token(b'f'),
            token_type: "Bearer".into(),
            expires_in: 900,
            user: AccountUser {
                id: user.into(),
                display_name: "合成用户".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                email: None,
                avatar_url: None,
            },
        },
        expires_at_unix_ms: SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 60_000,
        session_id: None,
    });
    MemoryStorage(Arc::new(Mutex::new(saved)))
}

/// 登录会话用令牌 `a`、匿名会话用令牌 `1` 的服务。
fn service(
    signed_in: bool,
) -> (
    BackendAiSkinService<RecordingApi, MemoryStorage>,
    RecordingApi,
) {
    let api = RecordingApi::default();
    let account = Arc::new(BackendAccountSession::new(
        api.clone(),
        storage(signed_in.then_some((b'a', "synthetic-user"))),
    ));
    let anonymous = Arc::new(BackendAccountSession::new(
        api.clone(),
        storage(Some((b'1', "synthetic-anonymous"))),
    ));
    (
        BackendAiSkinService::with_anonymous(api.clone(), account, anonymous),
        api,
    )
}

#[test]
fn a_signed_out_generation_runs_entirely_on_the_anonymous_account() {
    let (service, api) = service(false);
    let proposals = service
        .generate_detailed("合成的提示词", &AtomicBool::new(false), |_| {})
        .unwrap();
    assert_eq!(proposals.len(), 3);
    assert_eq!(
        api.distinct_calls(),
        ["models 1", "chat 1", "create 1", "get 1", "delete 1"]
    );
}

#[test]
fn the_signed_in_account_comes_first() {
    let (service, api) = service(true);
    service
        .generate_detailed("合成的提示词", &AtomicBool::new(false), |_| {})
        .unwrap();
    assert_eq!(
        api.distinct_calls(),
        ["models a", "chat a", "create a", "get a", "delete a"]
    );
}

#[test]
fn signed_out_job_requests_use_the_anonymous_account() {
    let (service, api) = service(false);
    let created = service.create_artwork_job("合成插画").unwrap();
    service.artwork_job(&created.id).unwrap();
    service.delete_artwork_job(&created.id).unwrap();
    assert_eq!(api.distinct_calls(), ["create 1", "get 1", "delete 1"]);
}

#[test]
fn without_an_anonymous_session_a_signed_out_request_stays_unauthorized() {
    let api = RecordingApi::default();
    let account = Arc::new(BackendAccountSession::new(api.clone(), storage(None)));
    let anonymous = Arc::new(BackendAccountSession::new(api.clone(), storage(None)));
    let service = BackendAiSkinService::with_anonymous(api.clone(), account, anonymous);
    assert_eq!(
        service.create_artwork_job("合成插画").unwrap_err(),
        AiSkinFailure::Account(AccountError::Unauthorized.into())
    );
    assert!(api.distinct_calls().is_empty());
}
