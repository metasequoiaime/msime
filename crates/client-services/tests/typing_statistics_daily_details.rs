//! 按日明细经真实共享库派生，历史未知值与已测量的零值分开保留。

use msime_client_services::typing_statistics::{
    summarize, SummaryInputs, TypingBreakdown, TypingStatistics,
};
use std::collections::BTreeMap;

#[test]
fn daily_details_preserve_unknown_time_and_fold_unclassified_characters() {
    let mut value = TypingStatistics {
        days: BTreeMap::from([
            ("2026-11-14".into(), 10),
            ("2026-11-15".into(), 8),
            ("2026-11-16".into(), 20),
            ("2026-11-17".into(), 99),
        ]),
        ..TypingStatistics::default()
    };
    value.daily_details.insert(
        "2026-11-16".into(),
        TypingBreakdown {
            characters: BTreeMap::from([
                ("han".into(), 2),
                ("latin".into(), 3),
                ("number".into(), 4),
                ("punctuation".into(), 1),
                ("otherLetter".into(), 2),
                ("emoji".into(), 1),
            ]),
            sources: BTreeMap::new(),
        },
    );
    value.daily_active_ms.insert("2026-11-15".into(), 0);
    value.daily_active_ms.insert("2026-11-16".into(), 30_000);
    let summary = summarize(&value, "2026-11-16", &SummaryInputs::default());
    let rows = &summary.daily_details;
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].day, "2026-11-16");
    assert_eq!(rows[0].total, 20);
    assert_eq!((rows[0].han, rows[0].latin), (2, 3));
    assert_eq!((rows[0].number, rows[0].punctuation), (4, 1));
    assert_eq!(rows[0].other, 10);
    assert_eq!(rows[0].active_ms, Some(30_000));
    assert_eq!(rows[0].speed, Some(14.0));
    assert_eq!(rows[1].other, 8);
    assert_eq!(rows[1].active_ms, Some(0));
    assert_eq!(rows[1].speed, Some(0.0));
    assert_eq!(rows[2].other, 10);
    assert_eq!(rows[2].active_ms, None);
    assert_eq!(rows[2].speed, None);
    let json = serde_json::to_value(&summary).unwrap();
    assert!(json["daily_details"][2]["active_ms"].is_null());
    assert_eq!(json["daily_details"][1]["speed"], 0.0);
}

#[test]
fn daily_details_select_thirty_recorded_days_instead_of_thirty_calendar_days() {
    let mut value = TypingStatistics::default();
    for day in 1..=31 {
        value.days.insert(format!("2026-10-{day:02}"), 1);
    }
    value.days.insert("2026-11-16".into(), 2);
    value.days.insert("2026-11-18".into(), 3);
    let rows = summarize(&value, "2026-11-17", &SummaryInputs::default()).daily_details;
    assert_eq!(rows.len(), 30);
    assert_eq!(rows.first().unwrap().day, "2026-11-16");
    assert_eq!(rows.last().unwrap().day, "2026-10-03");
    assert!(summarize(&value, "2026-02-30", &SummaryInputs::default())
        .daily_details
        .is_empty());
}
