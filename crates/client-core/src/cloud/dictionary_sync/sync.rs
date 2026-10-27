//! 一次同步：本机词库和日志之间的三方核对、拉取、写入本机、发送、压缩和清理。网盘和 Engine 都经接口注入（[`RemoteStore`]、[`LocalDictionary`]），这里不打开连接、不碰 Engine，测试因此能确定地模拟几台设备、断网、崩溃和网盘回退。
//!
//! 本机记三样东西，和日志位置一起原子写盘（[`SyncState`]）：
//!
//! - `mirror`：日志重放出的共享状态，只由日志决定（[`Ledger::replica`]）；
//! - `applied`：认为本机词库里现在有哪些「我的词条」——上一次本机和同步对上时的样子；
//! - `outbox`：本机改动得出、还没写进日志的操作，每个词条只留最新的一条。
//!
//! 本机词库相对 `applied` 的变化是本机的改动，记成带时间戳的操作放进 `outbox`；共享状态（叠上 `outbox`）相对 `applied` 的变化是要写进本机词库的远端改动。崩溃发生在任何两步之间，下一次重新比较就能接上，不会漏掉，也不会把同一个改动发两遍：见各函数的说明和 `docs/dictionary-webdav-sync.md` 的「崩溃与重试」。

use super::clock::{Clock, Hlc};
use super::crypto::{DataKey, NONCE_BYTES};
use super::entry::{WordKey, WordKind};
use super::log::{
    checkpoint_header, format_bytes, format_supported, skip_without_key, Draft, Ledger, LogError,
    Received, MAX_CHECKPOINT_BYTES, MAX_EVENT_BYTES, MAX_OPS_PER_EVENT, MAX_PAYLOAD_BYTES,
};
use super::replica::{Op, Record, Replica};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet};

// ---- 网盘上的路径 ----

/// 词库同步在用户选的文件夹下只用这个目录，与云剪贴板的 `msime-clipboard/` 互不相干。
pub const ROOT: &str = "msime-dictionary/";
pub const V1: &str = "msime-dictionary/v1/";
pub const FORMAT_FILE: &str = "msime-dictionary/v1/format.json";
const FORMAT_NAME: &str = "format.json";
const CHECKPOINT_NAME: &str = "checkpoint.json";
const MAX_SMALL_BYTES: u64 = 4096;
/// 一个 epoch 目录的列表上限：压缩让它通常不超过 66 项。
pub const MAX_LISTING_BYTES: u64 = 2 * 1024 * 1024;

pub fn epoch_dir(epoch: u64) -> String {
    format!("{V1}epoch-{epoch:06}/")
}

pub fn event_path(epoch: u64, seq: u64) -> String {
    format!("{}e-{seq:012}.json", epoch_dir(epoch))
}

pub fn checkpoint_path(epoch: u64) -> String {
    format!("{}{CHECKPOINT_NAME}", epoch_dir(epoch))
}

fn digits(value: &str, length: usize) -> Option<u64> {
    (value.len() == length && value.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| value.parse().ok())
        .flatten()
}

fn epoch_name(name: &str) -> Option<u64> {
    digits(name.strip_prefix("epoch-")?, 6)
}

fn event_name(name: &str) -> Option<u64> {
    digits(name.strip_prefix("e-")?.strip_suffix(".json")?, 12)
}

// ---- 上限 ----

/// 共享状态里的词条到这么多时，本机不再收新词（删除照常）。
pub const MAX_LIVE_WORDS: usize = 100_000;
/// 待发送的操作最多这么多个词条。超过时本机的改动先留在词库里，等发出去一批再记；词库本身就是这些改动的原件，所以不会丢。
pub const MAX_OUTBOX: usize = 20_000;
/// 一次写进本机词库的词条数，与 Engine 批量编辑的上限 `MAX_EDIT_BATCH` 相同。
pub const MAX_APPLY_BATCH: usize = 128;
/// 一次核对里本机删掉了这么多、而且至少是原有词条的一半，就先不同步删除，等用户确认（见 [`SyncState::guard`]）：多半是词库被重置或者损坏了，不该把其他设备上的词也删掉。
pub const MASS_DELETE_MIN: usize = 50;
/// 抢不到序号时重读再试的次数。
const MAX_PUBLISH_ATTEMPTS: usize = 6;
/// 一次同步最多跨几个 epoch。
const MAX_SYNC_ROUNDS: usize = 16;
/// 旧 epoch 在新检查点写好之后再留这么久才删，给正在读它的设备一点时间。
pub const OLD_EPOCH_GRACE_SECS: i64 = 600;
/// 本机确认的位置落后于发出去的、又这么久没写过事件时，写一个只带确认的事件。
pub const HEARTBEAT_SECS: i64 = 24 * 60 * 60;
/// 写进本机词库时每这么多批写一次盘，发送时每这么多个事件写一次盘。
const SAVE_EVERY_BATCHES: usize = 16;
const SAVE_EVERY_EVENTS: usize = 8;
/// 本机缓存的密钥最多几把：当前 epoch 的和密钥串现在的。
const MAX_CACHED_KEYS: usize = 3;

// ---- 注入的接口 ----

/// 同步失败的原因。宿主看到的码见 [`SyncError::code`]。
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SyncError {
    /// 连不上、超时、网盘答得不对，或者新写的文件一直列不出来。
    #[error("dictionary sync storage is unavailable")]
    Unavailable,
    /// 网盘拒绝了账号或应用密码。
    #[error("dictionary sync storage rejected the credentials")]
    Unauthorized,
    /// 网盘一直限流，或者本机这 30 分钟的请求额度用完了。
    #[error("dictionary sync is rate limited")]
    RateLimited,
    /// 网盘的响应超过了上限。
    #[error("dictionary sync response is too large")]
    TooLarge,
    /// 网盘上的日志比本机读到过的旧，或者接不上本机读过的那一段。
    #[error("dictionary sync log was rolled back or rewritten")]
    Rollback,
    /// 网盘上的格式是这一版不认识的（更新的客户端迁移过）。
    #[error("dictionary sync format is not supported")]
    Unsupported,
    /// 检查点和本机重放的结果不一致，或者格式不对：有设备写坏了日志。
    #[error("dictionary sync log is corrupt")]
    Corrupt,
    /// 本机的同步状态读写失败。
    #[error("dictionary sync state is unavailable")]
    Storage,
}

impl SyncError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "dictionary_sync_unavailable",
            Self::Unauthorized => "dictionary_sync_unauthorized",
            Self::RateLimited => "dictionary_sync_rate_limited",
            Self::TooLarge => "dictionary_sync_too_large",
            Self::Rollback => "dictionary_sync_rollback",
            Self::Unsupported => "dictionary_sync_unsupported",
            Self::Corrupt => "dictionary_sync_corrupt",
            Self::Storage => "dictionary_sync_storage",
        }
    }
}

/// 网盘。路径相对于用户选的文件夹，集合以 `/` 结尾。实现负责鉴权、限流退避和响应上限（超过时是 [`SyncError::TooLarge`]）。
pub trait RemoteStore {
    /// 一个集合里的条目名；集合不存在时为 `None`。
    fn list(&mut self, dir: &str) -> Result<Option<Vec<String>>, SyncError>;
    /// 一个文件的内容；不存在时为 `None`。
    fn get(&mut self, path: &str, limit: u64) -> Result<Option<Vec<u8>>, SyncError>;
    /// 独占创建：目标已经存在、或者它所在的集合不存在时为 `false`。
    fn create(&mut self, path: &str, bytes: &[u8]) -> Result<bool, SyncError>;
    /// 建一个集合，已经存在也算成功。
    fn mkcol(&mut self, dir: &str) -> Result<(), SyncError>;
    /// 删除一个文件或集合，不存在也算成功。
    fn delete(&mut self, path: &str) -> Result<(), SyncError>;
}

/// 写进本机词库的一个改动。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalChange {
    pub key: WordKey,
    /// 真是加入，假是删除。
    pub add: bool,
    /// 这次写入的请求编号：`dsync-` 加 32 位小写十六进制，由词条、加入还是删除和决定它的那条操作（时间戳、设备）唯一决定。Engine 按请求编号记回执，同一次写入重试不改变结果；同一个词删了又加回来是另一条操作，编号也不同。
    pub request_id: String,
}

fn request_id(key: &WordKey, add: bool, hlc: Hlc, device: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for part in [
        "msime-dictionary-v1-apply",
        key.kind().name(),
        key.code(),
        key.word(),
        if add { "add" } else { "delete" },
        &hlc.0.to_string(),
        device,
    ] {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    format!("dsync-{}", hex::encode(&hasher.finalize()[..16]))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalOutcome {
    /// 写好了（加入的词本来就在、删除的词本来就不在也算）。
    Applied,
    /// Engine 不收这个词条（例如这一版的拼音音节表里没有它），或者删不掉它。不再重试，直到这个词条的共享值变了。要删的词本来就不在时答 [`LocalOutcome::Applied`]，不是这个。
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalError {
    /// 输入法正占着词库，维护在预算内没能拿到。这一轮先不写，按 [`super::schedule`] 的策略稍后再试。
    Busy,
    /// 词库读写失败。
    Unavailable,
}

/// 本机词库。实现在宿主一侧（`host-api` 经 Engine 的个人词库接口），这里只看「我的词条」的身份。
pub trait LocalDictionary {
    /// 现在本机的全部「我的词条」，必须是同一时刻的一致视图（一个读事务里读出来）：分页时被插进来的改动会让核对误以为有词被删了。
    fn words(&mut self) -> Result<BTreeSet<WordKey>, LocalError>;
    /// 写一批，最多 [`MAX_APPLY_BATCH`] 项，结果与 `changes` 一一对应。整批要么写好、要么没写（[`LocalError`]）。同一批重复写一次不改变结果。
    fn apply(&mut self, changes: &[LocalChange]) -> Result<Vec<LocalOutcome>, LocalError>;

    /// 这一轮写完了，接下来可能要联网：放掉为写入拿的东西（例如请输入法让出词库的租约）。维护权本来就只在一批之内持有，这里放的是跨批保留、但不能跨网络请求保留的那些。
    fn release(&mut self) {}
}

/// 时钟和随机数。测试换成确定的。
pub trait Environment {
    fn now_ms(&self) -> u64;
    fn random(&mut self, bytes: &mut [u8]) -> Result<(), SyncError>;
    fn now(&self) -> i64 {
        (self.now_ms() / 1000) as i64
    }
}

/// 原子写盘同步状态：要么整份换成新的，要么保持旧的。
pub trait StateSink {
    fn save(&mut self, state: &SyncState) -> Result<(), SyncError>;
}

/// 密钥串这一次给的：本机现在持有的当前密钥（获准之后才有）。
#[derive(Clone, Debug, Default)]
pub struct KeyRing {
    pub current: Option<DataKey>,
}

// ---- 本机状态 ----

/// 一条待发送的操作。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingOp {
    pub live: bool,
    pub hlc: Hlc,
}

/// 被用户挡下的大批删除：本机少了这么多词条。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Guard {
    pub deletions: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cleanup {
    epoch: u64,
    not_before: i64,
}

/// 一台设备的同步状态，整份原子写盘（[`StateSink`]）。里面有本机词条的明文和派生出的子密钥，与 Engine 的词库同等对待：只放在本机私有目录，不导出、不进诊断。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncState {
    pub format: u32,
    /// 本机在密钥串里的设备编号（32 位小写十六进制）。同一个安装的所有进程用同一个，见 `docs/dictionary-webdav-sync.md` 的「设备身份」。
    pub device: String,
    pub clock: Clock,
    #[serde(default, with = "ledger_serde")]
    ledger: Option<Ledger>,
    #[serde(default, with = "pairs")]
    outbox: BTreeMap<WordKey, PendingOp>,
    /// `None` 表示还没和本机词库对过（第一次加入，或者共享词库被重建过）。
    #[serde(default)]
    applied: Option<BTreeSet<WordKey>>,
    /// Engine 不收的词条，记着当时的共享值；共享值变了才再试。
    #[serde(default, with = "pairs")]
    unappliable: BTreeMap<WordKey, (Hlc, String)>,
    /// 本机词库已经持久地写到了日志的哪个序号。
    #[serde(default)]
    acked: u64,
    /// 要在下一个事件里宣布移除的设备。
    #[serde(default)]
    retire: Vec<String>,
    #[serde(default)]
    keys: Vec<DataKey>,
    #[serde(default)]
    guard: Option<Guard>,
    #[serde(default)]
    cleanup: Vec<Cleanup>,
    /// 本机最后一次写事件的时间（Unix 秒）。
    #[serde(default)]
    last_event_at: i64,
    /// 收到过比本机时间快一天以上的时间戳。
    #[serde(default)]
    skew: bool,
    #[serde(default)]
    pub schedule: super::schedule::Schedule,
}

pub const STATE_FORMAT: u32 = 1;

impl SyncState {
    /// 新的同步状态。`device` 是密钥串给本机的编号。
    pub fn new(device: &str) -> Option<Self> {
        crate::is_lower_hex(device, 32).then(|| Self {
            format: STATE_FORMAT,
            device: device.to_owned(),
            clock: Clock::default(),
            ledger: None,
            outbox: BTreeMap::new(),
            applied: None,
            unappliable: BTreeMap::new(),
            acked: 0,
            retire: Vec::new(),
            keys: Vec::new(),
            guard: None,
            cleanup: Vec::new(),
            last_event_at: 0,
            skew: false,
            schedule: Default::default(),
        })
    }

    pub fn ledger(&self) -> Option<&Ledger> {
        self.ledger.as_ref()
    }

    pub fn outbox_len(&self) -> usize {
        self.outbox.len()
    }

    pub fn guard(&self) -> Option<Guard> {
        self.guard
    }

    pub fn joined(&self) -> bool {
        self.applied.is_some()
    }

    pub fn acked(&self) -> u64 {
        self.acked
    }

    pub fn skew(&self) -> bool {
        self.skew
    }

    /// 用户移除了一台设备：在下一个事件里宣布，之后它不再拖住墓碑回收。
    pub fn retire(&mut self, device: &str) {
        if crate::is_lower_hex(device, 32)
            && device != self.device
            && !self.retire.iter().any(|id| id == device)
        {
            self.retire.push(device.to_owned());
        }
    }

    /// 网盘被用户在设置里重新信任（重新测试连接）：忘掉读到的位置和防回退的记录，下一次当作第一次加入，本机的词全部保留、发出去。
    pub fn forget_remote(&mut self) {
        self.ledger = None;
        self.applied = None;
        self.outbox.clear();
        self.unappliable.clear();
        self.acked = 0;
        self.cleanup.clear();
        self.guard = None;
    }

    /// 一个词条的有效值：共享值和本机待发送的操作里更晚的那个，连同决定它的（时间戳、设备）。两边都没有时为 `None`。
    fn effective(&self, key: &WordKey) -> Option<(bool, Hlc, &str)> {
        let record = self
            .ledger
            .as_ref()
            .and_then(|ledger| ledger.replica.entries.get(key));
        let own = self.device.as_str();
        match (record, self.outbox.get(key)) {
            (Some(record), Some(pending)) if !record.loses_to(pending.hlc, own, pending.live) => {
                Some((record.live, record.hlc, record.device.as_str()))
            }
            (_, Some(pending)) => Some((pending.live, pending.hlc, own)),
            (Some(record), None) => Some((record.live, record.hlc, record.device.as_str())),
            (None, None) => None,
        }
    }

    fn effective_live(&self, key: &WordKey) -> bool {
        self.effective(key).is_some_and(|(live, _, _)| live)
    }

    fn live_words(&self) -> usize {
        self.ledger
            .as_ref()
            .map_or(0, |ledger| ledger.replica.live_count())
    }

    /// 还要写进本机词库的改动（不含 Engine 不收的）。
    fn pending_apply(&self) -> Vec<LocalChange> {
        let Some(applied) = self.applied.as_ref() else {
            return Vec::new();
        };
        let mut keys: BTreeSet<&WordKey> = applied.iter().collect();
        if let Some(ledger) = self.ledger.as_ref() {
            keys.extend(ledger.replica.entries.keys());
        }
        keys.extend(self.outbox.keys());
        keys.into_iter()
            .filter_map(|key| {
                let (live, hlc, device) =
                    self.effective(key).unwrap_or((false, Hlc::default(), ""));
                if live == applied.contains(key) || self.rejected_now(key) {
                    return None;
                }
                Some(LocalChange {
                    key: key.clone(),
                    add: live,
                    request_id: request_id(key, live, hlc, device),
                })
            })
            .collect()
    }

    fn shared_version(&self, key: &WordKey) -> Option<(Hlc, String)> {
        self.ledger
            .as_ref()
            .and_then(|ledger| ledger.replica.entries.get(key))
            .map(|record| (record.hlc, record.device.clone()))
    }

    fn rejected_now(&self, key: &WordKey) -> bool {
        self.unappliable.get(key).is_some_and(|version| {
            *version
                == self
                    .shared_version(key)
                    .unwrap_or((Hlc::default(), String::new()))
        })
    }

    /// 给宿主看的概况。
    pub fn status(&self) -> Status {
        Status {
            joined: self.joined(),
            epoch: self.ledger.as_ref().map(|ledger| ledger.epoch),
            seq: self.ledger.as_ref().map_or(0, |ledger| ledger.seq),
            words: self.live_words(),
            outbox: self.outbox.len(),
            pending_apply: self.pending_apply().len(),
            guard: self.guard,
            skew: self.skew,
        }
    }

    /// 本机缓存的那把编号为 `key_id` 的子密钥。
    fn key(&self, key_id: &str) -> Option<&DataKey> {
        self.keys.iter().find(|key| key.key_id() == key_id)
    }

    /// 记下密钥串现在给的密钥。只留当前 epoch 的那把和最新的几把。
    fn remember(&mut self, ring: &KeyRing) {
        if let Some(current) = &ring.current {
            if self.key(current.key_id()).is_none() {
                self.keys.push(current.clone());
            }
        }
        let epoch_key = self.ledger.as_ref().map(|ledger| ledger.key_id.clone());
        let current = ring.current.as_ref().map(|key| key.key_id().to_owned());
        while self.keys.len() > MAX_CACHED_KEYS {
            let Some(index) = self.keys.iter().position(|key| {
                Some(key.key_id()) != epoch_key.as_deref()
                    && Some(key.key_id()) != current.as_deref()
            }) else {
                break;
            };
            self.keys.remove(index);
        }
    }
}

/// [`SyncState::status`] 的结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub joined: bool,
    pub epoch: Option<u64>,
    pub seq: u64,
    /// 共享词库里现在有多少个词条。
    pub words: usize,
    pub outbox: usize,
    pub pending_apply: usize,
    pub guard: Option<Guard>,
    pub skew: bool,
}

/// 一次同步做了什么。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// 本机改动记成了几条操作。
    pub captured: usize,
    /// 读到了几个新事件。
    pub received_events: usize,
    /// 写进本机词库的改动数。
    pub applied: usize,
    /// Engine 正忙，还有改动没写进本机词库。
    pub deferred: bool,
    /// 发出去的操作数和事件数。
    pub published_ops: usize,
    pub published_events: usize,
    /// 本机没有读写当前 epoch 所需的密钥：还在等待批准，或者密钥换过、还没有设备把日志换成新密钥。
    pub pending_key: bool,
    /// 这一次封口了一个 epoch。
    pub compacted: bool,
    /// 共享词库被另一台设备以它为准重建过，本机重新加入了。
    pub rejoined: bool,
}

/// 一次同步要做的事。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// 只做本机的事（记录本机改动、写入已拉到的远端改动），不联网。
    pub offline: bool,
    /// 拉取，但不发送本机的操作（例如额度不够）。
    pub skip_push: bool,
    /// 用户确认了被挡下的大批删除：照常同步出去。
    pub confirm_deletions: bool,
    /// 用户选择不删：从共享词库把它们写回本机。
    pub restore_deletions: bool,
}

/// 同步时用到的几样东西。
pub struct Parts<'a> {
    pub remote: &'a mut dyn RemoteStore,
    pub local: &'a mut dyn LocalDictionary,
    pub env: &'a mut dyn Environment,
    pub sink: &'a mut dyn StateSink,
}

fn nonce(env: &mut dyn Environment) -> Result<[u8; NONCE_BYTES], SyncError> {
    let mut bytes = [0u8; NONCE_BYTES];
    env.random(&mut bytes)?;
    Ok(bytes)
}

fn random_hex(env: &mut dyn Environment) -> Result<String, SyncError> {
    let mut bytes = [0u8; 16];
    env.random(&mut bytes)?;
    Ok(hex::encode(bytes))
}

fn local_error(error: LocalError) -> SyncError {
    match error {
        LocalError::Busy | LocalError::Unavailable => SyncError::Storage,
    }
}

/// 一次同步。顺序固定：
///
/// 1. 记录本机改动（不联网，见 [`capture`]）；
/// 2. 拉取新事件，跨过封口，必要时从检查点重新开始；与位置一起写盘；
/// 3. 第一次加入时与本机词库合并（见 [`join`]）；
/// 4. 把远端改动分批写进本机词库，每批写盘一次；Engine 忙就留到下次；
/// 5. 发送待发送的操作，每写成一个事件写盘一次；
/// 6. 该压缩时封口、写下一个 epoch 的检查点；
/// 7. 删掉过了宽限期的旧 epoch。
///
/// 网络错误在第 2、5、6 步返回，已经写盘的进度都保留；写本机词库从不跨网络请求持有维护权（接口一次只写一批）。
pub fn run(
    state: &mut SyncState,
    parts: &mut Parts<'_>,
    ring: &KeyRing,
    options: Options,
) -> Result<Report, SyncError> {
    let mut report = Report::default();
    state.remember(ring);
    if options.restore_deletions {
        restore(state, parts)?;
    }
    report.captured = capture_with(state, parts, options.confirm_deletions)?;
    if !options.offline && !pull(state, parts, ring, &mut report)? {
        report.pending_key = true;
    }
    if state.ledger.is_none() {
        return Ok(report);
    }
    // 拉取停在没有密钥的地方时，已经读到的部分照样合并、写进本机词库：这些都不需要密钥。
    if !state.joined() {
        join(state, parts)?;
    }
    apply_remote(state, parts, &mut report)?;
    if !options.offline && !options.skip_push && !report.pending_key {
        // 换钥先做：密钥串换了钥之后旧 epoch 不再收新事件，待发送的操作要等到新 epoch 才能发。
        compact(state, parts, ring, &mut report)?;
        push(state, parts, ring, &mut report)?;
        compact(state, parts, ring, &mut report)?;
        cleanup(state, parts)?;
    }
    Ok(report)
}

/// 只记录本机改动，不联网：宿主在用户改词之后马上调用，时间戳因此接近改动发生的时刻。
pub fn capture(state: &mut SyncState, parts: &mut Parts<'_>) -> Result<usize, SyncError> {
    capture_with(state, parts, false)
}

/// 本机词库相对 `applied` 的变化记成操作。已经和有效值一致的（上一次写进本机词库之后、写盘之前崩溃留下的）只修正 `applied`，不发操作。大批删除先挡下。
fn capture_with(
    state: &mut SyncState,
    parts: &mut Parts<'_>,
    confirm_deletions: bool,
) -> Result<usize, SyncError> {
    if !state.joined() || state.ledger.is_none() {
        return Ok(0);
    }
    let words = match parts.local.words() {
        Ok(words) => words,
        Err(LocalError::Busy) => return Ok(0),
        Err(error) => return Err(local_error(error)),
    };
    let applied = state.applied.clone().unwrap_or_default();
    let mut additions = Vec::new();
    let mut deletions = Vec::new();
    for key in words.difference(&applied) {
        if state.effective_live(key) {
            state
                .applied
                .get_or_insert_with(BTreeSet::new)
                .insert(key.clone());
        } else {
            additions.push(key.clone());
        }
    }
    for key in applied.difference(&words) {
        if state.effective_live(key) {
            deletions.push(key.clone());
        } else if let Some(applied) = state.applied.as_mut() {
            applied.remove(key);
        }
    }
    let guard_before = state.guard;
    let guarded = !confirm_deletions
        && deletions.len() >= MASS_DELETE_MIN
        && deletions.len() * 2 >= applied.len();
    if guarded {
        state.guard = Some(Guard {
            deletions: deletions.len(),
        });
        deletions.clear();
    } else {
        state.guard = None;
    }
    let now_ms = parts.env.now_ms();
    let mut captured = 0;
    let mut live = state.live_words() + state.outbox.values().filter(|op| op.live).count();
    for (key, add) in additions
        .into_iter()
        .map(|key| (key, true))
        .chain(deletions.into_iter().map(|key| (key, false)))
    {
        if !state.outbox.contains_key(&key) && state.outbox.len() >= MAX_OUTBOX {
            break;
        }
        if add && live >= MAX_LIVE_WORDS {
            continue;
        }
        let Some(hlc) = state.clock.tick(now_ms) else {
            break;
        };
        state
            .outbox
            .insert(key.clone(), PendingOp { live: add, hlc });
        let applied = state.applied.get_or_insert_with(BTreeSet::new);
        if add {
            applied.insert(key);
            live += 1;
        } else {
            applied.remove(&key);
        }
        captured += 1;
    }
    if captured > 0 || state.applied.as_ref() != Some(&applied) || state.guard != guard_before {
        parts.sink.save(state)?;
    }
    Ok(captured)
}

/// 用户选择不同步被挡下的删除：把本机缺的那些从 `applied` 里去掉，第 4 步就会从共享词库把它们写回来。
fn restore(state: &mut SyncState, parts: &mut Parts<'_>) -> Result<(), SyncError> {
    if state.guard.is_none() {
        return Ok(());
    }
    let words = parts.local.words().map_err(local_error)?;
    if let Some(applied) = state.applied.as_mut() {
        applied.retain(|key| words.contains(key));
    }
    state.guard = None;
    parts.sink.save(state)
}

/// 第一次加入（或共享词库被重建之后重新加入）：本机已有的词都算在本机词库里；共享词库里没有的（包括那边删过的）作为新词发出去，本机的词一个也不删；共享词库里有、本机没有的随后写进来。
fn join(state: &mut SyncState, parts: &mut Parts<'_>) -> Result<(), SyncError> {
    let words = match parts.local.words() {
        Ok(words) => words,
        Err(LocalError::Busy) => return Ok(()),
        Err(error) => return Err(local_error(error)),
    };
    let now_ms = parts.env.now_ms();
    let mut applied = BTreeSet::new();
    let mut live = state.live_words();
    for key in words {
        let shared = state
            .ledger
            .as_ref()
            .is_some_and(|ledger| ledger.replica.is_live(&key));
        if shared {
            applied.insert(key);
            continue;
        }
        // 放不下的先不算进 `applied`，之后的核对会把它们当成本机新加的词再记一次。
        if state.outbox.len() >= MAX_OUTBOX || live >= MAX_LIVE_WORDS {
            continue;
        }
        let Some(hlc) = state.clock.tick(now_ms) else {
            break;
        };
        state
            .outbox
            .insert(key.clone(), PendingOp { live: true, hlc });
        applied.insert(key);
        live += 1;
    }
    state.applied = Some(applied);
    parts.sink.save(state)
}

/// 把远端改动分批写进本机词库。每批写好就写盘；之后没有要写的了，本机确认的位置推进到当前序号。
fn apply_remote(
    state: &mut SyncState,
    parts: &mut Parts<'_>,
    report: &mut Report,
) -> Result<(), SyncError> {
    let result = apply_batches(state, parts, report);
    parts.local.release();
    result
}

fn apply_batches(
    state: &mut SyncState,
    parts: &mut Parts<'_>,
    report: &mut Report,
) -> Result<(), SyncError> {
    if !state.joined() {
        return Ok(());
    }
    // 要写的改动只算一次：写的过程中只有 `applied` 在变，而它正是按这份清单改的。
    let pending = state.pending_apply();
    let mut unsaved = 0;
    for changes in pending.chunks(MAX_APPLY_BATCH) {
        let outcomes = match parts.local.apply(changes) {
            Ok(outcomes) if outcomes.len() == changes.len() => outcomes,
            Ok(_) => return Err(SyncError::Storage),
            Err(LocalError::Busy) => {
                report.deferred = true;
                if unsaved > 0 {
                    parts.sink.save(state)?;
                }
                return Ok(());
            }
            Err(error) => return Err(local_error(error)),
        };
        for (change, outcome) in changes.iter().zip(outcomes) {
            match (outcome, change.add) {
                (LocalOutcome::Applied, add) => {
                    if let Some(applied) = state.applied.as_mut() {
                        if add {
                            applied.insert(change.key.clone());
                        } else {
                            applied.remove(&change.key);
                        }
                    }
                    state.unappliable.remove(&change.key);
                }
                // Engine 删不掉（本来就不在的删除算写好了，不会走到这里）：这个词还在本机词库里，`applied` 照实留着它，否则下一次核对会把它当成本机新加的词发出去，在所有设备上复活；和不收的词一样，共享值变了才再试。
                (LocalOutcome::Rejected, _) => {
                    let version = state
                        .shared_version(&change.key)
                        .unwrap_or((Hlc::default(), String::new()));
                    state.unappliable.insert(change.key.clone(), version);
                }
            }
            report.applied += 1;
        }
        // 每批都写盘太贵（状态里有整个共享词库）。崩溃时没写盘的那几批会再写一次：加入已有的词、删除已经不在的词都不改变结果，核对时也认得出本机已经和有效值一致（见 [`capture_with`]）。
        unsaved += 1;
        if unsaved >= SAVE_EVERY_BATCHES {
            parts.sink.save(state)?;
            unsaved = 0;
        }
    }
    if unsaved > 0 {
        parts.sink.save(state)?;
    }
    // 没有要写的了：本机词库已经持久地反映了日志到当前序号为止的全部内容。
    let seq = state.ledger.as_ref().map_or(0, |ledger| ledger.seq);
    if state.acked != seq && state.pending_apply().is_empty() {
        state.acked = seq;
        parts.sink.save(state)?;
    }
    Ok(())
}

/// 读到一个事件之后：本机时钟推到它的时间戳之后；本机自己写的、已经在日志里的操作从待发送里去掉（崩溃在发出之后、写盘之前时靠这一步认出来，不再发第二遍）；被共享值盖过的待发送操作也去掉，它们永远不会胜出，晚些发出去反而会在墓碑回收之后让删掉的词复活。
fn absorb(state: &mut SyncState, received: &Received, now_ms: u64) {
    for op in &received.ops {
        if state.clock.observe(op.hlc, now_ms) {
            state.skew = true;
        }
        if received.device.as_deref() == Some(state.device.as_str())
            && state
                .outbox
                .get(&op.key)
                .is_some_and(|pending| pending.hlc == op.hlc && pending.live == op.live)
        {
            state.outbox.remove(&op.key);
        }
    }
    prune_outbox(state);
}

fn prune_outbox(state: &mut SyncState) {
    let Some(ledger) = state.ledger.as_ref() else {
        return;
    };
    let device = state.device.as_str();
    state.outbox.retain(|key, pending| {
        ledger
            .replica
            .entries
            .get(key)
            .is_none_or(|record| record.loses_to(pending.hlc, device, pending.live))
    });
}

fn log_error(error: LogError) -> SyncError {
    match error {
        LogError::Chain => SyncError::Rollback,
        LogError::Malformed => SyncError::Corrupt,
        LogError::Sealed | LogError::MissingKey => SyncError::Unavailable,
    }
}

/// 拉取：当前 epoch 的新事件按序号接上，遇到封口就进入下一个 epoch，当前 epoch 已经被删掉时从最新的检查点重新开始。读不了（没有需要的密钥）时返回 `false`，位置不变。
fn pull(
    state: &mut SyncState,
    parts: &mut Parts<'_>,
    ring: &KeyRing,
    report: &mut Report,
) -> Result<bool, SyncError> {
    let now_ms = parts.env.now_ms();
    if state.ledger.is_none() && !bootstrap(state, parts, ring, report)? {
        return Ok(false);
    }
    for _ in 0..MAX_SYNC_ROUNDS * 2 {
        let mut ledger = state.ledger.clone().ok_or(SyncError::Unavailable)?;
        if let Some(seal) = &ledger.sealed {
            if seal.upgrade.is_some() {
                return Err(SyncError::Unsupported);
            }
            if !cross_seal(state, parts, ring, ledger, report)? {
                return Ok(false);
            }
            continue;
        }
        let Some(names) = parts.remote.list(&epoch_dir(ledger.epoch))? else {
            if !bootstrap(state, parts, ring, report)? {
                return Ok(false);
            }
            continue;
        };
        let seqs: BTreeSet<u64> = names.iter().filter_map(|name| event_name(name)).collect();
        detect_truncation(parts, &ledger, &seqs)?;
        let key = state.key(&ledger.key_id).cloned();
        let mut absorbed = Vec::new();
        while seqs.contains(&(ledger.seq + 1)) {
            // 列出来却还读不到的事件当作还没写好，下次再读。
            let Some(bytes) = parts
                .remote
                .get(&event_path(ledger.epoch, ledger.seq + 1), MAX_EVENT_BYTES)?
            else {
                break;
            };
            let received = match ledger.apply(&bytes, key.as_ref()) {
                Ok(received) => received,
                Err(LogError::MissingKey) => return Ok(false),
                Err(error) => return Err(log_error(error)),
            };
            report.received_events += 1;
            absorbed.push(received);
            if ledger.sealed.is_some() {
                break;
            }
        }
        state.ledger = Some(ledger);
        for received in &absorbed {
            absorb(state, received, now_ms);
        }
        parts.sink.save(state)?;
        if state
            .ledger
            .as_ref()
            .is_some_and(|ledger| ledger.sealed.is_some())
        {
            continue;
        }
        return Ok(true);
    }
    Err(SyncError::Unavailable)
}

/// 日志尾部被删过时，只看链头发现不了：被删的正好是本机读到的最后几个事件，接下来的写入又接在本机的链头后面。所以列目录时再核对两件事：本机读过的最后一个事件还在；列出了更大的序号时，下一个序号也在。列表可能滞后，两种情况都先直接 `GET` 一次确认，确实没有才是 [`SyncError::Rollback`]——更大的序号只能在下一个序号写好之后才写得出来。
fn detect_truncation(
    parts: &mut Parts<'_>,
    ledger: &Ledger,
    seqs: &BTreeSet<u64>,
) -> Result<(), SyncError> {
    if ledger.events > 0 && !seqs.contains(&ledger.seq) {
        let path = event_path(ledger.epoch, ledger.seq);
        if parts.remote.get(&path, MAX_EVENT_BYTES)?.is_none() {
            return Err(SyncError::Rollback);
        }
    }
    let next = ledger.seq + 1;
    if !seqs.contains(&next) && seqs.iter().any(|&seq| seq > next) {
        let path = event_path(ledger.epoch, next);
        if parts.remote.get(&path, MAX_EVENT_BYTES)?.is_none() {
            return Err(SyncError::Rollback);
        }
    }
    Ok(())
}

/// 下一个 epoch 用哪把密钥：密钥串换了钥、本机两把都有时用新的（换钥），否则沿用这一个 epoch 的。
fn next_key<'a>(state: &'a SyncState, ring: &'a KeyRing, ledger: &Ledger) -> Option<&'a DataKey> {
    match &ring.current {
        Some(current)
            if current.key_id() != ledger.key_id && state.key(&ledger.key_id).is_some() =>
        {
            Some(current)
        }
        _ => state.key(&ledger.key_id),
    }
}

/// 跨过封口进入下一个 epoch：检查点已经有了就读它，并核对它就是本机按日志算出的那一份；还没有（封口的设备中断了）就由本机写出来。
fn cross_seal(
    state: &mut SyncState,
    parts: &mut Parts<'_>,
    ring: &KeyRing,
    sealed: Ledger,
    report: &mut Report,
) -> Result<bool, SyncError> {
    let dir = epoch_dir(sealed.epoch + 1);
    let path = checkpoint_path(sealed.epoch + 1);
    let have_checkpoint = match parts.remote.list(&dir)? {
        Some(names) => names.iter().any(|name| name == CHECKPOINT_NAME),
        None => false,
    };
    if !have_checkpoint {
        let Some(key) = next_key(state, ring, &sealed).cloned() else {
            return Ok(false);
        };
        let next = sealed
            .next_epoch(key.key_id())
            .ok_or(SyncError::Unavailable)?;
        parts.remote.mkcol(&dir)?;
        let bytes = next.checkpoint(&key, nonce(parts.env)?);
        parts.remote.create(&path, &bytes)?;
    }
    let bytes = parts
        .remote
        .get(&path, MAX_CHECKPOINT_BYTES)?
        .ok_or(SyncError::Unavailable)?;
    let header = checkpoint_header(&bytes).ok_or(SyncError::Corrupt)?;
    if header.seq != sealed.seq || header.head != sealed.head {
        return Err(SyncError::Corrupt);
    }
    if header.lineage != sealed.lineage {
        // 另一台设备以它为准重建了共享词库。
        if header.generation <= sealed.generation {
            return Err(SyncError::Rollback);
        }
        schedule_cleanup(state, sealed.epoch, parts.env.now());
        return adopt(state, parts, &bytes, sealed.epoch + 1, true, report);
    }
    let Some(key) = state.key(&header.key_id).cloned() else {
        return Ok(false);
    };
    let next = Ledger::from_checkpoint(sealed.epoch + 1, &bytes, Some(&key)).map_err(log_error)?;
    let expected = sealed
        .next_epoch(&header.key_id)
        .ok_or(SyncError::Unavailable)?;
    if next != expected {
        return Err(SyncError::Corrupt);
    }
    schedule_cleanup(state, sealed.epoch, parts.env.now());
    state.ledger = Some(next);
    prune_outbox(state);
    parts.sink.save(state)?;
    Ok(true)
}

/// 采用一份检查点作为本机的位置。`rejoin` 为真时共享词库是重建过的，本机重新加入（[`join`]）。
fn adopt(
    state: &mut SyncState,
    parts: &mut Parts<'_>,
    bytes: &[u8],
    epoch: u64,
    rejoin: bool,
    report: &mut Report,
) -> Result<bool, SyncError> {
    let Some(header) = checkpoint_header(bytes) else {
        return Err(SyncError::Corrupt);
    };
    let Some(key) = state.key(&header.key_id).cloned() else {
        return Ok(false);
    };
    let ledger = Ledger::from_checkpoint(epoch, bytes, Some(&key)).map_err(log_error)?;
    observe_replica(state, &ledger.replica, parts.env.now_ms());
    state.ledger = Some(ledger);
    if rejoin {
        state.applied = None;
        state.outbox.clear();
        state.unappliable.clear();
        report.rejoined = true;
    }
    prune_outbox(state);
    parts.sink.save(state)?;
    Ok(true)
}

/// 从检查点开始时没有逐条读到操作，本机时钟也要推到检查点里最晚的时间戳之后：否则第一次加入时为本机的词取的时间戳可能比某个墓碑还早，反而把本机的词删掉。
fn observe_replica(state: &mut SyncState, replica: &Replica, now_ms: u64) {
    if let Some(latest) = replica.entries.values().map(|record| record.hlc).max() {
        if state.clock.observe(latest, now_ms) {
            state.skew = true;
        }
    }
}

fn schedule_cleanup(state: &mut SyncState, epoch: u64, now: i64) {
    if !state.cleanup.iter().any(|entry| entry.epoch == epoch) {
        state.cleanup.push(Cleanup {
            epoch,
            not_before: now + OLD_EPOCH_GRACE_SECS,
        });
    }
}

/// 从网盘上最新的检查点开始：本机还没有位置，或者本机所在的 epoch 已经被压缩删掉了。位置、密钥代数不许比本机记得的小，序号相同时链头也要相同；共享词库的编号变了时代数必须变大，本机重新加入。网盘上还没有这份日志时建好它（需要密钥）；但本机已经读到过事件时不重建，那是有人把日志删了。
fn bootstrap(
    state: &mut SyncState,
    parts: &mut Parts<'_>,
    ring: &KeyRing,
    report: &mut Report,
) -> Result<bool, SyncError> {
    let previous = state.ledger.clone();
    let seen = previous.is_some();
    let names = match parts.remote.list(V1)? {
        Some(names) => names,
        None if seen => return Err(SyncError::Rollback),
        None => {
            let Some(key) = ring.current.clone() else {
                return Ok(false);
            };
            parts.remote.mkcol(ROOT)?;
            parts.remote.mkcol(V1)?;
            parts.remote.create(FORMAT_FILE, &format_bytes())?;
            let genesis = Ledger::genesis(&random_hex(parts.env)?, key.key_id());
            parts.remote.mkcol(&epoch_dir(0))?;
            parts.remote.create(
                &checkpoint_path(0),
                &genesis.checkpoint(&key, nonce(parts.env)?),
            )?;
            parts.remote.list(V1)?.ok_or(SyncError::Unavailable)?
        }
    };
    if names.iter().any(|name| name == FORMAT_NAME) {
        if !seen {
            let bytes = parts
                .remote
                .get(FORMAT_FILE, MAX_SMALL_BYTES)?
                .ok_or(SyncError::Unavailable)?;
            if !format_supported(&bytes) {
                return Err(SyncError::Unsupported);
            }
        }
    } else {
        parts.remote.create(FORMAT_FILE, &format_bytes())?;
    }
    let mut epochs: Vec<u64> = names.iter().filter_map(|name| epoch_name(name)).collect();
    epochs.sort_unstable_by(|a, b| b.cmp(a));
    if epochs.is_empty() {
        if seen {
            return Err(SyncError::Rollback);
        }
        let Some(key) = ring.current.clone() else {
            return Ok(false);
        };
        let genesis = Ledger::genesis(&random_hex(parts.env)?, key.key_id());
        parts.remote.mkcol(&epoch_dir(0))?;
        parts.remote.create(
            &checkpoint_path(0),
            &genesis.checkpoint(&key, nonce(parts.env)?),
        )?;
        epochs.push(0);
    }
    for &epoch in &epochs {
        let Some(bytes) = parts
            .remote
            .get(&checkpoint_path(epoch), MAX_CHECKPOINT_BYTES)?
        else {
            // 封口的设备建了目录还没写检查点：从更早的 epoch 读到封口，再由本机补写。
            continue;
        };
        let header = checkpoint_header(&bytes).ok_or(SyncError::Corrupt)?;
        if header.epoch != epoch {
            return Err(SyncError::Corrupt);
        }
        let mut rejoin = false;
        if let Some(previous) = previous.as_ref() {
            if header.epoch < previous.epoch
                || header.seq < previous.seq
                || header.generation < previous.generation
                || (header.seq == previous.seq && header.head != previous.head)
            {
                return Err(SyncError::Rollback);
            }
            if header.lineage != previous.lineage {
                if header.generation <= previous.generation {
                    return Err(SyncError::Rollback);
                }
                rejoin = true;
            }
        }
        // 已经在这个 epoch 里（只是重新列目录）时不必重读检查点。
        if previous
            .as_ref()
            .is_some_and(|previous| previous.epoch == epoch && !rejoin)
        {
            return Ok(true);
        }
        let older: Vec<u64> = epochs
            .iter()
            .copied()
            .filter(|older| *older < epoch)
            .collect();
        if !adopt(state, parts, &bytes, epoch, rejoin, report)? {
            return Ok(false);
        }
        let now = parts.env.now();
        for older in older {
            schedule_cleanup(state, older, now);
        }
        return Ok(true);
    }
    Err(SyncError::Unavailable)
}

/// 下一个要发的事件：待发送的操作按词条顺序取一批，不超过 [`MAX_OPS_PER_EVENT`] 条和 [`MAX_PAYLOAD_BYTES`]。
fn next_draft(state: &SyncState) -> Draft {
    let mut ops = Vec::new();
    let mut bytes = 256;
    for (key, pending) in &state.outbox {
        let op = Op {
            key: key.clone(),
            live: pending.live,
            hlc: pending.hlc,
        };
        let size = Draft::op_bytes(&op);
        if ops.len() >= MAX_OPS_PER_EVENT || bytes + size > MAX_PAYLOAD_BYTES {
            break;
        }
        bytes += size;
        ops.push(op);
    }
    Draft {
        ops,
        ack: state.acked,
        retire: state.retire.clone(),
    }
}

impl SyncState {
    /// 有没有不是词条的东西要写进日志：要宣布移除的设备，或者该登记、该确认了（见 [`heartbeat_due`]）。调度据此决定要不要发送。
    pub fn wants_event(&self, now_ms: u64) -> bool {
        self.joined() && (!self.retire.is_empty() || heartbeat_due(self, (now_ms / 1000) as i64))
    }
}

/// 要不要写一个只带确认的事件：本机还没在日志里出现过（加入之后登记一次），或者确认的位置落后于日志里记着的、又有一天没写过事件。
fn heartbeat_due(state: &SyncState, now: i64) -> bool {
    let Some(ledger) = state.ledger.as_ref() else {
        return false;
    };
    match ledger.replica.devices.get(&state.device) {
        None => true,
        Some(info) => info.ack < state.acked && now - state.last_event_at >= HEARTBEAT_SECS,
    }
}

/// 能不能往当前 epoch 写：已经加入、没封口、这个 epoch 的密钥就是密钥串现在的密钥而且本机有它。被移除的设备即使还留着旧密钥，密钥串换了钥以后也不再写。
fn writable(state: &SyncState, ring: &KeyRing) -> Option<DataKey> {
    let ledger = state.ledger.as_ref()?;
    if !state.joined() || ledger.sealed.is_some() {
        return None;
    }
    let current = ring.current.as_ref()?;
    (current.key_id() == ledger.key_id).then(|| current.clone())
}

/// 发送：每个事件独占创建下一个序号，抢到了就当作读到了自己的事件接上日志、从待发送里去掉、写盘；没抢到就拉取后再试。
fn push(
    state: &mut SyncState,
    parts: &mut Parts<'_>,
    ring: &KeyRing,
    report: &mut Report,
) -> Result<(), SyncError> {
    let mut attempts = 0;
    let mut unsaved = 0;
    while let Some(key) = writable(state, ring) {
        let draft = next_draft(state);
        let now = parts.env.now();
        if draft.ops.is_empty() && draft.retire.is_empty() && !heartbeat_due(state, now) {
            break;
        }
        let ledger = state.ledger.as_ref().ok_or(SyncError::Unavailable)?;
        let path = event_path(ledger.epoch, ledger.seq + 1);
        let bytes = ledger.event(&state.device, &draft, now, &key, nonce(parts.env)?);
        if bytes.len() as u64 > MAX_EVENT_BYTES {
            return Err(SyncError::TooLarge);
        }
        if parts.remote.create(&path, &bytes)? {
            let received = state
                .ledger
                .as_mut()
                .ok_or(SyncError::Unavailable)?
                .apply(&bytes, Some(&key))
                .map_err(log_error)?;
            absorb(state, &received, parts.env.now_ms());
            state.retire.retain(|id| !draft.retire.contains(id));
            state.last_event_at = now;
            report.published_ops += draft.ops.len();
            report.published_events += 1;
            attempts = 0;
            // 不必每个事件都写盘：崩溃后重启时，拉取会认出已经在日志里的操作（[`absorb`]），不会重发。
            unsaved += 1;
            if unsaved >= SAVE_EVERY_EVENTS {
                parts.sink.save(state)?;
                unsaved = 0;
            }
            // 第一次加入时一次要发很多批：攒够了就在中途封口，免得一个 epoch 太大。
            if state.ledger.as_ref().is_some_and(Ledger::compaction_due) {
                compact(state, parts, ring, report)?;
                unsaved = 0;
            }
            continue;
        }
        attempts += 1;
        if attempts >= MAX_PUBLISH_ATTEMPTS {
            return Err(SyncError::Unavailable);
        }
        // 另一台设备先写了这个序号：读进来再试。
        if !pull(state, parts, ring, report)? {
            break;
        }
        apply_remote(state, parts, report)?;
        unsaved = 0;
    }
    if unsaved > 0 {
        parts.sink.save(state)?;
    }
    Ok(())
}

/// 该封口时封口：事件攒多了，或者密钥串换了钥、本机两把密钥都有（换钥要把共享状态用新密钥重新写一份）。封口不需要读写本机词库。
fn compact(
    state: &mut SyncState,
    parts: &mut Parts<'_>,
    ring: &KeyRing,
    report: &mut Report,
) -> Result<(), SyncError> {
    let Some(ledger) = state.ledger.clone() else {
        return Ok(());
    };
    if ledger.sealed.is_some() || state.key(&ledger.key_id).is_none() {
        return Ok(());
    }
    let rekey = ring
        .current
        .as_ref()
        .is_some_and(|current| current.key_id() != ledger.key_id);
    if !ledger.compaction_due() && !rekey {
        return Ok(());
    }
    let mut sealed = ledger;
    let seal = sealed.seal(parts.env.now());
    if !parts
        .remote
        .create(&event_path(sealed.epoch, sealed.seq + 1), &seal)?
    {
        // 另一台设备刚写过：下一次同步再说。
        return Ok(());
    }
    sealed.apply(&seal, None).map_err(log_error)?;
    state.ledger = Some(sealed.clone());
    parts.sink.save(state)?;
    report.compacted = true;
    cross_seal(state, parts, ring, sealed, report)?;
    Ok(())
}

/// 删掉过了宽限期的旧 epoch。失败不再试：留着的旧 epoch 不影响读写。
fn cleanup(state: &mut SyncState, parts: &mut Parts<'_>) -> Result<(), SyncError> {
    let now = parts.env.now();
    let current = state.ledger.as_ref().map_or(0, |ledger| ledger.epoch);
    let due: Vec<u64> = state
        .cleanup
        .iter()
        .filter(|entry| entry.not_before <= now && entry.epoch < current)
        .map(|entry| entry.epoch)
        .collect();
    if due.is_empty() {
        return Ok(());
    }
    for epoch in &due {
        let _ = parts.remote.delete(&epoch_dir(*epoch));
    }
    state.cleanup.retain(|entry| !due.contains(&entry.epoch));
    parts.sink.save(state)
}

/// 以本机为准重新开始：读不了当前 epoch（只有密钥串的新密钥，持有旧密钥的设备都不在了）时由用户选择。不解密地走到日志末尾，封口，用新密钥写一份只有本机词条的检查点，换一个新的共享词库编号。其他设备拿到新密钥后认出编号变了，把它们的词也合并进来（[`join`]），不会因此删词。
pub fn restart(
    state: &mut SyncState,
    parts: &mut Parts<'_>,
    ring: &KeyRing,
) -> Result<(), SyncError> {
    let key = ring.current.clone().ok_or(SyncError::Unavailable)?;
    state.remember(ring);
    if state.ledger.is_none() && !bootstrap_position(state, parts)? {
        return Err(SyncError::Unavailable);
    }
    let mut ledger = state.ledger.clone().ok_or(SyncError::Unavailable)?;
    for _ in 0..MAX_PUBLISH_ATTEMPTS {
        let Some(names) = parts.remote.list(&epoch_dir(ledger.epoch))? else {
            // 本机所在的 epoch 已经被压缩删掉了：重新定位到最新的那个。
            if !bootstrap_position(state, parts)? {
                return Err(SyncError::Unavailable);
            }
            ledger = state.ledger.clone().ok_or(SyncError::Unavailable)?;
            continue;
        };
        let seqs: BTreeSet<u64> = names.iter().filter_map(|name| event_name(name)).collect();
        while ledger.sealed.is_none() && seqs.contains(&(ledger.seq + 1)) {
            let Some(bytes) = parts
                .remote
                .get(&event_path(ledger.epoch, ledger.seq + 1), MAX_EVENT_BYTES)?
            else {
                break;
            };
            skip_without_key(&mut ledger, &bytes).map_err(log_error)?;
        }
        if ledger.sealed.is_some() {
            break;
        }
        let seal = ledger.seal(parts.env.now());
        if parts
            .remote
            .create(&event_path(ledger.epoch, ledger.seq + 1), &seal)?
        {
            skip_without_key(&mut ledger, &seal).map_err(log_error)?;
            break;
        }
    }
    if ledger.sealed.is_none() {
        return Err(SyncError::Unavailable);
    }
    let words = parts.local.words().map_err(local_error)?;
    let now_ms = parts.env.now_ms();
    let mut replica = Replica::default();
    for word in &words {
        let hlc = state.clock.tick(now_ms).ok_or(SyncError::Unavailable)?;
        replica.entries.insert(
            word.clone(),
            Record {
                live: true,
                hlc,
                device: state.device.clone(),
                seq: ledger.seq,
            },
        );
    }
    let lineage = random_hex(parts.env)?;
    let next = ledger
        .restart(&lineage, key.key_id(), replica)
        .ok_or(SyncError::Unavailable)?;
    let dir = epoch_dir(next.epoch);
    parts.remote.mkcol(&dir)?;
    if !parts.remote.create(
        &checkpoint_path(next.epoch),
        &next.checkpoint(&key, nonce(parts.env)?),
    )? {
        return Err(SyncError::Unavailable);
    }
    schedule_cleanup(state, ledger.epoch, parts.env.now());
    state.ledger = Some(next);
    state.applied = Some(words);
    state.outbox.clear();
    state.unappliable.clear();
    state.guard = None;
    parts.sink.save(state)
}

/// [`restart`] 用：本机还没有位置时，只读检查点的明文头部定位到最新的 epoch（不需要能解开它）。
fn bootstrap_position(state: &mut SyncState, parts: &mut Parts<'_>) -> Result<bool, SyncError> {
    let names = parts.remote.list(V1)?.ok_or(SyncError::Unavailable)?;
    let mut epochs: Vec<u64> = names.iter().filter_map(|name| epoch_name(name)).collect();
    epochs.sort_unstable_by(|a, b| b.cmp(a));
    for epoch in epochs {
        let Some(bytes) = parts
            .remote
            .get(&checkpoint_path(epoch), MAX_CHECKPOINT_BYTES)?
        else {
            continue;
        };
        let header = checkpoint_header(&bytes).ok_or(SyncError::Corrupt)?;
        let mut ledger = Ledger::genesis(&header.lineage, &header.key_id);
        ledger.epoch = epoch;
        ledger.seq = header.seq;
        ledger.head = header.head;
        ledger.generation = header.generation;
        state.ledger = Some(ledger);
        return Ok(true);
    }
    Ok(false)
}

// ---- 序列化 ----

impl Serialize for WordKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.kind().name(), self.code(), self.word()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for WordKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (kind, code, word) = <(String, String, String)>::deserialize(deserializer)?;
        WordKind::from_name(&kind)
            .and_then(|kind| WordKey::canonical(kind, &code, &word))
            .ok_or_else(|| serde::de::Error::custom("invalid word key"))
    }
}

/// 以词条为键的表写成 `[[词条, 值], …]`：JSON 对象的键只能是字符串。
mod pairs {
    use super::WordKey;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<V: Serialize, S: Serializer>(
        map: &BTreeMap<WordKey, V>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(map.iter())
    }

    pub fn deserialize<'de, V: Deserialize<'de>, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<WordKey, V>, D::Error> {
        Ok(Vec::<(WordKey, V)>::deserialize(deserializer)?
            .into_iter()
            .collect())
    }
}

mod ledger_serde {
    use super::super::log::{Ledger, ReplicaFile, SealInfo};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Stored {
        epoch: u64,
        seq: u64,
        head: String,
        events: u32,
        bytes: u64,
        sealed: Option<SealInfo>,
        key_id: String,
        generation: u64,
        lineage: String,
        replica: ReplicaFile,
    }

    pub fn serialize<S: Serializer>(
        ledger: &Option<Ledger>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        ledger
            .as_ref()
            .map(|ledger| Stored {
                epoch: ledger.epoch,
                seq: ledger.seq,
                head: ledger.head.clone(),
                events: ledger.events,
                bytes: ledger.bytes,
                sealed: ledger.sealed.clone(),
                key_id: ledger.key_id.clone(),
                generation: ledger.generation,
                lineage: ledger.lineage.clone(),
                replica: ReplicaFile::from_replica(&ledger.replica),
            })
            .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Ledger>, D::Error> {
        let Some(stored) = Option::<Stored>::deserialize(deserializer)? else {
            return Ok(None);
        };
        let replica = stored
            .replica
            .into_replica(stored.seq)
            .ok_or_else(|| serde::de::Error::custom("invalid replica"))?;
        Ok(Some(Ledger {
            epoch: stored.epoch,
            seq: stored.seq,
            head: stored.head,
            events: stored.events,
            bytes: stored.bytes,
            sealed: stored.sealed,
            key_id: stored.key_id,
            generation: stored.generation,
            lineage: stored.lineage,
            replica,
        }))
    }
}
