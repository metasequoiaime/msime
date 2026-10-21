//! 把打字统计的每日计数上传到云端（msime-cloud `PUT /v1/users/me/typing-statistics`），给 QQ 群助手的 `/stats` 读。
//!
//! 只在用户打开「上传到云端」后上传，默认关闭。上传的只有计数：每天的字数、活跃秒数、每小时的字数、各字符类别和输入来源的字数，以及全部历史的候选位次；没有打出的内容，也不传按键、徽章、皮肤和语音时长。开关、设备标识和上次上传的时间存在统计目录的 `typing-statistics-upload.json` 里，和统计文件同一把锁。设备标识是这里第一次需要时生成的随机 UUID，不复用匿名遥测的 `install_id`：那个标识承诺过不和账号关联。
//!
//! 构建和节流是纯函数加文件状态，发送由调用方给的闭包完成，所以这里不依赖账号会话，Tauri 外壳和以后的原生宿主都能用。

use super::{TypingStatistics, TypingStatisticsError, TypingStatisticsStore, HOURS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const UPLOAD_STATE_FILE: &str = "typing-statistics-upload.json";
/// 状态文件很小，超过这个大小的当成坏文件。
const MAX_UPLOAD_STATE_BYTES: u64 = 4096;
/// 一次上传最近有字数的几天。服务端一次最多收 31 天，近 7 天足够 `/stats` 比较「今天」和「上一周」：每次都重传最近 7 天，漏传的日子下次补上。
pub const UPLOAD_DAYS: usize = 7;
/// 只传今天往前这么多天以内的日子，和服务端接受的窗口（UTC 今天往前 61 天）一致，再留一天给时区差。
const UPLOAD_WINDOW_DAYS: i64 = 60;
/// 两次自动上传之间至少隔这么久；打开开关时不等。
pub const UPLOAD_INTERVAL_MS: u64 = 10 * 60 * 1000;
/// 服务端对每个计数的上限（`typingStatisticsMaxCount`），超过的整天不传，免得整份被 400 拒掉。
const MAX_DAY_COUNT: u64 = 10_000_000;
/// 服务端对候选位次累计的上限。
const MAX_RANK_COUNT: u64 = 1_000_000_000;
/// 服务端对字符类别和输入来源各自的种类上限。
const MAX_KEYS: usize = 32;

/// 本机的上传状态。
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadState {
    #[serde(default)]
    pub enabled: bool,
    /// 随机 UUID 的 32 位十六进制；第一次需要时生成。
    #[serde(default)]
    pub device: String,
    /// 上次上传成功的 Unix 毫秒，0 表示还没有。
    #[serde(default)]
    pub last_upload_ms: u64,
}

/// 设置页读的上传状态。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadStatus {
    pub enabled: bool,
    pub last_upload_ms: Option<u64>,
}

impl From<&UploadState> for UploadStatus {
    fn from(state: &UploadState) -> Self {
        Self {
            enabled: state.enabled,
            last_upload_ms: (state.last_upload_ms > 0).then_some(state.last_upload_ms),
        }
    }
}

/// 一天的计数，字段名和服务端的 `TypingDay` 一致。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UploadDay {
    pub day: String,
    pub characters: u64,
    pub active_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hours: Option<Vec<u64>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub categories: BTreeMap<String, u64>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub sources: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UploadSelections {
    pub ranks: Vec<u64>,
    pub beyond: u64,
}

/// `PUT /v1/users/me/typing-statistics` 的请求体。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TypingStatisticsUpload {
    pub device: String,
    pub platform: String,
    pub days: Vec<UploadDay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selections: Option<UploadSelections>,
}

/// 服务端认的平台（`typingStatisticsPlatforms`）。
pub const UPLOAD_PLATFORMS: [&str; 6] = ["windows", "macos", "linux", "android", "ios", "harmony"];

/// 服务端对计数表键名的要求：`^[A-Za-z][A-Za-z0-9_]{0,31}$`。
fn valid_key(key: &str) -> bool {
    let bytes = key.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 32
        && bytes[0].is_ascii_alphabetic()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
}

/// 去掉服务端不认的键和超出上限的值，最多留 `MAX_KEYS` 个（按字数从多到少，字数相同时按键名）。
fn counts(source: &BTreeMap<String, u64>) -> BTreeMap<String, u64> {
    let mut kept: Vec<(&String, u64)> = source
        .iter()
        .filter(|(key, count)| valid_key(key) && **count > 0 && **count <= MAX_DAY_COUNT)
        .map(|(key, count)| (key, *count))
        .collect();
    kept.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    kept.into_iter()
        .take(MAX_KEYS)
        .map(|(key, count)| (key.clone(), count))
        .collect()
}

/// 从统计里取最近 `UPLOAD_DAYS` 个有字数的日子（`today` 往前 `UPLOAD_WINDOW_DAYS` 天以内，不含 `today` 以后的），按日期从早到晚。一天也没有时返回 `None`。`today` 是宿主的本地日期。
pub fn build_upload(
    statistics: &TypingStatistics,
    today: &str,
    device: &str,
    platform: &str,
) -> Option<TypingStatisticsUpload> {
    let earliest = crate::calendar::shift_day(today, -UPLOAD_WINDOW_DAYS)?;
    let mut days: Vec<UploadDay> = statistics
        .days
        .iter()
        .rev()
        .filter(|(day, count)| {
            day.as_str() <= today
                && day.as_str() >= earliest.as_str()
                && **count > 0
                && **count <= MAX_DAY_COUNT
        })
        .take(UPLOAD_DAYS)
        .map(|(day, count)| {
            let detail = statistics.daily_details.get(day);
            UploadDay {
                day: day.clone(),
                characters: *count,
                active_seconds: statistics
                    .daily_active_ms
                    .get(day)
                    .map_or(0, |ms| (ms / 1000).min(24 * 3600)),
                hours: statistics
                    .daily_hours
                    .get(day)
                    .filter(|hours| {
                        hours.len() == HOURS && hours.iter().all(|n| *n <= MAX_DAY_COUNT)
                    })
                    .cloned(),
                categories: detail.map(|d| counts(&d.characters)).unwrap_or_default(),
                sources: detail.map(|d| counts(&d.sources)).unwrap_or_default(),
            }
        })
        .collect();
    if days.is_empty() {
        return None;
    }
    days.reverse();
    let selections = &statistics.selections;
    let selections = (selections.total() > 0
        && selections.beyond <= MAX_DAY_COUNT
        && selections.ranks.iter().all(|n| *n <= MAX_RANK_COUNT))
    .then(|| UploadSelections {
        ranks: selections.ranks.clone(),
        beyond: selections.beyond,
    });
    Some(TypingStatisticsUpload {
        device: device.to_owned(),
        platform: if UPLOAD_PLATFORMS.contains(&platform) {
            platform.to_owned()
        } else {
            String::new()
        },
        days,
        selections,
    })
}

/// 一次自动上传的结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UploadOutcome {
    /// 用户没有打开上传。
    Disabled,
    /// 离上次上传还不到 `UPLOAD_INTERVAL_MS`。
    NotDue,
    /// 最近没有可传的日子。
    Nothing,
    /// 传了这么多天。
    Uploaded(usize),
}

#[derive(Debug)]
pub enum UploadError<E> {
    Storage(TypingStatisticsError),
    Send(E),
}

impl<E> From<TypingStatisticsError> for UploadError<E> {
    fn from(error: TypingStatisticsError) -> Self {
        Self::Storage(error)
    }
}

impl TypingStatisticsStore {
    fn read_upload_state(
        &self,
        lock: &super::StatisticsLock,
    ) -> Result<UploadState, TypingStatisticsError> {
        let file = match crate::file_lock::open_private_file_at(
            &lock.directory,
            std::ffi::OsStr::new(UPLOAD_STATE_FILE),
        ) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(UploadState::default());
            }
            Err(_) => return Err(TypingStatisticsError::InvalidDocument),
        };
        let bytes = crate::bounded_io::read_bounded_file(file, MAX_UPLOAD_STATE_BYTES, || {
            TypingStatisticsError::InvalidDocument
        })?;
        // 坏掉的状态文件按关闭处理：宁可少传，也不在用户不知情时上传。
        Ok(serde_json::from_slice(&bytes).unwrap_or_default())
    }

    fn write_upload_state(
        &self,
        lock: &super::StatisticsLock,
        state: &UploadState,
    ) -> Result<(), TypingStatisticsError> {
        let bytes = serde_json::to_vec(state)?;
        crate::file_lock::write_private_file_at(
            &lock.directory,
            std::ffi::OsStr::new(UPLOAD_STATE_FILE),
            &bytes,
        )?;
        Ok(())
    }

    /// 本机的上传状态。
    pub fn upload_state(&self) -> Result<UploadState, TypingStatisticsError> {
        let lock = self.lock()?;
        self.read_upload_state(&lock)
    }

    /// 打开或关闭上传。打开时第一次生成设备标识；关闭时清掉上次上传的时间，下次打开立刻上传。设备标识保留，重新打开时服务端仍认作同一台设备。
    pub fn set_upload_enabled(&self, enabled: bool) -> Result<UploadState, TypingStatisticsError> {
        let lock = self.lock()?;
        let mut state = self.read_upload_state(&lock)?;
        state.enabled = enabled;
        if enabled && state.device.is_empty() {
            state.device = uuid::Uuid::new_v4().simple().to_string();
        }
        if !enabled {
            state.last_upload_ms = 0;
        }
        self.write_upload_state(&lock, &state)?;
        Ok(state)
    }

    /// 准备一次上传：打开了上传、离上次超过 `UPLOAD_INTERVAL_MS`（`force` 时不看间隔）、最近有可传的日子时交回请求体，否则交回不传的原因（[`UploadOutcome::Disabled`]、[`UploadOutcome::NotDue`] 或 [`UploadOutcome::Nothing`]）。只在读状态和统计时持锁。自己发 HTTP 的宿主（Android、iOS、鸿蒙）用它和 [`Self::record_upload`] 两步走，发送夹在中间；用 Rust 账号会话的宿主用 [`Self::upload_if_due`]。`today` 是宿主的本地日期，`now_ms` 是 Unix 毫秒。
    pub fn prepare_upload(
        &self,
        today: &str,
        platform: &str,
        now_ms: u64,
        force: bool,
    ) -> Result<Result<TypingStatisticsUpload, UploadOutcome>, TypingStatisticsError> {
        let lock = self.lock()?;
        let state = self.read_upload_state(&lock)?;
        if !state.enabled || state.device.is_empty() {
            return Ok(Err(UploadOutcome::Disabled));
        }
        // 墙上时钟回拨时旧的成功时间不能继续阻止补传；成功后会以修正后的时间重新节流。
        if !force
            && now_ms
                .checked_sub(state.last_upload_ms)
                .is_some_and(|elapsed| elapsed < UPLOAD_INTERVAL_MS)
        {
            return Ok(Err(UploadOutcome::NotDue));
        }
        let statistics = self.read_locked(&lock)?;
        Ok(build_upload(&statistics, today, &state.device, platform).ok_or(UploadOutcome::Nothing))
    }

    /// 记下一次成功的上传：`device` 是 [`Self::prepare_upload`] 交回的请求体里的设备标识。发送期间用户关掉了上传（或换了设备标识）时不记，免得关掉后的状态被写回。交回记完以后的状态。
    pub fn record_upload(
        &self,
        device: &str,
        now_ms: u64,
    ) -> Result<UploadState, TypingStatisticsError> {
        let lock = self.lock()?;
        let mut current = self.read_upload_state(&lock)?;
        if current.enabled && !device.is_empty() && current.device == device {
            current.last_upload_ms = now_ms;
            self.write_upload_state(&lock, &current)?;
        }
        Ok(current)
    }

    /// 打开了上传、离上次超过 `UPLOAD_INTERVAL_MS`（`force` 时不看间隔）就构建并交给 `send` 发送，成功后记下时间。发送期间不持有统计文件的锁：网络请求可能要几十秒，输入法在这期间照常记数。`today` 是宿主的本地日期，`now_ms` 是 Unix 毫秒。
    pub fn upload_if_due<E>(
        &self,
        today: &str,
        platform: &str,
        now_ms: u64,
        force: bool,
        send: impl FnOnce(&TypingStatisticsUpload) -> Result<(), E>,
    ) -> Result<UploadOutcome, UploadError<E>> {
        let payload = match self.prepare_upload(today, platform, now_ms, force)? {
            Ok(payload) => payload,
            Err(outcome) => return Ok(outcome),
        };
        send(&payload).map_err(UploadError::Send)?;
        self.record_upload(&payload.device, now_ms)?;
        Ok(UploadOutcome::Uploaded(payload.days.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn statistics() -> TypingStatistics {
        let mut value = TypingStatistics::default();
        for (day, count) in [
            ("2026-08-01", 5),
            ("2026-10-01", 10),
            ("2026-10-10", 20),
            ("2026-10-11", 0),
            ("2026-10-12", 30),
            ("2026-10-13", 40),
            ("2026-10-14", 50),
            ("2026-10-15", 60),
            ("2026-10-16", 70),
            ("2026-10-17", 80),
            ("2026-10-18", 90),
        ] {
            value.days.insert(day.to_owned(), count);
        }
        value
            .daily_active_ms
            .insert("2026-10-17".to_owned(), 61_999);
        let mut hours = vec![0; HOURS];
        hours[22] = 80;
        value.daily_hours.insert("2026-10-17".to_owned(), hours);
        value
            .daily_hours
            .insert("2026-10-16".to_owned(), vec![1, 2]);
        let detail = value
            .daily_details
            .entry("2026-10-17".to_owned())
            .or_default();
        detail.characters.insert("han".to_owned(), 70);
        detail.characters.insert("otherLetter".to_owned(), 10);
        detail.sources.insert("smart_abc".to_owned(), 60);
        detail.sources.insert("voice".to_owned(), 20);
        detail.sources.insert("9bad".to_owned(), 1);
        detail.sources.insert("zero".to_owned(), 0);
        value.selections.ranks = vec![8, 2];
        value.selections.beyond = 1;
        value
    }

    #[test]
    fn the_upload_holds_the_latest_days_with_counts_only() {
        let upload = build_upload(&statistics(), "2026-10-17", "abc", "macos").unwrap();
        let days: Vec<&str> = upload.days.iter().map(|d| d.day.as_str()).collect();
        // 今天以后的不传，零字数的跳过，最多 7 天，从早到晚。
        assert_eq!(
            days,
            [
                "2026-10-10",
                "2026-10-12",
                "2026-10-13",
                "2026-10-14",
                "2026-10-15",
                "2026-10-16",
                "2026-10-17"
            ]
        );
        let today = upload.days.last().unwrap();
        assert_eq!(today.characters, 80);
        assert_eq!(today.active_seconds, 61);
        assert_eq!(today.hours.as_ref().unwrap()[22], 80);
        assert_eq!(today.categories.get("otherLetter"), Some(&10));
        assert_eq!(
            today.sources.keys().map(String::as_str).collect::<Vec<_>>(),
            ["smart_abc", "voice"]
        );
        // 不是 24 个桶的小时数据不传。
        assert_eq!(upload.days[5].hours, None);
        assert_eq!(upload.days[0].active_seconds, 0);
        assert_eq!(
            upload.selections,
            Some(UploadSelections {
                ranks: vec![8, 2],
                beyond: 1
            })
        );
        assert_eq!(upload.platform, "macos");
        let json = serde_json::to_value(&upload).unwrap();
        assert_eq!(json["device"], "abc");
        assert!(json["days"][0].get("hours").is_none());
        assert!(json["days"][0].get("categories").is_none());
        assert_eq!(json["days"][6]["active_seconds"], 61);
    }

    #[test]
    fn old_days_unknown_platforms_and_empty_statistics_are_left_out() {
        // 2026-08-01 在 60 天以外。
        let upload = build_upload(&statistics(), "2026-10-02", "abc", "beos").unwrap();
        assert_eq!(
            upload
                .days
                .iter()
                .map(|d| d.day.as_str())
                .collect::<Vec<_>>(),
            ["2026-10-01"]
        );
        assert_eq!(upload.platform, "");
        assert_eq!(
            build_upload(&statistics(), "2026-07-31", "abc", "linux"),
            None
        );
        assert_eq!(
            build_upload(&TypingStatistics::default(), "2026-10-17", "abc", "linux"),
            None
        );
        assert_eq!(
            build_upload(&statistics(), "not-a-day", "abc", "linux"),
            None
        );
        let mut no_selections = statistics();
        no_selections.selections = Default::default();
        assert_eq!(
            build_upload(&no_selections, "2026-10-17", "abc", "linux")
                .unwrap()
                .selections,
            None
        );
    }

    #[test]
    fn counts_keep_at_most_the_largest_server_keys() {
        let mut source = BTreeMap::new();
        for i in 0..40u64 {
            source.insert(format!("k{i:02}"), i + 1);
        }
        source.insert("bad-key".to_owned(), 1000);
        source.insert(format!("a{}", "b".repeat(32)), 1000);
        source.insert("huge".to_owned(), MAX_DAY_COUNT + 1);
        let kept = counts(&source);
        assert_eq!(kept.len(), MAX_KEYS);
        assert!(kept.contains_key("k39") && !kept.contains_key("k00"));
        assert!(!kept.contains_key("bad-key") && !kept.contains_key("huge"));
    }

    #[test]
    fn upload_operations_wait_for_each_other() {
        let directory = tempfile::tempdir().unwrap();
        let store = TypingStatisticsStore::new(directory.path());
        let first = store.lock_upload_operation();
        let second_store = store.clone();
        let (attempting_tx, attempting_rx) = std::sync::mpsc::channel();
        let acquired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let acquired_by_worker = std::sync::Arc::clone(&acquired);
        let (released_tx, released_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            attempting_tx.send(()).unwrap();
            let _second = second_store.lock_upload_operation();
            acquired_by_worker.store(true, std::sync::atomic::Ordering::SeqCst);
            released_rx.recv().unwrap();
        });

        attempting_rx.recv().unwrap();
        assert!(!acquired.load(std::sync::atomic::Ordering::SeqCst));
        drop(first);
        while !acquired.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::yield_now();
        }
        released_tx.send(()).unwrap();
        worker.join().unwrap();
    }

    #[test]
    fn upload_state_is_opt_in_throttled_and_survives_a_failed_send() {
        let directory = tempfile::tempdir().unwrap();
        let store = TypingStatisticsStore::new(directory.path().join("statistics"));
        store.set_enabled(true).unwrap();
        assert_eq!(store.upload_state().unwrap(), UploadState::default());
        let never = |_: &TypingStatisticsUpload| -> Result<(), ()> { panic!("sent while off") };
        assert_eq!(
            store
                .upload_if_due("2026-10-17", "linux", 1, true, never)
                .unwrap(),
            UploadOutcome::Disabled
        );

        let state = store.set_upload_enabled(true).unwrap();
        assert!(state.enabled && state.device.len() == 32);
        // 没有统计时什么也不传。
        assert_eq!(
            store
                .upload_if_due("2026-10-17", "linux", 1, true, never)
                .unwrap(),
            UploadOutcome::Nothing
        );
        store
            .record(
                "今天",
                super::super::TypingSource::Quanpin,
                "2026-10-17",
                Some(9),
            )
            .unwrap();

        let mut sent = Vec::new();
        let outcome = store
            .upload_if_due(
                "2026-10-17",
                "linux",
                UPLOAD_INTERVAL_MS,
                false,
                |payload| {
                    sent.push(payload.clone());
                    Ok::<(), ()>(())
                },
            )
            .unwrap();
        assert_eq!(outcome, UploadOutcome::Uploaded(1));
        assert_eq!(sent[0].device, state.device);
        assert_eq!(sent[0].days[0].characters, 2);
        assert_eq!(
            store.upload_state().unwrap().last_upload_ms,
            UPLOAD_INTERVAL_MS
        );

        // 十分钟内不再自动上传，`force` 不看间隔。
        assert_eq!(
            store
                .upload_if_due("2026-10-17", "linux", UPLOAD_INTERVAL_MS + 1, false, never)
                .unwrap(),
            UploadOutcome::NotDue
        );
        let failed =
            store.upload_if_due("2026-10-17", "linux", 3 * UPLOAD_INTERVAL_MS, true, |_| {
                Err("offline")
            });
        assert!(matches!(failed, Err(UploadError::Send("offline"))));
        assert_eq!(
            store.upload_state().unwrap().last_upload_ms,
            UPLOAD_INTERVAL_MS
        );

        // 关掉后清掉时间、保留设备标识；重新打开时还是同一台设备。
        let off = store.set_upload_enabled(false).unwrap();
        assert!(!off.enabled && off.last_upload_ms == 0 && off.device == state.device);
        assert_eq!(store.set_upload_enabled(true).unwrap().device, state.device);
        assert_eq!(
            UploadStatus::from(&off),
            UploadStatus {
                enabled: false,
                last_upload_ms: None
            }
        );
    }

    #[test]
    fn automatic_upload_recovers_after_the_wall_clock_moves_backwards() {
        const UPLOADED_AT: u64 = 1_800_000_000_000;
        const CORRECTED_NOW: u64 = UPLOADED_AT - 7 * 24 * 60 * 60 * 1000;
        let directory = tempfile::tempdir().unwrap();
        let store = TypingStatisticsStore::new(directory.path().join("statistics"));
        store.set_enabled(true).unwrap();
        let device = store.set_upload_enabled(true).unwrap().device;
        store
            .record(
                "字",
                super::super::TypingSource::Quanpin,
                "2026-10-17",
                None,
            )
            .unwrap();
        assert_eq!(
            store
                .upload_if_due("2026-10-17", "linux", UPLOADED_AT, false, |_| Ok::<(), ()>(
                    ()
                ))
                .unwrap(),
            UploadOutcome::Uploaded(1)
        );

        // 新建 store 仍读到校时前的成功时间，不能等一周后墙上时钟追上它才补传。
        let store = TypingStatisticsStore::new(store.directory());
        let payload = store
            .prepare_upload("2026-10-17", "linux", CORRECTED_NOW, false)
            .unwrap()
            .expect("时钟回拨后应允许自动补传");
        assert_eq!(payload.device, device);
        assert!(matches!(
            store.upload_if_due("2026-10-17", "linux", CORRECTED_NOW, false, |_| Err(
                "offline"
            )),
            Err(UploadError::Send("offline"))
        ));
        assert_eq!(store.upload_state().unwrap().last_upload_ms, UPLOADED_AT);
        assert_eq!(
            store
                .upload_if_due(
                    "2026-10-17",
                    "linux",
                    CORRECTED_NOW,
                    false,
                    |_| Ok::<(), ()>(())
                )
                .unwrap(),
            UploadOutcome::Uploaded(1)
        );
        assert_eq!(store.upload_state().unwrap().last_upload_ms, CORRECTED_NOW);

        // 成功后从修正后的时间重新节流，区间内和恰好十分钟的边界都保留。
        assert_eq!(
            store
                .prepare_upload(
                    "2026-10-17",
                    "linux",
                    CORRECTED_NOW + UPLOAD_INTERVAL_MS - 1,
                    false
                )
                .unwrap(),
            Err(UploadOutcome::NotDue)
        );
        assert!(store
            .prepare_upload(
                "2026-10-17",
                "linux",
                CORRECTED_NOW + UPLOAD_INTERVAL_MS,
                false
            )
            .unwrap()
            .is_ok());
        store.set_upload_enabled(false).unwrap();
        assert_eq!(
            store
                .prepare_upload("2026-10-17", "linux", CORRECTED_NOW - 1, false)
                .unwrap(),
            Err(UploadOutcome::Disabled)
        );
    }

    #[test]
    fn turning_upload_off_while_sending_is_not_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let store = TypingStatisticsStore::new(directory.path().join("statistics"));
        store.set_enabled(true).unwrap();
        store.set_upload_enabled(true).unwrap();
        store
            .record(
                "字",
                super::super::TypingSource::Quanpin,
                "2026-10-17",
                None,
            )
            .unwrap();
        let outcome = store
            .upload_if_due("2026-10-17", "linux", 5, true, |_| {
                store.set_upload_enabled(false).map(|_| ())
            })
            .unwrap();
        assert_eq!(outcome, UploadOutcome::Uploaded(1));
        let state = store.upload_state().unwrap();
        assert!(!state.enabled && state.last_upload_ms == 0);
    }

    #[test]
    fn a_damaged_state_file_reads_as_off() {
        let directory = tempfile::tempdir().unwrap();
        let store = TypingStatisticsStore::new(directory.path());
        store.set_upload_enabled(true).unwrap();
        std::fs::write(directory.path().join(UPLOAD_STATE_FILE), b"{not json").unwrap();
        assert!(!store.upload_state().unwrap().enabled);
    }

    #[test]
    fn the_two_step_upload_records_only_for_the_device_it_prepared() {
        let directory = tempfile::tempdir().unwrap();
        let store = TypingStatisticsStore::new(directory.path().join("statistics"));
        store.set_enabled(true).unwrap();
        assert_eq!(
            store
                .prepare_upload("2026-10-17", "android", 1, false)
                .unwrap(),
            Err(UploadOutcome::Disabled)
        );
        let state = store.set_upload_enabled(true).unwrap();
        assert_eq!(
            store
                .prepare_upload("2026-10-17", "android", 1, true)
                .unwrap(),
            Err(UploadOutcome::Nothing)
        );
        store
            .record(
                "字",
                super::super::TypingSource::Quanpin,
                "2026-10-17",
                None,
            )
            .unwrap();
        let payload = store
            .prepare_upload("2026-10-17", "android", UPLOAD_INTERVAL_MS, false)
            .unwrap()
            .unwrap();
        assert_eq!(
            (payload.device.as_str(), payload.platform.as_str()),
            (state.device.as_str(), "android")
        );
        // 准备不记时间：没发成功时下一次照样要传。
        assert_eq!(store.upload_state().unwrap().last_upload_ms, 0);
        // 别的设备标识和空标识不记。
        assert_eq!(store.record_upload("other", 7).unwrap().last_upload_ms, 0);
        assert_eq!(store.record_upload("", 7).unwrap().last_upload_ms, 0);
        assert_eq!(
            store
                .record_upload(&payload.device, 7)
                .unwrap()
                .last_upload_ms,
            7
        );
        assert_eq!(
            store
                .prepare_upload("2026-10-17", "android", 8, false)
                .unwrap(),
            Err(UploadOutcome::NotDue)
        );
        // 关掉以后再记成功也不写回。
        store.set_upload_enabled(false).unwrap();
        let after = store.record_upload(&payload.device, 9).unwrap();
        assert!(!after.enabled && after.last_upload_ms == 0);
    }
}
