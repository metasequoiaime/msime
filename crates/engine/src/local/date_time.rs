//! `T` mode (date_time_query.cpp, core-session.md §10.2): `rq`/`riqi`/`date`, `sj`/`shijian`/`time`, `xq`/`xingqi`/`week`, formatted from an injected local time. The lunar date comes from `lunar-lite` rather than the hand-typed 1900-2100 table.
//!
//! `nl`/`nongli`/`yinli` 是参考实现之外新增的农历关键词（#5952）：农历原本只排在 `rq` 列表最后一行，单独的关键词让它不用翻页就能选到。

use lunar_lite::{solar_to_lunar, SolarDate};
use time::{Date, Month, OffsetDateTime};

use crate::types::{CandidateSource, WordItem};

pub const RESULT_LIMIT: usize = 17;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LocalDateTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    /// 0 = Sunday.
    pub weekday: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// Whether the date fields describe a real Gregorian calendar date.
pub(crate) fn is_valid_calendar_date(year: i32, month: u32, day: u32) -> bool {
    let Some(month) = u8::try_from(month)
        .ok()
        .and_then(|month| Month::try_from(month).ok())
    else {
        return false;
    };
    let Some(day) = u8::try_from(day).ok() else {
        return false;
    };
    Date::from_calendar_date(year, month, day).is_ok()
}

pub(crate) const WEEKDAYS: [&str; 7] = [
    "星期日",
    "星期一",
    "星期二",
    "星期三",
    "星期四",
    "星期五",
    "星期六",
];
const SHORT_WEEKDAYS: [&str; 7] = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];
const ENGLISH_WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const ENGLISH_FULL_WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
/// Year digits read one by one, with 〇 for zero.
const YEAR_DIGITS: [&str; 10] = ["〇", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
/// Counting digits; zero is spelled by omission.
const NUMBER_DIGITS: [&str; 10] = ["", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
const FINANCIAL_DIGITS: [&str; 10] = ["零", "壹", "贰", "叁", "肆", "伍", "陆", "柒", "捌", "玖"];
const STEMS: [&str; 10] = ["甲", "乙", "丙", "丁", "戊", "己", "庚", "辛", "壬", "癸"];
const BRANCHES: [&str; 12] = [
    "子", "丑", "寅", "卯", "辰", "巳", "午", "未", "申", "酉", "戌", "亥",
];

/// The wall clock in the local offset, via `time`; zeros when the offset cannot be determined, as the reference did on a `localtime` failure.
#[cfg_attr(
    all(target_family = "wasm", target_os = "unknown", not(test)),
    expect(
        dead_code,
        reason = "wasm 上 `now_local` 会 panic，`Clock::default` 改读宿主注入的 UTC 时钟"
    )
)]
pub fn current_local_date_time() -> LocalDateTime {
    // A clock that cannot be read still yields well-formed rows (date_time_query.cpp:312-328), so there is nothing to report.
    let Ok(now) = OffsetDateTime::now_local() else {
        return LocalDateTime::default();
    };
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

/// Generated rows, weight `count - index`, at most 17; nothing for an unknown keyword.
pub fn query_date_time(keyword: &str, now: &LocalDateTime) -> Vec<WordItem> {
    query_date_time_with_limit(keyword, now, RESULT_LIMIT)
}

/// `query_date_time` with the reference's explicit limit; zero answers nothing.
pub fn query_date_time_with_limit(
    keyword: &str,
    now: &LocalDateTime,
    limit: usize,
) -> Vec<WordItem> {
    let texts = match keyword {
        "rq" | "riqi" | "date" => date_candidates(now),
        "sj" | "shijian" | "time" => time_candidates(now),
        "xq" | "xingqi" | "week" => week_candidates(now),
        "nl" | "nongli" | "yinli" => lunar_candidates(now),
        _ => return Vec::new(),
    };
    let count = texts.len().min(limit);
    texts
        .into_iter()
        .take(count)
        .enumerate()
        .map(|(index, text)| {
            WordItem::new(
                "",
                text,
                (count - index) as i64,
                CandidateSource::Generated,
                "",
            )
        })
        .collect()
}

fn weekday_index(now: &LocalDateTime) -> usize {
    now.weekday.min(6) as usize
}

fn date_candidates(now: &LocalDateTime) -> Vec<String> {
    let LocalDateTime {
        year,
        month,
        day,
        hour,
        minute,
        ..
    } = *now;
    let weekday = weekday_index(now);
    let mut results = vec![
        format!("{year}年{month}月{day}日"),
        format!("{year:04}-{month:02}-{day:02}"),
        format!("{year:04}/{month:02}/{day:02}"),
        format!("{year:04}.{month:02}.{day:02}"),
        format!("{year:04}{month:02}{day:02}"),
        format!("{:02}年{month}月{day}日", year.rem_euclid(100)),
        format!("{month}月{day}日"),
        format!("{month:02}-{day:02}"),
        format!("{month:02}{day:02}"),
        format!("{year}年{month}月{day}日 {}", WEEKDAYS[weekday]),
        format!("{month}月{day}日 {}", SHORT_WEEKDAYS[weekday]),
        format!(
            "{year:04}-{month:02}-{day:02} {}",
            ENGLISH_WEEKDAYS[weekday]
        ),
        format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}"),
        format!("{month}月{day}日 {hour:02}:{minute:02}"),
        format!(
            "{}年{}月{}日",
            year_digits(year.unsigned_abs()),
            chinese_number(month),
            chinese_number(day)
        ),
        format!(
            "{}年{}月{}日",
            financial_digits(year.unsigned_abs(), 1),
            financial_digits(month, 1),
            financial_digits(day, 2)
        ),
    ];
    // A date the calendar cannot convert drops the row rather than showing a blank one that commits nothing (date_time_query.cpp:262-267).
    if let Some(lunar) = lunar_date(now) {
        results.push(lunar);
    }
    results
}

fn time_candidates(now: &LocalDateTime) -> Vec<String> {
    let LocalDateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
        ..
    } = *now;
    let hour12 = if hour % 12 == 0 { 12 } else { hour % 12 };
    let morning = hour < 12;
    let period = if morning { "上午" } else { "下午" };
    let meridiem_upper = if morning { "AM" } else { "PM" };
    let meridiem_lower = if morning { "am" } else { "pm" };
    let colloquial_hour = if hour12 == 2 {
        "两".to_owned()
    } else {
        chinese_number(hour12)
    };
    let colloquial_minutes = match minute {
        0 => String::new(),
        30 => "半".to_owned(),
        _ => format!("{}分", chinese_number(minute)),
    };
    vec![
        format!("{hour:02}:{minute:02}"),
        format!("{hour:02}:{minute:02}:{second:02}"),
        format!("{hour:02}{minute:02}"),
        format!("{hour:02}{minute:02}{second:02}"),
        format!("{period}{hour12}:{minute:02}"),
        format!("{period}{hour12}点{minute:02}分"),
        format!("{period}{colloquial_hour}点{colloquial_minutes}"),
        format!("{hour12}:{minute:02} {meridiem_upper}"),
        format!("{hour12}:{minute:02}{meridiem_lower}"),
        format!("{hour12:02}:{minute:02}:{second:02} {meridiem_upper}"),
        format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}"),
        format!("{year}年{month}月{day}日 {hour:02}:{minute:02}"),
        format!("{month}月{day}日 {period}{hour12}:{minute:02}"),
    ]
}

fn week_candidates(now: &LocalDateTime) -> Vec<String> {
    let weekday = weekday_index(now);
    let mut results = vec![WEEKDAYS[weekday].to_owned()];
    if weekday == 0 {
        results.push("星期天".to_owned());
    }
    results.push(ENGLISH_FULL_WEEKDAYS[weekday].to_owned());
    results.push(ENGLISH_WEEKDAYS[weekday].to_owned());
    results
}

/// 农历日期，再加带星期的两种写法；日历换算不了的日期（超出 lunar-lite 覆盖范围或零时钟）没有任何行，和 `rq` 列表丢掉农历行的做法一致。
fn lunar_candidates(now: &LocalDateTime) -> Vec<String> {
    let Some(lunar) = lunar_date(now) else {
        return Vec::new();
    };
    let weekday = weekday_index(now);
    let full = format!("{lunar} {}", WEEKDAYS[weekday]);
    let short = format!("{lunar} {}", SHORT_WEEKDAYS[weekday]);
    vec![lunar, full, short]
}

pub(crate) fn year_digits(value: u32) -> String {
    value
        .to_string()
        .bytes()
        .map(|digit| YEAR_DIGITS[usize::from(digit - b'0')])
        .collect()
}

/// 0 is the empty string, 10 is 十, 11-19 are 十X, other multiples of ten X十, the rest X十Y (date_time_query.cpp:58-78). Only month, day, hour and minute values reach it.
pub(crate) fn chinese_number(value: u32) -> String {
    let digit = |index: u32| NUMBER_DIGITS[index as usize];
    match value {
        0..=9 => digit(value).to_owned(),
        10 => "十".to_owned(),
        11..=19 => format!("十{}", digit(value - 10)),
        _ if value.is_multiple_of(10) => format!("{}十", digit(value / 10 % 10)),
        _ => format!("{}十{}", digit(value / 10 % 10), digit(value % 10)),
    }
}

fn financial_digits(value: u32, minimum_digits: usize) -> String {
    format!("{value:0minimum_digits$}")
        .bytes()
        .map(|digit| FINANCIAL_DIGITS[usize::from(digit - b'0')])
        .collect()
}

/// `<stem><branch>年[闰]<month>月<day>日` of the lunar year, or `None` outside what the calendar covers (lunar years 1850..=2150) or for an impossible date such as the zero clock.
pub(crate) fn lunar_date(now: &LocalDateTime) -> Option<String> {
    let solar = SolarDate {
        year: now.year,
        month: u8::try_from(now.month).ok()?,
        day: u8::try_from(now.day).ok()?,
    };
    let lunar = solar_to_lunar(solar).ok()?;
    let cycle = lunar.year - 4;
    Some(format!(
        "{}{}年{}{}月{}日",
        STEMS[cycle.rem_euclid(10) as usize],
        BRANCHES[cycle.rem_euclid(12) as usize],
        if lunar.is_leap_month { "闰" } else { "" },
        chinese_number(u32::from(lunar.month)),
        chinese_number(u32::from(lunar.day))
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_time() -> LocalDateTime {
        LocalDateTime {
            year: 2026,
            month: 8,
            day: 9,
            weekday: 0,
            hour: 14,
            minute: 30,
            second: 0,
        }
    }

    fn at(year: i32, month: u32, day: u32, weekday: u32) -> LocalDateTime {
        LocalDateTime {
            year,
            month,
            day,
            weekday,
            hour: 9,
            minute: 5,
            second: 7,
        }
    }

    #[track_caller]
    fn assert_words(actual: &[WordItem], expected: &[&str]) {
        let words: Vec<&str> = actual.iter().map(|row| row.word.as_str()).collect();
        assert_eq!(words, expected);
        assert_generated(actual, expected.len());
    }

    /// No blank row, Generated, and a contiguous descending weight run (test_local_modes.cpp:94-106).
    #[track_caller]
    fn assert_generated(actual: &[WordItem], expected_len: usize) {
        assert_eq!(actual.len(), expected_len);
        for (index, row) in actual.iter().enumerate() {
            assert!(!row.word.is_empty());
            assert!(row.pinyin.is_empty());
            assert_eq!(row.source, CandidateSource::Generated);
            assert_eq!(row.weight, (actual.len() - index) as i64);
        }
    }

    /// test_local_modes.cpp:113-138.
    #[test]
    fn date_aliases_in_windows_order() {
        let expected = [
            "2026年8月9日",
            "2026-08-09",
            "2026/08/09",
            "2026.08.09",
            "20260809",
            "26年8月9日",
            "8月9日",
            "08-09",
            "0809",
            "2026年8月9日 星期日",
            "8月9日 周日",
            "2026-08-09 Sun",
            "2026-08-09 14:30",
            "8月9日 14:30",
            "二〇二六年八月九日",
            "贰零贰陆年捌月零玖日",
            "丙午年六月二十七日",
        ];
        for keyword in ["rq", "riqi", "date"] {
            assert_words(&query_date_time(keyword, &sample_time()), &expected);
        }
    }

    /// test_local_modes.cpp:140-160.
    #[test]
    fn time_aliases_in_windows_order() {
        let expected = [
            "14:30",
            "14:30:00",
            "1430",
            "143000",
            "下午2:30",
            "下午2点30分",
            "下午两点半",
            "2:30 PM",
            "2:30pm",
            "02:30:00 PM",
            "2026-08-09 14:30:00",
            "2026年8月9日 14:30",
            "8月9日 下午2:30",
        ];
        for keyword in ["sj", "shijian", "time"] {
            assert_words(&query_date_time(keyword, &sample_time()), &expected);
        }
    }

    #[test]
    fn morning_and_colloquial_times() {
        let mut now = sample_time();
        now.hour = 0;
        now.minute = 0;
        now.second = 5;
        let rows = query_date_time("time", &now);
        let words: Vec<&str> = rows.iter().map(|row| row.word.as_str()).collect();
        assert_eq!(words[4], "上午12:00");
        assert_eq!(words[6], "上午十二点");
        assert_eq!(words[7], "12:00 AM");
        assert_eq!(words[8], "12:00am");
        assert_eq!(words[9], "12:00:05 AM");

        now.hour = 2;
        now.minute = 45;
        let rows = query_date_time("time", &now);
        assert_eq!(rows[6].word, "上午两点四十五分");
        now.hour = 23;
        now.minute = 11;
        let rows = query_date_time("time", &now);
        assert_eq!(rows[6].word, "下午十一点十一分");
        assert_eq!(rows[9].word, "11:11:05 PM");
    }

    /// test_local_modes.cpp:162-173.
    #[test]
    fn weekday_aliases() {
        for keyword in ["xq", "xingqi", "week"] {
            assert_words(
                &query_date_time(keyword, &sample_time()),
                &["星期日", "星期天", "Sunday", "Sun"],
            );
        }
        let mut monday = sample_time();
        monday.weekday = 1;
        assert_words(
            &query_date_time("week", &monday),
            &["星期一", "Monday", "Mon"],
        );
        let mut out_of_range = sample_time();
        out_of_range.weekday = 9;
        assert_words(
            &query_date_time("xq", &out_of_range),
            &["星期六", "Saturday", "Sat"],
        );
    }

    #[test]
    fn lunar_aliases_with_weekday_variants() {
        for keyword in ["nl", "nongli", "yinli"] {
            assert_words(
                &query_date_time(keyword, &sample_time()),
                &[
                    "丙午年六月二十七日",
                    "丙午年六月二十七日 星期日",
                    "丙午年六月二十七日 周日",
                ],
            );
        }
        // 闰月照样标出，星期取注入时钟的那一天。
        assert_words(
            &query_date_time("nongli", &at(2023, 3, 22, 3)),
            &[
                "癸卯年闰二月一日",
                "癸卯年闰二月一日 星期三",
                "癸卯年闰二月一日 周三",
            ],
        );
        assert_words(
            &query_date_time_with_limit("nl", &sample_time(), 1),
            &["丙午年六月二十七日"],
        );
        // 日历换算不了的日期不给出空行或只有星期的行。
        assert!(query_date_time("nl", &at(2200, 6, 15, 0)).is_empty());
        assert!(query_date_time("yinli", &LocalDateTime::default()).is_empty());
        assert!(query_date_time("NL", &sample_time()).is_empty());
    }

    /// test_local_modes.cpp:175-183.
    #[test]
    fn keywords_and_limits() {
        assert_words(
            &query_date_time("week", &sample_time()),
            &["星期日", "星期天", "Sunday", "Sun"],
        );
        assert!(query_date_time("today", &sample_time()).is_empty());
        assert!(query_date_time("RQ", &sample_time()).is_empty());
        assert!(query_date_time("", &sample_time()).is_empty());
        assert!(query_date_time_with_limit("rq", &sample_time(), 0).is_empty());
        let limited = query_date_time_with_limit("rq", &sample_time(), 3);
        assert_words(&limited, &["2026年8月9日", "2026-08-09", "2026/08/09"]);
    }

    /// test_local_modes.cpp:185-215, moved to lunar-lite's range: its table covers lunar years 1850..=2150 where the reference's ended at 2101-01-28, so the dropped-row cases use dates past that range instead.
    #[test]
    fn lunar_row_only_when_the_calendar_resolves() {
        assert_generated(&query_date_time("rq", &at(2101, 1, 28, 5)), 17);
        assert_generated(&query_date_time("rq", &at(2101, 1, 29, 6)), 17);
        assert_generated(&query_date_time("date", &at(1900, 1, 1, 1)), 17);

        assert_generated(&query_date_time("date", &at(1800, 1, 1, 3)), 16);
        let far_future = query_date_time("riqi", &at(2200, 6, 15, 0));
        assert_generated(&far_future, 16);
        assert_eq!(far_future.last().unwrap().word, "贰贰零零年陆月壹伍日");
        assert_eq!(far_future.last().unwrap().weight, 1);

        assert_generated(&query_date_time("rq", &at(2023, 2, 30, 0)), 16);
    }

    /// A clock that could not be read is all zeros; every keyword still gives well-formed rows (test_local_modes.cpp:217-226).
    #[test]
    fn unreadable_clock() {
        let zero = LocalDateTime::default();
        assert_generated(&query_date_time("rq", &zero), 16);
        assert_generated(&query_date_time("sj", &zero), 13);
        assert_generated(&query_date_time("xq", &zero), 4);
        let dates = query_date_time("rq", &zero);
        assert_eq!(dates[0].word, "0年0月0日");
        assert_eq!(dates[1].word, "0000-00-00");
        assert_eq!(dates[14].word, "〇年月日");
        assert_eq!(dates[15].word, "零年零月零零日");
    }

    #[test]
    fn leap_months() {
        // 2023-03-22 is the first day of the leap second month of 癸卯.
        let rows = query_date_time("rq", &at(2023, 3, 22, 3));
        assert_eq!(rows.last().unwrap().word, "癸卯年闰二月一日");
        assert_eq!(
            lunar_date(&at(2023, 1, 21, 6)).unwrap(),
            "壬寅年十二月三十日"
        );
        assert_eq!(lunar_date(&at(2023, 1, 22, 0)).unwrap(), "癸卯年一月一日");
        // GB/T 33661-2017 puts the 2033 leap month after the eleventh; the reference table had the older 闰七月.
        assert_eq!(lunar_date(&at(2033, 8, 25, 4)).unwrap(), "癸丑年八月一日");
        assert_eq!(
            lunar_date(&at(2033, 12, 22, 4)).unwrap(),
            "癸丑年闰十一月一日"
        );
    }

    #[test]
    fn chinese_numbers() {
        let cases = [
            (0, ""),
            (1, "一"),
            (9, "九"),
            (10, "十"),
            (11, "十一"),
            (19, "十九"),
            (20, "二十"),
            (21, "二十一"),
            (30, "三十"),
            (59, "五十九"),
        ];
        for (value, expected) in cases {
            assert_eq!(chinese_number(value), expected, "{value}");
        }
        assert_eq!(financial_digits(7, 2), "零柒");
        assert_eq!(financial_digits(12, 2), "壹贰");
        assert_eq!(year_digits(2006), "二〇〇六");
    }

    #[test]
    fn wall_clock_is_a_valid_date() {
        let now = current_local_date_time();
        assert!((1..=12).contains(&now.month));
        assert!((1..=31).contains(&now.day));
        assert!(now.weekday <= 6);
        assert!(now.hour <= 23 && now.minute <= 59 && now.second <= 60);
    }

    // ---- the reference's lunar table, kept only to pin down where lunar-lite answers differently ----

    const REFERENCE_LUNAR_YEAR_INFO: [u32; 201] = [
        0x04bd8, 0x04ae0, 0x0a570, 0x054d5, 0x0d260, 0x0d950, 0x16554, 0x056a0, 0x09ad0, 0x055d2,
        0x04ae0, 0x0a5b6, 0x0a4d0, 0x0d250, 0x1d255, 0x0b540, 0x0d6a0, 0x0ada2, 0x095b0, 0x14977,
        0x04970, 0x0a4b0, 0x0b4b5, 0x06a50, 0x06d40, 0x1ab54, 0x02b60, 0x09570, 0x052f2, 0x04970,
        0x06566, 0x0d4a0, 0x0ea50, 0x06e95, 0x05ad0, 0x02b60, 0x186e3, 0x092e0, 0x1c8d7, 0x0c950,
        0x0d4a0, 0x1d8a6, 0x0b550, 0x056a0, 0x1a5b4, 0x025d0, 0x092d0, 0x0d2b2, 0x0a950, 0x0b557,
        0x06ca0, 0x0b550, 0x15355, 0x04da0, 0x0a5b0, 0x14573, 0x052b0, 0x0a9a8, 0x0e950, 0x06aa0,
        0x0aea6, 0x0ab50, 0x04b60, 0x0aae4, 0x0a570, 0x05260, 0x0f263, 0x0d950, 0x05b57, 0x056a0,
        0x096d0, 0x04dd5, 0x04ad0, 0x0a4d0, 0x0d4d4, 0x0d250, 0x0d558, 0x0b540, 0x0b6a0, 0x195a6,
        0x095b0, 0x049b0, 0x0a974, 0x0a4b0, 0x0b27a, 0x06a50, 0x06d40, 0x0af46, 0x0ab60, 0x09570,
        0x04af5, 0x04970, 0x064b0, 0x074a3, 0x0ea50, 0x06b58, 0x055c0, 0x0ab60, 0x096d5, 0x092e0,
        0x0c960, 0x0d954, 0x0d4a0, 0x0da50, 0x07552, 0x056a0, 0x0abb7, 0x025d0, 0x092d0, 0x0cab5,
        0x0a950, 0x0b4a0, 0x0baa4, 0x0ad50, 0x055d9, 0x04ba0, 0x0a5b0, 0x15176, 0x052b0, 0x0a930,
        0x07954, 0x06aa0, 0x0ad50, 0x05b52, 0x04b60, 0x0a6e6, 0x0a4e0, 0x0d260, 0x0ea65, 0x0d530,
        0x05aa0, 0x076a3, 0x096d0, 0x04bd7, 0x04ad0, 0x0a4d0, 0x1d0b6, 0x0d250, 0x0d520, 0x0dd45,
        0x0b5a0, 0x056d0, 0x055b2, 0x049b0, 0x0a577, 0x0a4b0, 0x0aa50, 0x1b255, 0x06d20, 0x0ada0,
        0x14b63, 0x09370, 0x049f8, 0x04970, 0x064b0, 0x168a6, 0x0ea50, 0x06b20, 0x1a6c4, 0x0aae0,
        0x092e0, 0x0d2e3, 0x0c960, 0x0d557, 0x0d4a0, 0x0da50, 0x05d55, 0x056a0, 0x0a6d0, 0x055d4,
        0x052d0, 0x0a9b8, 0x0a950, 0x0b4a0, 0x0b6a6, 0x0ad50, 0x055a0, 0x0aba4, 0x0a5b0, 0x052b0,
        0x0b273, 0x06930, 0x07337, 0x06aa0, 0x0ad50, 0x14b55, 0x04b60, 0x0a570, 0x054e4, 0x0d260,
        0x0e968, 0x0d520, 0x0daa0, 0x16aa6, 0x056d0, 0x04ae0, 0x0a9d4, 0x0a4d0, 0x0d150, 0x0f252,
        0x0d520,
    ];

    /// (year, month, day, leap) for `days` after 1900-01-31 by the reference's walk (date_time_query.cpp:188-239).
    fn reference_lunar(mut days: u32) -> Option<(i32, u32, u32, bool)> {
        let info = |year: u32| REFERENCE_LUNAR_YEAR_INFO[(year - 1900) as usize];
        let leap_month = |year: u32| info(year) & 0xf;
        let leap_days = |year: u32| match leap_month(year) {
            0 => 0,
            _ if info(year) & 0x10000 != 0 => 30,
            _ => 29,
        };
        let month_days = |year: u32, month: u32| {
            if info(year) & (0x10000 >> month) != 0 {
                30
            } else {
                29
            }
        };
        let year_days = |year: u32| {
            348 + leap_days(year)
                + (4..16).filter(|bit| info(year) & (1 << bit) != 0).count() as u32
        };
        let mut year = 1900;
        while year <= 2100 && days >= year_days(year) {
            days -= year_days(year);
            year += 1;
        }
        if year > 2100 {
            return None;
        }
        let mut month = 1;
        let mut leap = false;
        while month <= 12 {
            if days < month_days(year, month) {
                break;
            }
            days -= month_days(year, month);
            if leap_month(year) == month {
                if days < leap_days(year) {
                    leap = true;
                    break;
                }
                days -= leap_days(year);
            }
            month += 1;
        }
        Some((year as i32, month, days + 1, leap))
    }

    /// Every day the reference table covered agrees except six month boundaries where the table disagrees with the calendar lunar-lite was generated from: it makes the 1933 leap fifth month and the 1996 fifth and seventh months 29 days where lunar-lite has 30, it carries the old 2033 闰七月 instead of GB/T 33661-2017's 闰十一月, and it puts the 2089 and 2097 new moons that begin the eighth and seventh months a day late. Any other difference would be a regression the lunar row would show users.
    #[test]
    fn lunar_lite_matches_the_reference_table() {
        let epoch = time::Date::from_calendar_date(1900, time::Month::January, 31).unwrap();
        let mut date = epoch;
        let mut days = 0;
        let mut mismatches = Vec::new();
        while let Some((year, month, day, leap)) = reference_lunar(days) {
            let lunar = solar_to_lunar(SolarDate {
                year: date.year(),
                month: u8::from(date.month()),
                day: date.day(),
            })
            .unwrap();
            let actual = (
                lunar.year,
                u32::from(lunar.month),
                u32::from(lunar.day),
                lunar.is_leap_month,
            );
            if actual != (year, month, day, leap) {
                mismatches.push(date);
            }
            date = date.next_day().unwrap();
            days += 1;
        }
        assert_eq!(
            date,
            time::Date::from_calendar_date(2101, time::Month::January, 29).unwrap()
        );
        // Runs of consecutive disagreeing days, as (first day, length).
        let mut runs: Vec<(time::Date, usize)> = Vec::new();
        for date in mismatches {
            match runs.last_mut() {
                Some((first, length))
                    if first.checked_add(time::Duration::days(*length as i64)) == Some(date) =>
                {
                    *length += 1
                }
                _ => runs.push((date, 1)),
            }
        }
        let day = |year, month, day| time::Date::from_calendar_date(year, month, day).unwrap();
        use time::Month::{August, July, September};
        assert_eq!(
            runs,
            [
                (day(1933, July, 22), 30),
                (day(1996, July, 15), 30),
                (day(1996, September, 12), 30),
                (day(2033, August, 25), 148),
                (day(2089, September, 4), 30),
                (day(2097, August, 7), 30),
            ]
        );
    }
}
