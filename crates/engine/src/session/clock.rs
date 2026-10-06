//! Injectable clocks. The personal-learning windows (3 s context keep, 8 s chain pause, 10 s pick-pair gap) read the steady clock and the date/time mode reads the local wall clock; tests replace both.

use crate::time::Instant;

use crate::local::date_time::LocalDateTime;

pub struct Clock {
    pub steady: Box<dyn Fn() -> Instant + Send>,
    pub local: Box<dyn Fn() -> LocalDateTime + Send>,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            steady: Box::new(Instant::now),
            local: Box::new(default_local),
        }
    }
}

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
fn default_local() -> LocalDateTime {
    crate::local::date_time::current_local_date_time()
}

/// wasm 上 `OffsetDateTime::now_local` 会 panic，所以按 UTC 读宿主注入的 Unix 毫秒；没有注入时是 Unix 纪元。网页宿主关闭了本地模式，这里只是保护。
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
fn default_local() -> LocalDateTime {
    utc_local_date_time(crate::time::host_wall_ms().unwrap_or(0.0))
}

#[cfg(any(test, all(target_family = "wasm", target_os = "unknown")))]
fn utc_local_date_time(unix_ms: f64) -> LocalDateTime {
    let nanos = if unix_ms.is_finite() {
        // 越界的读数由 from_unix_timestamp_nanos 拒绝，落回纪元
        (unix_ms * 1_000_000.0) as i128
    } else {
        0
    };
    let now = ::time::OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .unwrap_or(::time::OffsetDateTime::UNIX_EPOCH);
    LocalDateTime {
        year: now.year(),
        month: u32::from(u8::from(now.month())),
        day: u32::from(now.day()),
        weekday: u32::from(now.weekday().number_days_from_sunday()),
        hour: u32::from(now.hour()),
        minute: u32::from(now.minute()),
        second: u32::from(now.second()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_reading_becomes_a_local_date_time() {
        // 2024-02-29 13:45:30.250 UTC，星期四
        let now = utc_local_date_time(1_709_214_330_250.0);
        assert_eq!(
            now,
            LocalDateTime {
                year: 2024,
                month: 2,
                day: 29,
                weekday: 4,
                hour: 13,
                minute: 45,
                second: 30,
            }
        );
    }

    #[test]
    fn unusable_readings_fall_back_to_the_epoch() {
        let epoch = utc_local_date_time(0.0);
        assert_eq!(
            (epoch.year, epoch.month, epoch.day, epoch.weekday),
            (1970, 1, 1, 4)
        );
        assert_eq!(utc_local_date_time(f64::NAN), epoch);
        assert_eq!(utc_local_date_time(f64::MAX), epoch);
    }
}
