//! 显式的一次学习同步：先上传本机，再逐份认证、完整校验和合并。
//! 维护租约忙时保留已认证密文，空闲认领时重新校验；尚无自动调度。

mod identity;
mod metadata;
mod pending;

use super::pending::Pending;
use super::state::{Acceptance, StateError, StateStore, Uploaded};
use super::transport::{BlobRequest, BlobResponse};
use super::upload::{prepare_upload, PreparedUpload};
use super::AccountLearningError;
use crate::account::{
    AccountApi, AccountError, AccountLoginBinding, AccountSessionStorage, BackendAccountSession,
};
use crate::dictionary::access::DictionaryAccess;
use identity::Identity;
use metadata::{Blob, Metadata};
use msime_client_core::cloud::learning_e2e::{Operation, SealError};
use msime_engine::{
    advance_learning_sync_clock, merge_learning_sync_state, read_learning_sync,
    stream_learning_sync_state, LearningSyncReadError, LearningSyncState, RuntimePaths,
};
use std::io::Cursor;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum RoundError {
    #[error(transparent)]
    Account(#[from] AccountError),
    #[error(transparent)]
    Learning(#[from] AccountLearningError),
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    Pending(#[from] super::PendingError),
    #[error("learning sync is busy")]
    Busy,
    #[error("learning key not ready")]
    NotReady,
    #[error("learning key changed")]
    KeyChanged,
    #[error("invalid learning metadata")]
    InvalidMetadata,
    #[error("local learning state unavailable")]
    Local,
    #[error("learning stream preparation failed")]
    Stream(#[from] LearningSyncReadError),
    #[error("learning encryption failed")]
    Seal(SealError),
    #[error("learning HTTP error {status}")]
    Http {
        status: u16,
        retry_after_seconds: Option<u64>,
    },
}

/// 只返回进度和固定状态，不带学习原文、密文、服务端错误正文或密钥。
#[derive(Debug, Default)]
pub struct RoundReport {
    pub uploaded: Option<u64>,
    pub upload_unchanged: bool,
    pub dropped_weights: usize,
    pub upload_error: Option<RoundError>,
    pub downloads: Vec<DownloadReport>,
    pub pending: Vec<DownloadReport>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DownloadReport {
    pub device_id: String,
    /// 仅在信封认证与完整流校验成功后填写，不信任元数据或本地文件头。
    pub generation: Option<u64>,
    pub outcome: DownloadOutcome,
    pub dictionary_mismatch: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub enum DownloadOutcome {
    Applied,
    AlreadyApplied,
    Undecryptable,
    Rollback,
    /// 已持久保存认证后的密文和 verified，等待独占维护权，不推进 applied。
    Pending,
    MergeFailed,
    Abandoned,
    Http {
        status: u16,
        retry_after_seconds: Option<u64>,
    },
}

/// 宿主显式传入已选定的一代词库；调用方负责本机学习开关与同步授权。
/// 当前仅供手动轮次接线，自动调度须补持久节流后再使用。
pub struct RoundContext<'a> {
    pub paths: &'a RuntimePaths,
    pub state_directory: &'a Path,
    pub content_id: &'a str,
    pub main_dictionary: bool,
}

impl<A: AccountApi, S: AccountSessionStorage> BackendAccountSession<A, S> {
    pub fn learning_sync_once(
        &self,
        login: &AccountLoginBinding,
        device_name: &str,
        context: &RoundContext<'_>,
    ) -> Result<RoundReport, RoundError> {
        context.paths.validate().map_err(|_| RoundError::Local)?;
        let store = StateStore::new(context.state_directory);
        let _round = store.try_round()?.ok_or(RoundError::Busy)?;
        let status = self.learning_operation(Operation::Status, device_name, Some(login))?;
        if status["status"] != "ready" {
            return Err(RoundError::NotReady);
        }
        let identity = self.learning_identity(login)?;
        let own_id = identity.keys.device_id();
        let key = identity.keys.ready_key_id().ok_or(RoundError::NotReady)?;
        let metadata = self.learning_round_request(
            &BlobRequest::Metadata { device_id: &own_id },
            login,
            &identity,
        )?;
        successful(&metadata)?;
        let metadata = Metadata::read(&metadata.body, &own_id, key)?;

        let (local, observed_max) = self.with_learning_identity(login, &identity, || {
            let _maintenance = maintenance(context.paths)?.ok_or(RoundError::Busy)?;
            let mut state = LearningSyncState::default();
            let exported = stream_learning_sync_state(context.paths, &mut |record| {
                state.insert(record.clone()).is_ok()
            })
            .map_err(|_| RoundError::Local)?;
            store.observe(&login.user_id, exported.observed_max)?;
            advance_learning_sync_clock(context.paths, store.read(&login.user_id)?.observed_max)
                .map_err(|_| RoundError::Local)?;
            Ok((state, exported.observed_max))
        })?;
        // 压缩和截断在释放维护权以后执行；导出已先持久观察，失败不能发布半份快照。
        let prepared = prepare_upload(&local, context.content_id, observed_max);
        drop(local);
        let mut report = RoundReport::default();
        match prepared {
            Ok(prepared) => {
                report.dropped_weights = prepared.dropped_weights;
                match self.learning_round_upload(login, &identity, &store, &metadata, &prepared) {
                    Ok(Some(generation)) => report.uploaded = Some(generation),
                    Ok(None) => report.upload_unchanged = true,
                    Err(error) if upload_failure_can_continue(&error) => {
                        report.upload_error = Some(error)
                    }
                    Err(error) => return Err(error),
                }
            }
            Err(LearningSyncReadError::TooLarge) => {
                report.upload_error = Some(RoundError::Stream(LearningSyncReadError::TooLarge));
            }
            Err(error) => return Err(error.into()),
        }
        // 首次本机上传和远端合并明确有先后顺序，不能提前把远端状态混进本机首份。
        match self.merge_learning_pending_for_identity(login, &identity, context) {
            Ok(pending) => report.pending = pending,
            Err(RoundError::Busy) => {}
            Err(error) => return Err(error),
        }
        for blob in metadata
            .blobs
            .iter()
            .filter(|blob| blob.device_id != own_id)
        {
            // 旧队列已处理同代次或更新的一份，保留本轮真实结果并避免重复下载或重试。
            if let Some(index) = report.pending.iter().rposition(|pending| {
                pending.device_id == blob.device_id
                    && pending
                        .generation
                        .is_some_and(|generation| generation >= blob.revision)
                    && matches!(
                        pending.outcome,
                        DownloadOutcome::Applied
                            | DownloadOutcome::MergeFailed
                            | DownloadOutcome::Abandoned
                    )
            }) {
                report.downloads.push(report.pending.remove(index));
            } else {
                report
                    .downloads
                    .push(self.learning_round_download(login, &identity, &store, context, blob)?);
            }
        }
        self.with_learning_identity(login, &identity, || Ok(()))?;
        Ok(report)
    }

    fn learning_round_request(
        &self,
        request: &BlobRequest<'_>,
        login: &AccountLoginBinding,
        identity: &Identity,
    ) -> Result<BlobResponse, RoundError> {
        self.with_learning_identity(login, identity, || Ok(()))?;
        let response = self.learning_blob_for_login(request, login)?;
        self.with_learning_identity(login, identity, || Ok(()))?;
        Ok(response)
    }

    fn learning_round_upload(
        &self,
        login: &AccountLoginBinding,
        identity: &Identity,
        store: &StateStore,
        metadata: &Metadata,
        prepared: &PreparedUpload,
    ) -> Result<Option<u64>, RoundError> {
        let device = identity.keys.device_id();
        let key = identity.keys.ready_key_id().ok_or(RoundError::NotReady)?;
        let server = metadata.blobs.iter().find(|blob| blob.device_id == device);
        let (unchanged, plan) = self.with_learning_identity(login, identity, || {
            let progress = store.read(&login.user_id)?;
            let unchanged = progress.uploads.get(&device).is_some_and(|uploaded| {
                uploaded.key_id == key
                    && uploaded.sha256 == prepared.sha256
                    && server.is_some_and(|server| server.revision == uploaded.generation)
            });
            let plan = if unchanged {
                None
            } else {
                Some(store.plan_upload(
                    &login.user_id,
                    &device,
                    server.map_or(0, |blob| blob.revision),
                )?)
            };
            Ok((unchanged, plan))
        })?;
        if unchanged {
            return Ok(None);
        }
        let plan = plan.ok_or(RoundError::Local)?;
        let envelope = prepared
            .seal(&identity.keys, plan.generation)
            .map_err(RoundError::Seal)?;
        if plan.delete_remote_first {
            let response = self.learning_round_request(
                &BlobRequest::Delete { device_id: &device },
                login,
                identity,
            )?;
            if response.status != 404 {
                successful(&response)?;
            }
        }
        let response = self.learning_round_request(
            &BlobRequest::Upload {
                device_id: &device,
                generation: plan.generation,
                envelope: &envelope,
            },
            login,
            identity,
        )?;
        successful(&response)?;
        self.with_learning_identity(login, identity, || {
            store.record_uploaded(
                &login.user_id,
                &device,
                Uploaded {
                    generation: plan.generation,
                    key_id: key.to_owned(),
                    sha256: prepared.sha256.clone(),
                },
            )?;
            Ok(())
        })?;
        Ok(Some(plan.generation))
    }

    fn learning_round_download(
        &self,
        login: &AccountLoginBinding,
        identity: &Identity,
        store: &StateStore,
        context: &RoundContext<'_>,
        blob: &Blob,
    ) -> Result<DownloadReport, RoundError> {
        let mut report = DownloadReport {
            device_id: blob.device_id.clone(),
            generation: None,
            outcome: DownloadOutcome::Undecryptable,
            dictionary_mismatch: false,
        };
        let response = self.learning_round_request(
            &BlobRequest::Download {
                device_id: &blob.device_id,
            },
            login,
            identity,
        )?;
        if !(200..300).contains(&response.status) {
            report.outcome = DownloadOutcome::Http {
                status: response.status,
                retry_after_seconds: response.retry_after_seconds,
            };
            return Ok(report);
        }
        // open_blob 同时认证请求设备、当前密钥和头部代次，元数据不能冒充认证来源。
        let Some(opened) = identity.keys.open_blob(&blob.device_id, &response.body) else {
            return Ok(report);
        };
        if opened.generation < blob.revision {
            return Ok(report);
        }
        let Ok(remote) = read_learning_sync(Cursor::new(opened.stream)) else {
            return Ok(report);
        };
        report.generation = Some(opened.generation);
        report.dictionary_mismatch = remote.content_id != context.content_id;
        let observed_max = remote.observed_max;
        drop(remote);
        report.outcome = self.with_learning_identity(login, identity, || {
            let key = identity.keys.ready_key_id().ok_or(RoundError::NotReady)?;
            let acceptance = match store.accept_verified(
                &login.user_id,
                key,
                &blob.device_id,
                opened.generation,
                observed_max,
            ) {
                Ok(acceptance) => acceptance,
                Err(StateError::Rollback) => return Ok(DownloadOutcome::Rollback),
                Err(error) => return Err(error.into()),
            };
            if acceptance == Acceptance::AlreadyApplied {
                return Ok(DownloadOutcome::AlreadyApplied);
            }
            let queue = Pending::open(context.state_directory, identity.pending_binding(login)?)?;
            queue.publish(&blob.device_id, opened.generation, &response.body)?;
            let Some(_maintenance) = maintenance(context.paths)? else {
                return Ok(DownloadOutcome::Pending);
            };
            for _ in 0..2 {
                let is_current = match queue.claim(&blob.device_id) {
                    Ok(Some(entry)) => entry.generation == opened.generation,
                    Ok(None) => return Err(RoundError::Local),
                    Err(super::PendingError::Invalid) => false,
                    Err(error) => return Err(error.into()),
                };
                let applied = pending::apply_pending_source(
                    &queue,
                    store,
                    login,
                    identity,
                    context,
                    &blob.device_id,
                )?
                .ok_or(RoundError::Local)?;
                // 先清理旧认领；当前下载的结果只取自当前代次，不能误报旧文件的结果。
                if is_current {
                    return Ok(applied.outcome);
                }
            }
            Err(RoundError::Local)
        })?;
        Ok(report)
    }
}

fn maintenance(paths: &RuntimePaths) -> Result<Option<DictionaryAccess>, RoundError> {
    DictionaryAccess::try_maintenance(&paths.user_data, &paths.dictionaries)
        .map_err(|_| RoundError::Local)
}

fn successful(response: &BlobResponse) -> Result<(), RoundError> {
    if (200..300).contains(&response.status) {
        Ok(())
    } else {
        Err(RoundError::Http {
            status: response.status,
            retry_after_seconds: response.retry_after_seconds,
        })
    }
}

fn upload_failure_can_continue(error: &RoundError) -> bool {
    matches!(
        error,
        RoundError::Http { status: 413, .. } | RoundError::Seal(SealError::TooLarge)
    )
}
