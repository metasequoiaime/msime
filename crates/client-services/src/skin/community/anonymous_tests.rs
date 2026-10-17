//! `BackendCommunitySkinService::download_with_anonymous`：登录账号优先，没有登录时用本机匿名账号下载，401 后只在同一个账号里刷新重试；只用登录账号的 `download` 不变。不联网。

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

fn tokens(access: char, refresh: char, user: &str) -> AccountTokens {
    AccountTokens {
        access_token: token(access),
        refresh_token: token(refresh),
        token_type: "Bearer".into(),
        expires_in: 900,
        user: AccountUser {
            id: user.into(),
            display_name: "合成用户".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            email: None,
            avatar_url: None,
        },
    }
}

#[derive(Clone, Default)]
struct MemoryStorage(Arc<Mutex<Option<SavedAccountSession>>>);

impl MemoryStorage {
    fn with(access: char, refresh: char, user: &str) -> Self {
        Self(Arc::new(Mutex::new(Some(SavedAccountSession {
            tokens: tokens(access, refresh, user),
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

/// 记下每次下载用的令牌首字符；`stale` 里的令牌以 401 拒绝。匿名账号的刷新令牌 `2` 换成 `3`。
#[derive(Clone, Default)]
struct RecordingApi {
    downloads: Arc<Mutex<Vec<char>>>,
    stale: Arc<Mutex<Vec<String>>>,
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
    fn refresh(&self, refresh_token: &str) -> Result<AccountTokens, AccountError> {
        if refresh_token == token('2') {
            Ok(tokens('3', '4', "synthetic-anonymous"))
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
    fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
        unreachable!("logout")
    }
    fn delete_account(&self, _: &str) -> Result<(), AccountError> {
        unreachable!("delete_account")
    }
}

impl CommunitySkinApi for RecordingApi {
    fn community_skins(
        &self,
        _: usize,
        _: &str,
        _: bool,
        _: Option<SkinCategory>,
        _: Option<&str>,
    ) -> Result<CommunitySkinPage, AccountError> {
        unreachable!("community_skins")
    }
    fn community_skin(&self, _: Uuid, _: Option<&str>) -> Result<CommunitySkin, AccountError> {
        unreachable!("community_skin")
    }
    fn download_community_skin(
        &self,
        _: Uuid,
        bearer: &str,
    ) -> Result<TouchKeyboardSkinDesign, AccountError> {
        self.downloads
            .lock()
            .unwrap()
            .push(bearer.chars().next().unwrap());
        if self
            .stale
            .lock()
            .unwrap()
            .iter()
            .any(|stale| stale == bearer)
        {
            return Err(AccountError::Unauthorized);
        }
        Ok(TouchKeyboardSkinDesign::default())
    }
    fn rate_community_skin(&self, _: Uuid, _: u8, _: &str) -> Result<(), AccountError> {
        unreachable!("rate_community_skin")
    }
    fn publish_community_skin(
        &self,
        _: Uuid,
        _: &str,
        _: &str,
        _: &TouchKeyboardSkinDesign,
        _: Option<SkinCategory>,
        _: &str,
    ) -> Result<(), AccountError> {
        unreachable!("publish_community_skin")
    }
    fn unpublish_community_skin(&self, _: Uuid, _: &str) -> Result<(), AccountError> {
        unreachable!("unpublish_community_skin")
    }
    fn set_community_skin_category(
        &self,
        _: Uuid,
        _: SkinCategory,
        _: &str,
    ) -> Result<CommunitySkin, AccountError> {
        unreachable!("set_community_skin_category")
    }
}

fn item() -> Uuid {
    Uuid::parse_str("10000000-0000-4000-8000-000000000001").unwrap()
}

/// 登录会话（给了时令牌是 `a`）、匿名会话（给了时令牌是 `1`、刷新令牌 `2`）和下载服务。
fn fixture(
    signed_in: bool,
    anonymous: bool,
) -> (
    BackendCommunitySkinService<RecordingApi, MemoryStorage>,
    BackendAccountSession<RecordingApi, MemoryStorage>,
    RecordingApi,
) {
    let api = RecordingApi::default();
    let account = if signed_in {
        MemoryStorage::with('a', 'b', "synthetic-user")
    } else {
        MemoryStorage::default()
    };
    let anonymous_storage = if anonymous {
        MemoryStorage::with('1', '2', "synthetic-anonymous")
    } else {
        MemoryStorage::default()
    };
    (
        BackendCommunitySkinService::new(
            api.clone(),
            Arc::new(BackendAccountSession::new(api.clone(), account)),
        ),
        BackendAccountSession::new(api.clone(), anonymous_storage),
        api,
    )
}

#[test]
fn a_signed_out_download_uses_the_anonymous_account() {
    let (service, anonymous, api) = fixture(false, true);
    assert_eq!(
        service.download_with_anonymous(&anonymous, item()).unwrap(),
        TouchKeyboardSkinDesign::default()
    );
    assert_eq!(*api.downloads.lock().unwrap(), ['1']);
    // 只用登录账号的 download 仍要求登录。
    assert_eq!(service.download(item()), Err(AccountError::Unauthorized));
    assert_eq!(*api.downloads.lock().unwrap(), ['1']);
}

#[test]
fn the_signed_in_account_comes_first() {
    let (service, anonymous, api) = fixture(true, true);
    service.download_with_anonymous(&anonymous, item()).unwrap();
    assert_eq!(*api.downloads.lock().unwrap(), ['a']);
}

#[test]
fn a_rejected_anonymous_token_is_refreshed_within_the_anonymous_account() {
    let (service, anonymous, api) = fixture(false, true);
    api.stale.lock().unwrap().push(token('1'));
    service.download_with_anonymous(&anonymous, item()).unwrap();
    assert_eq!(*api.downloads.lock().unwrap(), ['1', '3']);
}

#[test]
fn without_either_session_it_is_unauthorized_before_the_network() {
    let (service, anonymous, api) = fixture(false, false);
    assert_eq!(
        service.download_with_anonymous(&anonymous, item()),
        Err(AccountError::Unauthorized)
    );
    assert_eq!(
        service.download_with_anonymous(&anonymous, Uuid::nil()),
        Err(AccountError::Invalid)
    );
    assert!(api.downloads.lock().unwrap().is_empty());
}
