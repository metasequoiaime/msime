//! 引擎读取的单调时钟。原生平台就是 `std::time::Instant`；wasm32-unknown-unknown 的 std 没有时钟，`std::time::Instant::now` 会 panic，所以那里换成宿主经 `set_host_clock` 注入的毫秒读数（浏览器里是 `performance.now()` 和 `Date.now()`）。注入之前 `Instant::now()` 返回零点，经过的时间读作 0，不会 panic。

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
pub use std::time::Instant;

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub use host::Instant;

/// 宿主提供的两个时钟读数，都是毫秒。
#[derive(Clone, Copy)]
pub struct HostClock {
    /// 单调毫秒（performance.now）
    pub steady_ms: fn() -> f64,
    /// Unix 毫秒（Date.now）
    pub wall_ms: fn() -> f64,
}

/// 只在 wasm 上有效；原生平台是空操作
pub fn set_host_clock(clock: HostClock) {
    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    host::store(clock);
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    let _ = clock;
}

/// wasm 上没有注入时返回 None
pub fn host_wall_ms() -> Option<f64> {
    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    return host::injected().map(|clock| (clock.wall_ms)());
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    None
}

/// wasm 上的 `Instant`。原生测试也编译这一份，让算术和注入前的行为能在原生 `cargo test` 里验证。
#[cfg(any(test, all(target_family = "wasm", target_os = "unknown")))]
mod host {
    use std::ops::{Add, AddAssign, Sub, SubAssign};
    use std::sync::{PoisonError, RwLock};
    use std::time::Duration;

    use super::HostClock;

    static CLOCK: RwLock<Option<HostClock>> = RwLock::new(None);

    #[cfg_attr(
        all(test, not(all(target_family = "wasm", target_os = "unknown"))),
        expect(
            dead_code,
            reason = "原生平台的 set_host_clock 是空操作，测试也不注入全局时钟，避免并行测试互相干扰"
        )
    )]
    pub(super) fn store(clock: HostClock) {
        *CLOCK.write().unwrap_or_else(PoisonError::into_inner) = Some(clock);
    }

    pub(super) fn injected() -> Option<HostClock> {
        *CLOCK.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// 自宿主时钟原点起的时间。
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Instant(Duration);

    impl Instant {
        pub fn now() -> Self {
            Self::read(injected())
        }

        /// 没有注入时钟时是零点。
        pub(super) fn read(clock: Option<HostClock>) -> Self {
            Self::from_ms(clock.map_or(0.0, |clock| (clock.steady_ms)()))
        }

        /// 负数、NaN 和无穷按零点处理，超出 `Duration` 范围的读数取最大值，所以任何宿主读数都不会 panic。
        pub(super) fn from_ms(millis: f64) -> Self {
            if !millis.is_finite() || millis <= 0.0 {
                return Instant(Duration::ZERO);
            }
            Instant(Duration::try_from_secs_f64(millis / 1000.0).unwrap_or(Duration::MAX))
        }

        pub fn saturating_duration_since(&self, earlier: Instant) -> Duration {
            self.0.saturating_sub(earlier.0)
        }

        /// 与 std 的当前行为一致：`earlier` 更晚时返回零而不是 panic。
        pub fn duration_since(&self, earlier: Instant) -> Duration {
            self.saturating_duration_since(earlier)
        }

        pub fn elapsed(&self) -> Duration {
            Instant::now().saturating_duration_since(*self)
        }

        pub fn checked_add(&self, duration: Duration) -> Option<Instant> {
            self.0.checked_add(duration).map(Instant)
        }

        pub fn checked_sub(&self, duration: Duration) -> Option<Instant> {
            self.0.checked_sub(duration).map(Instant)
        }
    }

    impl Add<Duration> for Instant {
        type Output = Instant;
        /// 与 std 不同，溢出时饱和而不 panic。
        fn add(self, other: Duration) -> Instant {
            Instant(self.0.saturating_add(other))
        }
    }

    impl AddAssign<Duration> for Instant {
        fn add_assign(&mut self, other: Duration) {
            *self = *self + other;
        }
    }

    impl Sub<Duration> for Instant {
        type Output = Instant;
        /// 与 std 不同，减到时钟原点之前时停在原点而不 panic：注入前或页面刚打开时读数接近零，`now() - 窗口` 不能因此中断输入。
        fn sub(self, other: Duration) -> Instant {
            Instant(self.0.saturating_sub(other))
        }
    }

    impl SubAssign<Duration> for Instant {
        fn sub_assign(&mut self, other: Duration) {
            *self = *self - other;
        }
    }

    impl Sub<Instant> for Instant {
        type Output = Duration;
        fn sub(self, other: Instant) -> Duration {
            self.saturating_duration_since(other)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::host::Instant as HostInstant;
    use super::HostClock;

    #[test]
    fn native_instant_is_std() {
        let now: std::time::Instant = super::Instant::now();
        assert!(now.elapsed() < Duration::from_secs(60));
    }

    #[test]
    fn native_has_no_host_wall_clock() {
        super::set_host_clock(HostClock {
            steady_ms: || 5.0,
            wall_ms: || 7.0,
        });
        assert_eq!(super::host_wall_ms(), None);
    }

    #[test]
    fn uninjected_clock_reads_zero() {
        let zero = HostInstant::read(None);
        assert_eq!(zero, HostInstant::from_ms(0.0));
        assert_eq!(
            zero.saturating_duration_since(HostInstant::from_ms(0.0)),
            Duration::ZERO
        );
    }

    #[test]
    fn host_instant_before_injection_is_the_origin() {
        // 原生测试从不调用 host::store，所以全局时钟始终没有注入
        let now = HostInstant::now();
        assert_eq!(now, HostInstant::from_ms(0.0));
        assert_eq!(now.elapsed(), Duration::ZERO);
    }

    #[test]
    fn injected_clock_is_read() {
        let clock = HostClock {
            steady_ms: || 1500.0,
            wall_ms: || 0.0,
        };
        assert_eq!(
            HostInstant::read(Some(clock)).duration_since(HostInstant::from_ms(0.0)),
            Duration::from_millis(1500)
        );
    }

    #[test]
    fn invalid_readings_clamp_to_zero() {
        let zero = HostInstant::from_ms(0.0);
        assert_eq!(HostInstant::from_ms(-3.0), zero);
        assert_eq!(HostInstant::from_ms(f64::NAN), zero);
        assert_eq!(HostInstant::from_ms(f64::INFINITY), zero);
    }

    #[test]
    fn arithmetic_matches_std_semantics() {
        let early = HostInstant::from_ms(1000.0);
        let late = HostInstant::from_ms(3500.0);
        assert_eq!(late - early, Duration::from_millis(2500));
        assert_eq!(early - late, Duration::ZERO);
        assert_eq!(late.duration_since(early), Duration::from_millis(2500));
        assert_eq!(early.duration_since(late), Duration::ZERO);
        assert_eq!(early + Duration::from_millis(2500), late);
        assert_eq!(late - Duration::from_millis(2500), early);
        assert_eq!(
            early.checked_sub(Duration::from_secs(2)),
            None,
            "before the clock origin"
        );
        assert_eq!(early.checked_add(Duration::from_millis(2500)), Some(late));
        assert_eq!(
            HostInstant::from_ms(f64::MAX).checked_add(Duration::MAX),
            None
        );
        assert_eq!(
            HostInstant::from_ms(f64::MAX) + Duration::MAX,
            HostInstant::from_ms(f64::MAX)
        );
        let mut moving = early;
        moving += Duration::from_millis(2500);
        assert_eq!(moving, late);
        moving -= Duration::from_millis(2500);
        assert_eq!(moving, early);
        assert!(early < late);
    }

    #[test]
    fn subtracting_past_origin_stops_at_origin() {
        assert_eq!(
            HostInstant::from_ms(1.0) - Duration::from_secs(1),
            HostInstant::from_ms(0.0)
        );
    }
}
