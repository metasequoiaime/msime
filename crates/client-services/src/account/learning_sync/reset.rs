//! 账号屏障、Engine 时钟下限与清除操作共用一次独占维护边界。

use super::round::{RoundContext, RoundError};
use super::state::StateStore;
use crate::account::{
    AccountApi, AccountLoginBinding, AccountSessionStorage, BackendAccountSession,
};
use crate::dictionary::access::DictionaryAccess;
use msime_engine::{advance_learning_sync_clock, stream_learning_sync_state};
use std::time::{SystemTime, UNIX_EPOCH};

impl<A: AccountApi, S: AccountSessionStorage> BackendAccountSession<A, S> {
    /// 只清本机：不需要 ready 密钥或同步授权，不联网、不刷新过期令牌。
    /// 清除闭包在屏障持久化和时钟推进成功后执行，且始终持有维护权。
    /// 闭包不能再次取账号锁或 Engine 维护权；失败保留屏障，允许重试。
    pub fn with_learning_reset<T>(
        &self,
        login: &AccountLoginBinding,
        context: &RoundContext<'_>,
        clear: impl FnOnce() -> Result<T, RoundError>,
    ) -> Result<T, RoundError> {
        context.paths.validate().map_err(|_| RoundError::Local)?;
        // 与同步轮次保持存储锁 → 会话锁 → 维护租约 → 进度文件锁的次序。
        self.storage.with_refresh_lock(|| {
            self.with_local_login(login, || {
                Ok((|| {
                    let _maintenance = DictionaryAccess::try_maintenance(
                        &context.paths.user_data,
                        &context.paths.dictionaries,
                    )
                    .map_err(|_| RoundError::Local)?
                    .ok_or(RoundError::Busy)?;
                    let exported = stream_learning_sync_state(context.paths, &mut |_| true)
                        .map_err(|_| RoundError::Local)?;
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_err(|_| RoundError::Local)?
                        .as_secs();
                    let floor = StateStore::new(context.state_directory).prepare_reset(
                        &login.user_id,
                        now,
                        exported.observed_max,
                    )?;
                    advance_learning_sync_clock(context.paths, floor)
                        .map_err(|_| RoundError::Local)?;
                    clear()
                })())
            })
        })?
    }
}
