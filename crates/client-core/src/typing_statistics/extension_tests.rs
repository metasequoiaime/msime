//! 上屏效率、连续输入、语音时长、皮肤和徽章这几项新增数据的存储行为。

use super::*;

fn enabled_store() -> (tempfile::TempDir, TypingStatisticsStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = TypingStatisticsStore::new(directory.path());
    store.set_enabled(true).unwrap();
    (directory, store)
}

#[test]
fn a_document_written_before_the_new_fields_still_loads() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("typing-statistics.json"),
        r#"{"enabled":true,"total":3,"days":{"2026-09-20":3},"selections":{"ranks":[2,1],"beyond":0}}"#,
    )
    .unwrap();
    let value = TypingStatisticsStore::new(directory.path()).load().unwrap();
    assert_eq!(value.total, 3);
    assert_eq!(value.efficiency, CommitEfficiency::default());
    assert_eq!(value.current_run, TypingRun::default());
    assert_eq!(value.longest_run, TypingRun::default());
    assert!(value.daily_voice_ms.is_empty());
    assert!(value.skins_tried.is_empty());
    assert!(value.achievements.is_empty());
}

#[test]
fn the_new_fields_round_trip_in_camel_case() {
    let (directory, store) = enabled_store();
    store.record_voice("2026-09-20", 1_000).unwrap();
    store.record_skin("songlin").unwrap();
    let mut batch = CommitEfficiency::default();
    batch.count_commit(4, 8, true, false);
    store.record_efficiency(&batch).unwrap();
    store
        .record_at("你好", TypingSource::Quanpin, "2026-09-20", Some(9), 1_000)
        .unwrap();
    let text = std::fs::read_to_string(directory.path().join("typing-statistics.json")).unwrap();
    for key in [
        "\"efficiency\":{\"commits\":1,\"typedKeys\":4,\"spelledKeys\":8,\"sentenceCommits\":1,\"predictionCommits\":0}",
        "\"currentRun\":{\"characters\":2,\"day\":\"2026-09-20\"}",
        "\"longestRun\":{\"characters\":2,\"day\":\"2026-09-20\"}",
        "\"dailyVoiceMs\":{\"2026-09-20\":1000}",
        "\"skinsTried\":[\"songlin\"]",
    ] {
        assert!(text.contains(key), "{key} missing from {text}");
    }
}

#[test]
fn a_run_grows_while_commits_stay_within_the_active_gap() {
    let (_directory, store) = enabled_store();
    let day = "2026-09-28";
    store
        .record_at("你好", TypingSource::Quanpin, day, None, 1_000)
        .unwrap();
    store
        .record_at("世界", TypingSource::Quanpin, day, None, 3_000)
        .unwrap();
    store
        .record_at("再见", TypingSource::Quanpin, day, None, 13_000)
        .unwrap();
    let value = store.load().unwrap();
    // 10 秒整仍算活跃，和活跃时间用的是同一个界限。
    assert_eq!(value.current_run.characters, 6);
    assert_eq!(value.longest_run.characters, 6);
    assert_eq!(value.longest_run.day, day);
}

#[test]
fn a_pause_longer_than_ten_seconds_starts_a_new_run() {
    let (_directory, store) = enabled_store();
    store
        .record_at("一二三四", TypingSource::Quanpin, "2026-09-28", None, 1_000)
        .unwrap();
    store
        .record_at("五", TypingSource::Quanpin, "2026-09-29", None, 11_001)
        .unwrap();
    let value = store.load().unwrap();
    assert_eq!(
        value.current_run,
        TypingRun {
            characters: 1,
            day: "2026-09-29".into()
        }
    );
    assert_eq!(
        value.longest_run,
        TypingRun {
            characters: 4,
            day: "2026-09-28".into()
        }
    );
    // 并列不替换：更早的那段留着。
    store
        .record_at("六七八", TypingSource::Quanpin, "2026-09-29", None, 13_000)
        .unwrap();
    let value = store.load().unwrap();
    assert_eq!(value.current_run.characters, 4);
    assert_eq!(value.longest_run.day, "2026-09-28");
}

#[test]
fn a_run_across_midnight_continues_and_keeps_its_first_day() {
    let (_directory, store) = enabled_store();
    store
        .record_at("晚安", TypingSource::Quanpin, "2026-09-28", Some(23), 1_000)
        .unwrap();
    store
        .record_at("早安", TypingSource::Quanpin, "2026-09-29", Some(0), 6_000)
        .unwrap();
    let value = store.load().unwrap();
    assert_eq!(
        value.longest_run,
        TypingRun {
            characters: 4,
            day: "2026-09-28".into()
        }
    );
}

#[test]
fn a_clock_set_back_does_not_extend_a_run() {
    let (_directory, store) = enabled_store();
    store
        .record_at("你好", TypingSource::Quanpin, "2026-09-28", None, 50_000)
        .unwrap();
    store
        .record_at("世界", TypingSource::Quanpin, "2026-09-28", None, 40_000)
        .unwrap();
    assert_eq!(store.load().unwrap().current_run.characters, 2);
}

#[test]
fn a_run_is_rejected_when_its_day_is_missing() {
    let value = TypingStatistics {
        longest_run: TypingRun {
            characters: 3,
            day: String::new(),
        },
        ..TypingStatistics::default()
    };
    assert!(matches!(
        value.validate(),
        Err(TypingStatisticsError::InvalidDocument)
    ));
}

#[test]
fn voice_time_is_capped_per_call_and_per_day() {
    let (_directory, store) = enabled_store();
    assert!(matches!(
        store.record_voice("2026-09-28", MAX_VOICE_MS_PER_CALL + 1),
        Err(TypingStatisticsError::InvalidVoiceDuration)
    ));
    assert!(matches!(
        store.record_voice("2026-09-28", 0),
        Err(TypingStatisticsError::InvalidVoiceDuration)
    ));
    assert!(matches!(
        store.record_voice("2026-9-28", 1_000),
        Err(TypingStatisticsError::InvalidDay)
    ));
    assert_eq!(
        store
            .record_voice("2026-09-28", MAX_VOICE_MS_PER_CALL)
            .unwrap(),
        MAX_VOICE_MS_PER_CALL
    );
    for _ in 0..200 {
        store
            .record_voice("2026-09-28", MAX_VOICE_MS_PER_CALL)
            .unwrap();
    }
    assert_eq!(
        store.load().unwrap().daily_voice_ms["2026-09-28"],
        MAX_ACTIVE_MS_PER_DAY
    );
    assert_eq!(
        store
            .record_voice("2026-09-28", MAX_VOICE_MS_PER_CALL)
            .unwrap(),
        0
    );
}

#[test]
fn nothing_new_is_written_while_statistics_are_off() {
    let directory = tempfile::tempdir().unwrap();
    let store = TypingStatisticsStore::new(directory.path());
    assert_eq!(store.record_voice("2026-09-28", 1_000).unwrap(), 0);
    assert!(!store.record_skin("songlin").unwrap());
    let mut batch = CommitEfficiency::default();
    batch.count_commit(2, 4, false, true);
    store.record_efficiency(&batch).unwrap();
    assert!(!directory.path().join("typing-statistics.json").exists());
    let summary = store
        .summary(
            "2026-09-28",
            &SummaryInputs {
                user_words: Some(80),
            },
        )
        .unwrap();
    let words = summary
        .achievements
        .iter()
        .find(|achievement| achievement.id == "words_50")
        .unwrap();
    // 关闭时照样报告条件已满足，只是不记下来。
    assert_eq!(words.unlocked_day.as_deref(), Some("2026-09-28"));
    assert!(!directory.path().join("typing-statistics.json").exists());
}

#[test]
fn voice_days_follow_the_retention_window() {
    let (_directory, store) = enabled_store();
    store.record_voice("2026-01-01", 1_000).unwrap();
    store
        .set_retention(StatisticsRetention::Days30, "2026-03-01")
        .unwrap();
    assert!(store.load().unwrap().daily_voice_ms.is_empty());
}

#[test]
fn skins_are_deduplicated_validated_and_capped() {
    let (_directory, store) = enabled_store();
    assert!(store.record_skin("songlin").unwrap());
    assert!(!store.record_skin("songlin").unwrap());
    assert!(store
        .record_skin("10000000-0000-4000-8000-000000000042")
        .unwrap());
    assert!(store
        .record_skin("20000000-0000-4000-8000-00000000ABCD")
        .unwrap());
    for id in [
        "",
        "Bad",
        "../etc",
        "a/b",
        "{10000000-0000-4000-8000-000000000042}",
    ] {
        assert!(
            matches!(
                store.record_skin(id),
                Err(TypingStatisticsError::InvalidSkinId)
            ),
            "{id} accepted"
        );
    }
    for index in 0..MAX_SKINS_TRIED {
        store.record_skin(&format!("skin.{index}")).unwrap();
    }
    let value = store.load().unwrap();
    assert_eq!(value.skins_tried.len(), MAX_SKINS_TRIED);
    assert!(!store.record_skin("one.more").unwrap());
}

#[test]
fn a_document_with_an_invalid_skin_or_too_many_is_rejected() {
    let mut value = TypingStatistics::default();
    value.skins_tried.insert("Not A Skin".into());
    assert!(value.validate().is_err());
    let value = TypingStatistics {
        skins_tried: (0..=MAX_SKINS_TRIED)
            .map(|index| format!("s{index}"))
            .collect(),
        ..TypingStatistics::default()
    };
    assert!(value.validate().is_err());
}

#[test]
fn efficiency_batches_add_up_and_reject_impossible_counts() {
    let (_directory, store) = enabled_store();
    let mut batch = CommitEfficiency::default();
    assert!(batch.is_empty());
    batch.count_commit(4, 8, true, false);
    batch.count_commit(0, 2, false, true);
    store.record_efficiency(&batch).unwrap();
    store.record_efficiency(&batch).unwrap();
    assert_eq!(
        store.load().unwrap().efficiency,
        CommitEfficiency {
            commits: 4,
            typed_keys: 8,
            spelled_keys: 20,
            sentence_commits: 2,
            prediction_commits: 2,
        }
    );
    let impossible = CommitEfficiency {
        commits: 1,
        sentence_commits: 10,
        ..CommitEfficiency::default()
    };
    assert!(store.record_efficiency(&impossible).is_err());
    let overflow = CommitEfficiency {
        commits: MAX_COUNT,
        ..CommitEfficiency::default()
    };
    assert!(matches!(
        store.record_efficiency(&overflow),
        Err(TypingStatisticsError::CountExhausted)
    ));
    assert_eq!(store.load().unwrap().efficiency.commits, 4);
}

#[test]
fn an_unknown_achievement_or_bad_unlock_day_is_rejected() {
    let mut value = TypingStatistics::default();
    value
        .achievements
        .insert("not_a_badge".into(), "2026-09-28".into());
    assert!(value.validate().is_err());
    let mut value = TypingStatistics::default();
    value
        .achievements
        .insert("skins_5".into(), "yesterday".into());
    assert!(value.validate().is_err());
}

#[test]
fn unlocked_achievements_survive_retention_and_reset_clears_them() {
    let (_directory, store) = enabled_store();
    for _ in 0..6 {
        store
            .record_voice("2026-01-01", MAX_VOICE_MS_PER_CALL)
            .unwrap();
    }
    let summary = store
        .summary("2026-01-02", &SummaryInputs::default())
        .unwrap();
    let voice = |summary: &TypingSummary| {
        summary
            .achievements
            .iter()
            .find(|achievement| achievement.id == "voice_1h")
            .cloned()
            .unwrap()
    };
    assert_eq!(voice(&summary).unlocked_day.as_deref(), Some("2026-01-02"));
    assert_eq!(voice(&summary).current, 60);
    assert_eq!(store.load().unwrap().achievements["voice_1h"], "2026-01-02");

    // 保留期删掉了语音记录，徽章仍然记着第一次解锁的那天。
    store
        .set_retention(StatisticsRetention::Days30, "2026-03-01")
        .unwrap();
    let summary = store
        .summary("2026-03-01", &SummaryInputs::default())
        .unwrap();
    assert_eq!(voice(&summary).current, 0);
    assert_eq!(voice(&summary).unlocked_day.as_deref(), Some("2026-01-02"));

    let value = store.reset().unwrap();
    assert!(value.achievements.is_empty());
    assert_eq!(value.efficiency, CommitEfficiency::default());
    assert_eq!(value.longest_run, TypingRun::default());
    assert!(value.daily_voice_ms.is_empty());
    assert!(value.skins_tried.is_empty());
    let summary = store
        .summary("2026-03-01", &SummaryInputs::default())
        .unwrap();
    assert_eq!(voice(&summary).unlocked_day, None);
}

#[test]
fn summary_rejects_an_invalid_day() {
    let (_directory, store) = enabled_store();
    assert!(matches!(
        store.summary("2026-02-30", &SummaryInputs::default()),
        Err(TypingStatisticsError::InvalidDay)
    ));
}
