//! 按日明细的共享派生模型；宿主只显示结果，不另算分类和速度。

use super::TypingStatistics;
use serde::Serialize;

const DETAIL_DAYS: usize = 30;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DailyDetailRow {
    pub day: String,
    pub total: u64,
    pub han: u64,
    pub latin: u64,
    pub number: u64,
    pub punctuation: u64,
    /// 其他文字、表情、符号与历史未分类字符。
    pub other: u64,
    /// 测量引入前的日期为 `None`，不伪装成已测量的零。
    pub active_ms: Option<u64>,
    pub speed: Option<f64>,
}

pub(super) fn daily_details(statistics: &TypingStatistics, today: &str) -> Vec<DailyDetailRow> {
    if !crate::calendar::is_valid_day(today) {
        return Vec::new();
    }
    statistics
        .days
        .iter()
        .rev()
        .filter(|(day, _)| day.as_str() <= today && crate::calendar::is_valid_day(day))
        .take(DETAIL_DAYS)
        .map(|(day, total)| {
            let detail = statistics
                .daily_details
                .get(day)
                .cloned()
                .unwrap_or_default()
                .including_unclassified(*total);
            let count = |kind: &str| detail.characters.get(kind).copied().unwrap_or(0);
            let active_ms = statistics.active_ms(day);
            let readable = count("han")
                .saturating_add(count("latin"))
                .saturating_add(count("otherLetter"));
            DailyDetailRow {
                day: day.clone(),
                total: *total,
                han: count("han"),
                latin: count("latin"),
                number: count("number"),
                punctuation: count("punctuation"),
                other: detail
                    .characters
                    .iter()
                    .filter(|(kind, _)| {
                        !matches!(kind.as_str(), "han" | "latin" | "number" | "punctuation")
                    })
                    .fold(0_u64, |sum, (_, count)| sum.saturating_add(*count)),
                active_ms,
                speed: active_ms.map(|ms| {
                    if ms == 0 {
                        0.0
                    } else {
                        readable as f64 / (ms as f64 / 60_000.0)
                    }
                }),
            }
        })
        .collect()
}
