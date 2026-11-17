//! 授权绑定具体登录；关闭、重启、重新登录都不能清掉账号的节流和额度限制。

use super::*;
use crate::account::AccountLoginBinding;

pub const AUTOMATIC_INTERVAL_MS: u64 = 60 * 60 * 1000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleProgress {
    pub session_id: uuid::Uuid,
    pub enabled: bool,
    pub revision: u64,
    pub automatic_after_ms: u64,
    pub retry_after_ms: u64,
    pub upload_paused: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ScheduleStatus {
    pub enabled: bool,
    pub automatic_after_ms: u64,
    pub retry_after_ms: u64,
    pub upload_paused: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    Disabled,
    Throttled,
    Backoff,
}

impl ScheduleProgress {
    fn new(login: &AccountLoginBinding) -> Self {
        Self {
            session_id: login.session_id,
            enabled: false,
            revision: 0,
            automatic_after_ms: 0,
            retry_after_ms: 0,
            upload_paused: false,
        }
    }

    fn status(&self, login: &AccountLoginBinding) -> ScheduleStatus {
        ScheduleStatus {
            enabled: self.enabled && self.session_id == login.session_id,
            automatic_after_ms: self.automatic_after_ms,
            retry_after_ms: self.retry_after_ms,
            upload_paused: self.upload_paused,
        }
    }

    fn changed(&mut self) -> Result<(), StateError> {
        self.revision = self.revision.checked_add(1).ok_or(StateError::Exhausted)?;
        Ok(())
    }
}

impl StateStore {
    pub(in crate::account::learning_sync) fn schedule_status(
        &self,
        login: &AccountLoginBinding,
    ) -> Result<ScheduleStatus, StateError> {
        self.transaction(&login.user_id, |progress| {
            Ok(progress
                .schedule
                .as_ref()
                .unwrap_or(&ScheduleProgress::new(login))
                .status(login))
        })
    }

    pub(in crate::account::learning_sync) fn set_schedule_enabled(
        &self,
        login: &AccountLoginBinding,
        enabled: bool,
    ) -> Result<ScheduleStatus, StateError> {
        self.transaction(&login.user_id, |progress| {
            let state = progress
                .schedule
                .get_or_insert_with(|| ScheduleProgress::new(login));
            if state.session_id != login.session_id || state.enabled != enabled {
                state.changed()?;
                state.session_id = login.session_id;
                state.enabled = enabled;
            }
            Ok(state.status(login))
        })
    }

    pub(in crate::account::learning_sync) fn resume_schedule_upload(
        &self,
        login: &AccountLoginBinding,
    ) -> Result<ScheduleStatus, StateError> {
        self.transaction(&login.user_id, |progress| {
            let state = progress
                .schedule
                .get_or_insert_with(|| ScheduleProgress::new(login));
            if state.session_id != login.session_id || !state.enabled {
                return Err(StateError::Invalid);
            }
            state.changed()?;
            state.upload_paused = false;
            Ok(state.status(login))
        })
    }

    /// 生命周期只使在途轮次失效，不替用户关闭独立授权，也不清除任何限制。
    pub(in crate::account::learning_sync) fn cancel_schedule(
        &self,
        login: &AccountLoginBinding,
    ) -> Result<ScheduleStatus, StateError> {
        self.transaction(&login.user_id, |progress| {
            let Some(state) = &mut progress.schedule else {
                return Ok(ScheduleProgress::new(login).status(login));
            };
            if state.status(login).enabled {
                state.changed()?;
            }
            Ok(state.status(login))
        })
    }

    /// 网络前先持久占用本小时；进程在发出请求后崩溃也不能靠重启重复自动上传。
    pub(in crate::account::learning_sync) fn reserve_schedule(
        &self,
        login: &AccountLoginBinding,
        now_ms: u64,
        automatic: bool,
    ) -> Result<Result<(u64, ScheduleStatus), SkipReason>, StateError> {
        self.transaction(&login.user_id, |progress| {
            let state = progress
                .schedule
                .get_or_insert_with(|| ScheduleProgress::new(login));
            if !state.status(login).enabled {
                return Ok(Err(SkipReason::Disabled));
            }
            if now_ms < state.retry_after_ms || state.retry_after_ms == u64::MAX {
                return Ok(Err(SkipReason::Backoff));
            }
            if automatic
                && (now_ms < state.automatic_after_ms || state.automatic_after_ms == u64::MAX)
            {
                return Ok(Err(SkipReason::Throttled));
            }
            state.automatic_after_ms = state
                .automatic_after_ms
                .max(now_ms.saturating_add(AUTOMATIC_INTERVAL_MS));
            Ok(Ok((state.revision, state.status(login))))
        })
    }

    pub(in crate::account::learning_sync) fn schedule_authorized(
        &self,
        login: &AccountLoginBinding,
        revision: u64,
    ) -> Result<bool, StateError> {
        self.transaction(&login.user_id, |progress| {
            Ok(progress
                .schedule
                .as_ref()
                .is_some_and(|state| state.status(login).enabled && state.revision == revision))
        })
    }

    /// 失败响应也要先记限制，再检查轮次是否取消；关闭后立刻重开不能绕过在途的 429。
    pub(in crate::account::learning_sync) fn restrict_schedule(
        &self,
        login: &AccountLoginBinding,
        retry_after_ms: Option<u64>,
        upload_paused: bool,
    ) -> Result<(), StateError> {
        self.transaction(&login.user_id, |progress| {
            let state = progress
                .schedule
                .get_or_insert_with(|| ScheduleProgress::new(login));
            if let Some(until) = retry_after_ms {
                state.retry_after_ms = state.retry_after_ms.max(until);
            }
            state.upload_paused |= upload_paused;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests;
