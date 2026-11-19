//! 空闲认领不联网；重新认证密文、检查 verified 与重置屏障后才写入 Engine。

use super::*;
use crate::account::learning_sync::pending::{Pending, PendingError, MAX_ATTEMPTS};
use sha2::{Digest, Sha256};

impl Identity {
    pub(super) fn pending_binding(
        &self,
        login: &AccountLoginBinding,
    ) -> Result<String, RoundError> {
        let mut hash = Sha256::new();
        hash.update(b"msime-learning-pending-v1\0");
        for field in [
            login.user_id.as_str(),
            &login.session_id.to_string(),
            &self.keys.device_id(),
            self.keys.ready_key_id().ok_or(RoundError::NotReady)?,
        ] {
            hash.update((field.len() as u64).to_le_bytes());
            hash.update(field.as_bytes());
        }
        Ok(hex::encode(hash.finalize()))
    }
}

impl<A: AccountApi, S: AccountSessionStorage> BackendAccountSession<A, S> {
    /// 宿主持有词库的一方在空闲时显式调用，不发出数据请求、不替代同步授权检查。
    pub fn learning_merge_pending(
        &self,
        login: &AccountLoginBinding,
        context: &RoundContext<'_>,
    ) -> Result<Vec<DownloadReport>, RoundError> {
        context.paths.validate().map_err(|_| RoundError::Local)?;
        let identity = self.learning_identity(login)?;
        self.merge_learning_pending_for_identity(login, &identity, context)
    }

    pub(super) fn merge_learning_pending_for_identity(
        &self,
        login: &AccountLoginBinding,
        identity: &Identity,
        context: &RoundContext<'_>,
    ) -> Result<Vec<DownloadReport>, RoundError> {
        self.with_learning_identity(login, identity, || {
            let queue = Pending::open(context.state_directory, identity.pending_binding(login)?)?;
            let sources = queue.sources()?;
            if sources.is_empty() {
                return Ok(Vec::new());
            }
            let _maintenance = maintenance(context.paths)?.ok_or(RoundError::Busy)?;
            let store = StateStore::new(context.state_directory);
            let mut reports = Vec::new();
            for source in sources {
                // 同一来源最多一份已认领和一份较新的待认领，不能无限重试失败文件。
                for _ in 0..2 {
                    let Some(report) =
                        apply_pending_source(&queue, &store, login, identity, context, &source)?
                    else {
                        break;
                    };
                    let failed = report.outcome == DownloadOutcome::MergeFailed;
                    reports.push(report);
                    if failed {
                        break;
                    }
                }
            }
            Ok(reports)
        })
    }
}

pub(super) fn apply_pending_source(
    queue: &Pending,
    store: &StateStore,
    login: &AccountLoginBinding,
    identity: &Identity,
    context: &RoundContext<'_>,
    source: &str,
) -> Result<Option<DownloadReport>, RoundError> {
    let mut report = DownloadReport {
        device_id: source.to_owned(),
        generation: None,
        outcome: DownloadOutcome::Undecryptable,
        dictionary_mismatch: false,
    };
    let mut entry = match queue.claim(source) {
        Ok(Some(entry)) => entry,
        Ok(None) => return Ok(None),
        Err(PendingError::Invalid) => {
            queue.discard(source)?;
            return Ok(Some(report));
        }
        Err(error) => return Err(error.into()),
    };
    let opened = identity
        .keys
        .open_blob(source, &entry.envelope)
        .filter(|opened| opened.generation == entry.generation);
    let remote = opened.and_then(|opened| read_learning_sync(Cursor::new(opened.stream)).ok());
    let Some(remote) = remote else {
        queue.discard(source)?;
        return Ok(Some(report));
    };
    report.generation = Some(entry.generation);
    report.dictionary_mismatch = remote.content_id != context.content_id;
    let key = identity.keys.ready_key_id().ok_or(RoundError::NotReady)?;
    let progress = store.read(&login.user_id)?;
    let verified = progress
        .sources
        .get(key)
        .and_then(|sources| sources.0.get(source));
    match verified {
        Some(progress) if entry.generation < progress.verified => {
            report.outcome = DownloadOutcome::Rollback;
        }
        Some(progress) if entry.generation == progress.applied => {
            report.outcome = DownloadOutcome::AlreadyApplied;
        }
        Some(source_progress) if entry.generation == source_progress.verified => {
            // 队列本身不能推进 verified；且文件中的最大时间必须已被发布前的事务观察。
            if remote.observed_max > progress.observed_max {
                queue.discard(source)?;
                return Ok(Some(report));
            }
            if entry.attempts >= MAX_ATTEMPTS {
                report.outcome = DownloadOutcome::Abandoned;
            } else {
                // 先持久记下这次尝试，进程在 SQLite 提交前后崩溃也不会无界重试。
                queue.started(source, &mut entry)?;
                if advance_learning_sync_clock(context.paths, progress.observed_max).is_ok()
                    && merge_learning_sync_state(
                        context.paths,
                        context.main_dictionary,
                        &remote.state,
                        progress.reset_floor,
                    )
                    .is_ok()
                {
                    store.record_applied(&login.user_id, key, source, entry.generation)?;
                    report.outcome = DownloadOutcome::Applied;
                } else if entry.attempts >= MAX_ATTEMPTS {
                    report.outcome = DownloadOutcome::Abandoned;
                } else {
                    report.outcome = DownloadOutcome::MergeFailed;
                    return Ok(Some(report));
                }
            }
        }
        _ => {}
    }
    queue.discard(source)?;
    Ok(Some(report))
}
