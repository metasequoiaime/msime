//! 派生指标的边界，以及和桌面设置页 TS 实现的一致性用例。

use super::super::{CommitEfficiency, SelectionCounts, TypingBreakdown, TypingRun};
use super::*;
use serde_json::{json, Value};

/// 往 `statistics` 里加一天：`han`、`latin`、`digits` 三类字符，来源都记成 `source`，可选的活跃时间和逐小时分布。
fn add_day(
    statistics: &mut TypingStatistics,
    day: &str,
    counts: (u64, u64, u64),
    source: &str,
    active_ms: Option<u64>,
    hours: &[(usize, u64)],
) {
    let (han, latin, digits) = counts;
    let count = han + latin + digits;
    statistics.total += count;
    statistics.days.insert(day.into(), count);
    let mut characters = BTreeMap::new();
    for (kind, value) in [("han", han), ("latin", latin), ("number", digits)] {
        if value > 0 {
            characters.insert(kind.to_owned(), value);
        }
    }
    let detail = TypingBreakdown {
        characters,
        sources: [(source.to_owned(), count)].into(),
    };
    for (kind, value) in &detail.characters {
        *statistics
            .detail
            .characters
            .entry(kind.clone())
            .or_default() += value;
    }
    *statistics
        .detail
        .sources
        .entry(source.to_owned())
        .or_default() += count;
    statistics.daily_details.insert(day.into(), detail);
    if let Some(active_ms) = active_ms {
        statistics.daily_active_ms.insert(day.into(), active_ms);
    }
    if !hours.is_empty() {
        let mut buckets = vec![0; HOURS];
        for (hour, value) in hours {
            buckets[*hour] += value;
        }
        statistics.daily_hours.insert(day.into(), buckets);
    }
}

fn enabled() -> TypingStatistics {
    TypingStatistics {
        enabled: true,
        ..TypingStatistics::default()
    }
}

fn keys(entries: &[(&str, u64)]) -> BTreeMap<String, u64> {
    entries
        .iter()
        .map(|(key, count)| ((*key).to_owned(), *count))
        .collect()
}

fn achievement<'a>(summary: &'a TypingSummary, id: &str) -> &'a AchievementSummary {
    summary
        .achievements
        .iter()
        .find(|achievement| achievement.id == id)
        .unwrap()
}

#[test]
fn an_empty_document_has_no_rates_and_nothing_unlocked() {
    let summary = summarize(&enabled(), "2026-10-05", &SummaryInputs::default());
    let overview = &summary.overview;
    assert_eq!(overview.week_total, 0);
    assert_eq!(overview.last7.len(), 7);
    assert_eq!(overview.last7[0].day, "2026-09-29");
    assert_eq!(overview.last7[6].day, "2026-10-05");
    assert_eq!(overview.average_speed, None);
    assert_eq!(overview.previous_average_speed, None);
    assert_eq!(overview.first_candidate_rate, None);
    assert_eq!(overview.keystrokes_saved_rate, None);
    assert_eq!(overview.current_streak, 0);
    assert_eq!(overview.longest_streak, 0);
    let habits = &summary.habits;
    assert_eq!(habits.weeks12.len(), 84);
    assert_eq!(habits.weeks12[0].day, "2026-07-14");
    assert_eq!(habits.hours24, vec![0; HOURS]);
    assert_eq!(habits.usual_hours, None);
    assert_eq!(habits.peak_window, None);
    assert_eq!(habits.active_days, 0);
    let keys = &summary.keys;
    assert_eq!(keys.per_character_keys, None);
    assert_eq!(keys.backspace_rate, None);
    assert_eq!(keys.prediction_rate, None);
    assert_eq!(keys.longest_run, None);
    assert_eq!(keys.positions, None);
    assert_eq!(summary.achievements.len(), 16);
    assert!(summary
        .achievements
        .iter()
        .all(|achievement| achievement.unlocked_day.is_none() && achievement.current == 0));
}

#[test]
fn an_invalid_today_leaves_every_window_empty() {
    let summary = summarize(&enabled(), "not-a-day", &SummaryInputs::default());
    assert!(summary.overview.last7.is_empty());
    assert!(summary.habits.weeks12.is_empty());
}

#[test]
fn speed_is_null_without_active_time_and_counts_prose_only() {
    let mut statistics = enabled();
    add_day(
        &mut statistics,
        "2026-10-04",
        (50, 10, 40),
        "quanpin",
        None,
        &[],
    );
    let summary = summarize(&statistics, "2026-10-05", &SummaryInputs::default());
    assert_eq!(summary.overview.average_speed, None);
    statistics
        .daily_active_ms
        .insert("2026-10-04".into(), 30_000);
    let summary = summarize(&statistics, "2026-10-05", &SummaryInputs::default());
    // 数字不算可读字符：60 个字用了半分钟。
    assert_eq!(summary.overview.average_speed, Some(120.0));
    assert_eq!(summary.overview.previous_average_speed, None);
    assert_eq!(summary.overview.week_total, 100);
}

#[test]
fn first_candidate_rate_needs_fifty_selections() {
    let mut statistics = enabled();
    statistics.selections = SelectionCounts {
        ranks: vec![40, 9, 0, 0, 0, 0, 0, 0, 0],
        beyond: 0,
    };
    let summary = summarize(&statistics, "2026-10-05", &SummaryInputs::default());
    assert_eq!(summary.overview.first_candidate_rate, None);
    assert!(summary.keys.positions.is_some());
    statistics.selections.beyond = 1;
    let summary = summarize(&statistics, "2026-10-05", &SummaryInputs::default());
    assert_eq!(summary.overview.first_candidate_rate, Some(0.8));
    assert_eq!(summary.keys.positions, Some([0.8, 0.18, 0.0, 0.02]));
}

#[test]
fn efficiency_rates_follow_the_commit_counts() {
    let mut statistics = enabled();
    statistics.efficiency = CommitEfficiency {
        commits: 10,
        typed_keys: 60,
        spelled_keys: 100,
        sentence_commits: 3,
        prediction_commits: 4,
    };
    let summary = summarize(&statistics, "2026-10-05", &SummaryInputs::default());
    assert_eq!(summary.overview.keystrokes_saved_rate, Some(0.4));
    assert_eq!(summary.keys.prediction_rate, Some(0.4));
    // 输入码比全拼还长时不出现负的「少按键」。
    statistics.efficiency.typed_keys = 150;
    let summary = summarize(&statistics, "2026-10-05", &SummaryInputs::default());
    assert_eq!(summary.overview.keystrokes_saved_rate, Some(0.0));
}

#[test]
fn key_metrics_use_the_last_seven_days() {
    let mut statistics = enabled();
    add_day(
        &mut statistics,
        "2026-10-05",
        (10, 0, 0),
        "quanpin",
        None,
        &[],
    );
    add_day(
        &mut statistics,
        "2026-09-25",
        (5, 0, 0),
        "quanpin",
        None,
        &[],
    );
    statistics.daily_keys.insert(
        "2026-10-05".into(),
        keys(&[("KeyN", 12), ("Nine4", 8), ("Space", 6), ("Backspace", 4)]),
    );
    // 只有按键、没有上屏的一天也算进分母。
    statistics
        .daily_keys
        .insert("2026-10-01".into(), keys(&[("Backspace", 10)]));
    statistics
        .daily_keys
        .insert("2026-09-25".into(), keys(&[("KeyA", 15)]));
    let summary = summarize(&statistics, "2026-10-05", &SummaryInputs::default());
    assert_eq!(summary.keys.per_character_keys, Some(2.0));
    let mut without_keys = statistics.clone();
    without_keys.daily_keys.clear();
    let summary_without_keys = summarize(&without_keys, "2026-10-05", &SummaryInputs::default());
    assert_eq!(summary_without_keys.keys.per_character_keys, None);
    assert_eq!(summary.keys.previous_per_character_keys, Some(3.0));
    assert_eq!(summary.keys.backspace_rate, Some(14.0 / 40.0));
}

#[test]
fn hours_take_the_last_week_and_the_peak_may_wrap_midnight() {
    let mut statistics = enabled();
    add_day(
        &mut statistics,
        "2026-10-04",
        (30, 0, 0),
        "quanpin",
        None,
        &[(23, 10), (0, 15)],
    );
    add_day(
        &mut statistics,
        "2026-10-05",
        (8, 0, 0),
        "quanpin",
        None,
        &[(9, 8)],
    );
    add_day(
        &mut statistics,
        "2026-09-01",
        (40, 0, 0),
        "quanpin",
        None,
        &[(12, 40)],
    );
    let summary = summarize(&statistics, "2026-10-05", &SummaryInputs::default());
    assert_eq!(summary.habits.hours24[23], 10);
    assert_eq!(summary.habits.hours24[0], 15);
    assert_eq!(summary.habits.hours24[12], 0);
    assert_eq!(
        summary.habits.peak_window,
        Some(PeakWindow { start: 23, end: 1 })
    );
    // 常用时段不含今天，按有记录的天平均。
    let usual = summary.habits.usual_hours.unwrap();
    assert_eq!(usual[12], 20.0);
    assert_eq!(usual[23], 5.0);
    assert_eq!(usual[9], 0.0);
}

#[test]
fn badges_unlock_on_their_thresholds() {
    let mut statistics = enabled();
    add_day(
        &mut statistics,
        "2026-10-05",
        (10_000, 0, 0),
        "shuangpin",
        Some(600_000),
        &[(4, 1), (23, 600), (2, 400)],
    );
    statistics.selections.ranks = vec![451, 49, 0, 0, 0, 0, 0, 0, 0];
    statistics.efficiency = CommitEfficiency {
        commits: 1_000,
        sentence_commits: 1_000,
        ..CommitEfficiency::default()
    };
    statistics
        .daily_voice_ms
        .insert("2026-10-05".into(), 3_600_000);
    statistics.skins_tried = (0..5).map(|index| format!("skin{index}")).collect();
    statistics.longest_run = TypingRun {
        characters: 86,
        day: "2026-10-05".into(),
    };
    let summary = summarize(
        &statistics,
        "2026-10-05",
        &SummaryInputs {
            user_words: Some(50),
        },
    );
    let unlocked: Vec<&str> = summary
        .achievements
        .iter()
        .filter(|achievement| achievement.unlocked_day.is_some())
        .map(|achievement| achievement.id)
        .collect();
    assert_eq!(
        unlocked,
        [
            "chars_10k",
            "speed_60",
            "accuracy_90",
            "sentence_1000",
            "night_owl",
            "early_bird",
            "shuangpin_10k",
            "voice_1h",
            "words_50",
            "skins_5",
        ]
    );
    assert_eq!(achievement(&summary, "speed_60").current, 1_000);
    assert_eq!(achievement(&summary, "accuracy_90").current, 90);
    assert_eq!(achievement(&summary, "chars_100k").current, 10_000);
    assert_eq!(achievement(&summary, "chars_100k").target, 100_000);
    assert_eq!(summary.keys.longest_run.as_ref().unwrap().characters, 86);

    // 刚好 90% 不算「超 90%」，样本不足 500 也不算。
    statistics.selections.ranks = vec![450, 50, 0, 0, 0, 0, 0, 0, 0];
    let summary = summarize(&statistics, "2026-10-05", &SummaryInputs::default());
    assert_eq!(achievement(&summary, "accuracy_90").unlocked_day, None);
    statistics.selections.ranks = vec![450, 0, 0, 0, 0, 0, 0, 0, 0];
    let summary = summarize(&statistics, "2026-10-05", &SummaryInputs::default());
    assert_eq!(achievement(&summary, "accuracy_90").current, 0);
    assert_eq!(achievement(&summary, "accuracy_90").unlocked_day, None);
    // 快手要求至少 10 分钟活跃时间。
    statistics
        .daily_active_ms
        .insert("2026-10-05".into(), 599_999);
    let summary = summarize(&statistics, "2026-10-05", &SummaryInputs::default());
    assert_eq!(achievement(&summary, "speed_60").unlocked_day, None);
}

#[test]
fn a_stored_unlock_wins_over_the_current_numbers() {
    let mut statistics = enabled();
    statistics
        .achievements
        .insert("chars_1m".into(), "2025-01-01".into());
    let summary = summarize(&statistics, "2026-10-05", &SummaryInputs::default());
    let badge = achievement(&summary, "chars_1m");
    assert_eq!(badge.unlocked_day.as_deref(), Some("2025-01-01"));
    assert_eq!(badge.current, 0);
}

#[test]
fn every_badge_has_copy_and_a_known_group() {
    let specs = achievement_specs();
    assert_eq!(specs.len(), 16);
    let ids: std::collections::BTreeSet<&str> = specs.iter().map(|spec| spec.id).collect();
    assert_eq!(ids.len(), 16);
    for spec in specs {
        assert!(!spec.title.is_empty() && !spec.description.is_empty() && !spec.glyph.is_empty());
        assert!(spec.target > 0);
        assert!(is_achievement_id(spec.id));
    }
    let groups = |group| specs.iter().filter(|spec| spec.group == group).count();
    assert_eq!(groups(AchievementGroup::Volume), 3);
    assert_eq!(groups(AchievementGroup::Streak), 5);
    assert_eq!(groups(AchievementGroup::Skill), 4);
    assert_eq!(groups(AchievementGroup::Fun), 4);
}

#[test]
fn streaks_cross_months_leap_days_and_years() {
    let recorded = [
        "2027-12-30",
        "2027-12-31",
        "2028-01-01",
        "2028-02-27",
        "2028-02-28",
        "2028-02-29",
        "2028-03-01",
    ];
    assert_eq!(longest_streak(&recorded), 4);
    assert_eq!(current_streak(&recorded, "2028-03-01"), 4);
    // 今天还没记录：截止到昨天。
    assert_eq!(current_streak(&recorded, "2028-03-02"), 4);
    assert_eq!(current_streak(&recorded, "2028-03-03"), 0);
    assert_eq!(current_streak(&recorded, "2028-01-01"), 3);
}

/// 一致性用例的输入文档：每组都是一个能通过 `validate` 的统计文件，加上宿主的今天和自定义词数。
fn parity_cases() -> Vec<(&'static str, &'static str, Option<u64>, TypingStatistics)> {
    let mut cases = Vec::new();
    cases.push(("an empty document", "2026-10-05", None, enabled()));

    let legacy: TypingStatistics = serde_json::from_str(
        r#"{"enabled":true,"total":45,"days":{"2026-09-20":10,"2026-09-21":20,"2026-10-04":15}}"#,
    )
    .unwrap();
    cases.push((
        "a document from before active time and hours were measured",
        "2026-10-05",
        None,
        legacy,
    ));

    let mut rich = enabled();
    for offset in 0..14_i64 {
        let day = crate::calendar::shift_day("2026-10-05", -offset).unwrap();
        let han = 300 + 37 * offset as u64;
        let latin = 20 + offset as u64 % 3;
        let digits = offset as u64 % 4;
        let active = (offset % 5 != 2).then_some(240_000 + 13_337 * offset as u64);
        let hours = [
            ((offset as usize * 5) % HOURS, han / 3),
            (21, han / 4),
            (22, han / 5),
        ];
        add_day(
            &mut rich,
            &day,
            (han, latin, digits),
            "quanpin",
            active,
            &hours,
        );
    }
    for (index, day) in ["2026-08-01", "2026-08-02", "2026-08-03", "2026-08-05"]
        .iter()
        .enumerate()
    {
        add_day(
            &mut rich,
            day,
            (70 + index as u64, 3, 0),
            "shuangpin",
            Some(61_000 * (index as u64 + 1)),
            &[(4, 2), (23, 30)],
        );
    }
    // 一整天的小时全为 0：常用时段不计这天。
    add_day(
        &mut rich,
        "2026-08-10",
        (5, 0, 0),
        "handwriting",
        Some(9_000),
        &[],
    );
    rich.daily_hours.insert("2026-08-10".into(), vec![0; HOURS]);
    rich.selections = SelectionCounts {
        ranks: vec![91, 16, 7, 2, 1, 0, 0, 0, 0],
        beyond: 3,
    };
    rich.efficiency = CommitEfficiency {
        commits: 420,
        typed_keys: 1_240,
        spelled_keys: 2_000,
        sentence_commits: 37,
        prediction_commits: 172,
    };
    rich.daily_keys.insert(
        "2026-10-05".into(),
        keys(&[
            ("KeyN", 120),
            ("KeyI", 80),
            ("Backspace", 21),
            ("Space", 40),
        ]),
    );
    rich.daily_keys
        .insert("2026-09-27".into(), keys(&[("Nine6", 50), ("Nine4", 30)]));
    rich.daily_voice_ms.insert("2026-09-30".into(), 1_234_567);
    rich.skins_tried = ["songlin", "zhuyu", "10000000-0000-4000-8000-000000000042"]
        .iter()
        .map(|id| (*id).to_owned())
        .collect();
    rich.longest_run = TypingRun {
        characters: 86,
        day: "2026-09-28".into(),
    };
    rich.current_run = TypingRun {
        characters: 12,
        day: "2026-10-05".into(),
    };
    rich.achievements
        .insert("chars_100k".into(), "2026-03-14".into());
    cases.push((
        "two recorded weeks ending today with older gaps",
        "2026-10-05",
        Some(64),
        rich,
    ));

    let mut leap = enabled();
    for (index, day) in [
        "2028-02-20",
        "2028-02-27",
        "2028-02-28",
        "2028-02-29",
        "2028-03-01",
        "2028-03-02",
    ]
    .iter()
    .enumerate()
    {
        let active = (index % 2 == 0).then_some(95_000 + 1_001 * index as u64);
        add_day(
            &mut leap,
            day,
            (123 + index as u64 * 11, 7, 1),
            "wubi",
            active,
            &[(index * 3, 50), (9, 3)],
        );
    }
    leap.selections.ranks = vec![30, 10, 5, 1, 0, 0, 0, 0, 0];
    cases.push((
        "today unrecorded after a run across a leap day",
        "2028-03-03",
        None,
        leap,
    ));

    let mut year = enabled();
    for (index, day) in ["2025-12-30", "2025-12-31", "2026-01-01", "2026-01-02"]
        .iter()
        .enumerate()
    {
        add_day(
            &mut year,
            day,
            (0, 40 + index as u64, 0),
            "english",
            Some(7_777 * (index as u64 + 1)),
            &[(23, 20)],
        );
    }
    cases.push(("a streak across new year", "2026-01-02", Some(3), year));
    cases
}

/// 桌面设置页的 TS 实现照着这份用例跑：`apps/desktop/tests/settings/typing-metrics-parity.test.ts` 断言 `activityMetrics` 的 `averageSpeed`、`currentStreak`、`longestStreak` 和 `usualHours` 与这里一致。改了算法或用例以后，用 `MSIME_WRITE_STATISTICS_CASES=1 cargo test -p msime-client-core typing_statistics` 重新生成文件，再用 `pnpm exec vp fmt packages/ui/src/settings/typing-metrics-cases.json` 按仓库的格式排版（比较只看内容，不看空白）。
#[test]
fn typing_metrics_cases_match_the_checked_in_copy() {
    let mut cases = Vec::new();
    for (name, today, user_words, statistics) in parity_cases() {
        statistics
            .validate()
            .expect("parity case is a valid document");
        let summary = summarize(&statistics, today, &SummaryInputs { user_words });
        let recorded: Vec<String> = statistics.days.keys().cloned().collect();
        cases.push(json!({
            "name": name,
            "today": today,
            "userWords": user_words,
            "statistics": statistics,
            "allTimeAverageSpeed": average_speed_over(&statistics, &recorded),
            "summary": summary,
        }));
    }
    let cases = Value::Array(cases);
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/ui/src/settings/typing-metrics-cases.json"
    );
    // 两边都从文本解析再比较：serde_json 默认的浮点解析不保证逐位还原 `f64`，但同样的数字文本总是解析成同样的值；这样也不受仓库格式化工具改动空白的影响。
    let text = serde_json::to_string_pretty(&cases).expect("cases print") + "\n";
    if std::env::var_os("MSIME_WRITE_STATISTICS_CASES").is_some() {
        std::fs::write(path, &text).expect("typing metrics cases are writable");
    }
    let expected: Value = serde_json::from_str(&text).expect("cases parse");
    let copy: Value = std::fs::read_to_string(path)
        .ok()
        .and_then(|copy| serde_json::from_str(&copy).ok())
        .expect("packages/ui/src/settings/typing-metrics-cases.json is missing; rerun this test with MSIME_WRITE_STATISTICS_CASES=1");
    assert!(
        copy == expected,
        "packages/ui/src/settings/typing-metrics-cases.json is stale; rerun this test with MSIME_WRITE_STATISTICS_CASES=1"
    );
}
