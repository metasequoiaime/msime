//! 自动同步的持久化调度与逐请求预扣；与同步事务共用同一把进程间锁。

use super::*;
use sha2::{Digest, Sha256};
use std::sync::Mutex;

mod policy;
pub use policy::Trigger as SettingsSyncTrigger;

const FILE: &str = "settings-sync-schedule";
const LIMIT: u64 = 16 * 1024;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Schedule {
    requests: Vec<u64>,
    automatic_requests: Vec<u64>,
    observed: Option<String>,
    dirty_since: Option<u64>,
    last_change: u64,
    last_pull: Option<u64>,
    retry_at: u64,
    failures: u32,
    last_success: Option<u64>,
    last_error: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u64,
    relay: String,
    space: String,
    schedule: Schedule,
}

#[derive(Debug, Serialize)]
pub struct SettingsSyncScheduleStatus {
    pub pending_changes: bool,
    pub last_success_ms: Option<u64>,
    pub last_error: Option<String>,
    pub budget_wait_until: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct SettingsSyncScheduledRun {
    pub network: bool,
    /// 未到期时没有执行结果；已执行的错误只保留稳定错误码。
    pub report: Option<Result<SettingsSyncReport, String>>,
    pub next_check_ms: u64,
    pub status: SettingsSyncScheduleStatus,
}

impl Schedule {
    fn view(&self) -> policy::View<'_> {
        policy::View {
            requests: &self.requests,
            automatic_requests: &self.automatic_requests,
            dirty_since: self.dirty_since,
            last_change: self.last_change,
            last_pull: self.last_pull,
            retry_at: self.retry_at,
        }
    }

    fn reconcile(&mut self, now: u64) {
        for requests in [&mut self.requests, &mut self.automatic_requests] {
            requests.iter_mut().for_each(|at| *at = (*at).min(now));
            requests.retain(|&at| now - at < policy::WINDOW_MS);
        }
        self.dirty_since = self.dirty_since.map(|at| at.min(now));
        self.last_change = self.last_change.min(now);
        self.last_pull = self.last_pull.map(|at| at.min(now));
        self.retry_at = self.retry_at.min(now.saturating_add(policy::ATTENTION_MS));
    }

    fn observe(&mut self, digest: String, now: u64) {
        if self.observed.as_ref() != Some(&digest) {
            self.observed = Some(digest);
            self.dirty_since.get_or_insert(now);
            self.last_change = now;
        }
    }

    fn status(&self, now: u64) -> SettingsSyncScheduleStatus {
        SettingsSyncScheduleStatus {
            pending_changes: self.dirty_since.is_some(),
            last_success_ms: self.last_success,
            last_error: self.last_error.clone(),
            budget_wait_until: self
                .view()
                .decide(SettingsSyncTrigger::Periodic, now)
                .budget_wait_until,
        }
    }
}

fn fingerprint(document: &SettingsSyncDocument) -> Result<String, SettingsWebDavError> {
    Ok(hex::encode(Sha256::digest(document.to_json()?)))
}

impl SettingsWebDav {
    fn load_schedule(&self) -> Result<Schedule, SettingsWebDavError> {
        let Some(bytes) = self
            .storage()
            .read_private(FILE, LIMIT)
            .map_err(|_| SettingsWebDavError::Storage)?
        else {
            return Ok(Schedule::default());
        };
        let stored: Stored =
            serde_json::from_slice(&bytes).map_err(|_| SettingsWebDavError::Storage)?;
        if stored.version != 1
            || stored.schedule.requests.len() > policy::TOTAL_REQUESTS
            || stored.schedule.automatic_requests.len() > policy::AUTOMATIC_REQUESTS
            || stored.schedule.observed.as_ref().is_some_and(|digest| {
                digest.len() != 64
                    || !digest
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
            || stored.schedule.last_error.as_ref().is_some_and(|code| {
                code.len() > 128 || !code.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
            })
        {
            return Err(SettingsWebDavError::Storage);
        }
        let schedule = stored.schedule;
        if stored.relay != self.config.url() || stored.space != self.config.space() {
            // 换网盘重新建立观察基线，但不能通过反复改配置重领本安装的请求额度。
            return Ok(Schedule {
                requests: schedule.requests,
                automatic_requests: schedule.automatic_requests,
                ..Schedule::default()
            });
        }
        Ok(schedule)
    }

    fn save_schedule(&self, schedule: &Schedule) -> Result<(), SettingsWebDavError> {
        save(&self.storage(), &self.config, schedule)
    }

    /// 离线查询不会建目录、登记设备或发送请求。
    pub fn schedule_status(&self) -> Result<SettingsSyncScheduleStatus, SettingsWebDavError> {
        let mut schedule = self.load_schedule()?;
        let now = self.clock.now_ms();
        schedule.reconcile(now);
        Ok(schedule.status(now))
    }

    /// 宿主从工作线程报告保存、前台或定时信号；本机专属字段不触发上传。
    pub fn run_scheduled(
        &self,
        preferences: &PreferencesStore,
        trigger: SettingsSyncTrigger,
    ) -> Result<SettingsSyncScheduledRun, SettingsWebDavError> {
        let _lock = self.lock()?;
        let now = self.clock.now_ms();
        let mut schedule = self.load_schedule()?;
        schedule.reconcile(now);
        let snapshot = preferences.load()?;
        let document = SettingsSyncDocument::from_preferences(&snapshot.preferences)?;
        schedule.observe(fingerprint(&document)?, now);
        let decision = schedule.view().decide(trigger, now);
        self.save_schedule(&schedule)?;
        if !decision.network {
            return Ok(SettingsSyncScheduledRun {
                network: false,
                report: None,
                next_check_ms: decision.next_check_ms,
                status: schedule.status(now),
            });
        }
        let shared = Arc::new(Mutex::new(schedule));
        let metered = MeteredHttp {
            inner: self.http.clone(),
            schedule: shared.clone(),
            config: self.config.clone(),
            directory: self.directory.clone(),
            clock: self.clock.clone(),
            automatic: trigger != SettingsSyncTrigger::Manual,
        };
        let service = SettingsWebDav {
            config: self.config.clone(),
            directory: self.directory.clone(),
            device_name: self.device_name.clone(),
            http: Arc::new(metered),
            create_identity: self.create_identity,
            clock: self.clock.clone(),
        };
        let result = service.run_locked(preferences);
        let finished = self.clock.now_ms();
        let mut schedule = shared.lock().map_err(|_| SettingsWebDavError::Storage)?;
        schedule.reconcile(finished);
        match &result {
            Ok(report) if report.conflicts.is_empty() => {
                schedule.last_pull = Some(finished);
                schedule.last_success = Some(finished);
                schedule.retry_at = 0;
                schedule.failures = 0;
                schedule.last_error = None;
                // 同步确认的基线与此刻本机投影一致才清掉待发送标记，网络期间的新编辑不能丢。
                let current =
                    SettingsSyncDocument::from_preferences(&preferences.load()?.preferences)?;
                let acknowledged = service
                    .status()?
                    .device_id
                    .map(|device| service.load_state(&device))
                    .transpose()?
                    .flatten();
                schedule.observed = Some(fingerprint(&current)?);
                if acknowledged.is_some_and(|state| state.base == current) {
                    schedule.dirty_since = None;
                } else {
                    schedule.dirty_since.get_or_insert(finished);
                    schedule.last_change = finished;
                }
            }
            _ => {
                let code = result
                    .as_ref()
                    .err()
                    .map_or("settings_conflict", |error| error.code());
                schedule.last_error = Some(code.to_owned());
                let delay = match result.as_ref().err() {
                    Some(SettingsWebDavError::Relay(RelayError::RateLimited)) => policy::WINDOW_MS,
                    Some(SettingsWebDavError::Storage | SettingsWebDavError::Conflict)
                    | Some(SettingsWebDavError::Relay(
                        RelayError::Unavailable | RelayError::Storage,
                    )) => {
                        let delay = policy::failure_delay(schedule.failures);
                        schedule.failures = schedule.failures.saturating_add(1);
                        delay
                    }
                    _ => policy::ATTENTION_MS,
                };
                schedule.retry_at = finished.saturating_add(delay);
            }
        }
        self.save_schedule(&schedule)?;
        let next = schedule
            .view()
            .decide(SettingsSyncTrigger::Periodic, finished);
        Ok(SettingsSyncScheduledRun {
            network: true,
            report: Some(result.map_err(|error| error.code().to_owned())),
            next_check_ms: next.next_check_ms,
            status: schedule.status(finished),
        })
    }
}

fn save(
    storage: &FileAccountSessionStorage,
    config: &RelayConfig,
    schedule: &Schedule,
) -> Result<(), SettingsWebDavError> {
    let bytes = serde_json::to_vec(&Stored {
        version: 1,
        relay: config.url().to_owned(),
        space: config.space().to_owned(),
        schedule: schedule.clone(),
    })
    .map_err(|_| SettingsWebDavError::Storage)?;
    if bytes.len() as u64 > LIMIT {
        return Err(SettingsWebDavError::Storage);
    }
    storage
        .write_private(FILE, &bytes)
        .map_err(|_| SettingsWebDavError::Storage)
}

struct MeteredHttp {
    inner: Arc<dyn DavHttp>,
    schedule: Arc<Mutex<Schedule>>,
    config: RelayConfig,
    directory: PathBuf,
    clock: Arc<dyn Environment>,
    automatic: bool,
}

impl DavHttp for MeteredHttp {
    fn send(&self, request: &DavRequest<'_>) -> Result<DavResponse, RelayError> {
        {
            let mut schedule = self.schedule.lock().map_err(|_| RelayError::Storage)?;
            let now = self.clock.now_ms();
            schedule.reconcile(now);
            let trigger = if self.automatic {
                SettingsSyncTrigger::Periodic
            } else {
                SettingsSyncTrigger::Manual
            };
            if schedule.view().decide(trigger, now).remaining == 0 {
                return Err(RelayError::RateLimited);
            }
            schedule.requests.push(now);
            if self.automatic {
                schedule.automatic_requests.push(now);
            }
            let storage =
                FileAccountSessionStorage::new(&self.directory, AccountSessionFileLayout::Native);
            // 先私有落盘再联网；响应丢失、失败重试和进程退出都不能把已经发出的请求退回额度。
            save(&storage, &self.config, &schedule).map_err(|_| RelayError::Storage)?;
        }
        self.inner.send(request)
    }
}
