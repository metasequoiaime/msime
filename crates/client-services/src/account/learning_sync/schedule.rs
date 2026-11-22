//! 原生宿主的持久调度入口；本模块不创建线程、定时器或设备密钥。

use super::round::{RoundContext, RoundError, RoundReport};
use super::state::{ScheduleStatus, SkipReason, StateStore};
use super::transport::{BlobRequest, BlobResponse};
use crate::account::{
    AccountApi, AccountError, AccountLoginBinding, AccountSessionStorage, BackendAccountSession,
};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct ScheduledReport {
    pub skipped: Option<SkipReason>,
    pub status: ScheduleStatus,
    pub round: Option<RoundReport>,
}

pub(super) struct Lease {
    pub directory: PathBuf,
    pub revision: u64,
}

fn now_ms() -> Result<u64, RoundError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RoundError::Local)?;
    u64::try_from(elapsed.as_millis()).map_err(|_| RoundError::Local)
}

impl Lease {
    pub fn check(&self, login: &AccountLoginBinding) -> Result<(), RoundError> {
        if StateStore::new(&self.directory).schedule_authorized(login, self.revision)? {
            Ok(())
        } else {
            Err(AccountError::Cancelled.into())
        }
    }

    pub fn response(
        &self,
        login: &AccountLoginBinding,
        request: &BlobRequest<'_>,
        response: &BlobResponse,
    ) -> Result<(), RoundError> {
        let retry = if response.status == 429 {
            Some(
                now_ms()?.saturating_add(
                    response
                        .retry_after_seconds
                        .unwrap_or(60)
                        .saturating_mul(1000),
                ),
            )
        } else {
            None
        };
        let quota = response.status == 413
            && matches!(request, BlobRequest::Upload { .. })
            && response.body.len() <= 1024 * 1024
            && serde_json::from_slice::<serde_json::Value>(&response.body)
                .ok()
                .is_some_and(|body| body["error"]["code"] == "learning_quota");
        if retry.is_some() || quota {
            StateStore::new(&self.directory).restrict_schedule(login, retry, quota)?;
        }
        Ok(())
    }
}

impl<A: AccountApi, S: AccountSessionStorage> BackendAccountSession<A, S> {
    fn with_schedule<T>(
        &self,
        login: &AccountLoginBinding,
        directory: &Path,
        operation: impl FnOnce(&StateStore) -> Result<T, RoundError>,
    ) -> Result<T, RoundError> {
        if !directory.is_absolute() {
            return Err(super::state::StateError::Invalid.into());
        }
        self.storage.with_refresh_lock(|| {
            self.with_local_login(login, || Ok(operation(&StateStore::new(directory))))
        })?
    }

    /// 只读持久授权及限制，不登记设备、不生成密钥、不联网。
    pub fn learning_schedule_status(
        &self,
        login: &AccountLoginBinding,
        directory: &Path,
    ) -> Result<ScheduleStatus, RoundError> {
        self.with_schedule(login, directory, |store| Ok(store.schedule_status(login)?))
    }

    /// 只修改本机独立授权；重新登录后必须再次开启，原账号的限制仍保留。
    pub fn learning_set_sync_enabled(
        &self,
        login: &AccountLoginBinding,
        directory: &Path,
        enabled: bool,
    ) -> Result<ScheduleStatus, RoundError> {
        self.with_schedule(login, directory, |store| {
            Ok(store.set_schedule_enabled(login, enabled)?)
        })
    }

    /// 用户明确恢复额度暂停；此操作不清除服务端退避或小时节流。
    pub fn learning_resume_upload(
        &self,
        login: &AccountLoginBinding,
        directory: &Path,
    ) -> Result<ScheduleStatus, RoundError> {
        self.with_schedule(login, directory, |store| {
            Ok(store.resume_schedule_upload(login)?)
        })
    }

    /// 宿主停止、切换运行时或关闭本机学习时取消在途结果，保留用户授权与限制。
    pub fn learning_cancel_scheduled(
        &self,
        login: &AccountLoginBinding,
        directory: &Path,
    ) -> Result<ScheduleStatus, RoundError> {
        self.with_schedule(login, directory, |store| Ok(store.cancel_schedule(login)?))
    }

    /// 自动和手动共用授权、轮次锁、退避及额度暂停；手动只绕过小时节流。
    pub fn learning_sync_scheduled(
        &self,
        login: &AccountLoginBinding,
        device_name: &str,
        context: &RoundContext<'_>,
        automatic: bool,
    ) -> Result<ScheduledReport, RoundError> {
        if !context.state_directory.is_absolute() {
            return Err(super::state::StateError::Invalid.into());
        }
        context.paths.validate().map_err(|_| RoundError::Local)?;
        let store = StateStore::new(context.state_directory);
        let _round = store.try_round()?.ok_or(RoundError::Busy)?;
        let reservation = self.with_schedule(login, context.state_directory, |store| {
            Ok(store.reserve_schedule(login, now_ms()?, automatic)?)
        })?;
        let (revision, status) = match reservation {
            Ok(reservation) => reservation,
            Err(reason) => {
                return Ok(ScheduledReport {
                    skipped: Some(reason),
                    status: self.learning_schedule_status(login, context.state_directory)?,
                    round: None,
                })
            }
        };
        let lease = Lease {
            directory: context.state_directory.to_owned(),
            revision,
        };
        let round = self.learning_sync_round(
            login,
            device_name,
            context,
            Some(lease),
            status.upload_paused,
        )?;
        Ok(ScheduledReport {
            skipped: None,
            status: self.learning_schedule_status(login, context.state_directory)?,
            round: Some(round),
        })
    }
}
