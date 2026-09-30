//! Calendar-day arithmetic on `YYYY-MM-DD` strings.
//!
//! Hosts send the local day they resolved; this crate never derives one from a timezone. The
//! algorithms below are therefore pure functions on the calendar fields, which is what keeps a
//! day string comparable across a store written on one machine and read on another.
//!
//! This lives on its own because two callers need it. `typing_statistics` prunes a retention
//! window backwards and `vocabulary` schedules a review forwards, and a second copy of a
//! leap-year algorithm is exactly the kind of duplication that drifts silently: the copies stay
//! equal right up until one of them is fixed.

/// A stored day is exactly `YYYY-MM-DD`, ASCII digits naming a real proleptic Gregorian date.
pub fn is_valid_day(day: &str) -> bool {
    let bytes = day.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
        && day[0..4]
            .parse::<u32>()
            .ok()
            .zip(day[5..7].parse::<u32>().ok())
            .zip(day[8..10].parse::<u32>().ok())
            .is_some_and(|((year, month), date)| {
                (1..=12).contains(&month) && (1..=days_in_month(year, month)).contains(&date)
            })
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

/// `day` moved by `days`, negative for earlier. `None` when `day` is not a day.
///
/// Days-since-epoch arithmetic on the calendar fields, so it stays correct across months, years
/// and leap days without pulling a timezone into a pure function.
pub(crate) fn shift_day(day: &str, days: i64) -> Option<String> {
    if !is_valid_day(day) {
        return None;
    }
    let year: i64 = day.get(0..4)?.parse().ok()?;
    let month: i64 = day.get(5..7)?.parse().ok()?;
    let date: i64 = day.get(8..10)?.parse().ok()?;
    let shifted = days_from_civil(year, month, date).checked_add(days)?;
    let (year, month, date) = civil_from_days(shifted);
    Some(format!("{year:04}-{month:02}-{date:02}"))
}

/// Howard Hinnant's civil-date algorithms, for a proleptic Gregorian calendar.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = (year - era * 400) as u64;
    let month_position = if month > 2 { month - 3 } else { month + 9 } as u64;
    let day_of_year = (153 * month_position + 2) / 5 + day as u64 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era as i64 - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = (days - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_position + 2) / 5 + 1) as i64;
    let month = if month_position < 10 {
        month_position + 3
    } else {
        month_position - 9
    } as i64;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shifts_across_months_years_and_leap_days() {
        assert_eq!(shift_day("2026-09-21", 0).as_deref(), Some("2026-09-21"));
        assert_eq!(shift_day("2026-09-21", -30).as_deref(), Some("2026-08-22"));
        assert_eq!(shift_day("2026-01-05", -30).as_deref(), Some("2025-12-06"));
        assert_eq!(shift_day("2026-12-22", 30).as_deref(), Some("2027-01-21"));

        // 2028 是闰年，2026 不是；两边都要落在各自真实存在的那一天上。
        assert_eq!(shift_day("2028-03-01", -1).as_deref(), Some("2028-02-29"));
        assert_eq!(shift_day("2026-03-01", -1).as_deref(), Some("2026-02-28"));
        assert_eq!(shift_day("2028-02-28", 1).as_deref(), Some("2028-02-29"));
        assert_eq!(shift_day("2026-02-28", 1).as_deref(), Some("2026-03-01"));

        assert_eq!(shift_day("2027-01-01", -365).as_deref(), Some("2026-01-01"));
        assert_eq!(shift_day("not-a-day", 30), None);
        assert_eq!(shift_day("", 1), None);
        assert_eq!(shift_day("2026-02-31", 1), None);
        assert_eq!(shift_day("2026-04-31", -1), None);
    }

    #[test]
    fn accepts_only_a_ten_byte_ascii_day() {
        assert!(is_valid_day("2026-09-21"));
        assert!(is_valid_day("2026-12-31"));
        assert!(!is_valid_day("2026-13-01"));
        assert!(!is_valid_day("2026-00-01"));
        assert!(!is_valid_day("2026-09-32"));
        assert!(!is_valid_day("2026-09-00"));
        assert!(!is_valid_day("2026-9-21"));
        assert!(!is_valid_day("2026/09/21"));
        assert!(!is_valid_day(""));
        assert!(!is_valid_day("2026-02-29"));
        assert!(!is_valid_day("2026-02-31"));
        assert!(is_valid_day("2028-02-29"));
        assert!(!is_valid_day("2026-04-31"));
    }
}
