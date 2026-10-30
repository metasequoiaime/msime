//! 什么时候联网、联网时最多发几个请求。坚果云免费账号每 30 分钟 600 个请求，与这个账号授权的所有第三方应用（包括水杉的云剪贴板）共用，所以词库同步给自己定了额度：每台设备每 30 分钟一共最多 [`MANUAL_REQUESTS`] 个请求，其中自动同步自己发的最多 [`AUTOMATIC_REQUESTS`] 个。用户的操作（「立即同步」，以及加入共享词库之前的第一次配置、等待批准和获准后的第一次加入，见 [`Charge`]）只占总额度、不占自动额度：第一台设备配置一次就要三十几个请求，记成自动的会让之后半小时的自动同步一个请求也发不出去。数字的由来见 `docs/dictionary-webdav-sync.md` 的「请求量」。
//!
//! 宿主不必自己算：每次调用都告诉它下一次什么时候再来（[`Outcome::next_check_ms`]），并且只在该联网时联网。输入线程从不等网络：宿主在工作线程上调用。

use super::sync::{run, KeyRing, Options, Parts, RemoteStore, Report, SyncError, SyncState};
use serde::{Deserialize, Serialize};

/// 额度按这么长的滚动窗口算。
pub const WINDOW_MS: u64 = 30 * 60 * 1000;
/// 自动同步（改词之后、打开设置、定时）自己发的请求，在一个窗口里最多这么多个。
pub const AUTOMATIC_REQUESTS: usize = 40;
/// 自动同步和用户的操作（[`Charge::User`]）加起来，一个窗口里最多的请求数，也就是一台设备在一个窗口里最多的请求数。
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

/// 一轮的请求记在哪一边。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charge {
    /// 自动同步：占自动额度，也占总额度。
    Automatic,
    /// 用户的操作：「立即同步」，以及本机还没加入共享词库时的每一轮（第一次配置时生成密钥串和网盘上的布局、等待批准、获准后读检查点并发出本机的词）。只占总额度。加入之前的请求只是因为用户刚配置了同步才有，每台设备一次；按自动算的话，第一台设备配置的三十几个请求就把自动额度用完了。
    User,
}

impl Charge {
    /// `joined` 是本机这一轮开始时有没有加入共享词库。
    pub fn of(trigger: Trigger, joined: bool) -> Self {
        if trigger == Trigger::Manual || !joined {
            Self::User
        } else {
            Self::Automatic
        }
    }
}

/// `requests` 里落在 `now_ms` 所在窗口里的个数；回拨留下的未来记录按刚发生算。
fn in_window(requests: &[u64], now_ms: u64) -> usize {
    requests
        .iter()
        .filter(|&&at| now_ms.saturating_sub(at) < WINDOW_MS)
        .count()
}

/// 窗口里的请求过期到不超过 `allowed` 个的最早时间；已经不超过时就是现在。
fn released_at(requests: &[u64], now_ms: u64, allowed: usize) -> u64 {
    let mut active: Vec<u64> = requests
        .iter()
        .copied()
        .map(|at| at.min(now_ms))
        .filter(|&at| now_ms - at < WINDOW_MS)
        .collect();
    if active.len() <= allowed {
        return now_ms;
    }
    active.sort_unstable();
    // 不能只等最早的一条：必须过期到剩余请求数不超过 allowed 才能开始。
    active[active.len() - allowed - 1] + WINDOW_MS
}

/// 调度要记住的，随同步状态一起写盘。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    /// 这个窗口里每个请求的时间（Unix 毫秒），自动同步的和用户操作的都在里面，最多 [`MANUAL_REQUESTS`] 个。
    #[serde(default)]
    requests: Vec<u64>,
    /// 其中自动同步（[`Charge::Automatic`]）发的那些，最多 [`AUTOMATIC_REQUESTS`] 个。这个字段之前写的状态里没有它，那时的请求只算进总额度。
    #[serde(default)]
    automatic_requests: Vec<u64>,
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

    fn reconcile_time(&mut self, now_ms: u64) {
        // 回拨后仍保留已用额度，从现在重新等完整窗口，不能清空未来请求重领额度。
        for requests in [&mut self.requests, &mut self.automatic_requests] {
            for at in requests.iter_mut() {
                *at = (*at).min(now_ms);
            }
            requests.retain(|&at| now_ms - at < WINDOW_MS);
        }
        // 这些动作已经发生过：未来时间归到现在，重新遵守原有防抖和最小间隔。
        for at in [
            &mut self.last_change_ms,
            &mut self.dirty_since_ms,
            &mut self.last_push_ms,
            &mut self.last_pull_ms,
        ] {
            *at = (*at).min(now_ms);
        }
        self.retry_at_ms = self.retry_at(now_ms);
    }

    fn retry_at(&self, now_ms: u64) -> u64 {
        // 截止时间本来就在未来，只能识别超出既有最大退避窗口的异常值。
        let max_wait = if self.last_error.as_deref() == Some(SyncError::RateLimited.code()) {
            RATE_LIMIT_BACKOFF_MS
        } else {
            ATTENTION_BACKOFF_MS
        };
        self.retry_at_ms.min(now_ms.saturating_add(max_wait))
    }

    /// 这个窗口里记在 `charge` 一边的还能发几个请求：用户的操作只看总额度，自动同步还要看自动额度。
    pub fn remaining(&self, now_ms: u64, charge: Charge) -> usize {
        let total = MANUAL_REQUESTS.saturating_sub(in_window(&self.requests, now_ms));
        match charge {
            Charge::User => total,
            Charge::Automatic => total.min(
                AUTOMATIC_REQUESTS.saturating_sub(in_window(&self.automatic_requests, now_ms)),
            ),
        }
    }

    /// 这一轮发了 `count` 个请求，记在 `charge` 一边。
    fn charge(&mut self, charge: Charge, count: usize, now_ms: u64) {
        self.requests.extend(std::iter::repeat_n(now_ms, count));
        if charge == Charge::Automatic {
            self.automatic_requests
                .extend(std::iter::repeat_n(now_ms, count));
        }
    }

    /// 自动同步因为额度在等时，能再开始一轮的时间（Unix 毫秒）；没在等时为 `None`。本机额度不够一轮，或者被限流（网盘答 429、503 退避之后仍不行，或者本机额度在一轮中途用完）之后的等待都算。`joined` 是本机有没有加入共享词库：没加入时下一轮记在用户一边，只看总额度。宿主据此显示「额度用完，将于某时继续同步」，而不是看上去卡住了。
    pub fn budget_wait_until(&self, now_ms: u64, joined: bool) -> Option<u64> {
        let mut until = self.ready_at(now_ms, Charge::of(Trigger::Periodic, joined));
        if self.last_error.as_deref() == Some(SyncError::RateLimited.code()) {
            until = until.max(self.retry_at(now_ms));
        }
        (until > now_ms).then_some(until)
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

    /// 这一次要不要联网、要不要发送。`joined` 是本机有没有加入共享词库。
    fn plan(&self, trigger: Trigger, now_ms: u64, outbox: bool, joined: bool) -> (bool, bool) {
        if trigger == Trigger::Manual {
            return (self.remaining(now_ms, Charge::User) > 0, true);
        }
        if now_ms < self.retry_at_ms
            || self.remaining(now_ms, Charge::of(trigger, joined)) < MIN_RUN_REQUESTS
        {
            return (false, false);
        }
        let push = self.push_due(outbox).is_some_and(|due| due <= now_ms);
        let pull = match trigger {
            Trigger::Opened => now_ms >= self.last_pull_ms + OPEN_PULL_MIN_MS,
            Trigger::Periodic => now_ms >= self.last_pull_ms + PULL_INTERVAL_MS,
            Trigger::LocalChange | Trigger::Manual => false,
        };
        // 加入共享词库的那一次把本机的词一起发出去，不再等下一轮再花一次额度。
        (push || pull, push || (pull && !joined))
    }

    /// 滚动窗口恢复到能开始一轮的最早时间：总额度里要留得出一轮的请求，记在自动一边时自动额度里也要留得出。
    fn ready_at(&self, now_ms: u64, charge: Charge) -> u64 {
        let total = released_at(&self.requests, now_ms, MANUAL_REQUESTS - MIN_RUN_REQUESTS);
        match charge {
            Charge::User => total,
            Charge::Automatic => total.max(released_at(
                &self.automatic_requests,
                now_ms,
                AUTOMATIC_REQUESTS - MIN_RUN_REQUESTS,
            )),
        }
    }

    /// 宿主下一次什么时候再调用（距现在的毫秒数）。`charge` 是下一轮自动同步记在哪一边。
    fn next_check(&self, now_ms: u64, outbox: bool, deferred: bool, charge: Charge) -> u64 {
        let mut due = (self.last_pull_ms + PULL_INTERVAL_MS).max(self.retry_at_ms);
        if let Some(push) = self.push_due(outbox) {
            due = due.min(push.max(self.retry_at_ms));
        }
        due = due.max(self.ready_at(now_ms, charge));
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

/// 这一次要不要联网、要不要发送，请求记在哪一边。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub network: bool,
    pub push: bool,
    pub charge: Charge,
}

/// 决定这一次做什么：记下本机刚改过词，按防抖、间隔、退避和额度得出要不要联网、请求记在哪一边。宿主在联网之前还要先做别的请求时（词库的密钥串），先调它，那些请求用 [`record_requests`] 按同一个 [`Plan::charge`] 计进额度，再调 [`execute`]。
pub fn plan(state: &mut SyncState, trigger: Trigger, now_ms: u64) -> Plan {
    state.schedule.reconcile_time(now_ms);
    if trigger == Trigger::LocalChange {
        state.schedule.last_change_ms = now_ms;
        if state.schedule.dirty_since_ms == 0 {
            state.schedule.dirty_since_ms = now_ms;
        }
    }
    let outbox =
        state.outbox_len() > 0 || trigger == Trigger::LocalChange || state.wants_event(now_ms);
    let joined = state.joined();
    let (network, push) = state.schedule.plan(trigger, now_ms, outbox, joined);
    Plan {
        network,
        push,
        charge: Charge::of(trigger, joined),
    }
}

/// 宿主在 [`execute`] 之外发出的请求（例如词库密钥串的读写）也计进这个窗口的额度，记在 `charge` 一边。
pub fn record_requests(state: &mut SyncState, charge: Charge, count: usize, now_ms: u64) {
    state.schedule.charge(charge, count, now_ms);
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
    let Plan {
        network,
        push,
        charge,
    } = plan;
    let manual = trigger == Trigger::Manual;
    let limit = state.schedule.remaining(now_ms, charge);
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
        let spent = match (request_start, metered.inner.request_count()) {
            (Some(start), Some(end)) => end.saturating_sub(start),
            _ => metered.requests.len(),
        };
        state.schedule.charge(charge, spent, now_ms);
        result
    };
    // 手动同步时额度已经用完：照常记录了本机改动，但告诉用户为什么没有同步。
    let result = match result {
        Ok(_) if manual && !network => Err(SyncError::RateLimited),
        result => result,
    };
    state.schedule.finished(now_ms, network, &result);
    let deferred = result.as_ref().is_ok_and(|report| report.deferred);
    // 下一轮按这一轮之后的样子记账：刚加入共享词库的设备，下一轮就是自动同步了。
    let next_charge = Charge::of(Trigger::Periodic, state.joined());
    let next_check_ms =
        state
            .schedule
            .next_check(now_ms, state.outbox_len() > 0, deferred, next_charge);
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

    /// 这些请求都是自动同步发的。
    fn automatic(requests: Vec<u64>) -> Schedule {
        Schedule {
            automatic_requests: requests.clone(),
            requests,
            ..Default::default()
        }
    }

    #[test]
    fn local_changes_are_debounced_and_pushes_are_spaced() {
        let mut schedule = Schedule {
            last_change_ms: NOW,
            ..Default::default()
        };
        assert_eq!(
            schedule.plan(Trigger::LocalChange, NOW, true, true),
            (false, false)
        );
        assert_eq!(
            schedule.plan(Trigger::Periodic, NOW + PUSH_DEBOUNCE_MS, true, true),
            (true, true)
        );
        schedule.last_push_ms = NOW + PUSH_DEBOUNCE_MS;
        schedule.last_pull_ms = NOW + PUSH_DEBOUNCE_MS;
        schedule.last_change_ms = NOW + PUSH_DEBOUNCE_MS + 1;
        // 刚发过：五分钟之内不再自动发，下一次检查约在五分钟后。
        let soon = NOW + 2 * PUSH_DEBOUNCE_MS + 1;
        assert_eq!(
            schedule.plan(Trigger::Periodic, soon, true, true),
            (false, false)
        );
        let wait = schedule.next_check(soon, true, false, Charge::Automatic);
        assert_eq!(soon + wait, NOW + PUSH_DEBOUNCE_MS + MIN_PUSH_INTERVAL_MS);
        // 手动同步不受防抖和间隔限制。
        assert_eq!(
            schedule.plan(Trigger::Manual, soon, true, true),
            (true, true)
        );
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
            schedule.plan(Trigger::LocalChange, at - 1, true, true),
            (false, false)
        );
        schedule.last_change_ms = at;
        assert_eq!(
            schedule.plan(Trigger::LocalChange, at, true, true),
            (true, true)
        );
        // 五分钟之内发过就仍然只记录。
        schedule.last_push_ms = at - MIN_PUSH_INTERVAL_MS + 1;
        assert_eq!(
            schedule.plan(Trigger::LocalChange, at, true, true),
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
            schedule.plan(Trigger::Opened, NOW + 30_000, false, true),
            (false, false)
        );
        assert_eq!(
            schedule.plan(Trigger::Opened, NOW + OPEN_PULL_MIN_MS, false, true),
            (true, false)
        );
        assert_eq!(
            schedule.plan(Trigger::Periodic, NOW + OPEN_PULL_MIN_MS, false, true),
            (false, false)
        );
        assert_eq!(
            schedule.plan(Trigger::Periodic, NOW + PULL_INTERVAL_MS, false, true),
            (true, false)
        );
        assert_eq!(
            NOW + schedule.next_check(NOW, false, false, Charge::Automatic),
            NOW + PULL_INTERVAL_MS
        );
    }

    #[test]
    fn clock_rewind_restarts_the_pull_interval_once_after_reload() {
        let future = NOW + 2 * 60 * 60 * 1_000;
        let mut state = SyncState::new(&"a".repeat(32)).unwrap();
        state
            .schedule
            .finished(future, true, &Ok(Report::default()));
        let mut state: SyncState =
            serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert!(!plan(&mut state, Trigger::Periodic, NOW).network);
        assert_eq!(
            state.schedule.next_check(NOW, false, false, Charge::User),
            PULL_INTERVAL_MS
        );
        // 再次调用不能把恢复点继续往后推；打开设置仍遵守一分钟间隔。
        assert!(!plan(&mut state, Trigger::Opened, NOW + OPEN_PULL_MIN_MS - 1).network);
        assert!(plan(&mut state, Trigger::Opened, NOW + OPEN_PULL_MIN_MS).network);
        assert!(plan(&mut state, Trigger::Periodic, NOW + PULL_INTERVAL_MS).network);
        // 展示用的历史成功时间保留原值。
        assert_eq!(state.schedule.last_success_ms(), future);
    }

    #[test]
    fn clock_rewind_keeps_pending_edits_within_the_normal_push_interval() {
        let future = NOW + 2 * 60 * 60 * 1_000;
        let mut state = SyncState::new(&"a".repeat(32)).unwrap();
        state.schedule = Schedule {
            last_change_ms: future,
            dirty_since_ms: future,
            last_push_ms: future,
            last_pull_ms: future,
            ..Default::default()
        };
        assert!(!plan(&mut state, Trigger::Periodic, NOW).network);
        assert_eq!(
            state.schedule.next_check(NOW, true, false, Charge::User),
            MIN_PUSH_INTERVAL_MS
        );
        assert!(!plan(&mut state, Trigger::LocalChange, NOW).network);
        assert_eq!(
            state.schedule.next_check(NOW, true, false, Charge::User),
            MIN_PUSH_INTERVAL_MS
        );
        assert_eq!(
            state.schedule.plan(
                Trigger::Periodic,
                NOW + MIN_PUSH_INTERVAL_MS - 1,
                true,
                true
            ),
            (false, false)
        );
        assert_eq!(
            state
                .schedule
                .plan(Trigger::Periodic, NOW + MIN_PUSH_INTERVAL_MS, true, true),
            (true, true)
        );
    }

    #[test]
    fn clock_rewind_preserves_the_request_budget_for_a_full_window() {
        let future = NOW + 2 * 60 * 60 * 1_000;
        let mut state = SyncState::new(&"a".repeat(32)).unwrap();
        state
            .schedule
            .charge(Charge::Automatic, AUTOMATIC_REQUESTS, future);
        state
            .schedule
            .charge(Charge::User, MANUAL_REQUESTS - AUTOMATIC_REQUESTS, future);
        // 状态查询在 plan 之前也不能把未来记录当成未用额度。
        assert_eq!(state.schedule.remaining(NOW, Charge::Automatic), 0);
        assert_eq!(state.schedule.remaining(NOW, Charge::User), 0);
        assert_eq!(
            state.schedule.budget_wait_until(NOW, true),
            Some(NOW + WINDOW_MS)
        );
        assert!(!plan(&mut state, Trigger::Manual, NOW).network);
        assert!(!plan(&mut state, Trigger::Manual, NOW + WINDOW_MS - 1).network);
        assert!(plan(&mut state, Trigger::Manual, NOW + WINDOW_MS).network);
        assert_eq!(
            state.schedule.remaining(NOW + WINDOW_MS, Charge::Automatic),
            AUTOMATIC_REQUESTS
        );
    }

    #[test]
    fn clock_rewind_bounds_error_backoff_without_bypassing_it() {
        let future = NOW + 2 * 60 * 60 * 1_000;
        for (error, wait) in [
            (SyncError::RateLimited, RATE_LIMIT_BACKOFF_MS),
            (SyncError::Rollback, ATTENTION_BACKOFF_MS),
            (SyncError::Unavailable, ATTENTION_BACKOFF_MS),
        ] {
            let mut state = SyncState::new(&"a".repeat(32)).unwrap();
            state.schedule.finished(future, true, &Err(error));
            if error == SyncError::RateLimited {
                assert_eq!(
                    state.schedule.budget_wait_until(NOW, true),
                    Some(NOW + wait)
                );
            }
            assert!(!plan(&mut state, Trigger::Periodic, NOW).network);
            assert_eq!(
                state.schedule.next_check(NOW, false, false, Charge::User),
                wait
            );
            assert!(!plan(&mut state, Trigger::Periodic, NOW + wait - 1).network);
            assert!(plan(&mut state, Trigger::Periodic, NOW + wait).network);
            assert!(plan(&mut state, Trigger::Manual, NOW).network);
        }
    }

    #[test]
    fn the_request_budget_is_a_rolling_window_with_room_for_manual_syncs() {
        let mut schedule = automatic(vec![NOW; AUTOMATIC_REQUESTS]);
        assert_eq!(schedule.remaining(NOW, Charge::Automatic), 0);
        assert_eq!(
            schedule.remaining(NOW, Charge::User),
            MANUAL_REQUESTS - AUTOMATIC_REQUESTS
        );
        assert_eq!(
            schedule.plan(Trigger::Opened, NOW + OPEN_PULL_MIN_MS, false, true),
            (false, false)
        );
        assert_eq!(
            schedule.plan(Trigger::Manual, NOW, false, true),
            (true, true)
        );
        schedule.reconcile_time(NOW + WINDOW_MS);
        assert_eq!(
            schedule.remaining(NOW + WINDOW_MS, Charge::Automatic),
            AUTOMATIC_REQUESTS
        );
    }

    #[test]
    fn quota_wake_waits_for_enough_requests_in_an_unordered_window() {
        let now = NOW + 120_000;
        for used in [38, 39, 40, MANUAL_REQUESTS] {
            let mut schedule = Schedule {
                last_pull_ms: now,
                last_change_ms: NOW,
                ..automatic(
                    (0..used)
                        .rev()
                        .map(|index| NOW + index as u64 * 1_000)
                        .collect(),
                )
            };
            // 已过期的请求不参与恢复时间，即使列表不是按时间排序。
            schedule.requests.push(now - WINDOW_MS);
            schedule.automatic_requests.push(now - WINDOW_MS);
            let released = used - (AUTOMATIC_REQUESTS - MIN_RUN_REQUESTS);
            let ready = NOW + (released - 1) as u64 * 1_000 + WINDOW_MS;
            assert_eq!(
                schedule.plan(Trigger::Periodic, now, true, true),
                (false, false)
            );
            assert_eq!(
                schedule.next_check(now, true, false, Charge::Automatic),
                ready - now
            );
            assert_eq!(
                schedule.plan(Trigger::Periodic, ready - 1, true, true),
                (false, false)
            );
            assert_eq!(
                schedule.plan(Trigger::Periodic, ready, true, true),
                (true, true)
            );
        }
    }

    #[test]
    fn quota_wake_preserves_manual_sync_local_retries_and_error_backoff() {
        let mut schedule = Schedule {
            last_pull_ms: NOW,
            last_change_ms: NOW - PUSH_DEBOUNCE_MS,
            apply_retry_at_ms: NOW + BUSY_RETRY_MS[0],
            ..automatic(vec![NOW; AUTOMATIC_REQUESTS])
        };
        assert_eq!(
            schedule.plan(Trigger::Manual, NOW, true, true),
            (true, true)
        );
        assert_eq!(
            schedule.next_check(NOW, true, true, Charge::Automatic),
            BUSY_RETRY_MS[0]
        );
        // 恢复了足够额度仍要遵守原有网络错误退避。
        schedule.retry_at_ms = NOW + WINDOW_MS + 60_000;
        assert_eq!(
            schedule.next_check(NOW, true, false, Charge::Automatic),
            WINDOW_MS + 60_000
        );
        // 还有三个额度时不推迟原本到期的自动发送。
        for requests in [&mut schedule.requests, &mut schedule.automatic_requests] {
            requests.truncate(AUTOMATIC_REQUESTS - MIN_RUN_REQUESTS);
        }
        schedule.retry_at_ms = 0;
        assert_eq!(
            schedule.plan(Trigger::Periodic, NOW, true, true),
            (true, true)
        );
        assert_eq!(
            schedule.next_check(NOW, true, false, Charge::Automatic),
            1_000
        );
    }

    #[test]
    fn failures_back_off_and_attention_errors_pause_automatic_sync() {
        let mut schedule = Schedule::default();
        schedule.finished(NOW, true, &Err(SyncError::Unavailable));
        assert_eq!(schedule.retry_at_ms, NOW + FAILURE_BACKOFF_MS[0]);
        schedule.finished(NOW, true, &Err(SyncError::Unavailable));
        assert_eq!(schedule.retry_at_ms, NOW + FAILURE_BACKOFF_MS[1]);
        assert_eq!(
            schedule.plan(
                Trigger::Periodic,
                NOW + FAILURE_BACKOFF_MS[1] - 1,
                false,
                true
            ),
            (false, false)
        );
        assert_eq!(
            schedule.plan(Trigger::Periodic, NOW + FAILURE_BACKOFF_MS[1], false, true),
            (true, false)
        );
        schedule.finished(NOW, true, &Err(SyncError::RateLimited));
        assert_eq!(schedule.retry_at_ms, NOW + RATE_LIMIT_BACKOFF_MS);
        schedule.finished(NOW, true, &Err(SyncError::Rollback));
        assert_eq!(schedule.retry_at_ms, NOW + ATTENTION_BACKOFF_MS);
        assert_eq!(
            schedule.plan(Trigger::Opened, NOW + 1, false, true),
            (false, false)
        );
        assert_eq!(
            schedule.plan(Trigger::Manual, NOW + 1, false, true),
            (true, true)
        );
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
            assert_eq!(
                schedule.next_check(NOW, false, true, Charge::Automatic),
                *wait
            );
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
            assert_eq!(
                schedule.plan(Trigger::Periodic, now, true, true),
                (false, false)
            );
            now += 10_000;
        }
        schedule.last_change_ms = now;
        assert_eq!(
            schedule.plan(Trigger::Periodic, now, true, true),
            (true, true)
        );
    }

    #[test]
    fn user_requests_take_the_total_budget_but_leave_the_automatic_one() {
        // 第一台设备配置用掉三十几个请求：记在用户一边，自动同步只受总额度限制。
        let setup = 38;
        let mut schedule = Schedule {
            last_pull_ms: NOW - PULL_INTERVAL_MS,
            ..Default::default()
        };
        schedule.charge(Charge::User, setup, NOW);
        assert_eq!(
            schedule.remaining(NOW, Charge::Automatic),
            MANUAL_REQUESTS - setup
        );
        assert_eq!(
            schedule.remaining(NOW, Charge::User),
            MANUAL_REQUESTS - setup
        );
        assert_eq!(
            schedule.plan(Trigger::Periodic, NOW, false, true),
            (true, false)
        );
        assert_eq!(schedule.budget_wait_until(NOW, true), None);
        // 自动同步自己的请求两边都占；自动额度用完时自动同步等，手动照常。
        let mut busy = Schedule::default();
        busy.charge(Charge::Automatic, AUTOMATIC_REQUESTS, NOW);
        assert_eq!(busy.remaining(NOW, Charge::Automatic), 0);
        assert_eq!(
            busy.remaining(NOW, Charge::User),
            MANUAL_REQUESTS - AUTOMATIC_REQUESTS
        );
        assert_eq!(busy.budget_wait_until(NOW, true), Some(NOW + WINDOW_MS));
        // 总额度用完时谁都不能再发：一台设备一个窗口最多 MANUAL_REQUESTS 个请求。
        let mut full = Schedule::default();
        full.charge(Charge::User, MANUAL_REQUESTS, NOW);
        assert_eq!(full.remaining(NOW, Charge::Automatic), 0);
        assert_eq!(full.remaining(NOW, Charge::User), 0);
        assert_eq!(
            full.plan(Trigger::Opened, NOW + OPEN_PULL_MIN_MS, false, true),
            (false, false)
        );
        assert_eq!(full.plan(Trigger::Manual, NOW, false, true), (false, true));
        // 总额度要等三个请求过期，自动额度一个也没用：恢复时间只看总额度。
        let ready = NOW + WINDOW_MS;
        assert_eq!(full.ready_at(NOW, Charge::Automatic), ready);
        assert_eq!(full.budget_wait_until(NOW, true), Some(ready));
    }

    #[test]
    fn a_device_that_has_not_joined_charges_its_rounds_to_the_user() {
        assert_eq!(Charge::of(Trigger::Opened, false), Charge::User);
        assert_eq!(Charge::of(Trigger::Periodic, false), Charge::User);
        assert_eq!(Charge::of(Trigger::Manual, true), Charge::User);
        assert_eq!(Charge::of(Trigger::Opened, true), Charge::Automatic);
        assert_eq!(Charge::of(Trigger::LocalChange, true), Charge::Automatic);
        // 自动额度已经用完（例如加入前的旧状态里记成了自动）：还没加入的设备照样能去加入，加入那一次连同本机的词一起发。
        let schedule = Schedule {
            last_pull_ms: NOW - OPEN_PULL_MIN_MS,
            ..automatic(vec![NOW; AUTOMATIC_REQUESTS])
        };
        assert_eq!(
            schedule.plan(Trigger::Opened, NOW, false, false),
            (true, true)
        );
        assert_eq!(
            schedule.plan(Trigger::Opened, NOW, false, true),
            (false, false)
        );
        assert_eq!(schedule.budget_wait_until(NOW, false), None);
        assert_eq!(schedule.budget_wait_until(NOW, true), Some(NOW + WINDOW_MS));
        // 改词本身不联网，加没加入都一样。
        let changed = Schedule {
            last_change_ms: NOW,
            ..Default::default()
        };
        assert_eq!(
            changed.plan(Trigger::LocalChange, NOW, true, false),
            (false, false)
        );
    }

    #[test]
    fn a_rate_limited_round_reports_when_automatic_sync_resumes() {
        let mut schedule = Schedule::default();
        schedule.finished(NOW, true, &Err(SyncError::RateLimited));
        assert_eq!(
            schedule.budget_wait_until(NOW, true),
            Some(NOW + RATE_LIMIT_BACKOFF_MS)
        );
        // 别的错误的退避不是额度的事。
        schedule.finished(NOW, true, &Err(SyncError::Unavailable));
        assert_eq!(schedule.budget_wait_until(NOW, true), None);
        schedule.finished(NOW, true, &Ok(Report::default()));
        assert_eq!(schedule.budget_wait_until(NOW, true), None);
    }
}
