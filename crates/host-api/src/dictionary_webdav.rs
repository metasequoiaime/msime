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
use std::collections::{BTreeSet, HashMap};

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
        #[serde(default)]
        trust_remote: bool,
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
        trust_remote,
    } = &action
    {
        probe(url, username, password, trust_remote.then_some(directory)).map_err(code)?;
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

#[cfg(test)]
#[path = "dictionary_webdav_tests.rs"]
mod tests;
