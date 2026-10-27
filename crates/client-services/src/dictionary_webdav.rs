//! 「我的词条」经用户的 WebDAV 网盘同步：把不碰网络的协议核心（`msime_client_core::cloud::dictionary_sync`）接到网盘上。存储布局、合并规则、上限和请求量是契约，见 `docs/dictionary-webdav-sync.md`。
//!
//! 网盘上分两部分，都在用户选的文件夹下的 `msime-dictionary/` 里：
//!
//! - `msime-dictionary/v1/`：词库的事件日志和检查点，由 [`DavStore`] 按核心的接口读写，独占创建与云剪贴板同一条代码路径（`clipboard_webdav::Dav`）。
//! - `msime-dictionary/keyring/`：词库自己的密钥串。它就是云剪贴板端到端加密的另一个实例——同一个驱动、同一个日志核心、同一个虚拟中继——只是每个请求的路径前面加上这个前缀（[`Prefixed`]），所以里面是一份完整的云剪贴板 WebDAV 布局。词库只用它的设备批准和密钥，不用条目。
//!
//! 密钥串的全部用法收在 [`KeyRingAccess`] 一处：云剪贴板的驱动在改（例如同一台设备重复登记的修复），这里只依赖它的设备操作和「读出本机密钥串」。
//!
//! 本机的东西都在宿主给的一个目录里（同一台设备的输入法、设置应用和面板要给同一个）：同步状态 [`STATE_FILE`]（里面有词条明文，只放本机私有目录）、密钥串 `cloud-clipboard-relay-keystore` 和它的日志缓存 `cloud-clipboard-webdav-state`（文件名沿用云剪贴板的，目录不同所以互不相干），以及两把锁：整次同步持有 [`LOCK_FILE`]，密钥串的每个操作在里面再持有 `cloud-clipboard-relay.lock`，顺序总是这样，不会互相等死。
//!
//! 每个方法都会联网（[`DictionarySync::status`] 除外），不要在界面线程或输入线程上调用。

use crate::account::{
    AccountClipboardDevices, AccountSessionFileLayout, FileAccountSessionStorage,
};
use crate::clipboard_relay::{RelayClipboard, RelayConfig, RelayError};
use crate::clipboard_webdav::{
    Dav, DavHttp, DavRequest, DavResponse, Environment as DavEnvironment, HttpDav, VirtualRelay,
    WEBDAV_STATE_FILE,
};
use crate::cloud::clipboard_e2e::Keystore;
use crate::cloud::dictionary_sync::{
    self as core, DataKey, KeyRing, LocalDictionary, Options, Parts, Plan, RemoteStore, Report,
    StateSink, SyncError, SyncState, Trigger,
};
use crate::preferences::DictionarySyncWebDavPreferences;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 本机同步状态的文件名。
pub const STATE_FILE: &str = "dictionary-sync-state";
/// 整次同步持有的锁。
pub const LOCK_FILE: &str = "dictionary-sync.lock";
const ROOT: &str = "msime-dictionary/";
const KEYRING: &str = "msime-dictionary/keyring/";
const TMP: &str = "msime-dictionary/v1/tmp/";
/// 同步状态里有整个共享词库（10 万个词条约 10 MB）。
const MAX_STATE_BYTES: u64 = 64 * 1024 * 1024;
/// 等另一个进程跑完一次同步。一次同步最多几十个请求，每个 30 秒超时。
const LOCK_TIMEOUT: Duration = Duration::from_secs(120);
const LOCK_POLL: Duration = Duration::from_millis(50);
/// 临时文件放了这么久还在，说明写入的设备在 PUT 和 MOVE 之间中断了。
const STALE_TMP_SECS: i64 = 600;

/// 词库同步失败的原因。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DictionarySyncError {
    /// 偏好里没有配置词库同步，或者地址、账号、应用密码不合规。
    #[error("dictionary sync is not configured")]
    NotConfigured,
    /// 请求本身不合规（设备编号、目录），没有发出去。
    #[error("invalid dictionary sync request")]
    Invalid,
    #[error(transparent)]
    Sync(#[from] SyncError),
    /// 密钥串的驱动给出的结果，例如本机还在等待批准（`clipboard_device_pending`）、要批准的设备不存在（`not_found`）。
    #[error("dictionary sync key ring operation failed: {0}")]
    KeyRing(String),
}

impl DictionarySyncError {
    /// 交给宿主的错误码。
    pub fn code(&self) -> &str {
        match self {
            Self::NotConfigured => "dictionary_sync_not_configured",
            Self::Invalid => "invalid_dictionary_sync",
            Self::Sync(error) => error.code(),
            Self::KeyRing(code) => code,
        }
    }

    fn from_relay(error: RelayError) -> Self {
        match error {
            RelayError::NotConfigured => Self::NotConfigured,
            RelayError::Invalid => Self::Invalid,
            RelayError::Unauthorized => Self::Sync(SyncError::Unauthorized),
            RelayError::Unavailable => Self::Sync(SyncError::Unavailable),
            RelayError::Storage => Self::Sync(SyncError::Storage),
            RelayError::RateLimited => Self::Sync(SyncError::RateLimited),
            RelayError::Rollback => Self::Sync(SyncError::Rollback),
            RelayError::Unsupported => Self::Sync(SyncError::Unsupported),
            error @ RelayError::Clipboard(_) => Self::KeyRing(error.code().to_owned()),
        }
    }
}

// ---- 网盘 ----

/// 每个请求的路径前面加上 `prefix`：把一整套云剪贴板 WebDAV 布局放进 `msime-dictionary/keyring/` 下。顺带数一数发了多少个请求，计进词库同步的额度。
struct Prefixed {
    inner: Box<dyn DavHttp>,
    prefix: &'static str,
    requests: Arc<AtomicUsize>,
}

impl DavHttp for Prefixed {
    fn send(&self, request: &DavRequest<'_>) -> Result<DavResponse, RelayError> {
        self.requests.fetch_add(1, Ordering::SeqCst);
        let path = format!("{}{}", self.prefix, request.path);
        let destination = request
            .destination
            .map(|destination| format!("{}{destination}", self.prefix));
        self.inner.send(&DavRequest {
            method: request.method,
            path: &path,
            depth: request.depth,
            destination: destination.as_deref(),
            body: request.body,
            limit: request.limit,
        })
    }
}

/// 限流退避用的真实时钟。
struct WallClock;

impl DavEnvironment for WallClock {
    fn now_ms(&self) -> u64 {
        now_ms()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

/// 核心要的网盘：`msime-dictionary/v1/` 下的列目录、读、独占创建、建目录和删除。401 是凭据不对，429、503 已经按 `Retry-After` 退避过，成功响应超过上限是「响应过大」，其余连不上或答得不对都是「连不上」。
pub struct DavStore<'a> {
    dav: Dav<'a>,
    http: &'a HttpDav,
    tmp_ready: bool,
}

impl<'a> DavStore<'a> {
    fn new(http: &'a HttpDav, clock: &'a WallClock) -> Self {
        Self {
            dav: Dav::new(http, clock),
            http,
            tmp_ready: false,
        }
    }

    fn map(&self, error: RelayError) -> SyncError {
        if self.http.take_oversized() {
            return SyncError::TooLarge;
        }
        match error {
            RelayError::Unauthorized => SyncError::Unauthorized,
            RelayError::RateLimited => SyncError::RateLimited,
            RelayError::Rollback => SyncError::Rollback,
            RelayError::Unsupported => SyncError::Unsupported,
            _ => SyncError::Unavailable,
        }
    }

    /// 删掉放了 10 分钟以上的临时文件（写入的设备在 PUT 和 MOVE 之间中断留下的）。失败不要紧。
    fn sweep_tmp(&mut self, now: i64) {
        let Ok(Some(names)) = self.dav.list(TMP) else {
            return;
        };
        for name in names {
            let written = name
                .strip_suffix(".json")
                .and_then(|stem| stem.split_once('-'))
                .filter(|(_, random)| crate::cloud::clipboard_log::is_lower_hex(random, 32))
                .and_then(|(time, _)| time.parse::<i64>().ok());
            if written.is_some_and(|written| written < now - STALE_TMP_SECS) {
                let _ = self.dav.delete(&format!("{TMP}{name}"));
            }
        }
    }
}

impl RemoteStore for DavStore<'_> {
    fn list(&mut self, dir: &str) -> Result<Option<Vec<String>>, SyncError> {
        self.dav.list(dir).map_err(|error| self.map(error))
    }

    fn get(&mut self, path: &str, limit: u64) -> Result<Option<Vec<u8>>, SyncError> {
        self.dav.get(path, limit).map_err(|error| self.map(error))
    }

    fn create(&mut self, path: &str, bytes: &[u8]) -> Result<bool, SyncError> {
        if !self.tmp_ready {
            self.dav.mkcol(TMP).map_err(|error| self.map(error))?;
            self.tmp_ready = true;
        }
        self.dav
            .create_in(TMP, path, bytes)
            .map_err(|error| self.map(error))
    }

    fn mkcol(&mut self, dir: &str) -> Result<(), SyncError> {
        self.dav
            .mkcol(dir)
            .map(|_| ())
            .map_err(|error| self.map(error))
    }

    fn delete(&mut self, path: &str) -> Result<(), SyncError> {
        self.dav
            .delete(path)
            .map(|_| ())
            .map_err(|error| self.map(error))
    }
}

// ---- 密钥串 ----

/// 词库的密钥串：云剪贴板端到端加密协议在 `msime-dictionary/keyring/` 下的另一个实例。词库只经这里用它。
struct KeyRingAccess {
    clipboard: RelayClipboard,
    requests: Arc<AtomicUsize>,
    directory: PathBuf,
    config: RelayConfig,
}

impl KeyRingAccess {
    fn new(
        config: &RelayConfig,
        directory: &Path,
        device_name: &str,
    ) -> Result<Self, DictionarySyncError> {
        let requests = Arc::new(AtomicUsize::new(0));
        let http = Prefixed {
            inner: Box::new(HttpDav::new(config).map_err(DictionarySyncError::from_relay)?),
            prefix: KEYRING,
            requests: requests.clone(),
        };
        let relay = VirtualRelay::with_http(
            config.clone(),
            directory.to_path_buf(),
            Box::new(http),
            false,
        );
        Ok(Self {
            clipboard: RelayClipboard::with_webdav(relay, directory, device_name),
            requests,
            directory: directory.to_path_buf(),
            config: config.clone(),
        })
    }

    fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }

    /// 本机的密钥串；还没有时为 `None`。
    fn keystore(&self) -> Result<Option<Keystore>, DictionarySyncError> {
        Ok(self
            .clipboard
            .load_keystore()
            .map_err(DictionarySyncError::from_relay)?
            .as_deref()
            .and_then(Keystore::parse))
    }

    /// 第一次用这个网盘时先建好 `msime-dictionary/` 和 `keyring/`：密钥串的虚拟中继只会在它自己的目录里建子目录。
    fn ensure_roots(&self) -> Result<(), DictionarySyncError> {
        if self.directory.join(WEBDAV_STATE_FILE).exists() {
            return Ok(());
        }
        let http = HttpDav::new(&self.config).map_err(DictionarySyncError::from_relay)?;
        let clock = WallClock;
        let dav = Dav::new(&http, &clock);
        for dir in [ROOT, KEYRING] {
            self.requests.fetch_add(1, Ordering::SeqCst);
            dav.mkcol(dir).map_err(DictionarySyncError::from_relay)?;
        }
        Ok(())
    }

    /// 读密钥串的状态：本机还没登记就登记，网盘上还没有密钥就由本机生成（第一台设备）。
    fn status(&self) -> Result<AccountClipboardDevices, DictionarySyncError> {
        self.ensure_roots()?;
        self.clipboard
            .devices()
            .map_err(DictionarySyncError::from_relay)
    }

    fn approve(&self, device_id: &str) -> Result<AccountClipboardDevices, DictionarySyncError> {
        self.clipboard
            .approve(device_id)
            .map_err(DictionarySyncError::from_relay)
    }

    fn reject(&self, device_id: &str) -> Result<AccountClipboardDevices, DictionarySyncError> {
        self.clipboard
            .reject(device_id)
            .map_err(DictionarySyncError::from_relay)
    }

    fn remove(&self, device_id: &str) -> Result<AccountClipboardDevices, DictionarySyncError> {
        self.clipboard
            .remove(device_id)
            .map_err(DictionarySyncError::from_relay)
    }

    fn reset(&self) -> Result<AccountClipboardDevices, DictionarySyncError> {
        self.ensure_roots()?;
        self.clipboard
            .reset()
            .map_err(DictionarySyncError::from_relay)
    }
}

// ---- 本机状态 ----

/// 写盘的同步状态记着它属于哪个网盘地址和账号，换了任何一个都当作没有。
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredState {
    relay: String,
    space: String,
    state: SyncState,
}

struct FileSink<'a> {
    storage: FileAccountSessionStorage,
    config: &'a RelayConfig,
}

impl StateSink for FileSink<'_> {
    fn save(&mut self, state: &SyncState) -> Result<(), SyncError> {
        let stored = StoredState {
            relay: self.config.url().to_owned(),
            space: self.config.space(),
            state: state.clone(),
        };
        let bytes = serde_json::to_vec(&stored).map_err(|_| SyncError::Storage)?;
        self.storage
            .write_private(STATE_FILE, &bytes)
            .map_err(|_| SyncError::Storage)
    }
}

/// 时钟和随机数取自系统。
struct SystemEnvironment;

impl core::Environment for SystemEnvironment {
    fn now_ms(&self) -> u64 {
        now_ms()
    }

    fn random(&mut self, bytes: &mut [u8]) -> Result<(), SyncError> {
        getrandom::fill(bytes).map_err(|_| SyncError::Unavailable)
    }
}

/// 给宿主看的概况。不含词条、凭据和密钥。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SyncView {
    /// 本机在词库密钥串里的设备编号和核对码；还没用过网盘时为空。
    pub device_id: Option<String>,
    pub code: Option<String>,
    /// 本机持有当前密钥（已获准，或者是第一台设备）。
    pub approved: bool,
    /// 已经和本机词库对过。
    pub joined: bool,
    /// 共享词库里的词条数。
    pub words: usize,
    /// 还没发出去的改动数，和还没写进本机词库的改动数。
    pub pending_upload: usize,
    pub pending_apply: usize,
    /// 被挡下的大批删除：本机少了这么多词。用户选择同步删除（`confirm_deletions`）或者从网盘恢复（`restore_deletions`）。
    pub held_deletions: Option<usize>,
    /// 收到过比本机时间快一天以上的时间戳。
    pub clock_skew: bool,
    /// 最后一次联网同步成功的时间（Unix 毫秒）和最后一次失败的错误码。
    pub last_success_ms: Option<u64>,
    pub last_error: Option<String>,
}

/// 一次同步的结果。
#[derive(Debug)]
pub struct RunResult {
    /// 这一次联网了没有。
    pub network: bool,
    pub report: Result<Report, DictionarySyncError>,
    /// 宿主下一次什么时候再调用（距现在的毫秒数）。
    pub next_check_ms: u64,
    pub view: SyncView,
}

/// 一个配置好的词库同步：网盘地址、账号和应用密码，加上本机放状态的目录。
pub struct DictionarySync {
    config: RelayConfig,
    directory: PathBuf,
    device_name: String,
}

impl std::fmt::Debug for DictionarySync {
    // 账号和应用密码不进任何日志。
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DictionarySync")
            .field("url", &self.config.url())
            .field("directory", &self.directory)
            .finish_non_exhaustive()
    }
}

impl DictionarySync {
    /// 偏好里的词库同步；没有配置时为 `None`。`directory` 是放本机状态的绝对路径目录，同一台设备上的各个进程要给同一个；`device_name` 是登记设备时显示给其他设备的名称。
    pub fn from_preferences(
        preferences: &DictionarySyncWebDavPreferences,
        directory: impl Into<PathBuf>,
        device_name: &str,
    ) -> Result<Option<Self>, DictionarySyncError> {
        if !preferences.is_configured() {
            return Ok(None);
        }
        Self::new(
            &preferences.url,
            &preferences.username,
            &preferences.password,
            directory,
            device_name,
        )
        .map(Some)
    }

    /// 地址、账号和应用密码的规则与云剪贴板的网盘相同（[`RelayConfig::webdav`]）。
    pub fn new(
        url: &str,
        username: &str,
        password: &str,
        directory: impl Into<PathBuf>,
        device_name: &str,
    ) -> Result<Self, DictionarySyncError> {
        let directory = directory.into();
        if !directory.is_absolute() {
            return Err(DictionarySyncError::Invalid);
        }
        let config = RelayConfig::webdav(url, username, password)
            .map_err(|_| DictionarySyncError::NotConfigured)?;
        Ok(Self {
            config,
            directory,
            device_name: device_name.to_owned(),
        })
    }

    fn storage(&self) -> FileAccountSessionStorage {
        FileAccountSessionStorage::new(&self.directory, AccountSessionFileLayout::Native)
    }

    fn sink(&self) -> FileSink<'_> {
        FileSink {
            storage: self.storage(),
            config: &self.config,
        }
    }

    fn lock(&self) -> Result<crate::file_lock::ScopedFileLock, DictionarySyncError> {
        let storage = self.storage();
        storage.create_directory().map_err(|_| SyncError::Storage)?;
        let file = crate::file_lock::open_private_lock_file(self.directory.join(LOCK_FILE))
            .map_err(|_| SyncError::Storage)?;
        let deadline = Instant::now() + LOCK_TIMEOUT;
        loop {
            if crate::file_lock::try_exclusive(&file).map_err(|_| SyncError::Storage)? {
                return Ok(crate::file_lock::ScopedFileLock::from_locked(file));
            }
            if Instant::now() >= deadline {
                return Err(SyncError::Unavailable.into());
            }
            std::thread::sleep(LOCK_POLL);
        }
    }

    /// 本机的同步状态；没有、读不懂、或者属于别的网盘地址或账号时为 `None`。
    fn load_state(&self) -> Result<Option<SyncState>, DictionarySyncError> {
        let Some(bytes) = self
            .storage()
            .read_private(STATE_FILE, MAX_STATE_BYTES)
            .map_err(|_| SyncError::Storage)?
        else {
            return Ok(None);
        };
        let Ok(stored) = serde_json::from_slice::<StoredState>(&bytes) else {
            return Ok(None);
        };
        Ok(
            (stored.relay == self.config.url() && stored.space == self.config.space())
                .then_some(stored.state),
        )
    }

    fn keyring(&self) -> Result<KeyRingAccess, DictionarySyncError> {
        KeyRingAccess::new(&self.config, &self.directory, &self.device_name)
    }

    fn view(state: Option<&SyncState>, keystore: Option<&Keystore>) -> SyncView {
        let status = state.map(SyncState::status);
        SyncView {
            device_id: keystore.map(Keystore::device_id),
            code: keystore.map(Keystore::verification_code),
            approved: keystore.is_some_and(|keystore| DataKey::from_keystore(keystore).is_some()),
            joined: status.as_ref().is_some_and(|status| status.joined),
            words: status.as_ref().map_or(0, |status| status.words),
            pending_upload: status.as_ref().map_or(0, |status| status.outbox),
            pending_apply: status.as_ref().map_or(0, |status| status.pending_apply),
            held_deletions: status
                .as_ref()
                .and_then(|status| status.guard)
                .map(|guard| guard.deletions),
            clock_skew: status.as_ref().is_some_and(|status| status.skew),
            last_success_ms: state
                .map(|state| state.schedule.last_success_ms())
                .filter(|at| *at > 0),
            last_error: state.and_then(|state| state.schedule.last_error().map(str::to_owned)),
        }
    }

    /// 不联网的概况：读本机的同步状态和密钥串。
    pub fn status(&self) -> Result<SyncView, DictionarySyncError> {
        let keystore = self.keyring()?.keystore()?;
        let state = self.load_state()?;
        Ok(Self::view(state.as_ref(), keystore.as_ref()))
    }

    /// 一次同步：记录本机改动，按调度决定要不要联网（`trigger`），联网时先读密钥串（必要时登记本机、第一台设备生成密钥），再按核心的步骤拉取、写入本机词库、发送和压缩。`choice` 里只有用户对被挡下删除的选择会被采用。
    pub fn run(
        &self,
        local: &mut dyn LocalDictionary,
        trigger: Trigger,
        choice: Options,
    ) -> Result<RunResult, DictionarySyncError> {
        let _lock = self.lock()?;
        self.run_locked(local, trigger, choice)
    }

    fn run_locked(
        &self,
        local: &mut dyn LocalDictionary,
        trigger: Trigger,
        choice: Options,
    ) -> Result<RunResult, DictionarySyncError> {
        let now = now_ms();
        let mut stored = self.load_state()?;
        let keyring = self.keyring()?;
        let mut keystore = keyring.keystore()?;
        let decided = match stored.as_mut() {
            Some(state) => core::plan(state, trigger, now),
            // 还没用过网盘：除了「刚改了词」以外都先去登记，不然什么也做不了。
            None => Plan {
                network: trigger != Trigger::LocalChange,
                push: true,
            },
        };
        let mut keyring_error = None;
        if decided.network {
            match keyring.status() {
                Ok(_) => keystore = keyring.keystore()?,
                Err(error) => keyring_error = Some(error),
            }
        }
        let Some(keystore) = keystore else {
            return match keyring_error {
                Some(error) => Err(error),
                None => Ok(RunResult {
                    network: false,
                    report: Ok(Report::default()),
                    next_check_ms: core::schedule::PULL_INTERVAL_MS,
                    view: Self::view(None, None),
                }),
            };
        };
        let device = keystore.device_id();
        // 设备身份变了（密钥串没了、重新生成）就是一台新设备：从头加入，本机的词都保留。
        let mut state = match stored {
            Some(state) if state.device == device => state,
            _ => SyncState::new(&device).ok_or(DictionarySyncError::Invalid)?,
        };
        core::record_requests(&mut state, keyring.requests(), now);
        let mut sink = self.sink();
        if let Some(error) = keyring_error {
            if let DictionarySyncError::Sync(sync) = &error {
                core::record_failure(&mut state, *sync, now);
            }
            let _ = sink.save(&state);
            return Err(error);
        }
        let ring = KeyRing {
            current: DataKey::from_keystore(&keystore),
        };
        let http = HttpDav::new(&self.config).map_err(DictionarySyncError::from_relay)?;
        let clock = WallClock;
        let mut store = DavStore::new(&http, &clock);
        let mut env = SystemEnvironment;
        let outcome = {
            let mut parts = Parts {
                remote: &mut store,
                local,
                env: &mut env,
                sink: &mut sink,
            };
            core::execute(&mut state, &mut parts, &ring, trigger, decided, choice)
        };
        if outcome.result.as_ref().is_ok_and(|report| report.compacted) {
            store.sweep_tmp((now / 1000) as i64);
        }
        Ok(RunResult {
            network: outcome.network,
            report: outcome.result.map_err(DictionarySyncError::from),
            next_check_ms: outcome.next_check_ms,
            view: Self::view(Some(&state), Some(&keystore)),
        })
    }

    /// 词库密钥串里的设备，以及本机的状态和核对码，形状与云剪贴板的设备管理相同（`docs/cloud-clipboard-e2e.md` 的「结果」）。本机还没登记时登记，网盘上还没有密钥时由本机生成。
    pub fn devices(&self) -> Result<AccountClipboardDevices, DictionarySyncError> {
        let _lock = self.lock()?;
        self.keyring()?.status()
    }

    /// 批准一台等待中的设备：把当前密钥封装给它。
    pub fn approve(&self, device_id: &str) -> Result<AccountClipboardDevices, DictionarySyncError> {
        valid_device(device_id)?;
        let _lock = self.lock()?;
        self.keyring()?.approve(device_id)
    }

    /// 拒绝一台等待中的设备：删掉它的登记。
    pub fn reject(&self, device_id: &str) -> Result<AccountClipboardDevices, DictionarySyncError> {
        valid_device(device_id)?;
        let _lock = self.lock()?;
        self.keyring()?.reject(device_id)
    }

    /// 移除一台设备：密钥串换钥（新密钥只给本机，其他设备要重新批准），日志里宣布移除，接着同步一次，把共享词库用新密钥重写一份检查点。
    pub fn remove(
        &self,
        local: &mut dyn LocalDictionary,
        device_id: &str,
    ) -> Result<(AccountClipboardDevices, RunResult), DictionarySyncError> {
        valid_device(device_id)?;
        let _lock = self.lock()?;
        let devices = self.keyring()?.remove(device_id)?;
        if let Some(mut state) = self.load_state()? {
            state.retire(device_id);
            self.sink().save(&state)?;
        }
        let run = self.run_locked(local, Trigger::Manual, Options::default())?;
        Ok((devices, run))
    }

    /// 换一把只给本机的新密钥，其他设备都要重新批准；接着同步一次，用新密钥重写检查点。
    pub fn reset(
        &self,
        local: &mut dyn LocalDictionary,
    ) -> Result<(AccountClipboardDevices, RunResult), DictionarySyncError> {
        let _lock = self.lock()?;
        let devices = self.keyring()?.reset()?;
        let run = self.run_locked(local, Trigger::Manual, Options::default())?;
        Ok((devices, run))
    }

    /// 以本机为准重新开始（`docs/dictionary-webdav-sync.md` 的「以本机为准重新开始」）：持有旧密钥的设备都不在了、本机只有新密钥时由用户选择。
    pub fn restart(
        &self,
        local: &mut dyn LocalDictionary,
    ) -> Result<SyncView, DictionarySyncError> {
        let _lock = self.lock()?;
        let keyring = self.keyring()?;
        keyring.status()?;
        let keystore = keyring.keystore()?.ok_or(SyncError::Unavailable)?;
        let ring = KeyRing {
            current: DataKey::from_keystore(&keystore),
        };
        let mut state = match self.load_state()? {
            Some(state) if state.device == keystore.device_id() => state,
            _ => SyncState::new(&keystore.device_id()).ok_or(DictionarySyncError::Invalid)?,
        };
        let http = HttpDav::new(&self.config).map_err(DictionarySyncError::from_relay)?;
        let clock = WallClock;
        let mut store = DavStore::new(&http, &clock);
        let mut env = SystemEnvironment;
        let mut sink = self.sink();
        let mut parts = Parts {
            remote: &mut store,
            local,
            env: &mut env,
            sink: &mut sink,
        };
        core::restart(&mut state, &mut parts, &ring)?;
        Ok(Self::view(Some(&state), Some(&keystore)))
    }
}

fn valid_device(device_id: &str) -> Result<(), DictionarySyncError> {
    if crate::cloud::clipboard_log::is_lower_hex(device_id, 32) {
        Ok(())
    } else {
        Err(DictionarySyncError::Invalid)
    }
}

/// 「测试连接」：保存之前在用户选的文件夹里试一遍日志要用的网盘行为（与云剪贴板的测试连接相同：独占创建两次第二次必须失败、刚写的马上列得出、四路并发恰好一个成功），探测目录建在 `msime-dictionary/keyring/msime-clipboard/` 下，测完删掉。给了 `directory` 时，通过后让本机从网盘现在的样子重新开始：忘掉读到的位置和防回退的记录（下一次按第一次加入，本机的词全部保留），也删掉密钥串的日志缓存。用户自己清理过网盘上的文件夹、本机因此报 `dictionary_sync_rollback` 时，这是恢复的办法。会联网，大约二十个请求。
pub fn probe(
    url: &str,
    username: &str,
    password: &str,
    directory: Option<&Path>,
) -> Result<(), DictionarySyncError> {
    if directory.is_some_and(|directory| !directory.is_absolute()) {
        return Err(DictionarySyncError::Invalid);
    }
    let config = RelayConfig::webdav(url, username, password)
        .map_err(|_| DictionarySyncError::NotConfigured)?;
    let http = HttpDav::new(&config).map_err(DictionarySyncError::from_relay)?;
    let clock = WallClock;
    let dav = Dav::new(&http, &clock);
    for dir in [ROOT, KEYRING] {
        dav.mkcol(dir).map_err(DictionarySyncError::from_relay)?;
    }
    let prefixed = Prefixed {
        inner: Box::new(HttpDav::new(&config).map_err(DictionarySyncError::from_relay)?),
        prefix: KEYRING,
        requests: Arc::new(AtomicUsize::new(0)),
    };
    crate::clipboard_webdav::probe_with(&prefixed, &clock)
        .map_err(DictionarySyncError::from_relay)?;
    let Some(directory) = directory else {
        return Ok(());
    };
    if !directory.exists() {
        return Ok(());
    }
    let sync = DictionarySync::new(url, username, password, directory, "")?;
    let _lock = sync.lock()?;
    if let Some(mut state) = sync.load_state()? {
        state.forget_remote();
        sync.sink().save(&state)?;
    }
    let _relay = crate::clipboard_relay::lock_directory(directory)
        .map_err(DictionarySyncError::from_relay)?;
    match std::fs::remove_file(directory.join(WEBDAV_STATE_FILE)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(SyncError::Storage.into()),
    }
}

#[cfg(test)]
#[path = "dictionary_webdav_tests.rs"]
mod tests;
