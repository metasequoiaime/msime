//! 设置与可移植主题经独立 WebDAV 命名空间同步。宿主在工作线程调用，不能阻塞输入。
//!
//! 密钥批准复用云剪贴板协议的独立实例；密钥串目录、远端路径和加密派生标签均隔离。
//! 每轮在同一把进程间锁里拉取事件、三方合并、独占追加密文事件并按修订号保存。

use crate::account::{
    AccountClipboardDevices, AccountSessionFileLayout, FileAccountSessionStorage,
};
use crate::clipboard_relay::{RelayClipboard, RelayConfig, RelayError, RELAY_KEYSTORE_FILE};
use crate::clipboard_webdav::{
    Dav, DavHttp, DavRequest, DavResponse, Environment, HttpDav, VirtualRelay,
};
use msime_client_core::preferences::{PreferencesError, PreferencesStore};
use msime_client_core::settings_sync::{
    SettingsSyncCipher, SettingsSyncDocument, SettingsSyncError, SettingsSyncEvent,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod log;
#[cfg(test)]
mod tests;
mod transaction;

const ROOT: &str = "msime-settings/";
const KEYRING: &str = "msime-settings/keyring/";
const STATE_FILE: &str = "settings-sync-state";
const LOCK_FILE: &str = "settings-sync.lock";
const STATE_LIMIT: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettingsWebDavError {
    #[error("invalid settings sync request")]
    Invalid,
    #[error("settings sync storage is unavailable")]
    Storage,
    #[error("settings changed during sync")]
    Conflict,
    #[error("settings sync recovery needs a choice between local edits and the pending download")]
    RecoveryConflict,
    #[error("settings sync protocol rejected the document")]
    Protocol,
    #[error("settings sync is waiting for device approval")]
    Pending,
    #[error("settings sync event log is too long")]
    LogFull,
    #[error(transparent)]
    Relay(#[from] RelayError),
}

impl SettingsWebDavError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid => "invalid_settings_sync",
            Self::Storage => "settings_sync_storage",
            Self::Conflict => "settings_conflict",
            Self::RecoveryConflict => "settings_sync_recovery_conflict",
            Self::Protocol => "settings_sync_protocol",
            Self::Pending => "clipboard_device_pending",
            Self::LogFull => "settings_sync_log_full",
            Self::Relay(error) => error.code(),
        }
    }
}

impl From<SettingsSyncError> for SettingsWebDavError {
    fn from(_: SettingsSyncError) -> Self {
        Self::Protocol
    }
}

impl From<PreferencesError> for SettingsWebDavError {
    fn from(error: PreferencesError) -> Self {
        match error {
            PreferencesError::Conflict => Self::Conflict,
            _ => Self::Storage,
        }
    }
}

struct Clock;
impl Environment for Clock {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|time| time.as_millis() as u64)
            .unwrap_or_default()
    }
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// 云剪贴板协议路径被映射到设置自己的密钥串，绝不复用剪贴板设备批准或缓存。
struct KeyRingHttp {
    inner: Arc<dyn DavHttp>,
}
impl DavHttp for KeyRingHttp {
    fn send(&self, request: &DavRequest<'_>) -> Result<DavResponse, RelayError> {
        let clock = Clock;
        let dav = Dav::new(self.inner.as_ref(), &clock);
        // 不在构造器联网。重复创建返回 405，与已有协议的初始化语义相同。
        if request.method == "MKCOL" {
            dav.mkcol(ROOT)?;
            dav.mkcol(KEYRING)?;
        }
        let map = |path: &str| -> Result<String, RelayError> {
            let suffix = path
                .strip_prefix("msime-clipboard/")
                .ok_or(RelayError::Invalid)?;
            Ok(format!("{KEYRING}{suffix}"))
        };
        let path = map(request.path)?;
        let destination = request.destination.map(map).transpose()?;
        self.inner.send(&DavRequest {
            path: &path,
            destination: destination.as_deref(),
            method: request.method,
            depth: request.depth,
            body: request.body,
            limit: request.limit,
        })
    }
}

/// 本机明文基线只放应用私有目录，绑定网盘地址、账号和设备身份；应用密码不落在此处。
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredState {
    version: u64,
    relay: String,
    space: String,
    device: String,
    base: SettingsSyncDocument,
    remote: SettingsSyncEvent,
}

#[derive(Debug, Serialize)]
pub struct SettingsSyncReport {
    pub revision: u64,
    pub uploaded: bool,
    pub downloaded: bool,
    /// 存在冲突时整轮保留本机和远端，不自动发布任何一方覆盖另一方。
    pub conflicts: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SettingsSyncStatus {
    pub device_id: Option<String>,
    pub code: Option<String>,
    pub approved: bool,
    pub joined: bool,
    pub sequence: Option<u64>,
    pub pending_write: bool,
}

/// 配置由宿主提供，目录必须是本安装独占的 `settings-sync` 私有子目录。
/// 这里不改变现有偏好默认值，也不替任何宿主自动开启联网。
pub struct SettingsWebDav {
    config: RelayConfig,
    directory: PathBuf,
    device_name: String,
    http: Arc<dyn DavHttp>,
    create_identity: bool,
}

impl SettingsWebDav {
    pub fn new(
        url: &str,
        username: &str,
        password: &str,
        directory: impl Into<PathBuf>,
        device_name: &str,
    ) -> Result<Self, SettingsWebDavError> {
        let directory = directory.into();
        if !directory.is_absolute() {
            return Err(SettingsWebDavError::Invalid);
        }
        let config = RelayConfig::webdav(url, username, password)?;
        let http = Arc::new(HttpDav::new(&config)?);
        Ok(Self {
            config,
            directory,
            device_name: device_name.to_owned(),
            http,
            create_identity: true,
        })
    }

    pub fn existing_identity_only(mut self) -> Self {
        self.create_identity = false;
        self
    }

    fn storage(&self) -> FileAccountSessionStorage {
        FileAccountSessionStorage::new(&self.directory, AccountSessionFileLayout::Native)
    }

    fn lock(&self) -> Result<crate::file_lock::ScopedFileLock, SettingsWebDavError> {
        if !self.create_identity && !self.directory.join(RELAY_KEYSTORE_FILE).exists() {
            return Err(RelayError::IdentityMissing.into());
        }
        self.storage()
            .create_directory()
            .map_err(|_| SettingsWebDavError::Storage)?;
        let file = crate::file_lock::open_private_lock_file(self.directory.join(LOCK_FILE))
            .map_err(|_| SettingsWebDavError::Storage)?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if crate::file_lock::try_exclusive(&file).map_err(|_| SettingsWebDavError::Storage)? {
                return Ok(crate::file_lock::ScopedFileLock::from_locked(file));
            }
            if Instant::now() >= deadline {
                return Err(RelayError::Unavailable.into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn keyring(&self) -> RelayClipboard {
        let relay = VirtualRelay::with_http(
            self.config.clone(),
            self.directory.clone(),
            Box::new(KeyRingHttp {
                inner: self.http.clone(),
            }),
            false,
        )
        .refresh_on_next_request();
        let clipboard = RelayClipboard::with_webdav(relay, &self.directory, &self.device_name);
        if self.create_identity {
            clipboard
        } else {
            clipboard.existing_identity_only()
        }
    }

    /// 仅从私有目录读取概况，不生成身份、不建目录，也不访问网盘。
    pub fn status(&self) -> Result<SettingsSyncStatus, SettingsWebDavError> {
        let encoded = self.keyring().load_keystore()?;
        let keystore = encoded
            .as_deref()
            .and_then(crate::cloud::clipboard_e2e::Keystore::parse);
        let state = match keystore.as_ref() {
            Some(keystore) => self.load_state(&keystore.device_id())?,
            None => None,
        };
        let pending_write = match keystore.as_ref() {
            Some(keystore) => self.has_pending(&keystore.device_id())?,
            None => false,
        };
        Ok(SettingsSyncStatus {
            device_id: keystore.as_ref().map(|keystore| keystore.device_id()),
            code: keystore
                .as_ref()
                .map(|keystore| keystore.verification_code()),
            approved: keystore
                .as_ref()
                .is_some_and(|keystore| SettingsSyncCipher::from_keystore(keystore).is_some()),
            joined: state.is_some(),
            sequence: state.map(|state| state.remote.sequence),
            pending_write,
        })
    }

    pub fn devices(&self) -> Result<AccountClipboardDevices, SettingsWebDavError> {
        let _lock = self.lock()?;
        Ok(self.keyring().devices()?)
    }

    pub fn approve(&self, device_id: &str) -> Result<AccountClipboardDevices, SettingsWebDavError> {
        let _lock = self.lock()?;
        Ok(self.keyring().approve(device_id)?)
    }

    pub fn reject(&self, device_id: &str) -> Result<AccountClipboardDevices, SettingsWebDavError> {
        let _lock = self.lock()?;
        Ok(self.keyring().reject(device_id)?)
    }

    fn load_state(&self, device: &str) -> Result<Option<StoredState>, SettingsWebDavError> {
        let Some(bytes) = self
            .storage()
            .read_private(STATE_FILE, STATE_LIMIT)
            .map_err(|_| SettingsWebDavError::Storage)?
        else {
            return Ok(None);
        };
        let state: StoredState =
            serde_json::from_slice(&bytes).map_err(|_| SettingsWebDavError::Storage)?;
        if state.version != 1 {
            return Err(SettingsWebDavError::Protocol);
        }
        if state.relay != self.config.url()
            || state.space != self.config.space()
            || state.device != device
        {
            return Ok(None);
        }
        state.base.to_json()?;
        state.remote.to_json()?;
        Ok(Some(state))
    }

    /// 首次加入有远端文档时采用远端选择；之后按本机基线三方合并。每次保存都用
    /// `PreferencesStore` 的修订号比较交换，网络等待期间的设置编辑不会被覆盖。
    pub fn run(
        &self,
        preferences: &PreferencesStore,
    ) -> Result<SettingsSyncReport, SettingsWebDavError> {
        let _lock = self.lock()?;
        let keyring = self.keyring();
        keyring.devices()?;
        let encoded = keyring
            .load_keystore()?
            .ok_or(SettingsWebDavError::Pending)?;
        let keystore = crate::cloud::clipboard_e2e::Keystore::parse(&encoded)
            .ok_or(SettingsWebDavError::Storage)?;
        let cipher =
            SettingsSyncCipher::from_keystore(&keystore).ok_or(SettingsWebDavError::Pending)?;
        let device = keystore.device_id();
        let clock = Clock;
        let log = log::EventLog::new(self.http.as_ref(), &clock);
        let stored = self.recover_round(&device, preferences, &cipher, &log)?;
        let remote = log.pull(&cipher, stored.as_ref().map(|state| &state.remote))?;
        let snapshot = preferences.load()?;
        let local = SettingsSyncDocument::from_preferences(&snapshot.preferences)?;
        let merged = match remote.as_ref() {
            Some(remote) => {
                let base = stored.as_ref().map(|state| &state.base).unwrap_or(&local);
                SettingsSyncDocument::merge(base, &local, &remote.document)?
            }
            None => msime_client_core::settings_sync::SettingsSyncMerge {
                document: local.clone(),
                conflicts: Vec::new(),
            },
        };
        if !merged.conflicts.is_empty() {
            return Ok(SettingsSyncReport {
                revision: snapshot.revision,
                uploaded: false,
                downloaded: false,
                conflicts: merged
                    .conflicts
                    .iter()
                    .map(|conflict| conflict.field.clone())
                    .collect(),
            });
        }
        let applied = merged
            .document
            .apply_to_preferences(&snapshot.preferences, preferences.edition())?;
        // 先比较有效偏好，再决定上传，避免迁移标记或版本收窄导致空同步反复写日志。
        let needs_upload = remote
            .as_ref()
            .is_none_or(|remote| remote.document != merged.document);
        let event = if needs_upload {
            let sequence = match remote.as_ref() {
                Some(previous) => previous
                    .sequence
                    .checked_add(1)
                    .ok_or(SettingsWebDavError::LogFull)?,
                None => 0,
            };
            let parent = remote
                .as_ref()
                .map(SettingsSyncEvent::head)
                .transpose()?
                .unwrap_or_default();
            SettingsSyncEvent::new(sequence, &device, parent, merged.document)?
        } else {
            remote.ok_or(SettingsWebDavError::Protocol)?
        };
        self.finish_round(
            &device,
            preferences,
            &cipher,
            &log,
            transaction::Round {
                snapshot,
                applied,
                event,
                upload: needs_upload,
            },
        )
    }
}
