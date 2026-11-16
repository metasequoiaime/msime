//! 轮次持有密钥快照；每次网络前后和本地提交时重新核对整份密钥串。

use super::*;
use crate::account::learning_sync::session::key_for_login;
use msime_client_core::cloud::learning_e2e::Keystore;

// 不实现 Debug；私钥只留在内存里，不进入轮次结果。
pub(super) struct Identity {
    encoded: String,
    pub keys: Keystore,
}

impl<A: AccountApi, S: AccountSessionStorage> BackendAccountSession<A, S> {
    pub(super) fn learning_identity(
        &self,
        login: &AccountLoginBinding,
    ) -> Result<Identity, RoundError> {
        self.storage
            .with_refresh_lock(|| {
                self.with_local_login(login, || {
                    let encoded = key_for_login(self.storage.load_learning_keystore()?, login)?
                        .ok_or(AccountError::Unavailable)?;
                    let keys = Keystore::parse(&encoded).ok_or(AccountError::Storage)?;
                    Ok(Identity { encoded, keys })
                })
            })
            .map_err(RoundError::from)
    }

    /// 先取存储锁再取会话锁；闭包不得回调账号。网络请求不能放在闭包中。
    pub(super) fn with_learning_identity<T>(
        &self,
        login: &AccountLoginBinding,
        identity: &Identity,
        operation: impl FnOnce() -> Result<T, RoundError>,
    ) -> Result<T, RoundError> {
        self.storage.with_refresh_lock(|| {
            self.with_local_login(login, || {
                if key_for_login(self.storage.load_learning_keystore()?, login)?.as_deref()
                    != Some(identity.encoded.as_str())
                {
                    return Err(AccountError::Cancelled);
                }
                Ok(operation())
            })
        })?
    }
}
