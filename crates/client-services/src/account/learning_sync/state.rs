//! `cloud-sync/learning.json` 独立保存只增的观察时间、重置屏障和设备代次。
//! 损坏或未知版本必须报错，不能退回空状态而重新接受旧密文。退出登录和换钥不清理此文件。

use crate::{file_lock, storage};
use msime_client_core::cloud::learning_e2e::MAX_GENERATION;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};

/// 与学习明文流的时间戳上限一致。
pub const MAX_TIMESTAMP: u64 = 1 << 40;
/// 超限时报错，不淘汰旧账号或防回滚下限。
pub const MAX_STATE_BYTES: usize = 4 * 1024 * 1024;
const STATE_FILE: &str = "learning.json";
const LOCK_FILE: &str = "learning.lock";

mod schedule;
pub use schedule::{ScheduleProgress, ScheduleStatus, SkipReason};

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum StateError {
    #[error("learning state unavailable")]
    Unavailable,
    #[error("invalid learning state")]
    Invalid,
    #[error("learning generation rollback")]
    Rollback,
    #[error("learning generation not verified")]
    NotVerified,
    #[error("learning generation exhausted")]
    Exhausted,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteProgress {
    pub verified: u64,
    pub applied: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Uploaded {
    pub generation: u64,
    pub key_id: String,
    /// 实际上传明文流的 SHA-256，小写十六进制；不存明文。
    pub sha256: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountProgress {
    pub observed_max: u64,
    pub reset_floor: u64,
    /// 第一层为密钥编号，第二层为认证后的来源设备编号。
    #[serde(deserialize_with = "unique_map")]
    pub sources: BTreeMap<String, Sources>,
    /// 本机设备上传代次跨换钥保留，服务端回滚后也继续递增。
    #[serde(deserialize_with = "unique_map")]
    pub uploads: BTreeMap<String, Uploaded>,
    /// 独立授权和调度限制与账号进度一起原子保存；旧文件缺此字段时不开启同步。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<ScheduleProgress>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Sources(#[serde(deserialize_with = "unique_map")] pub BTreeMap<String, RemoteProgress>);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    version: u32,
    #[serde(deserialize_with = "unique_map")]
    accounts: BTreeMap<String, AccountProgress>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            accounts: BTreeMap::new(),
        }
    }
}

// 普通 BTreeMap 反序列化会静默取重复键的最后一份，可能把屏障或防回滚下限变小。
fn unique_map<'de, D, T>(deserializer: D) -> Result<BTreeMap<String, T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Unique<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Unique<T> {
        type Value = BTreeMap<String, T>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("unique learning state keys")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> Result<Self::Value, A::Error> {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, T>()? {
                if result.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate learning state key"));
                }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(Unique(std::marker::PhantomData))
}

fn id(value: &str) -> bool {
    crate::text::is_lower_hex(value, 32)
}

fn account_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 1024 && !value.chars().any(char::is_control)
}

impl State {
    fn valid(&self) -> bool {
        self.version == 1
            && self.accounts.iter().all(|(account, progress)| {
                account_id(account)
                    && progress.reset_floor <= progress.observed_max
                    && progress.observed_max <= MAX_TIMESTAMP
                    && progress.sources.iter().all(|(key, sources)| {
                        id(key)
                            && sources.0.iter().all(|(device, source)| {
                                id(device)
                                    && source.applied <= source.verified
                                    && source.verified <= MAX_GENERATION
                            })
                    })
                    && progress.uploads.iter().all(|(device, uploaded)| {
                        id(device)
                            && id(&uploaded.key_id)
                            && (1..=MAX_GENERATION).contains(&uploaded.generation)
                            && crate::text::is_lower_hex(&uploaded.sha256, 64)
                    })
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Acceptance {
    Merge,
    AlreadyApplied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UploadPlan {
    pub generation: u64,
    /// 服务端代次耗尽时，先删本机密文，再按本机持久代次继续上传。
    pub delete_remote_first: bool,
}

#[derive(Clone)]
pub struct StateStore {
    directory: PathBuf,
}

impl StateStore {
    pub fn new(state_directory: &Path) -> Self {
        Self {
            directory: state_directory.join("cloud-sync"),
        }
    }

    /// 网络轮次跨进程串行，但不占用 Engine 的维护租约或进度文件的事务锁。
    pub(crate) fn try_round(&self) -> Result<Option<file_lock::ScopedFileLock>, StateError> {
        if !storage::create_directory_and_check(&self.directory)
            .map_err(|_| StateError::Unavailable)?
        {
            return Err(StateError::Unavailable);
        }
        let directory = file_lock::open_private_directory(&self.directory)
            .map_err(|_| StateError::Unavailable)?;
        let file =
            file_lock::open_private_lock_file_at(&directory, OsStr::new("learning-round.lock"))
                .map_err(|_| StateError::Unavailable)?;
        file_lock::ScopedFileLock::try_exclusive(file).map_err(|_| StateError::Unavailable)
    }

    fn transaction<T>(
        &self,
        account: &str,
        change: impl FnOnce(&mut AccountProgress) -> Result<T, StateError>,
    ) -> Result<T, StateError> {
        if !account_id(account) {
            return Err(StateError::Invalid);
        }
        if !storage::create_directory_and_check(&self.directory)
            .map_err(|_| StateError::Unavailable)?
        {
            return Err(StateError::Unavailable);
        }
        let directory = file_lock::open_private_directory(&self.directory)
            .map_err(|_| StateError::Unavailable)?;
        let lock = file_lock::open_private_lock_file_at(&directory, OsStr::new(LOCK_FILE))
            .map_err(|_| StateError::Unavailable)?;
        let _held =
            file_lock::ScopedFileLock::exclusive(lock).map_err(|_| StateError::Unavailable)?;
        let mut state = match file_lock::open_private_file_at(&directory, OsStr::new(STATE_FILE)) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(MAX_STATE_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| StateError::Unavailable)?;
                if bytes.len() > MAX_STATE_BYTES {
                    return Err(StateError::Invalid);
                }
                let state: State =
                    serde_json::from_slice(&bytes).map_err(|_| StateError::Invalid)?;
                if !state.valid() {
                    return Err(StateError::Invalid);
                }
                state
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(_) => return Err(StateError::Unavailable),
        };
        let before = state.clone();
        let mut progress = state.accounts.get(account).cloned().unwrap_or_default();
        let result = change(&mut progress)?;
        if progress != AccountProgress::default() || state.accounts.contains_key(account) {
            state.accounts.insert(account.to_owned(), progress);
        }
        if state != before {
            let bytes = serde_json::to_vec(&state).map_err(|_| StateError::Invalid)?;
            if !state.valid() || bytes.len() > MAX_STATE_BYTES {
                return Err(StateError::Invalid);
            }
            file_lock::write_private_file_at(&directory, OsStr::new(STATE_FILE), &bytes)
                .map_err(|_| StateError::Unavailable)?;
        }
        // 上一次替换后的目录同步可能失败；即使本次值没变，也要成功同步后才能确认屏障。
        directory.sync_all().map_err(|_| StateError::Unavailable)?;
        Ok(result)
    }

    pub fn read(&self, account: &str) -> Result<AccountProgress, StateError> {
        self.transaction(account, |progress| Ok(progress.clone()))
    }

    /// 发布本机导出之前调用；只有持久化成功，调用方才能继续发布。
    pub fn observe(&self, account: &str, maximum: u64) -> Result<(), StateError> {
        if maximum > MAX_TIMESTAMP {
            return Err(StateError::Invalid);
        }
        self.transaction(account, |progress| {
            progress.observed_max = progress.observed_max.max(maximum);
            Ok(())
        })
    }

    /// 调用方持有 Engine 独占维护权，在本函数成功后才能清空学习日志。
    /// 清空失败可重试，屏障继续保留；这里只保存屏障，不执行 Engine 重置。
    pub fn prepare_reset(
        &self,
        account: &str,
        unix_seconds: u64,
        local_maximum: u64,
    ) -> Result<u64, StateError> {
        if local_maximum > MAX_TIMESTAMP {
            return Err(StateError::Invalid);
        }
        self.transaction(account, |progress| {
            progress.observed_max = progress
                .observed_max
                .max(local_maximum)
                .max(unix_seconds.min(MAX_TIMESTAMP));
            progress.reset_floor = progress.reset_floor.max(progress.observed_max);
            Ok(progress.reset_floor)
        })
    }

    /// 仅在 AEAD、来源设备、当前密钥、元数据代次及整份明文流均校验成功后调用。
    /// 来源编号必须取认证后的头部。返回成功之前同时持久化观察时间和 verified；
    /// 同代次应用失败后仍返回 Merge，低于 verified 的代次始终拒收。
    pub fn accept_verified(
        &self,
        account: &str,
        key: &str,
        source: &str,
        generation: u64,
        observed_max: u64,
    ) -> Result<Acceptance, StateError> {
        if !id(key)
            || !id(source)
            || !(1..=MAX_GENERATION).contains(&generation)
            || observed_max > MAX_TIMESTAMP
        {
            return Err(StateError::Invalid);
        }
        self.transaction(account, |progress| {
            let remote = progress
                .sources
                .entry(key.to_owned())
                .or_default()
                .0
                .entry(source.to_owned())
                .or_default();
            if generation < remote.verified {
                return Err(StateError::Rollback);
            }
            remote.verified = generation;
            progress.observed_max = progress.observed_max.max(observed_max);
            Ok(if generation == remote.applied {
                Acceptance::AlreadyApplied
            } else {
                Acceptance::Merge
            })
        })
    }

    /// Engine 合并事务成功以后调用。未验证或已被更新代次取代的批次不能推进 applied。
    pub fn record_applied(
        &self,
        account: &str,
        key: &str,
        source: &str,
        generation: u64,
    ) -> Result<(), StateError> {
        if !id(key) || !id(source) || !(1..=MAX_GENERATION).contains(&generation) {
            return Err(StateError::Invalid);
        }
        self.transaction(account, |progress| {
            let remote = progress
                .sources
                .get_mut(key)
                .and_then(|sources| sources.0.get_mut(source))
                .ok_or(StateError::NotVerified)?;
            if generation < remote.verified {
                return Err(StateError::Rollback);
            }
            if generation > remote.verified {
                return Err(StateError::NotVerified);
            }
            remote.applied = generation;
            Ok(())
        })
    }

    /// 上传成功之前不推进本机 uploaded；多进程上传仍须由同步轮次串行，服务端使用 CAS。
    pub fn plan_upload(
        &self,
        account: &str,
        device: &str,
        server_generation: u64,
    ) -> Result<UploadPlan, StateError> {
        if !id(device) {
            return Err(StateError::Invalid);
        }
        self.transaction(account, |progress| {
            let uploaded = progress
                .uploads
                .get(device)
                .map_or(0, |upload| upload.generation);
            if uploaded == MAX_GENERATION {
                return Err(StateError::Exhausted);
            }
            let delete_remote_first = server_generation >= MAX_GENERATION;
            Ok(UploadPlan {
                generation: if delete_remote_first {
                    uploaded + 1
                } else {
                    server_generation.max(uploaded) + 1
                },
                delete_remote_first,
            })
        })
    }

    /// 服务端确认上传成功后记录实际信封代次及明文摘要；延迟返回的旧请求不回退进度。
    pub fn record_uploaded(
        &self,
        account: &str,
        device: &str,
        uploaded: Uploaded,
    ) -> Result<(), StateError> {
        if !id(device)
            || !id(&uploaded.key_id)
            || !(1..=MAX_GENERATION).contains(&uploaded.generation)
            || !crate::text::is_lower_hex(&uploaded.sha256, 64)
        {
            return Err(StateError::Invalid);
        }
        self.transaction(account, |progress| {
            if let Some(old) = progress.uploads.get(device) {
                if old.generation > uploaded.generation {
                    return Ok(());
                }
                if old.generation == uploaded.generation {
                    return if old == &uploaded {
                        Ok(())
                    } else {
                        Err(StateError::Invalid)
                    };
                }
            }
            progress.uploads.insert(device.to_owned(), uploaded);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests;
