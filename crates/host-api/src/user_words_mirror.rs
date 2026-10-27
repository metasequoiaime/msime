//! `msime_client_user_words_mirror`：同一台设备上两个各有一份 Engine 日志的进程之间交换「我的词条」，让两份收敛成同一组词条。HarmonyOS 从 API 12 起设置应用和输入法扩展各在自己的沙箱里，各有一份个人词库，这个入口就是为它写的；请求和结果见 `include/msime_client.h`，取舍见 `.agents/notes/implemented/feature/2026-10-27-harmony-unified-user-words.md`。
//!
//! 「我的词条」与 WebDAV 同步说的是同一组词：Engine 日志里 `user_inserted` 而且没删的词条，身份是（词库类别、编码、词），权重和学习记录不交换（读写都经 [`EngineDictionary`]，与 `dictionary_webdav` 用同一个实现）。
//!
//! 模型：每一边在自己的私有目录里给每个词条记一个「最后写入者胜」的寄存器（时间戳、在不在），外加本机词库里实际有没有它（`applied`），并把寄存器表发布到两边都读得到的地方（鸿蒙的共享沙箱）。一次 `sync` 按固定顺序做：
//!
//! 1. 记录本机改动：本机词库相对 `applied` 的变化记成带新时间戳的寄存器。输入法一侧一次少了 [`HELD_DELETION_MINIMUM`] 个以上、并且至少是原有词条一半时不记删除（输入法没有让用户一次删掉这么多词的操作，多半是它的日志坏了被换成空的），这些词只标成本机没有，另一边照样留着。设置应用一侧不挡：那边的大批删除来自用户在设置页的操作（删词、停用命名词库、清除学习数据）或者 WebDAV 同步（它自己挡过一次、让用户选过）。
//! 2. 合并对方发布的寄存器表：对方的更新（时间戳更大；相同时「在」胜过「不在」）就采用，本机词库与它不一致时写进去。每批最多 128 项，维护权只在一批之内持有；拿不到维护权或者到了这次的上限时，剩下的不采用、留到下一次。Engine 不收的词照实记下（`applied` 保持本机词库的样子），不再重试，免得下一次核对把它当成本机的改动发回去。
//! 3. 回收墓碑：对方发布里的 `merged`（它完整合并过的本方发布的修订号）不小于墓碑的时间戳时，对方已经见过这次删除，删掉它。墓碑另有总数上限，超过时去掉最早的。
//! 4. 存盘，再发布。
//!
//! 第一次运行时两边都没有寄存器，各自的词都记成新加的，合并的结果是并集：迁移从不删词。合并只看寄存器，与两边谁先运行、运行几次无关。

use crate::dictionary_webdav::EngineDictionary;
use crate::*;
use msime_client_core::cloud::dictionary_sync::{
    Clock, Hlc, LocalChange, LocalDictionary, LocalError, LocalOutcome, WordKey, WordKind,
};
use std::collections::{BTreeMap, BTreeSet};

/// 请求的上限：里面有一整份 HostOptions。
pub(crate) const MAX_REQUEST_BYTES: usize = HOST_OPTIONS_DOCUMENT_LIMIT + 64 * 1024;
pub(crate) const INVALID: &str = "invalid_user_words_mirror";
/// 另一次调用正持有本方私有目录的锁。
const BUSY: &str = "user_words_busy";
/// 私有状态或发布文件读写失败。
const STORAGE: &str = "user_words_storage";
/// 读不出本机词库。
const UNAVAILABLE: &str = "user_words_unavailable";
/// 寄存器超过上限，或者序列化后超过文件上限。
const TOO_LARGE: &str = "user_words_too_large";

const FORMAT: &str = "msime-user-words";
const VERSION: u32 = 1;
const STATE_FILE: &str = "user-words-state.json";
const LOCK_FILE: &str = "user-words.lock";
/// 寄存器（含墓碑）的上限，与 WebDAV 同步收新词的上限同一个量级。
pub(crate) const MAX_ENTRIES: usize = 100_000;
/// 本机没有、只为告诉对方「删了」而留着的墓碑的上限；对方一直不确认（例如键盘一直在基础访问模式下、不发布）时去掉最早的。
pub(crate) const MAX_TOMBSTONES: usize = 20_000;
/// 私有状态和发布文件的字节上限。
const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
/// 一次核对里少了这么多个、并且至少是原有词条一半时不记删除，与 WebDAV 同步挡下大批删除的门槛相同。
pub(crate) const HELD_DELETION_MINIMUM: usize = 50;
const DEFAULT_MAX_CHANGES: usize = 128;
const MAX_CHANGES: usize = 4096;
/// 一批写进 Engine 的上限，与 `dictionary_edit_batch` 相同。
const APPLY_BATCH: usize = 128;
const MAX_PATH_BYTES: usize = 4096;

/// 发布方。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Side {
    Settings,
    Keyboard,
}

/// 一个词条在本方的寄存器。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Register {
    pub stamp: Hlc,
    pub exists: bool,
    /// 本方的词库里有没有这个词。与 `exists` 不同只有两种情况：Engine 不收、删不掉，或者大批删除被挡下。
    pub applied: bool,
}

/// 本方私有目录里的状态。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MirrorState {
    pub clock: Clock,
    /// 完整合并过的对方发布的修订号；随本方的发布交给对方，对方据此回收墓碑。
    pub merged: u64,
    pub registers: BTreeMap<WordKey, Register>,
}

/// 一方发布的寄存器表。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Publication {
    pub publisher: Side,
    /// 发布时发布方时钟的值：比表里所有的时间戳都不小，发布方此后新记的改动都比它大。
    pub revision: u64,
    /// 发布方完整合并过的本方发布的修订号。
    pub merged: u64,
    pub entries: BTreeMap<WordKey, (Hlc, bool)>,
}

/// 一次调用做了什么。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Report {
    /// 新记下的本机改动。
    pub captured: usize,
    /// 写进本机词库的对方改动。
    pub applied: usize,
    /// Engine 不收的对方改动。
    pub rejected: usize,
    /// 这一次没写（忙或者到了上限）、留到下一次的对方改动。
    pub deferred: usize,
    /// 被挡下、没有记成删除的本机删除。
    pub held_deletions: usize,
    /// 现在本方寄存器里「在」的词条数。
    pub words: usize,
}

type StateEntry = (WordKind, String, String, u64, bool, bool);
type PublishedEntry = (WordKind, String, String, u64, bool);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StateFile {
    format: String,
    version: u32,
    clock: u64,
    merged: u64,
    entries: Vec<StateEntry>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicationFile {
    format: String,
    version: u32,
    publisher: Side,
    revision: u64,
    merged: u64,
    entries: Vec<PublishedEntry>,
}

fn key(kind: WordKind, code: &str, word: &str) -> Option<WordKey> {
    WordKey::canonical(kind, code, word)
}

impl MirrorState {
    pub(crate) fn encode(&self) -> Result<Vec<u8>, &'static str> {
        let file = StateFile {
            format: FORMAT.into(),
            version: VERSION,
            clock: self.clock.last(),
            merged: self.merged,
            entries: self
                .registers
                .iter()
                .map(|(key, register)| {
                    (
                        key.kind(),
                        key.code().to_owned(),
                        key.word().to_owned(),
                        register.stamp.0,
                        register.exists,
                        register.applied,
                    )
                })
                .collect(),
        };
        let bytes = serde_json::to_vec(&file).map_err(|_| STORAGE)?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(TOO_LARGE);
        }
        Ok(bytes)
    }

    /// 读不懂的状态（格式不对、时间戳不合法、词条不规范或重复）返回 `None`，调用方从头开始：第一次运行不删词，所以丢掉状态的代价只是以前在一边删过的词可能回来。
    pub(crate) fn decode(bytes: &[u8]) -> Option<Self> {
        let file: StateFile = serde_json::from_slice(bytes).ok()?;
        if file.format != FORMAT || file.version != VERSION || file.entries.len() > MAX_ENTRIES {
            return None;
        }
        let mut clock = Clock::default();
        clock.observe(Hlc(file.clock), 0);
        let mut registers = BTreeMap::new();
        for (kind, code, word, stamp, exists, applied) in file.entries {
            let key = key(kind, &code, &word)?;
            let stamp = Hlc::parse(stamp)?;
            clock.observe(stamp, 0);
            let register = Register {
                stamp,
                exists,
                applied,
            };
            if registers.insert(key, register).is_some() {
                return None;
            }
        }
        Some(Self {
            clock,
            merged: file.merged,
            registers,
        })
    }

    pub(crate) fn publication(&self, publisher: Side) -> Publication {
        Publication {
            publisher,
            revision: self.clock.last(),
            merged: self.merged,
            entries: self
                .registers
                .iter()
                .map(|(key, register)| (key.clone(), (register.stamp, register.exists)))
                .collect(),
        }
    }
}

impl Publication {
    pub(crate) fn encode(&self) -> Result<Vec<u8>, &'static str> {
        let file = PublicationFile {
            format: FORMAT.into(),
            version: VERSION,
            publisher: self.publisher,
            revision: self.revision,
            merged: self.merged,
            entries: self
                .entries
                .iter()
                .map(|(key, (stamp, exists))| {
                    (
                        key.kind(),
                        key.code().to_owned(),
                        key.word().to_owned(),
                        stamp.0,
                        *exists,
                    )
                })
                .collect(),
        };
        let bytes = serde_json::to_vec(&file).map_err(|_| STORAGE)?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(TOO_LARGE);
        }
        Ok(bytes)
    }

    /// 读不懂的发布（格式、时间戳、词条不合规或重复）整份不认，返回 `None`。
    pub(crate) fn decode(bytes: &[u8]) -> Option<Self> {
        let file: PublicationFile = serde_json::from_slice(bytes).ok()?;
        if file.format != FORMAT || file.version != VERSION || file.entries.len() > MAX_ENTRIES {
            return None;
        }
        let mut entries = BTreeMap::new();
        for (kind, code, word, stamp, exists) in file.entries {
            let key = key(kind, &code, &word)?;
            let stamp = Hlc::parse(stamp)?;
            if stamp.0 > file.revision || entries.insert(key, (stamp, exists)).is_some() {
                return None;
            }
        }
        Some(Self {
            publisher: file.publisher,
            revision: file.revision,
            merged: file.merged,
            entries,
        })
    }
}

/// 写进本机词库的请求编号：`umirror-` 加 32 位小写十六进制，由词条、加入还是删除和决定它的那个时间戳唯一决定。Engine 按它记回执，崩溃后重试同一次写入不改变结果。
fn request_id(key: &WordKey, add: bool, stamp: Hlc) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for part in [
        "msime-user-words-v1-apply",
        key.kind().name(),
        key.code(),
        key.word(),
        if add { "add" } else { "delete" },
        &stamp.0.to_string(),
    ] {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    format!("umirror-{}", hex::encode(&hasher.finalize()[..16]))
}

/// 第 1 步：把本机词库相对 `applied` 的变化记成寄存器。`hold` 为真时挡下大批删除（只在输入法一侧）。返回（新记下的改动数，被挡下的删除数）。
pub(crate) fn capture(
    state: &mut MirrorState,
    words: &BTreeSet<WordKey>,
    now_ms: u64,
    hold: bool,
) -> Result<(usize, usize), &'static str> {
    let present = state
        .registers
        .values()
        .filter(|register| register.exists && register.applied)
        .count();
    let removed: Vec<WordKey> = state
        .registers
        .iter()
        .filter(|(key, register)| register.exists && register.applied && !words.contains(*key))
        .map(|(key, _)| key.clone())
        .collect();
    let hold = hold && removed.len() >= HELD_DELETION_MINIMUM && removed.len() * 2 >= present;
    let mut captured = 0;
    for key in &removed {
        let register = if hold {
            Register {
                applied: false,
                ..state.registers[key]
            }
        } else {
            captured += 1;
            Register {
                stamp: state.clock.tick(now_ms).ok_or(STORAGE)?,
                exists: false,
                applied: false,
            }
        };
        state.registers.insert(key.clone(), register);
    }
    // 删不掉而留着的词后来不在了：照实记下，不发新的操作（寄存器本来就是删除）。
    for (key, register) in state.registers.iter_mut() {
        if !register.exists && register.applied && !words.contains(key) {
            register.applied = false;
        }
    }
    for key in words {
        match state.registers.get_mut(key) {
            // 本来就在，或者以前 Engine 不收、现在有了。
            Some(register) if register.exists => register.applied = true,
            // 删不掉而留着的词，不是新加的。
            Some(register) if register.applied => {}
            _ => {
                captured += 1;
                let stamp = state.clock.tick(now_ms).ok_or(STORAGE)?;
                state.registers.insert(
                    key.clone(),
                    Register {
                        stamp,
                        exists: true,
                        applied: true,
                    },
                );
            }
        }
    }
    Ok((captured, if hold { removed.len() } else { 0 }))
}

/// 对方的寄存器是不是比本方的新：时间戳大的胜，相同时「在」胜过「不在」。两边按同一个全序比较，结果与谁先合并无关。
fn newer(theirs: (Hlc, bool), own: Option<&Register>) -> bool {
    own.is_none_or(|own| theirs > (own.stamp, own.exists))
}

/// 第 2 步：合并对方的发布。写本机词库失败（不是忙）时返回错误码，已经写好的照样记进 `state`。
pub(crate) fn merge<L: LocalDictionary>(
    state: &mut MirrorState,
    local: &mut L,
    words: &BTreeSet<WordKey>,
    theirs: &Publication,
    max_changes: usize,
    now_ms: u64,
    report: &mut Report,
) -> Option<&'static str> {
    state.clock.observe(Hlc(theirs.revision), now_ms);
    let mut pending: Vec<(WordKey, Hlc, bool)> = Vec::new();
    for (key, (stamp, exists)) in &theirs.entries {
        if !newer((*stamp, *exists), state.registers.get(key)) {
            continue;
        }
        let present = words.contains(key);
        if *exists == present {
            state.registers.insert(
                key.clone(),
                Register {
                    stamp: *stamp,
                    exists: *exists,
                    applied: present,
                },
            );
        } else {
            pending.push((key.clone(), *stamp, *exists));
        }
    }
    let total = pending.len();
    let mut done = 0;
    let mut failure = None;
    for batch in pending.chunks(APPLY_BATCH) {
        let room = max_changes.saturating_sub(done);
        if room == 0 {
            break;
        }
        let batch = &batch[..batch.len().min(room)];
        let changes: Vec<LocalChange> = batch
            .iter()
            .map(|(key, stamp, exists)| LocalChange {
                key: key.clone(),
                add: *exists,
                request_id: request_id(key, *exists, *stamp),
            })
            .collect();
        let outcomes = match local.apply(&changes) {
            Ok(outcomes) => outcomes,
            Err(LocalError::Busy) => break,
            Err(LocalError::Unavailable) => {
                failure = Some(UNAVAILABLE);
                break;
            }
        };
        for ((key, stamp, exists), outcome) in batch.iter().zip(outcomes) {
            let applied = match outcome {
                LocalOutcome::Applied => {
                    report.applied += 1;
                    *exists
                }
                LocalOutcome::Rejected => {
                    report.rejected += 1;
                    words.contains(key)
                }
            };
            state.registers.insert(
                key.clone(),
                Register {
                    stamp: *stamp,
                    exists: *exists,
                    applied,
                },
            );
        }
        done += batch.len();
    }
    local.release();
    report.deferred = total - done;
    if report.deferred == 0 && failure.is_none() {
        state.merged = theirs.revision;
    }
    failure
}

/// 第 3 步：回收墓碑。只回收本方词库里也没有的：删不掉而留着的词的墓碑一直留着，免得下一次核对把它当成新加的词。
pub(crate) fn collect(state: &mut MirrorState, theirs: Option<&Publication>) {
    if let Some(theirs) = theirs {
        state.registers.retain(|_, register| {
            register.exists || register.applied || register.stamp.0 > theirs.merged
        });
    }
    let mut tombstones: Vec<(Hlc, WordKey)> = state
        .registers
        .iter()
        .filter(|(_, register)| !register.exists && !register.applied)
        .map(|(key, register)| (register.stamp, key.clone()))
        .collect();
    if tombstones.len() > MAX_TOMBSTONES {
        tombstones.sort();
        let excess = tombstones.len() - MAX_TOMBSTONES;
        for (_, key) in tombstones.into_iter().take(excess) {
            state.registers.remove(&key);
        }
    }
}

/// 一次调用的核心，不碰文件：`side` 是本方，`theirs` 为 `None` 时只记录本机改动。写本机词库失败时返回错误，已经做完的照样留在 `state` 里，调用方先存盘再报错。
pub(crate) fn run<L: LocalDictionary>(
    side: Side,
    state: &mut MirrorState,
    local: &mut L,
    theirs: Option<&Publication>,
    max_changes: usize,
    now_ms: u64,
) -> (Report, Option<&'static str>) {
    let mut report = Report::default();
    let words = match local.words() {
        Ok(words) => words,
        Err(_) => return (report, Some(UNAVAILABLE)),
    };
    match capture(state, &words, now_ms, side == Side::Keyboard) {
        Ok((captured, held)) => {
            report.captured = captured;
            report.held_deletions = held;
        }
        Err(code) => return (report, Some(code)),
    }
    let failure = theirs.and_then(|theirs| {
        merge(
            state,
            local,
            &words,
            theirs,
            max_changes,
            now_ms,
            &mut report,
        )
    });
    collect(state, theirs);
    report.words = state
        .registers
        .values()
        .filter(|register| register.exists)
        .count();
    if state.registers.len() > MAX_ENTRIES {
        return (report, Some(TOO_LARGE));
    }
    (report, failure)
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Operation {
    /// 记录本机改动、合并对方、存盘、发布。会写本机词库，要 Engine 的维护权。
    Sync,
    /// 只记录本机改动、存盘、发布，不读对方、不写本机词库，不要维护权：输入会话开着时也能调。
    Publish,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    operation: Operation,
    side: Side,
    options: Value,
    directory: String,
    #[serde(default)]
    theirs: Option<String>,
    #[serde(default)]
    publish: Option<String>,
    #[serde(default)]
    max_changes: Option<usize>,
}

fn absolute(path: &str) -> Option<&std::path::Path> {
    let candidate = std::path::Path::new(path);
    (path.len() <= MAX_PATH_BYTES && candidate.is_absolute()).then_some(candidate)
}

fn read_bounded(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    let file = msime_client_core::file_lock::open_private_file(path)?;
    crate::bounded_file::read(file, MAX_FILE_BYTES)
}

fn write_publication(path: &std::path::Path, bytes: &[u8]) -> Result<(), &'static str> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| STORAGE)?;
    }
    // 读的是另一个进程。两个进程是同一个应用、有效 uid 相同，属主可读就够；给同组读是为了两边的 uid 不同而 gid 相同时也读得到，与 ArkTS 镜像复制出来的文件一样不给其他人。
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        msime_client_core::file_lock::replace_private_file_with_permissions(
            path,
            bytes,
            &std::fs::Permissions::from_mode(0o640),
        )
        .map_err(|_| STORAGE)
    }
    #[cfg(not(unix))]
    {
        msime_client_core::file_lock::replace_private_file(path, bytes).map_err(|_| STORAGE)
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// 对方发布的读取结果，随回复交给宿主写日志。
fn theirs_label(read: &Result<Option<Publication>, ()>) -> &'static str {
    match read {
        Ok(Some(_)) => "read",
        Ok(None) => "absent",
        Err(()) => "unreadable",
    }
}

/// 一个请求：`{operation, side, options, directory, theirs?, publish?, max_changes?}`，见头文件。
pub(crate) fn request_json(bytes: &[u8]) -> Result<Value, String> {
    let request: Request = serde_json::from_slice(bytes).map_err(|_| INVALID)?;
    let directory = absolute(&request.directory).ok_or(INVALID)?;
    let theirs_path = match request.theirs.as_deref() {
        Some(path) => Some(absolute(path).ok_or(INVALID)?),
        None => None,
    };
    let publish_path = match request.publish.as_deref() {
        Some(path) => Some(absolute(path).ok_or(INVALID)?),
        None => None,
    };
    if theirs_path.is_some() && theirs_path == publish_path {
        return Err(INVALID.into());
    }
    let max_changes = request.max_changes.unwrap_or(DEFAULT_MAX_CHANGES);
    if max_changes == 0 || max_changes > MAX_CHANGES {
        return Err(INVALID.into());
    }
    let options = HostOptions::from_document(request.options).ok_or(INVALID)?;
    if options.api_version != 1 {
        return Err(INVALID.into());
    }
    let engine = options.into_engine_options();

    std::fs::create_dir_all(directory).map_err(|_| STORAGE)?;
    let lock = msime_client_core::file_lock::open_private_lock_file(directory.join(LOCK_FILE))
        .map_err(|_| STORAGE)?;
    let _guard = msime_client_core::file_lock::ScopedFileLock::try_exclusive(lock)
        .map_err(|_| STORAGE)?
        .ok_or(BUSY)?;

    let state_path = directory.join(STATE_FILE);
    let mut state = match read_bounded(&state_path) {
        Ok(bytes) => MirrorState::decode(&bytes).unwrap_or_else(|| {
            crate::diagnostics::report(
                "user words mirror state unreadable",
                "starting over; the first run never deletes words",
            );
            MirrorState::default()
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => MirrorState::default(),
        Err(_) => return Err(STORAGE.into()),
    };
    let theirs: Result<Option<Publication>, ()> = match (&request.operation, theirs_path) {
        (Operation::Sync, Some(path)) => match read_bounded(path) {
            Ok(bytes) => Publication::decode(&bytes)
                .filter(|publication| publication.publisher != request.side)
                .map(Some)
                .ok_or(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(()),
        },
        _ => Ok(None),
    };

    let mut local = EngineDictionary::new(&engine, false);
    let (report, failure) = run(
        request.side,
        &mut state,
        &mut local,
        theirs.as_ref().ok().and_then(Option::as_ref),
        max_changes,
        now_ms(),
    );
    // 读不出本机词库或者寄存器超限时什么都没做成，不存盘。
    if matches!(failure, Some(UNAVAILABLE)) && report.applied + report.rejected == 0
        || matches!(failure, Some(TOO_LARGE))
    {
        return Err(failure.unwrap_or(STORAGE).into());
    }
    msime_client_core::file_lock::replace_private_file(&state_path, &state.encode()?)
        .map_err(|_| STORAGE)?;
    if let Some(code) = failure {
        return Err(code.into());
    }
    let published = match publish_path {
        Some(path) => {
            write_publication(path, &state.publication(request.side).encode()?)?;
            true
        }
        None => false,
    };
    Ok(json!({
        "captured": report.captured,
        "applied": report.applied,
        "rejected": report.rejected,
        "deferred": report.deferred,
        "held_deletions": report.held_deletions,
        "words": report.words,
        "theirs": theirs_label(&theirs),
        "published": published,
    }))
}

#[cfg(test)]
#[path = "user_words_mirror_tests.rs"]
mod tests;
