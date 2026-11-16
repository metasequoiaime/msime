//! 跨进程操作用持久登录编号绑定，刷新令牌不改变绑定，同账号重新登录会失效。

use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountLoginBinding {
    pub user_id: String,
    pub session_id: uuid::Uuid,
}

impl<A: AccountApi, S: AccountSessionStorage> BackendAccountSession<A, S> {
    /// 只读本机登录身份；旧会话先在共享写锁内补持久编号，不返回凭据、不联网。
    pub fn login_binding(&self) -> Result<Option<AccountLoginBinding>, AccountError> {
        self.ensure_session_id()?;
        let mut state = self.lock()?;
        self.load_locked(&mut state)?;
        state
            .saved
            .as_ref()
            .map(|saved| {
                Ok(AccountLoginBinding {
                    user_id: saved.tokens.user.id.clone(),
                    session_id: saved.session_id.ok_or(AccountError::Unavailable)?,
                })
            })
            .transpose()
    }

    pub(in crate::account) fn credentials_for_login(
        &self,
        rejected_token: Option<&str>,
        expected_user_id: Option<&str>,
        expected_login: Option<&AccountLoginBinding>,
    ) -> Result<(String, String, u64), AccountError> {
        self.ensure_session_id()?;
        let matches = |saved: Option<&SavedAccountSession>| {
            expected_user_id
                .is_none_or(|user| saved.is_some_and(|saved| saved.tokens.user.id == user))
                && expected_login.is_none_or(|login| {
                    saved.is_some_and(|saved| {
                        saved.tokens.user.id == login.user_id
                            && saved.session_id == Some(login.session_id)
                    })
                })
        };
        {
            let mut state = self.lock()?;
            self.load_locked(&mut state)?;
            if !matches(state.saved.as_ref()) {
                return Err(AccountError::Cancelled);
            }
        }
        let token = self.access_token(rejected_token)?;
        let mut state = self.lock()?;
        self.load_locked(&mut state)?;
        let saved = state.saved.as_ref().ok_or(AccountError::Cancelled)?;
        if saved.tokens.access_token != token || !matches(Some(saved)) {
            return Err(AccountError::Cancelled);
        }
        Ok((saved.tokens.user.id.clone(), token, state.generation))
    }
}
