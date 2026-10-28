//! 什么时候联网、联网时最多发几个请求。坚果云免费账号每 30 分钟 600 个请求，与这个账号授权的所有第三方应用（包括水杉的云剪贴板）共用，所以词库同步给自己定了额度：每台设备每 30 分钟自动同步最多 [`AUTOMATIC_REQUESTS`] 个请求，加上用户点「立即同步」最多 [`MANUAL_REQUESTS`] 个。数字的由来见 `docs/dictionary-webdav-sync.md` 的「请求量」。
//!
//! 宿主不必自己算：每次调用都告诉它下一次什么时候再来（[`Outcome::next_check_ms`]），并且只在该联网时联网。输入线程从不等网络：宿主在工作线程上调用。

use super::sync::{run, KeyRing, Options, Parts, RemoteStore, Report, SyncError, SyncState};
use serde::{Deserialize, Serialize};

/// 额度按这么长的滚动窗口算。
pub const WINDOW_MS: u64 = 30 * 60 * 1000;
/// 自动同步（改词之后、打开设置、定时）在一个窗口里最多的请求数。
pub const AUTOMATIC_REQUESTS: usize = 40;
/// 加上用户手动「立即同步」，一个窗口里最多的请求数。
pub const MANUAL_REQUESTS: usize = 60;
/// 自动同步剩下的额度不到这么多就不开始：一次最少要列一次目录、写一个事件（两个请求）。
const MIN_RUN_REQUESTS: usize = 3;
/// 最后一次改词之后再等这么久才发出去，连着改几个词只发一次。
pub const PUSH_DEBOUNCE_MS: u64 = 60 * 1000;
/// 两次自动发送之间至少隔这么久。
pub const MIN_PUSH_INTERVAL_MS: u64 = 5 * 60 * 1000;
/// 一直在改词时防抖等不到头：第一个没发出去的改动最多等这么久。
pub const MAX_PUSH_DELAY_MS: u64 = 10 * 60 * 1000;
/// 宿主活着时定时拉取的间隔。
pub const PULL_INTERVAL_MS: u64 = 30 * 60 * 1000;
/// 打开设置时，上一次拉取比这更早才再拉。
pub const OPEN_PULL_MIN_MS: u64 = 60 * 1000;
/// 连不上时依次等这么久再自动重试，之后一直是最后一个。
const FAILURE_BACKOFF_MS: [u64; 6] = [60_000, 120_000, 300_000, 600_000, 1_800_000, 3_600_000];
/// 被限流或额度用完时等这么久。
pub const RATE_LIMIT_BACKOFF_MS: u64 = WINDOW_MS;
/// 要用户处理的错误（凭据不对、日志被回退、格式不认识、日志坏了）之后自动同步停这么久，手动同步照常。
const ATTENTION_BACKOFF_MS: u64 = 60 * 60 * 1000;
/// Engine 忙时依次等这么久再写本机词库（不联网），之后一直是最后一个。
pub const BUSY_RETRY_MS: [u64; 4] = [5_000, 30_000, 120_000, 600_000];

/// 宿主为什么调用。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// 用户刚改了词（设置页、导入、MCP、候选栏删除之后）：马上记录，通常不联网、按防抖稍后发送。例外是发送已经到期：本机改动从第一处没发出去的算起已满 [`MAX_PUSH_DELAY_MS`]（一直在改、防抖总被推迟时的上限），并且离上一次发送也满了 [`MIN_PUSH_INTERVAL_MS`]，这一次就联网拉取并发送（仍受退避和自动额度限制）。
    LocalChange,
    /// 打开了设置应用或词库页。
    Opened,
    /// 宿主的定时器到点了（[`Outcome::next_check_ms`]）。
    Periodic,
    /// 用户点了「立即同步」。
    Manual,
}

/// 调度要记住的，随同步状态一起写盘。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    /// 这个窗口里每个请求的时间（Unix 毫秒），最多 [`MANUAL_REQUESTS`] 个。
    #[serde(default)]
    requests: Vec<u64>,
    #[serde(default)]
    last_change_ms: u64,
    /// 上一次发送之后第一个改动的时间；0 表示没有。
    #[serde(default)]
    dirty_since_ms: u64,
    #[serde(default)]
    last_push_ms: u64,
    #[serde(default)]
    last_pull_ms: u64,
    #[serde(default)]
    failures: u32,
    /// 自动同步在这之前不联网。
    #[serde(default)]
    retry_at_ms: u64,
    #[serde(default)]
    busy_retries: u32,
    #[serde(default)]
    apply_retry_at_ms: u64,
    /// 最后一次联网同步成功的时间（Unix 毫秒），0 表示还没有过。
    #[serde(default)]
    last_success_ms: u64,
    /// 最后一次同步失败的错误码（[`SyncError::code`]）；之后成功一次就清掉。
    #[serde(default)]
    last_error: Option<String>,
}

impl Schedule {
    pub fn last_success_ms(&self) -> u64 {
        self.last_success_ms
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    fn trim(&mut self, now_ms: u64) {
        self.requests
            .retain(|&at| at + WINDOW_MS > now_ms && at <= now_ms);
    }

    /// 这个窗口里还能发几个请求。
    pub fn remaining(&self, now_ms: u64, manual: bool) -> usize {
        let used = self
            .requests
            .iter()
            .filter(|&&at| at + WINDOW_MS > now_ms && at <= now_ms)
            .count();
        let limit = if manual {
            MANUAL_REQUESTS
        } else {
            AUTOMATIC_REQUESTS
        };
        limit.saturating_sub(used)
    }

    /// 有东西要发时，什么时候发：最后一次改词之后过了防抖（一直在改时最多等 [`MAX_PUSH_DELAY_MS`]），并且离上一次发送够久。
    fn push_due(&self, outbox: bool) -> Option<u64> {
        outbox.then(|| {
            let settled = self.last_change_ms + PUSH_DEBOUNCE_MS;
            let capped = if self.dirty_since_ms > 0 {
                settled.min(self.dirty_since_ms + MAX_PUSH_DELAY_MS)
            } else {
                settled
            };
            capped.max(self.last_push_ms + MIN_PUSH_INTERVAL_MS)
        })
    }

    /// 这一次要不要联网、要不要发送。
    fn plan(&self, trigger: Trigger, now_ms: u64, outbox: bool) -> (bool, bool) {
        if trigger == Trigger::Manual {
            return (self.remaining(now_ms, true) > 0, true);
        }
        if now_ms < self.retry_at_ms || self.remaining(now_ms, false) < MIN_RUN_REQUESTS {
            return (false, false);
        }
        let push = self.push_due(outbox).is_some_and(|due| due <= now_ms);
        let pull = match trigger {
            Trigger::Opened => now_ms >= self.last_pull_ms + OPEN_PULL_MIN_MS,
            Trigger::Periodic => now_ms >= self.last_pull_ms + PULL_INTERVAL_MS,
            Trigger::LocalChange | Trigger::Manual => false,
        };
        (push || pull, push)
    }

    /// 滚动窗口恢复到一轮自动同步所需额度的最早时间，手动请求也占同一个窗口。
    fn automatic_ready_at(&self, now_ms: u64) -> u64 {
        let mut active: Vec<u64> = self
            .requests
            .iter()
            .copied()
            .filter(|&at| at <= now_ms && at + WINDOW_MS > now_ms)
            .collect();
        let allowed = AUTOMATIC_REQUESTS - MIN_RUN_REQUESTS;
        if active.len() <= allowed {
            return now_ms;
        }
        active.sort_unstable();
        // 不能只等最早的一条：必须过期到剩余请求数不超过 allowed 才能开始。
        active[active.len() - allowed - 1] + WINDOW_MS
    }

    /// 宿主下一次什么时候再调用（距现在的毫秒数）。
    fn next_check(&self, now_ms: u64, outbox: bool, deferred: bool) -> u64 {
        let mut due = (self.last_pull_ms + PULL_INTERVAL_MS).max(self.retry_at_ms);
        if let Some(push) = self.push_due(outbox) {
            due = due.min(push.max(self.retry_at_ms));
        }
        due = due.max(self.automatic_ready_at(now_ms));
        if deferred {
            due = due.min(self.apply_retry_at_ms);
        }
        due.saturating_sub(now_ms).max(1_000)
    }

    fn finished(&mut self, now_ms: u64, network: bool, result: &Result<Report, SyncError>) {
        match result {
            Ok(report) => {
                if network {
                    self.failures = 0;
                    self.retry_at_ms = 0;
                    self.last_pull_ms = now_ms;
                    self.last_success_ms = now_ms;
                    self.last_error = None;
                    if report.published_events > 0 {
                        self.last_push_ms = now_ms;
                        self.dirty_since_ms = 0;
                    }
                }
                if report.deferred {
                    let wait =
                        BUSY_RETRY_MS[(self.busy_retries as usize).min(BUSY_RETRY_MS.len() - 1)];
                    self.busy_retries = self.busy_retries.saturating_add(1);
                    self.apply_retry_at_ms = now_ms + wait;
                } else {
                    self.busy_retries = 0;
                }
            }
            Err(error) => {
                self.last_error = Some(error.code().to_owned());
                self.fail(now_ms, *error);
            }
        }
    }

    fn fail(&mut self, now_ms: u64, error: SyncError) {
        match error {
            SyncError::RateLimited => self.retry_at_ms = now_ms + RATE_LIMIT_BACKOFF_MS,
            SyncError::Unavailable | SyncError::TooLarge | SyncError::Storage => {
                let wait =
                    FAILURE_BACKOFF_MS[(self.failures as usize).min(FAILURE_BACKOFF_MS.len() - 1)];
                self.failures = self.failures.saturating_add(1);
                self.retry_at_ms = now_ms + wait;
            }
            _ => self.retry_at_ms = now_ms + ATTENTION_BACKOFF_MS,
        }
    }
}

/// 把请求计进额度的网盘：额度用完时不再发，答 [`SyncError::RateLimited`]。
struct Metered<'a> {
    inner: &'a mut dyn RemoteStore,
    requests: Vec<u64>,
    limit: usize,
    now_ms: u64,
}

impl Metered<'_> {
    fn spend(&mut self) -> Result<(), SyncError> {
        if self.requests.len() >= self.limit {
            return Err(SyncError::RateLimited);
        }
        self.requests.push(self.now_ms);
        Ok(())
    }
}

impl RemoteStore for Metered<'_> {
    fn list(&mut self, dir: &str) -> Result<Option<Vec<String>>, SyncError> {
        self.spend()?;
        self.inner.list(dir)
    }

    fn get(&mut self, path: &str, limit: u64) -> Result<Option<Vec<u8>>, SyncError> {
        self.spend()?;
        self.inner.get(path, limit)
    }

    fn create(&mut self, path: &str, bytes: &[u8]) -> Result<bool, SyncError> {
        // 独占创建在网盘上是 PUT 加 MOVE 两个请求。
        self.spend()?;
        self.spend()?;
        self.inner.create(path, bytes)
    }

    fn mkcol(&mut self, dir: &str) -> Result<(), SyncError> {
        self.spend()?;
        self.inner.mkcol(dir)
    }

    fn delete(&mut self, path: &str) -> Result<(), SyncError> {
        self.spend()?;
        self.inner.delete(path)
    }
}

/// 一次调用的结果。
#[derive(Debug)]
pub struct Outcome {
    /// 这一次联网了没有。
    pub network: bool,
    pub result: Result<Report, SyncError>,
    /// 宿主下一次什么时候再调用（距现在的毫秒数）。
    pub next_check_ms: u64,
}

/// 这一次要不要联网、要不要发送。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub network: bool,
    pub push: bool,
}

/// 决定这一次做什么：记下本机刚改过词，按防抖、间隔、退避和额度得出要不要联网。宿主在联网之前还要先做别的请求时（词库的密钥串），先调它，那些请求用 [`record_requests`] 计进额度，再调 [`execute`]。
pub fn plan(state: &mut SyncState, trigger: Trigger, now_ms: u64) -> Plan {
    state.schedule.trim(now_ms);
    if trigger == Trigger::LocalChange {
        state.schedule.last_change_ms = now_ms;
        if state.schedule.dirty_since_ms == 0 {
            state.schedule.dirty_since_ms = now_ms;
        }
    }
    let outbox =
        state.outbox_len() > 0 || trigger == Trigger::LocalChange || state.wants_event(now_ms);
    let (network, push) = state.schedule.plan(trigger, now_ms, outbox);
    Plan { network, push }
}

/// 宿主在 [`execute`] 之外发出的请求（例如词库密钥串的读写）也计进这个窗口的额度。
pub fn record_requests(state: &mut SyncState, count: usize, now_ms: u64) {
    state
        .schedule
        .requests
        .extend(std::iter::repeat_n(now_ms, count));
}

/// 在 [`execute`] 之前就失败了（例如读写密钥串时网盘拒绝了凭据）：照同样的规则退避，记下错误码。
pub fn record_failure(state: &mut SyncState, error: SyncError, now_ms: u64) {
    state.schedule.finished(now_ms, true, &Err(error));
}

/// 按 [`plan`] 的决定跑一次同步，联网时按剩下的额度计数，最后写盘。`options` 里只有用户的选择（确认或撤回被挡下的删除）会被采用，联网与否由 `plan` 决定。
pub fn execute(
    state: &mut SyncState,
    parts: &mut Parts<'_>,
    ring: &KeyRing,
    trigger: Trigger,
    plan: Plan,
    options: Options,
) -> Outcome {
    let now_ms = parts.env.now_ms();
    let Plan { network, push } = plan;
    let manual = trigger == Trigger::Manual;
    let limit = state.schedule.remaining(now_ms, manual);
    let options = Options {
        offline: !network,
        skip_push: !push && !manual,
        ..options
    };
    // 核心会在本轮中途保存整个状态，历史窗口必须始终留在状态里。
    let request_start = parts.remote.request_count();
    let result = {
        let mut metered = Metered {
            inner: &mut *parts.remote,
            requests: Vec::new(),
            limit,
            now_ms,
        };
        let mut metered_parts = Parts {
            remote: &mut metered,
            local: &mut *parts.local,
            env: &mut *parts.env,
            sink: &mut *parts.sink,
        };
        let result = run(state, &mut metered_parts, ring, options);
        if let Ok(report) = &result {
            metered.inner.finish(report, now_ms);
        }
        if let (Some(start), Some(end)) = (request_start, metered.inner.request_count()) {
            state
                .schedule
                .requests
                .extend(std::iter::repeat_n(now_ms, end.saturating_sub(start)));
        } else {
            state.schedule.requests.extend(metered.requests);
        }
        result
    };
    // 手动同步时额度已经用完：照常记录了本机改动，但告诉用户为什么没有同步。
    let result = match result {
        Ok(_) if manual && !network => Err(SyncError::RateLimited),
        result => result,
    };
    state.schedule.finished(now_ms, network, &result);
    let deferred = result.as_ref().is_ok_and(|report| report.deferred);
    let next_check_ms = state
        .schedule
        .next_check(now_ms, state.outbox_len() > 0, deferred);
    // 调度的记录写不进去不影响这一次的结果，下一次只是少算几个请求或者早一点重试。
    let _ = parts.sink.save(state);
    Outcome {
        network,
        result,
        next_check_ms,
    }
}

/// [`plan`] 加 [`execute`]：没有别的请求要先做的宿主用它。
pub fn drive(
    state: &mut SyncState,
    parts: &mut Parts<'_>,
    ring: &KeyRing,
    trigger: Trigger,
    options: Options,
) -> Outcome {
    let now_ms = parts.env.now_ms();
    let decided = plan(state, trigger, now_ms);
    execute(state, parts, ring, trigger, decided, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_790_000_000_000;

    #[test]
    fn local_changes_are_debounced_and_pushes_are_spaced() {
        let mut schedule = Schedule {
            last_change_ms: NOW,
            ..Default::default()
        };
        assert_eq!(
            schedule.plan(Trigger::LocalChange, NOW, true),
            (false, false)
        );
        assert_eq!(
            schedule.plan(Trigger::Periodic, NOW + PUSH_DEBOUNCE_MS, true),
            (true, true)
        );
        schedule.last_push_ms = NOW + PUSH_DEBOUNCE_MS;
        schedule.last_pull_ms = NOW + PUSH_DEBOUNCE_MS;
        schedule.last_change_ms = NOW + PUSH_DEBOUNCE_MS + 1;
        // 刚发过：五分钟之内不再自动发，下一次检查约在五分钟后。
        let soon = NOW + 2 * PUSH_DEBOUNCE_MS + 1;
        assert_eq!(schedule.plan(Trigger::Periodic, soon, true), (false, false));
        let wait = schedule.next_check(soon, true, false);
        assert_eq!(soon + wait, NOW + PUSH_DEBOUNCE_MS + MIN_PUSH_INTERVAL_MS);
        // 手动同步不受防抖和间隔限制。
        assert_eq!(schedule.plan(Trigger::Manual, soon, true), (true, true));
    }

    #[test]
    fn a_local_change_sends_at_once_once_unsent_edits_are_ten_minutes_old() {
        // `plan` 已经把这次改词记成 `last_change_ms`，所以一分钟的防抖总是还没到；到期的只能是十分钟的上限。
        let at = NOW + MAX_PUSH_DELAY_MS;
        let mut schedule = Schedule {
            dirty_since_ms: NOW,
            last_change_ms: at - 1,
            ..Default::default()
        };
        assert_eq!(
            schedule.plan(Trigger::LocalChange, at - 1, true),
            (false, false)
        );
        schedule.last_change_ms = at;
        assert_eq!(schedule.plan(Trigger::LocalChange, at, true), (true, true));
        // 五分钟之内发过就仍然只记录。
        schedule.last_push_ms = at - MIN_PUSH_INTERVAL_MS + 1;
        assert_eq!(
            schedule.plan(Trigger::LocalChange, at, true),
            (false, false)
        );
    }

    #[test]
    fn pulls_happen_on_open_and_periodically_but_not_more_often() {
        let schedule = Schedule {
            last_pull_ms: NOW,
            ..Default::default()
        };
        assert_eq!(
            schedule.plan(Trigger::Opened, NOW + 30_000, false),
            (false, false)
        );
        assert_eq!(
            schedule.plan(Trigger::Opened, NOW + OPEN_PULL_MIN_MS, false),
            (true, false)
        );
        assert_eq!(
            schedule.plan(Trigger::Periodic, NOW + OPEN_PULL_MIN_MS, false),
            (false, false)
        );
        assert_eq!(
            schedule.plan(Trigger::Periodic, NOW + PULL_INTERVAL_MS, false),
            (true, false)
        );
        assert_eq!(
            NOW + schedule.next_check(NOW, false, false),
            NOW + PULL_INTERVAL_MS
        );
    }

    #[test]
    fn the_request_budget_is_a_rolling_window_with_room_for_manual_syncs() {
        let mut schedule = Schedule {
            requests: vec![NOW; AUTOMATIC_REQUESTS],
            ..Default::default()
        };
        assert_eq!(schedule.remaining(NOW, false), 0);
        assert_eq!(
            schedule.remaining(NOW, true),
            MANUAL_REQUESTS - AUTOMATIC_REQUESTS
        );
        assert_eq!(
            schedule.plan(Trigger::Opened, NOW + OPEN_PULL_MIN_MS, false),
            (false, false)
        );
        assert_eq!(schedule.plan(Trigger::Manual, NOW, false), (true, true));
        schedule.trim(NOW + WINDOW_MS);
        assert_eq!(
            schedule.remaining(NOW + WINDOW_MS, false),
            AUTOMATIC_REQUESTS
        );
    }

    #[test]
    fn quota_wake_waits_for_enough_requests_in_an_unordered_window() {
        let now = NOW + 120_000;
        for used in [38, 39, 40, MANUAL_REQUESTS] {
            let mut schedule = Schedule {
                requests: (0..used)
                    .rev()
                    .map(|index| NOW + index as u64 * 1_000)
                    .collect(),
                last_pull_ms: now,
                last_change_ms: NOW,
                ..Default::default()
            };
            // 已过期的请求不参与恢复时间，即使列表不是按时间排序。
            schedule.requests.push(now - WINDOW_MS);
            let released = used - (AUTOMATIC_REQUESTS - MIN_RUN_REQUESTS);
            let ready = NOW + (released - 1) as u64 * 1_000 + WINDOW_MS;
            assert_eq!(schedule.plan(Trigger::Periodic, now, true), (false, false));
            assert_eq!(schedule.next_check(now, true, false), ready - now);
            assert_eq!(
                schedule.plan(Trigger::Periodic, ready - 1, true),
                (false, false)
            );
            assert_eq!(schedule.plan(Trigger::Periodic, ready, true), (true, true));
        }
    }

    #[test]
    fn quota_wake_preserves_manual_sync_local_retries_and_error_backoff() {
        let mut schedule = Schedule {
            requests: vec![NOW; AUTOMATIC_REQUESTS],
            last_pull_ms: NOW,
            last_change_ms: NOW - PUSH_DEBOUNCE_MS,
            apply_retry_at_ms: NOW + BUSY_RETRY_MS[0],
            ..Default::default()
        };
        assert_eq!(schedule.plan(Trigger::Manual, NOW, true), (true, true));
        assert_eq!(schedule.next_check(NOW, true, true), BUSY_RETRY_MS[0]);
        // 恢复了足够额度仍要遵守原有网络错误退避。
        schedule.retry_at_ms = NOW + WINDOW_MS + 60_000;
        assert_eq!(schedule.next_check(NOW, true, false), WINDOW_MS + 60_000);
        // 还有三个额度时不推迟原本到期的自动发送。
        schedule
            .requests
            .truncate(AUTOMATIC_REQUESTS - MIN_RUN_REQUESTS);
        schedule.retry_at_ms = 0;
        assert_eq!(schedule.plan(Trigger::Periodic, NOW, true), (true, true));
        assert_eq!(schedule.next_check(NOW, true, false), 1_000);
    }

    #[test]
    fn failures_back_off_and_attention_errors_pause_automatic_sync() {
        let mut schedule = Schedule::default();
        schedule.finished(NOW, true, &Err(SyncError::Unavailable));
        assert_eq!(schedule.retry_at_ms, NOW + FAILURE_BACKOFF_MS[0]);
        schedule.finished(NOW, true, &Err(SyncError::Unavailable));
        assert_eq!(schedule.retry_at_ms, NOW + FAILURE_BACKOFF_MS[1]);
        assert_eq!(
            schedule.plan(Trigger::Periodic, NOW + FAILURE_BACKOFF_MS[1] - 1, false),
            (false, false)
        );
        assert_eq!(
            schedule.plan(Trigger::Periodic, NOW + FAILURE_BACKOFF_MS[1], false),
            (true, false)
        );
        schedule.finished(NOW, true, &Err(SyncError::RateLimited));
        assert_eq!(schedule.retry_at_ms, NOW + RATE_LIMIT_BACKOFF_MS);
        schedule.finished(NOW, true, &Err(SyncError::Rollback));
        assert_eq!(schedule.retry_at_ms, NOW + ATTENTION_BACKOFF_MS);
        assert_eq!(
            schedule.plan(Trigger::Opened, NOW + 1, false),
            (false, false)
        );
        assert_eq!(schedule.plan(Trigger::Manual, NOW + 1, false), (true, true));
        schedule.finished(NOW + 2, true, &Ok(Report::default()));
        assert_eq!(schedule.retry_at_ms, 0);
        assert_eq!(schedule.failures, 0);
    }

    #[test]
    fn a_busy_engine_is_retried_locally_on_a_growing_interval() {
        let mut schedule = Schedule {
            last_pull_ms: NOW,
            ..Default::default()
        };
        let deferred = Ok(Report {
            deferred: true,
            ..Default::default()
        });
        for wait in BUSY_RETRY_MS.iter().chain([BUSY_RETRY_MS[3]].iter()) {
            schedule.finished(NOW, false, &deferred);
            assert_eq!(schedule.apply_retry_at_ms, NOW + wait);
            assert_eq!(schedule.next_check(NOW, false, true), *wait);
        }
        schedule.finished(NOW, false, &Ok(Report::default()));
        assert_eq!(schedule.busy_retries, 0);
    }

    #[test]
    fn continuous_editing_cannot_postpone_a_push_forever() {
        let mut schedule = Schedule {
            last_pull_ms: NOW,
            dirty_since_ms: NOW,
            ..Default::default()
        };
        // 每 10 秒改一个词，防抖永远等不到头；第一个改动之后十分钟照样发出去。
        let mut now = NOW;
        while now < NOW + MAX_PUSH_DELAY_MS {
            schedule.last_change_ms = now;
            assert_eq!(schedule.plan(Trigger::Periodic, now, true), (false, false));
            now += 10_000;
        }
        schedule.last_change_ms = now;
        assert_eq!(schedule.plan(Trigger::Periodic, now, true), (true, true));
    }
}
