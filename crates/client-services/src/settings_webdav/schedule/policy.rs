//! 设置同步的时间与额度决策，不读文件、不联网，时间由调用方提供。

pub const WINDOW_MS: u64 = 30 * 60 * 1000;
pub const AUTOMATIC_REQUESTS: usize = 40;
pub const TOTAL_REQUESTS: usize = 60;
pub const DEBOUNCE_MS: u64 = 60 * 1000;
pub const MAX_DELAY_MS: u64 = 10 * 60 * 1000;
pub const OPEN_INTERVAL_MS: u64 = 60 * 1000;
pub const PULL_INTERVAL_MS: u64 = 30 * 60 * 1000;
pub const ATTENTION_MS: u64 = 60 * 60 * 1000;
const MIN_ROUND_REQUESTS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    LocalChange,
    Opened,
    Periodic,
    Manual,
}

/// 已持久化状态的一次只读投影；请求记录包含已预扣、尚未收到响应的请求。
pub(super) struct View<'a> {
    pub requests: &'a [u64],
    pub automatic_requests: &'a [u64],
    pub dirty_since: Option<u64>,
    pub last_change: u64,
    pub last_pull: Option<u64>,
    pub retry_at: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Decision {
    pub network: bool,
    pub remaining: usize,
    pub next_check_ms: u64,
    pub budget_wait_until: Option<u64>,
}

fn active(requests: &[u64], now: u64) -> usize {
    requests
        .iter()
        .filter(|&&at| now.saturating_sub(at) < WINDOW_MS)
        .count()
}

fn released_at(requests: &[u64], now: u64, maximum: usize) -> u64 {
    let mut active: Vec<_> = requests
        .iter()
        .map(|&at| at.min(now))
        .filter(|&at| now - at < WINDOW_MS)
        .collect();
    if active.len() <= maximum {
        return now;
    }
    active.sort_unstable();
    active[active.len() - maximum - 1].saturating_add(WINDOW_MS)
}

impl View<'_> {
    pub fn decide(&self, trigger: Trigger, now: u64) -> Decision {
        let total = TOTAL_REQUESTS.saturating_sub(active(self.requests, now));
        let automatic = AUTOMATIC_REQUESTS.saturating_sub(active(self.automatic_requests, now));
        let manual = trigger == Trigger::Manual;
        let remaining = if manual { total } else { total.min(automatic) };
        let budget_ready = released_at(self.requests, now, TOTAL_REQUESTS - MIN_ROUND_REQUESTS)
            .max(if manual {
                now
            } else {
                released_at(
                    self.automatic_requests,
                    now,
                    AUTOMATIC_REQUESTS - MIN_ROUND_REQUESTS,
                )
            });
        let dirty_due = self.dirty_since.map(|first| {
            self.last_change
                .saturating_add(DEBOUNCE_MS)
                .min(first.saturating_add(MAX_DELAY_MS))
        });
        let interval = if trigger == Trigger::Opened {
            OPEN_INTERVAL_MS
        } else {
            PULL_INTERVAL_MS
        };
        let pull_due = self.last_pull.map_or(now, |at| at.saturating_add(interval));
        // 本机改动信号先记账，不能借首次轮询尚未发生而立即上传。
        let due = if trigger == Trigger::LocalChange
            || (trigger == Trigger::Periodic && self.last_pull.is_none())
        {
            dirty_due.unwrap_or(pull_due)
        } else {
            dirty_due.map_or(pull_due, |dirty| dirty.min(pull_due))
        };
        let retry_at = self.retry_at.min(now.saturating_add(ATTENTION_MS));
        let ready = due.max(retry_at).max(budget_ready);
        Decision {
            network: if manual {
                remaining > 0
            } else {
                remaining >= MIN_ROUND_REQUESTS && ready <= now
            },
            remaining,
            next_check_ms: ready.saturating_sub(now).max(1_000),
            budget_wait_until: (budget_ready > now).then_some(budget_ready),
        }
    }
}

pub(super) fn failure_delay(failures: u32) -> u64 {
    const DELAYS: [u64; 6] = [60_000, 120_000, 300_000, 600_000, 1_800_000, 3_600_000];
    DELAYS[(failures as usize).min(DELAYS.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View<'static> {
        View {
            requests: &[],
            automatic_requests: &[],
            dirty_since: None,
            last_change: 0,
            last_pull: None,
            retry_at: 0,
        }
    }

    #[test]
    fn changes_wait_for_quiet_but_continuous_changes_have_a_deadline() {
        let first = 10_000;
        let mut state = view();
        state.dirty_since = Some(first);
        state.last_change = first;
        assert!(!state.decide(Trigger::LocalChange, first).network);
        state.last_pull = Some(first);
        assert!(
            !state
                .decide(Trigger::Periodic, first + DEBOUNCE_MS - 1)
                .network
        );
        assert!(state.decide(Trigger::Periodic, first + DEBOUNCE_MS).network);
        for elapsed in (0..MAX_DELAY_MS).step_by(1_000) {
            state.last_change = first + elapsed;
            assert!(!state.decide(Trigger::LocalChange, first + elapsed).network);
        }
        state.last_change = first + MAX_DELAY_MS;
        assert!(
            state
                .decide(Trigger::LocalChange, first + MAX_DELAY_MS)
                .network
        );
    }

    #[test]
    fn opened_and_periodic_use_different_pull_intervals() {
        let mut state = view();
        assert!(state.decide(Trigger::Opened, 0).network);
        state.last_pull = Some(0);
        assert!(!state.decide(Trigger::Opened, OPEN_INTERVAL_MS - 1).network);
        assert!(state.decide(Trigger::Opened, OPEN_INTERVAL_MS).network);
        assert!(!state.decide(Trigger::Periodic, OPEN_INTERVAL_MS).network);
        assert!(state.decide(Trigger::Periodic, PULL_INTERVAL_MS).network);
    }

    #[test]
    fn initial_periodic_poll_keeps_the_first_edit_debounce() {
        let state = View {
            dirty_since: Some(10_000),
            last_change: 10_000,
            ..view()
        };
        assert!(!state.decide(Trigger::Periodic, 10_000).network);
        assert_eq!(
            state.decide(Trigger::Periodic, 10_000).next_check_ms,
            DEBOUNCE_MS
        );
        assert!(
            !state
                .decide(Trigger::Periodic, 10_000 + DEBOUNCE_MS - 1)
                .network
        );
        assert!(
            state
                .decide(Trigger::Periodic, 10_000 + DEBOUNCE_MS)
                .network
        );
        assert!(state.decide(Trigger::Opened, 10_000).network);
    }

    #[test]
    fn automatic_budget_reserves_capacity_for_user_actions() {
        let requests = vec![5_000; AUTOMATIC_REQUESTS];
        let state = View {
            requests: &requests,
            automatic_requests: &requests,
            ..view()
        };
        let auto = state.decide(Trigger::Periodic, 5_000);
        assert!(!auto.network);
        assert_eq!(auto.budget_wait_until, Some(5_000 + WINDOW_MS));
        assert_eq!(state.decide(Trigger::Manual, 5_000).remaining, 20);
        assert!(state.decide(Trigger::Manual, 5_000).network);
        assert!(state.decide(Trigger::Periodic, 5_000 + WINDOW_MS).network);
    }

    #[test]
    fn manual_cannot_bypass_total_budget_and_clock_rewind_keeps_charges() {
        let requests = vec![10_000; TOTAL_REQUESTS];
        let state = View {
            requests: &requests,
            ..view()
        };
        let decision = state.decide(Trigger::Manual, 1_000);
        assert!(!decision.network);
        assert_eq!(decision.budget_wait_until, Some(1_000 + WINDOW_MS));
    }

    #[test]
    fn retry_waits_survive_new_edits_and_manual_can_retry() {
        let state = View {
            retry_at: 120_000,
            ..view()
        };
        assert!(!state.decide(Trigger::Periodic, 60_000).network);
        assert_eq!(
            state.decide(Trigger::Periodic, 60_000).next_check_ms,
            60_000
        );
        assert!(state.decide(Trigger::Manual, 60_000).network);
        assert!(state.decide(Trigger::Periodic, 120_000).network);
        assert_eq!(failure_delay(0), 60_000);
        assert_eq!(failure_delay(u32::MAX), ATTENTION_MS);
    }
}
