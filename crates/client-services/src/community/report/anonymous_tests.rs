//! `BackendCommunityReportService::report_with_anonymous`：登录账号优先，没有登录时用本机匿名账号举报；只用登录账号的 `report` 不变。不联网。

use super::*;
use crate::account::{
    AccountChallenge, AccountProfile, AccountTokens, AccountUser, SavedAccountSession,
};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

fn token(character: char) -> String {
    character.to_string().repeat(64)
}

#[derive(Clone, Default)]
struct MemoryStorage(Arc<Mutex<Option<SavedAccountSession>>>);

impl MemoryStorage {
    fn with(access: char, user: &str) -> Self {
        Self(Arc::new(Mutex::new(Some(SavedAccountSession {
            tokens: AccountTokens {
                access_token: token(access),
                refresh_token: token('f'),
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
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                + 60_000,
            session_id: None,
        }))))
    }
}

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

/// 记下每次举报用的令牌首字符；`failure` 为 `Some` 时举报以它失败。
#[derive(Clone, Default)]
struct RecordingApi {
    reports: Arc<Mutex<Vec<char>>>,
    failure: Arc<Mutex<Option<AccountError>>>,
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

impl CommunityReportApi for RecordingApi {
    fn report_community_item(&self, _: &CommunityReport, token: &str) -> Result<(), AccountError> {
        self.reports
            .lock()
            .unwrap()
            .push(token.chars().next().unwrap());
        self.failure.lock().unwrap().clone().map_or(Ok(()), Err)
    }
}

fn report() -> CommunityReport {
    CommunityReport::new(
        CommunityReportKind::Skins,
        Uuid::parse_str("10000000-0000-4000-8000-000000000001").unwrap(),
        CommunityReportReason::Spam,
        "合成举报",
    )
    .unwrap()
}

fn fixture(
    signed_in: bool,
    anonymous: bool,
) -> (
    BackendCommunityReportService<RecordingApi, MemoryStorage>,
    BackendAccountSession<RecordingApi, MemoryStorage>,
    RecordingApi,
) {
    let api = RecordingApi::default();
    let account = if signed_in {
        MemoryStorage::with('a', "synthetic-user")
    } else {
        MemoryStorage::default()
    };
    let anonymous_storage = if anonymous {
        MemoryStorage::with('1', "synthetic-anonymous")
    } else {
        MemoryStorage::default()
    };
    (
        BackendCommunityReportService::new(
            api.clone(),
            Arc::new(BackendAccountSession::new(api.clone(), account)),
        ),
        BackendAccountSession::new(api.clone(), anonymous_storage),
        api,
    )
}

#[test]
fn a_signed_out_report_is_filed_with_the_anonymous_account() {
    let (service, anonymous, api) = fixture(false, true);
    service
        .report_with_anonymous(&anonymous, &report())
        .unwrap();
    assert_eq!(*api.reports.lock().unwrap(), ['1']);
    // 只用登录账号的 report 仍要求登录。
    assert_eq!(service.report(&report()), Err(AccountError::Unauthorized));
    assert_eq!(*api.reports.lock().unwrap(), ['1']);
}

#[test]
fn the_signed_in_account_comes_first_and_server_errors_keep_their_codes() {
    let (service, anonymous, api) = fixture(true, true);
    service
        .report_with_anonymous(&anonymous, &report())
        .unwrap();
    *api.failure.lock().unwrap() = Some(AccountError::RateLimited);
    assert_eq!(
        service.report_with_anonymous(&anonymous, &report()),
        Err(AccountError::RateLimited)
    );
    assert_eq!(*api.reports.lock().unwrap(), ['a', 'a']);
}

#[test]
fn without_either_session_it_is_unauthorized_before_the_network() {
    let (service, anonymous, api) = fixture(false, false);
    assert_eq!(
        service.report_with_anonymous(&anonymous, &report()),
        Err(AccountError::Unauthorized)
    );
    assert!(api.reports.lock().unwrap().is_empty());
}
