//! 混合逻辑时钟（HLC）。每条操作都带一个时间戳，词条的最终取值按（时间戳、设备编号、是否删除）的全序决定，与操作到达网盘的先后无关。

use serde::{Deserialize, Serialize};

/// 低 10 位是同一毫秒里的计数，其余是 Unix 毫秒。
const COUNTER_BITS: u32 = 10;
/// 时间戳的上限是 2^53 - 1：其他宿主（ArkTS、JavaScript）把 JSON 数字读成双精度浮点，超过它就不精确了。毫秒部分因此最多到 2^43 - 1，约公元 2248 年。
pub const MAX_HLC: u64 = (1 << 53) - 1;
/// 毫秒部分至少是 1，0 留给「还没有时间戳」。
pub const MIN_HLC: u64 = 1 << COUNTER_BITS;
const MAX_MILLIS: u64 = MAX_HLC >> COUNTER_BITS;
/// 收到的时间戳比本机时间快这么多，就在状态里提示时钟不准。只是提示：合并结果只由日志决定，不看本机时间。
pub const SKEW_WARNING_MS: u64 = 24 * 60 * 60 * 1000;

/// 一个时间戳。整数越大越晚：先比毫秒，同一毫秒再比计数。
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Hlc(pub u64);

impl Hlc {
    /// 由操作记录里的数字得出时间戳；不在 [`MIN_HLC`]..=[`MAX_HLC`] 之内的不合法，这条操作所有设备都同样跳过。
    pub fn parse(value: u64) -> Option<Self> {
        (MIN_HLC..=MAX_HLC).contains(&value).then_some(Self(value))
    }

    pub fn millis(self) -> u64 {
        self.0 >> COUNTER_BITS
    }
}

/// 本机的时钟：记着发出或见过的最大时间戳。随同步状态一起原子写盘，所以进程重启、系统时间往回调都不会发出比以前小的时间戳。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Clock {
    last: u64,
}

impl Clock {
    /// 为一条本机操作取下一个时间戳：比见过的都大，并且不早于本机时间。毫秒部分用到上限（只有时间戳被推到公元 2248 年以后才会）时为 `None`，不再发出操作。
    pub fn tick(&mut self, now_ms: u64) -> Option<Hlc> {
        let physical = now_ms.min(MAX_MILLIS) << COUNTER_BITS;
        let next = self.last.checked_add(1)?.max(physical);
        if next > MAX_HLC {
            return None;
        }
        self.last = next;
        Some(Hlc(next))
    }

    /// 收到别的设备的时间戳：之后本机的操作都比它晚。返回它是否比本机时间快了 [`SKEW_WARNING_MS`] 以上。
    pub fn observe(&mut self, remote: Hlc, now_ms: u64) -> bool {
        self.last = self.last.max(remote.0);
        remote.millis() > now_ms.saturating_add(SKEW_WARNING_MS)
    }

    pub fn last(self) -> u64 {
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_790_000_000_000;

    #[test]
    fn ticks_are_strictly_increasing_even_when_the_wall_clock_goes_back() {
        let mut clock = Clock::default();
        let first = clock.tick(NOW).unwrap();
        let second = clock.tick(NOW).unwrap();
        let after_jump_back = clock.tick(NOW - 3_600_000).unwrap();
        assert!(first < second && second < after_jump_back);
        assert_eq!(first.millis(), NOW);
        // 同一毫秒里超过 1024 次只是把毫秒部分往前推，仍然严格递增。
        let mut previous = after_jump_back;
        for _ in 0..3000 {
            let next = clock.tick(NOW).unwrap();
            assert!(next > previous);
            previous = next;
        }
    }

    #[test]
    fn observing_a_later_remote_stamp_makes_later_local_stamps_win() {
        let mut clock = Clock::default();
        clock.tick(NOW).unwrap();
        let remote = Hlc((NOW + 5_000) << COUNTER_BITS);
        assert!(!clock.observe(remote, NOW));
        assert!(clock.tick(NOW).unwrap() > remote);
    }

    #[test]
    fn a_far_future_stamp_is_reported_but_still_respected() {
        let mut clock = Clock::default();
        let future = Hlc((NOW + 2 * SKEW_WARNING_MS) << COUNTER_BITS);
        assert!(clock.observe(future, NOW));
        assert!(clock.tick(NOW).unwrap() > future);
    }

    #[test]
    fn the_clock_survives_a_restart_through_its_serialised_form() {
        let mut clock = Clock::default();
        let before = clock.tick(NOW).unwrap();
        let saved = serde_json::to_string(&clock).unwrap();
        let mut restored: Clock = serde_json::from_str(&saved).unwrap();
        // 重启后系统时间慢了一小时，也不会发出更小的时间戳。
        assert!(restored.tick(NOW - 3_600_000).unwrap() > before);
    }

    #[test]
    fn stamps_outside_the_valid_range_are_rejected_and_exhaustion_stops_ticking() {
        assert!(Hlc::parse(0).is_none());
        assert!(Hlc::parse(MIN_HLC - 1).is_none());
        assert!(Hlc::parse(MAX_HLC + 1).is_none());
        assert!(Hlc::parse(MAX_HLC).is_some());
        let mut clock = Clock::default();
        clock.observe(Hlc(MAX_HLC), NOW);
        assert_eq!(clock.tick(NOW), None);
        assert_eq!(clock.last(), MAX_HLC);
    }
}
