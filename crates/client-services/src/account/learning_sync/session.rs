//! 用持久登录身份约束 learning 请求、令牌重试与独立密钥串的跨进程读改写。

use super::transport::{BlobRequest, BlobResponse};
use crate::account::{
    AccountApi, AccountError, AccountLoginBinding, AccountSessionStorage, BackendAccountSession,
};
use msime_client_core::cloud::learning_e2e::{self, DriverError, Keystore, Operation, Persisted};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// 密钥身份与服务端登录会话同寿命；同账号重新登录也必须登记新设备。
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BoundKeystore {
    login: AccountLoginBinding,
    keystore: String,
}

fn key_for_login(
    stored: Option<String>,
    login: &AccountLoginBinding,
) -> Result<Option<String>, AccountError> {
    stored
        .map(|stored| {
            let bound: BoundKeystore =
                serde_json::from_str(&stored).map_err(|_| AccountError::Storage)?;
            Ok((bound.login == *login).then_some(bound.keystore))
        })
        .transpose()
        .map(Option::flatten)
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AccountLearningError {
    #[error(transparent)]
    Account(#[from] AccountError),
    #[error("learning operation failed: {}", .0.code)]
    Learning(DriverError),
}

impl AccountLearningError {
    pub fn code(&self) -> &str {
        match self {
            Self::Account(error) => error.code(),
            Self::Learning(error) => &error.code,
        }
    }
}

impl<A: AccountApi, S: AccountSessionStorage> BackendAccountSession<A, S> {
    /// 状态、批准、反向核对、移除、换钥和关闭都绑定到发起时的那次登录。
    pub fn learning_operation(
        &self,
        operation: Operation,
        device_name: &str,
        expected_login: Option<&AccountLoginBinding>,
    ) -> Result<Value, AccountLearningError> {
        let current;
        let login = match expected_login {
            Some(login) => login,
            None => {
                current = self.login_binding()?.ok_or(AccountError::Unauthorized)?;
                &current
            }
        };
        let (user, _, generation) = self.credentials_for_login(None, None, Some(login))?;
        let keystore = self.storage.with_refresh_lock(|| {
            self.with_generation(generation, Some(&user), || {
                key_for_login(self.storage.load_learning_keystore()?, login)
            })
        })?;
        let result = learning_e2e::run(
            operation,
            keystore.as_deref(),
            device_name,
            |request| {
                let (_, token, now) = self.credentials_for_login(None, Some(&user), Some(login))?;
                if now != generation {
                    return Err(AccountError::Cancelled);
                }
                let mut response = self.api.learning_e2e(request, &token)?;
                if response.status == 401 {
                    let (_, refreshed, now) =
                        self.credentials_for_login(Some(&token), Some(&user), Some(login))?;
                    if now != generation {
                        return Err(AccountError::Cancelled);
                    }
                    response = self.api.learning_e2e(request, &refreshed)?;
                }
                self.with_generation(generation, Some(&user), || Ok(response))
            },
            |value| {
                self.storage.with_refresh_lock(|| {
                    self.with_generation(generation, Some(&user), || {
                        let stored = key_for_login(self.storage.load_learning_keystore()?, login)?;
                        match Keystore::other_identity(stored, value) {
                            Some(stored) => Ok(Persisted::Conflict(stored)),
                            None => {
                                let bound = serde_json::to_string(&BoundKeystore {
                                    login: login.clone(),
                                    keystore: value.to_owned(),
                                })
                                .map_err(|_| AccountError::Storage)?;
                                self.storage
                                    .save_learning_keystore(&bound)
                                    .map(|()| Persisted::Saved)
                            }
                        }
                    })
                })
            },
            || AccountError::Unavailable,
        )?;
        self.with_generation(generation, Some(&user), || Ok(()))?;
        result.map_err(AccountLearningError::Learning)
    }

    /// 上传下载轮次共用的认证传输；同账号重新登录也不能沿用旧轮次。
    pub fn learning_blob_for_login(
        &self,
        request: &BlobRequest<'_>,
        login: &AccountLoginBinding,
    ) -> Result<BlobResponse, AccountError> {
        let (user, token, generation) = self.credentials_for_login(None, None, Some(login))?;
        let mut response = self.api.learning_blob(request, &token)?;
        if response.status == 401 {
            let (_, refreshed, now) =
                self.credentials_for_login(Some(&token), Some(&user), Some(login))?;
            if now != generation {
                return Err(AccountError::Cancelled);
            }
            response = self.api.learning_blob(request, &refreshed)?;
        }
        self.with_generation(generation, Some(&user), || Ok(response))
    }
}

#[cfg(test)]
mod tests;
