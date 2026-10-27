//! `msime_client_dictionary_sync`：「我的词条」经 WebDAV 网盘同步的宿主入口。请求和每个动作的结果见 `include/msime_client.h`，协议见 `docs/dictionary-webdav-sync.md`。
//!
//! 这里只做两件事：按宿主的 HostOptions 和偏好找到配置（[`DictionarySync`]），以及把 Engine 的个人词库接成协议核心要的本机词库（[`EngineDictionary`]）。
//!
//! 本机词库的写法：
//!
//! - 读：`stream_dictionary_state` 在一个只读事务里读出全部日志行，取 `user_inserted` 而且没删的那些——「我的词条」列表里的词，同一时刻的一致视图。
//! - 写：一批最多 128 项，经 `dictionary_edit_batch` 在一个事务里写；加入用默认权重，删除带上刚读到的权重（Engine 要核对它）；请求编号由核心给，重试幂等。
//! - 维护权：每批写之前取独占维护权，写完就放，从不跨网络请求持有。拿不到时：桌面端（Linux、macOS、Windows）在用户手动同步或打开设置时经 `QuiescedHosts` 请输入法让出词库（遇忙以 50 毫秒间隔在 2500 毫秒预算内重试，租约 30 秒，写完这一轮就撤）；自动同步和移动端不打扰输入会话，直接答「忙」，这一轮不写，按 5 秒、30 秒、2 分钟、10 分钟后再试。
//!
//! 整个调用会联网（`status` 除外），宿主放在工作线程上，输入线程从不等它。
//!
//! 输入法进程（macOS 的 IMK、Linux 的 IBus 和 Fcitx5）不自己调这个入口，而是经 `msime_client_dictionary_sync_start` 起一个后台调度（`msime_client_services::dictionary_webdav_scheduler`），宿主一侧是这里的 [`ImeHost`]：每一轮重读运行时配置文件和偏好，只用设置应用生成的设备身份，不请输入法让出词库。

use crate::*;
use msime_client_core::cloud::dictionary_sync::{
    LocalChange, LocalDictionary, LocalError, LocalOutcome, Options, Report, Trigger, WordKey,
    WordKind,
};
use msime_client_core::preferences::PreferencesStore;
use msime_client_services::dictionary::access::DictionaryAccess;
use msime_client_services::dictionary_webdav::{
    probe, DictionarySync, DictionarySyncError, RunResult, SyncView,
};
use msime_client_services::dictionary_webdav_scheduler::{
    fingerprint, Round, Scheduler, SchedulerHost, Watched, BACKGROUND_LOCK_WAIT,
};
use std::collections::{BTreeSet, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;

/// 请求的上限：HostOptions 文档里有整份偏好（最多 1 MiB）。
pub(crate) const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const INVALID: &str = "invalid_dictionary_sync";
const MAX_PATH_BYTES: usize = 16 * 1024;

/// 同步认识的词库类别；Engine 以后多出来的类别先不同步。
fn word_kind(kind: msime_engine::host::DictionaryKind) -> Option<WordKind> {
    use msime_engine::host::DictionaryKind;
    Some(match kind {
        DictionaryKind::Pinyin => WordKind::Pinyin,
        DictionaryKind::Wubi => WordKind::Wubi,
        DictionaryKind::Wubi98 => WordKind::Wubi98,
        DictionaryKind::QuickPhrase => WordKind::Quick,
        DictionaryKind::English => WordKind::English,
        _ => return None,
    })
}

fn engine_kind(kind: WordKind) -> msime_engine::host::DictionaryKind {
    use msime_engine::host::DictionaryKind;
    match kind {
        WordKind::Pinyin => DictionaryKind::Pinyin,
        WordKind::Wubi => DictionaryKind::Wubi,
        WordKind::Wubi98 => DictionaryKind::Wubi98,
        WordKind::Quick => DictionaryKind::QuickPhrase,
        WordKind::English => DictionaryKind::English,
    }
}

/// Engine 的个人词库，接成协议核心的本机词库。
pub(crate) struct EngineDictionary<'a> {
    options: &'a EngineOptions,
    /// 拿不到维护权时请输入法让出词库（只在桌面端、用户手动同步或打开设置时）。
    quiesce: bool,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    hosts: Option<msime_client_services::dictionary::quiesce::QuiescedHosts<'a, fn()>>,
    /// 上一次读到的权重：删除时 Engine 要核对它。
    weights: HashMap<WordKey, i64>,
}

impl<'a> EngineDictionary<'a> {
    pub(crate) fn new(options: &'a EngineOptions, quiesce: bool) -> Self {
        Self {
            options,
            quiesce,
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            hosts: None,
            weights: HashMap::new(),
        }
    }

    fn read(&mut self) -> Result<BTreeSet<WordKey>, LocalError> {
        let mut words = BTreeSet::new();
        let mut weights = HashMap::new();
        msime_engine::host::stream_dictionary_state(self.options, &mut |record| {
            if let msime_engine::host::DictionaryStateRecord::Entry {
                kind,
                key,
                value,
                weight,
                deleted: false,
                user_inserted: true,
                ..
            } = record
            {
                // 编码或词不合同步的规则（例如超长）的词条不同步，留在本机。
                if let Some(word) =
                    word_kind(*kind).and_then(|kind| WordKey::canonical(kind, key, value))
                {
                    weights.insert(word.clone(), *weight);
                    words.insert(word);
                }
            }
            true
        })
        .map_err(|_| LocalError::Unavailable)?;
        self.weights = weights;
        Ok(words)
    }

    /// 取独占维护权。
    fn maintenance(&mut self) -> Result<DictionaryAccess, LocalError> {
        let options: &'a EngineOptions = self.options;
        let attempt = move || {
            DictionaryAccess::try_maintenance(
                std::path::Path::new(&options.user_data),
                std::path::Path::new(&options.dictionaries),
            )
            .map_err(|_| "dictionary access unavailable".to_owned())?
            .ok_or_else(|| msime_client_services::dictionary::quiesce::BUSY.to_owned())
        };
        let busy = |reason: String| {
            if reason == msime_client_services::dictionary::quiesce::BUSY {
                LocalError::Busy
            } else {
                LocalError::Unavailable
            }
        };
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        if self.quiesce {
            let hosts = self.hosts.get_or_insert_with(|| {
                msime_client_services::dictionary::quiesce::QuiescedHosts::new(
                    Some(options.user_data.as_str()),
                    (|| {}) as fn(),
                )
            });
            return hosts.run(attempt).map_err(busy);
        }
        attempt().map_err(busy)
    }

    fn entry(&self, key: &WordKey, weight: i64) -> msime_engine::host::DictionaryEntry {
        msime_engine::host::DictionaryEntry {
            kind: engine_kind(key.kind()),
            key: key.code().to_owned(),
            value: key.word().to_owned(),
            weight,
        }
    }

    /// 一批写进 Engine，结果与 `changes` 一一对应；`None` 是这一项写不进去。
    fn write(&self, changes: &[(&LocalChange, Option<i64>)]) -> Result<Vec<bool>, LocalError> {
        let entries: Vec<(
            Option<msime_engine::host::DictionaryEntry>,
            Option<msime_engine::host::DictionaryEntry>,
        )> = changes
            .iter()
            .map(|(change, weight)| {
                if change.add {
                    (
                        None,
                        Some(self.entry(
                            &change.key,
                            msime_engine::host::DictionaryEntry::DEFAULT_WEIGHT,
                        )),
                    )
                } else {
                    (weight.map(|weight| self.entry(&change.key, weight)), None)
                }
            })
            .collect();
        let mut written = vec![false; changes.len()];
        let mut indexes = Vec::new();
        for (index, ((change, _), (previous, replacement))) in
            changes.iter().zip(&entries).enumerate()
        {
            if crate::validate_personal_dictionary_edit(
                self.options,
                previous.as_ref(),
                replacement.as_ref(),
                &change.request_id,
            )
            .is_ok()
            {
                indexes.push(index);
            }
        }
        let edits: Vec<msime_engine::host::DictionaryEdit<'_>> = indexes
            .iter()
            .map(|index| msime_engine::host::DictionaryEdit {
                previous: entries[*index].0.as_ref(),
                replacement: entries[*index].1.as_ref(),
                request_id: &changes[*index].0.request_id,
                // 加入时就是要它成为「我的词条」：主词库里已有同样的词也写一行用户词，否则它不会出现在这台设备的列表里，下一次核对会当成本机删掉了。
                preserve_existing: false,
            })
            .collect();
        if edits.is_empty() {
            return Ok(written);
        }
        let results = msime_engine::host::dictionary_edit_batch(self.options, &edits)
            .map_err(|_| LocalError::Unavailable)?;
        for (index, result) in indexes.into_iter().zip(results) {
            written[index] = result.is_ok();
        }
        Ok(written)
    }
}

impl LocalDictionary for EngineDictionary<'_> {
    fn words(&mut self) -> Result<BTreeSet<WordKey>, LocalError> {
        self.read()
    }

    fn apply(&mut self, changes: &[LocalChange]) -> Result<Vec<LocalOutcome>, LocalError> {
        let _access = self.maintenance()?;
        // 要删的词不在本机的「我的词条」里就是已经删好了，不用写。
        let mut outcomes = vec![LocalOutcome::Applied; changes.len()];
        let mut pending: Vec<usize> = (0..changes.len())
            .filter(|index| changes[*index].add || self.weights.contains_key(&changes[*index].key))
            .collect();
        // 删除带的权重可能刚被打字学习改过：失败的删除重读一次权重再试。
        for round in 0..2 {
            if pending.is_empty() {
                break;
            }
            let batch: Vec<(&LocalChange, Option<i64>)> = pending
                .iter()
                .map(|index| {
                    (
                        &changes[*index],
                        self.weights.get(&changes[*index].key).copied(),
                    )
                })
                .collect();
            let written = self.write(&batch)?;
            let failed: Vec<usize> = pending
                .iter()
                .zip(written)
                .filter_map(|(index, ok)| (!ok).then_some(*index))
                .collect();
            if round == 0 && failed.iter().any(|index| !changes[*index].add) {
                let present = self.read()?;
                pending = failed
                    .iter()
                    .copied()
                    .filter(|index| {
                        let change = &changes[*index];
                        !change.add && present.contains(&change.key)
                    })
                    .collect();
                for index in failed {
                    if changes[index].add {
                        outcomes[index] = LocalOutcome::Rejected;
                    }
                }
            } else {
                for index in &failed {
                    outcomes[*index] = LocalOutcome::Rejected;
                }
                pending.clear();
            }
        }
        Ok(outcomes)
    }

    fn release(&mut self) {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.hosts = None;
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncRequest {
    options: Value,
    directory: String,
    #[serde(default)]
    device_name: Option<String>,
    /// 为 `false` 时只用本机已有的设备身份（[`DictionarySync::existing_identity_only`]）。
    #[serde(default)]
    create_identity: Option<bool>,
    action: Value,
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Run {
        trigger: Trigger,
        #[serde(default)]
        confirm_deletions: bool,
        #[serde(default)]
        restore_deletions: bool,
    },
    Status,
    Devices,
    Approve {
        device_id: String,
    },
    Reject {
        device_id: String,
    },
    Remove {
        device_id: String,
    },
    Reset,
    Restart,
    Probe {
        url: String,
        username: String,
        password: String,
    },
}

fn report_json(report: &Report) -> Value {
    json!({
        "captured": report.captured,
        "received_events": report.received_events,
        "applied": report.applied,
        "deferred": report.deferred,
        "published_ops": report.published_ops,
        "pending_key": report.pending_key,
        "compacted": report.compacted,
        "rejoined": report.rejoined,
    })
}

fn view_json(view: &SyncView) -> Value {
    serde_json::to_value(view).unwrap_or(Value::Null)
}

fn run_json(run: &RunResult) -> Value {
    let mut value = json!({
        "network": run.network,
        "next_check_ms": run.next_check_ms,
        "status": view_json(&run.view),
    });
    match &run.report {
        Ok(report) => value["report"] = report_json(report),
        Err(error) => value["error"] = error.code().into(),
    }
    value
}

fn code(error: DictionarySyncError) -> String {
    error.code().to_owned()
}

/// 一个请求：`{options, directory, device_name?, action}`，见头文件。
pub(crate) fn request_json(bytes: &[u8]) -> Result<Value, String> {
    let request: SyncRequest = serde_json::from_slice(bytes).map_err(|_| INVALID)?;
    let action: Action = serde_json::from_value(request.action).map_err(|_| INVALID)?;
    let absolute =
        |path: &str| path.len() <= MAX_PATH_BYTES && std::path::Path::new(path).is_absolute();
    if !absolute(&request.directory) {
        return Err(INVALID.into());
    }
    let directory = std::path::Path::new(&request.directory);
    if let Action::Probe {
        url,
        username,
        password,
    } = &action
    {
        probe(url, username, password, Some(directory)).map_err(code)?;
        return Ok(json!({}));
    }
    let options = HostOptions::from_document(request.options).ok_or(INVALID)?;
    if options.api_version != 1 {
        return Err(INVALID.into());
    }
    let preferences_directory = options
        .preferences_directory
        .clone()
        .filter(|path| absolute(path))
        .ok_or(INVALID)?;
    let preferences = PreferencesStore::new(std::path::Path::new(&preferences_directory))
        .load()
        .map_err(|_| "dictionary_sync_storage")?
        .preferences;
    let device_name = request
        .device_name
        .filter(|name| !name.trim().is_empty() && msime_client_core::is_bounded_text(name, 256))
        .unwrap_or_else(msime_client_services::account::cloud_clipboard_device_name);
    let sync = DictionarySync::from_preferences(
        &preferences.dictionary_sync_webdav,
        directory,
        &device_name,
    )
    .map_err(code)?
    .ok_or("dictionary_sync_not_configured")?;
    let sync = if request.create_identity == Some(false) {
        sync.existing_identity_only()
    } else {
        sync
    };
    let engine = options.into_engine_options();
    // 用户在等着看结果的两种情况，桌面端可以请输入法让出词库。
    let interactive = matches!(
        action,
        Action::Run {
            trigger: Trigger::Manual | Trigger::Opened,
            ..
        } | Action::Remove { .. }
            | Action::Reset
            | Action::Restart
    );
    let mut local = EngineDictionary::new(&engine, interactive);
    match action {
        Action::Run {
            trigger,
            confirm_deletions,
            restore_deletions,
        } => {
            let choice = Options {
                confirm_deletions,
                restore_deletions,
                ..Options::default()
            };
            let run = sync.run(&mut local, trigger, choice).map_err(code)?;
            Ok(run_json(&run))
        }
        Action::Status => Ok(view_json(&sync.status().map_err(code)?)),
        Action::Devices => Ok(serde_json::to_value(sync.devices().map_err(code)?)
            .map_err(|_| "dictionary_sync_storage")?),
        Action::Approve { device_id } => Ok(serde_json::to_value(
            sync.approve(&device_id).map_err(code)?,
        )
        .map_err(|_| "dictionary_sync_storage")?),
        Action::Reject { device_id } => {
            Ok(serde_json::to_value(sync.reject(&device_id).map_err(code)?)
                .map_err(|_| "dictionary_sync_storage")?)
        }
        Action::Remove { device_id } => {
            let (devices, run) = sync.remove(&mut local, &device_id).map_err(code)?;
            Ok(json!({ "devices": devices, "run": run_json(&run) }))
        }
        Action::Reset => {
            let (devices, run) = sync.reset(&mut local).map_err(code)?;
            Ok(json!({ "devices": devices, "run": run_json(&run) }))
        }
        Action::Restart => Ok(view_json(&sync.restart(&mut local).map_err(code)?)),
        Action::Probe { .. } => Err(INVALID.into()),
    }
}

// ---- 输入法进程里的后台同步 ----

/// `msime_client_dictionary_sync_start` 的请求：运行时配置文件和放本机同步状态的目录，都是绝对路径。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartRequest {
    options_path: String,
    directory: String,
}

/// 输入法进程一侧的后台同步：每一轮重读运行时配置文件和偏好（设置应用随时可能配好或改掉网盘，词库也可能换了代次），只用设置应用生成的设备身份，不请输入法让出词库，另一个进程在同步时最多等 [`BACKGROUND_LOCK_WAIT`]。
pub(crate) struct ImeHost {
    options_path: PathBuf,
    directory: PathBuf,
    /// 上一轮读到的 Engine 选项，看个人词库文件和读「我的词条」用它；没配置时为 `None`。
    engine: Option<EngineOptions>,
}

impl ImeHost {
    pub(crate) fn new(options_path: PathBuf, directory: PathBuf) -> Self {
        Self {
            options_path,
            directory,
            engine: None,
        }
    }

    /// 读运行时配置文件。宿主自己也跟随符号链接读它，这里同样；不阻塞在一个 FIFO 上，大小有上限。
    fn options(&self) -> Option<HostOptions> {
        let mut open = std::fs::OpenOptions::new();
        open.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            open.custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC);
        }
        let file = open.open(&self.options_path).ok()?;
        if !file.metadata().ok()?.is_file() {
            return None;
        }
        let bytes = crate::bounded_file::read(file, HOST_OPTIONS_DOCUMENT_LIMIT as u64).ok()?;
        let options = HostOptions::from_document(serde_json::from_slice(&bytes).ok()?)?;
        (options.api_version == 1).then_some(options)
    }

    /// 这一轮的同步和 Engine 选项；没配置词库同步、配置读不出来时为 `None`。
    fn prepare(&self) -> Option<(DictionarySync, EngineOptions)> {
        let options = self.options()?;
        let preferences_directory = options
            .preferences_directory
            .clone()
            .filter(|path| Path::new(path).is_absolute())?;
        let preferences = PreferencesStore::new(Path::new(&preferences_directory))
            .load()
            .ok()?
            .preferences;
        // 设备名与设置应用不给名字时用的相同：只用已有身份时不登记，名字只在驱动刷新本机登记时用到。
        let sync = DictionarySync::from_preferences(
            &preferences.dictionary_sync_webdav,
            &self.directory,
            &msime_client_services::account::cloud_clipboard_device_name(),
        )
        .ok()??
        .existing_identity_only()
        .lock_wait(BACKGROUND_LOCK_WAIT);
        Some((sync, options.into_engine_options()))
    }
}

impl SchedulerHost for ImeHost {
    fn run(&mut self, trigger: Trigger) -> Option<Round> {
        let Some((sync, engine)) = self.prepare() else {
            self.engine = None;
            return None;
        };
        // 自动同步不打扰输入会话：拿不到维护权就答「忙」，按核心的间隔再试。
        let mut local = EngineDictionary::new(&engine, false);
        let mut watched = Watched::new(&mut local);
        let result = sync.run(&mut watched, trigger, Options::default());
        let seen = watched.seen();
        self.engine = Some(engine);
        Some(Round { result, seen })
    }

    fn fingerprint(&mut self) -> Option<u64> {
        let engine = self.engine.as_ref()?;
        let words = EngineDictionary::new(engine, false).words().ok()?;
        Some(fingerprint(&words))
    }

    fn stamp(&mut self) -> Option<u64> {
        let engine = self.engine.as_ref()?;
        let journal = Path::new(&engine.user_data).join(msime_engine::assets::USER_JOURNAL);
        let mut hasher = DefaultHasher::new();
        // 日志库开着 WAL：提交先写进 `-wal`，检查点才回到主文件，两个都看。文件还不存在也是一种状态，出现时戳就变了。
        for path in [journal.clone(), journal.with_extension("db-wal")] {
            match std::fs::metadata(&path) {
                Ok(metadata) => {
                    metadata.len().hash(&mut hasher);
                    metadata
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                        .hash(&mut hasher);
                }
                Err(_) => u64::MAX.hash(&mut hasher),
            }
        }
        Some(hasher.finish())
    }
}

/// 本进程里在跑的后台同步，最多一个。
static BACKGROUND: Mutex<Option<Scheduler>> = Mutex::new(None);

/// 起后台同步，见头文件的 `msime_client_dictionary_sync_start`。已经在跑时换成这一次的请求：旧的叫停、不等。
pub(crate) fn start_json(bytes: &[u8]) -> Result<Value, String> {
    let request: StartRequest = serde_json::from_slice(bytes).map_err(|_| INVALID)?;
    let absolute =
        |path: &str| path.len() <= MAX_PATH_BYTES && std::path::Path::new(path).is_absolute();
    if !absolute(&request.options_path) || !absolute(&request.directory) {
        return Err(INVALID.into());
    }
    let host = ImeHost::new(request.options_path.into(), request.directory.into());
    let scheduler = Scheduler::start(host).map_err(|_| "dictionary_sync_unavailable")?;
    let previous = BACKGROUND
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .replace(scheduler);
    if let Some(previous) = previous {
        previous.stop(Some(Duration::ZERO));
    }
    Ok(json!({}))
}

/// 叫停后台同步，最多等 `timeout_ms` 毫秒让它退出，`u32::MAX` 一直等。返回它是否已经退出；没有在跑的也算。
pub(crate) fn stop(timeout_ms: u32) -> bool {
    let scheduler = BACKGROUND
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    let Some(scheduler) = scheduler else {
        return true;
    };
    scheduler.stop((timeout_ms != u32::MAX).then(|| Duration::from_millis(timeout_ms.into())))
}

#[cfg(test)]
#[path = "dictionary_webdav_tests.rs"]
mod tests;
