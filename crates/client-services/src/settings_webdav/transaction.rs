//! 先记本机意图，再发布事件、写回偏好和推进基线；中途退出时不把下载重放成本机编辑。
use super::*;
use msime_client_core::preferences::{Preferences, PreferencesSnapshot};

const PENDING_FILE: &str = "settings-sync-pending";
const PENDING_LIMIT: u64 = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Prepared,
    Published,
    NotApplied,
}

/// 只记录可移植投影，不把偏好中的凭据或本机字段复制到事务文件。
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingWrite {
    version: u64,
    relay: String,
    space: String,
    device: String,
    revision: u64,
    before: SettingsSyncDocument,
    target: SettingsSyncDocument,
    event: SettingsSyncEvent,
    phase: Phase,
}

pub(super) struct Round {
    pub snapshot: PreferencesSnapshot,
    pub applied: Preferences,
    pub event: SettingsSyncEvent,
    pub upload: bool,
}

impl SettingsWebDav {
    fn save_private_json(
        &self,
        name: &str,
        value: &impl Serialize,
        limit: u64,
    ) -> Result<(), SettingsWebDavError> {
        let bytes = serde_json::to_vec(value).map_err(|_| SettingsWebDavError::Storage)?;
        if bytes.len() as u64 > limit {
            return Err(SettingsWebDavError::Storage);
        }
        self.storage()
            .write_private(name, &bytes)
            .map_err(|_| SettingsWebDavError::Storage)
    }

    fn clear_pending(&self) -> Result<(), SettingsWebDavError> {
        match crate::storage::remove_private_file(&self.directory.join(PENDING_FILE)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(SettingsWebDavError::Storage),
        }
    }

    fn load_pending(&self, device: &str) -> Result<Option<PendingWrite>, SettingsWebDavError> {
        let Some(bytes) = self
            .storage()
            .read_private(PENDING_FILE, PENDING_LIMIT)
            .map_err(|_| SettingsWebDavError::Storage)?
        else {
            return Ok(None);
        };
        let pending: PendingWrite =
            serde_json::from_slice(&bytes).map_err(|_| SettingsWebDavError::Storage)?;
        if pending.version != 1 {
            return Err(SettingsWebDavError::Protocol);
        }
        if pending.relay != self.config.url()
            || pending.space != self.config.space()
            || pending.device != device
        {
            return Ok(None);
        }
        pending.before.to_json()?;
        pending.target.to_json()?;
        pending.event.to_json()?;
        Ok(Some(pending))
    }

    pub(super) fn has_pending(&self, device: &str) -> Result<bool, SettingsWebDavError> {
        Ok(self.load_pending(device)?.is_some())
    }

    fn commit_round(
        &self,
        device: &str,
        base: SettingsSyncDocument,
        event: SettingsSyncEvent,
    ) -> Result<StoredState, SettingsWebDavError> {
        let state = StoredState {
            version: 1,
            relay: self.config.url().to_owned(),
            space: self.config.space(),
            device: device.to_owned(),
            base,
            remote: event,
        };
        self.save_private_json(STATE_FILE, &state, STATE_LIMIT)?;
        self.clear_pending()?;
        Ok(state)
    }

    /// 外层持有整轮锁。已确认的事件也是防回退锚点，不能为了恢复而忘掉它。
    pub(super) fn recover_round(
        &self,
        device: &str,
        preferences: &PreferencesStore,
        cipher: &SettingsSyncCipher,
        log: &log::EventLog<'_>,
    ) -> Result<Option<StoredState>, SettingsWebDavError> {
        let stored = self.load_state(device)?;
        let Some(pending) = self.load_pending(device)? else {
            return Ok(stored);
        };
        if stored.as_ref().is_some_and(|state| {
            state.remote == pending.event
                && (state.base == pending.target
                    || (pending.phase == Phase::NotApplied && state.base == pending.before))
        }) {
            // 基线已提交而清理事务文件失败；期间的新编辑仍按已提交基线比较。
            self.clear_pending()?;
            return Ok(stored);
        }
        let base = match pending.phase {
            Phase::Prepared => {
                let latest = log.pull(cipher, stored.as_ref().map(|state| &state.remote))?;
                let published = latest
                    .as_ref()
                    .is_some_and(|event| event.sequence >= pending.event.sequence)
                    && log.read_event(cipher, pending.event.sequence)?.as_ref()
                        == Some(&pending.event);
                if !published {
                    // 未取得序号或请求没有落地；偏好尚未写回，沿用旧基线重新计算。
                    self.clear_pending()?;
                    return Ok(stored);
                }
                // MOVE 已成功但响应丢失：只确认那次已经发布的本机快照。
                pending.before.clone()
            }
            Phase::NotApplied => {
                log.pull(cipher, Some(&pending.event))?;
                pending.before.clone()
            }
            Phase::Published => {
                log.pull(cipher, Some(&pending.event))?;
                let current = preferences.load()?;
                let document = SettingsSyncDocument::from_preferences(&current.preferences)?;
                if document == pending.target {
                    pending.target.clone()
                } else if current.revision == pending.revision && document == pending.before {
                    pending.before.clone()
                } else {
                    // 无法区分“写回后又编辑”和“写回前发生并发编辑”，保留双方，
                    // 不能凭修订号增加过就认定下载已生效，更不能恢复整份旧偏好。
                    return Err(SettingsWebDavError::RecoveryConflict);
                }
            }
        };
        self.commit_round(device, base, pending.event).map(Some)
    }

    pub(super) fn finish_round(
        &self,
        device: &str,
        preferences: &PreferencesStore,
        cipher: &SettingsSyncCipher,
        log: &log::EventLog<'_>,
        round: Round,
    ) -> Result<SettingsSyncReport, SettingsWebDavError> {
        let downloaded = round.applied != round.snapshot.preferences;
        let mut pending = PendingWrite {
            version: 1,
            relay: self.config.url().to_owned(),
            space: self.config.space(),
            device: device.to_owned(),
            revision: round.snapshot.revision,
            before: SettingsSyncDocument::from_preferences(&round.snapshot.preferences)?,
            target: SettingsSyncDocument::from_preferences(&round.applied)?,
            event: round.event,
            phase: if round.upload {
                Phase::Prepared
            } else {
                Phase::Published
            },
        };
        self.save_private_json(PENDING_FILE, &pending, PENDING_LIMIT)?;
        if round.upload {
            log.append(cipher, &pending.event)?;
            pending.phase = Phase::Published;
            self.save_private_json(PENDING_FILE, &pending, PENDING_LIMIT)?;
        }
        let saved = if downloaded {
            match preferences.save(round.snapshot.revision, round.applied) {
                Ok(saved) => saved,
                Err(PreferencesError::Conflict) => {
                    // CAS 明确拒绝时知道没有写回，可以确认已发布快照，保留更新的编辑。
                    pending.phase = Phase::NotApplied;
                    self.save_private_json(PENDING_FILE, &pending, PENDING_LIMIT)?;
                    self.commit_round(device, pending.before, pending.event)?;
                    return Err(SettingsWebDavError::Conflict);
                }
                Err(error) => return Err(error.into()),
            }
        } else {
            pending.phase = Phase::NotApplied;
            self.save_private_json(PENDING_FILE, &pending, PENDING_LIMIT)?;
            round.snapshot
        };
        self.commit_round(
            device,
            SettingsSyncDocument::from_preferences(&saved.preferences)?,
            pending.event,
        )?;
        Ok(SettingsSyncReport {
            revision: saved.revision,
            uploaded: round.upload,
            downloaded,
            conflicts: Vec::new(),
        })
    }
}
