use super::*;
use crate::update_check::Version;
use std::cell::Cell;

fn release(display: &str, run: Option<u64>, channel: UpdateChannel) -> ReleaseUpdate {
    ReleaseUpdate {
        version: Version {
            display: display.to_owned(),
            parts: display
                .split('.')
                .map(|part| part.parse().unwrap())
                .collect(),
        },
        release_url: "https://github.com/metasequoiaime/msime/releases/tag/linux-v9.9.9".into(),
        installer_name: None,
        installer_sha256: None,
        signed: Some(false),
        channel,
        build_run: run,
    }
}

fn stable(display: &str) -> ReleaseUpdate {
    release(display, None, UpdateChannel::Stable)
}

fn request(channel: Option<&str>) -> UpdateCheckRequest {
    UpdateCheckRequest {
        platform: "linux".into(),
        current_version: "1.2.3".into(),
        edition: None,
        arch: Some("x86_64".into()),
        channel: channel.map(str::to_owned),
    }
}

#[test]
fn background_check_runs_once_a_day_and_after_the_clock_moves_back() {
    let record = BackgroundRecord {
        checked_at: 1_000_000,
        attempted_at: 1_000_000,
        ..BackgroundRecord::default()
    };
    let stable = UpdateChannel::Stable;
    assert!(background_check_due(
        &BackgroundRecord::default(),
        1_000_000,
        stable
    ));
    assert!(!background_check_due(&record, 1_000_000 + 60, stable));
    assert!(!background_check_due(
        &record,
        1_000_000 + INTERVAL_SECONDS - 1,
        stable
    ));
    assert!(background_check_due(
        &record,
        1_000_000 + INTERVAL_SECONDS,
        stable
    ));
    assert!(background_check_due(&record, 999_000, stable));
}

#[test]
fn a_failed_check_waits_for_the_retry_interval_instead_of_every_launch() {
    let stable = UpdateChannel::Stable;
    // 从没成功过、刚才连不上：几分钟后不再联网，过了重试间隔才再试。
    let offline = BackgroundRecord {
        checked_at: 0,
        attempted_at: 2_000_000,
        ..BackgroundRecord::default()
    };
    assert!(!background_check_due(&offline, 2_000_000 + 60, stable));
    assert!(!background_check_due(
        &offline,
        2_000_000 + RETRY_SECONDS - 1,
        stable
    ));
    assert!(background_check_due(
        &offline,
        2_000_000 + RETRY_SECONDS,
        stable
    ));
    // 成功检查过的一天之内，失败的重试间隔不会让它提前再查。
    let checked = BackgroundRecord {
        checked_at: 2_000_000,
        attempted_at: 2_000_000,
        ..BackgroundRecord::default()
    };
    assert!(!background_check_due(
        &checked,
        2_000_000 + RETRY_SECONDS,
        stable
    ));
    assert!(background_check_due(&checked, 1_000, stable));
}

#[test]
fn switching_the_channel_makes_the_record_due_at_once() {
    let checked = BackgroundRecord {
        checked_at: 2_000_000,
        attempted_at: 2_000_000,
        ..BackgroundRecord::default()
    };
    assert!(!background_check_due(
        &checked,
        2_000_060,
        UpdateChannel::Stable
    ));
    assert!(background_check_due(
        &checked,
        2_000_060,
        UpdateChannel::Beta
    ));
    assert!(background_check_due(
        &checked,
        2_000_060,
        UpdateChannel::Nightly
    ));
}

#[test]
fn packagers_and_users_can_turn_the_background_check_off() {
    assert!(!background_check_disabled(None, None));
    assert!(!background_check_disabled(Some(""), Some(OsStr::new(""))));
    assert!(!background_check_disabled(Some("0"), Some(OsStr::new("0"))));
    assert!(background_check_disabled(Some("1"), None));
    assert!(background_check_disabled(None, Some(OsStr::new("1"))));
    assert!(background_check_disabled(
        Some("0"),
        Some(OsStr::new("yes"))
    ));
    // 测试是开发构建：整个后台检查都关着，入口不碰磁盘也不联网。
    assert!(automatic_checks_disabled());
    let directory = tempfile::tempdir().unwrap();
    assert_eq!(
        run_background_check(directory.path(), &request(None)).unwrap(),
        BackgroundOutcome {
            status: BackgroundStatus::Disabled,
            notify: None
        }
    );
    assert!(!directory.path().join(RECORD_FILE).exists());
}

#[test]
fn each_new_build_is_announced_once() {
    let fresh = BackgroundRecord::default();
    let available = |update: ReleaseUpdate| UpdateCheck::Available(update);
    assert_eq!(
        background_notification(&available(stable("9.9.9")), &fresh),
        Some(stable("9.9.9"))
    );
    // 以前的记录只有 `notified_version`：同一个版本（没有运行 ID）不再提醒。
    let told: BackgroundRecord =
        serde_json::from_str(r#"{"checked_at":1,"attempted_at":1,"notified_version":"9.9.9"}"#)
            .unwrap();
    assert_eq!(
        background_notification(&available(stable("9.9.9")), &told),
        None
    );
    assert_eq!(
        background_notification(&available(stable("9.10.0")), &told),
        Some(stable("9.10.0"))
    );
    // 同一个版本号的新构建（每日构建、正式版接在 beta 后面）要再提醒一次，同一个构建只提醒一次。
    let nightly = release("9.9.9", Some(500), UpdateChannel::Nightly);
    assert_eq!(
        background_notification(&available(nightly.clone()), &told),
        Some(nightly.clone())
    );
    let told_nightly = BackgroundRecord {
        notified_version: Some("9.9.9".into()),
        notified_run: Some(500),
        ..BackgroundRecord::default()
    };
    assert_eq!(
        background_notification(&available(nightly), &told_nightly),
        None
    );
    assert_eq!(
        background_notification(&UpdateCheck::Current(stable("9.9.9")), &fresh),
        None
    );
    assert_eq!(background_notification(&UpdateCheck::None, &fresh), None);
}

#[test]
fn an_old_record_reads_as_a_stable_record() {
    assert_eq!(
        serde_json::from_str::<BackgroundRecord>(r#"{"checked_at":5}"#).unwrap(),
        BackgroundRecord {
            checked_at: 5,
            ..BackgroundRecord::default()
        }
    );
    // Linux 设置应用以前写下的完整记录：发布描述里没有 `channel` 和 `build_run`。
    let old = r#"{"checked_at":7,"attempted_at":8,"notified_version":"9.9.9","available":{"version":{"display":"9.9.9","parts":[9,9,9]},"release_url":"https://github.com/metasequoiaime/msime/releases/tag/linux-v9.9.9","installer_name":null,"installer_sha256":null,"signed":false}}"#;
    let record: BackgroundRecord = serde_json::from_str(old).unwrap();
    assert_eq!(record.channel, UpdateChannel::Stable);
    assert_eq!(record.notified_run, None);
    assert_eq!(record.available, Some(stable("9.9.9")));
    assert_eq!(
        pending_from_record(&record, "1.2.3", UpdateChannel::Stable, None),
        Some(UpdateCheck::Available(stable("9.9.9")))
    );
    // 写回去再读出来是同一份记录；没有提醒过运行 ID 时不写这个键。
    let text = serde_json::to_string(&record).unwrap();
    assert!(!text.contains("notified_run"));
    assert!(text.contains(r#""channel":"stable""#));
    assert_eq!(
        serde_json::from_str::<BackgroundRecord>(&text).unwrap(),
        record
    );
}

#[test]
fn the_about_page_keeps_showing_a_found_update_until_it_is_installed() {
    let record = BackgroundRecord {
        checked_at: 1,
        attempted_at: 1,
        notified_version: Some("9.9.9".to_owned()),
        available: Some(stable("9.9.9")),
        ..BackgroundRecord::default()
    };
    let channel = UpdateChannel::Stable;
    assert_eq!(
        pending_from_record(&record, "1.2.3", channel, None),
        Some(UpdateCheck::Available(stable("9.9.9")))
    );
    assert_eq!(pending_from_record(&record, "9.9.9", channel, None), None);
    assert_eq!(pending_from_record(&record, "10.0.0", channel, None), None);
    assert_eq!(
        pending_from_record(&BackgroundRecord::default(), "1.2.3", channel, None),
        None
    );
    // 换了通道：旧通道找到的版本不再显示。
    assert_eq!(
        pending_from_record(&record, "1.2.3", UpdateChannel::Beta, None),
        None
    );
    // 同一版本号的每日构建：运行的构建知道自己的运行 ID 时，比它新才显示。
    let nightly = BackgroundRecord {
        channel: UpdateChannel::Nightly,
        available: Some(release("9.9.9", Some(500), UpdateChannel::Nightly)),
        ..BackgroundRecord::default()
    };
    let nightly_channel = UpdateChannel::Nightly;
    assert!(pending_from_record(&nightly, "9.9.9", nightly_channel, Some(499)).is_some());
    assert_eq!(
        pending_from_record(&nightly, "9.9.9", nightly_channel, Some(500)),
        None
    );
    assert_eq!(
        pending_from_record(&nightly, "9.9.9", nightly_channel, None),
        None
    );
}

#[test]
fn a_background_check_records_announces_once_and_waits_a_day() {
    let directory = tempfile::tempdir().unwrap();
    let found = release("9.9.9", Some(42), UpdateChannel::Stable);
    let calls = Cell::new(0);
    let check = |_: &UpdateCheckRequest| {
        calls.set(calls.get() + 1);
        Ok(UpdateCheck::Available(found.clone()))
    };
    let first =
        run_background_check_with(directory.path(), &request(None), 1_000_000, check).unwrap();
    assert_eq!(first.status, BackgroundStatus::Checked);
    assert_eq!(first.notify, Some(found.clone()));
    let record = read_record(directory.path());
    assert_eq!(record.checked_at, 1_000_000);
    assert_eq!(record.notified_version.as_deref(), Some("9.9.9"));
    assert_eq!(record.notified_run, Some(42));
    assert_eq!(record.available, Some(found.clone()));
    assert_eq!(
        pending_update(directory.path(), &request(None)).unwrap(),
        Some(UpdateCheck::Available(found.clone()))
    );

    // 一小时后：没到时间，不联网。
    let later =
        run_background_check_with(directory.path(), &request(None), 1_003_600, check).unwrap();
    assert_eq!(later.status, BackgroundStatus::NotDue);
    assert_eq!(calls.get(), 1);

    // 一天后：再查一次，同一个构建不再提醒。
    let next_day = run_background_check_with(
        directory.path(),
        &request(None),
        1_000_000 + INTERVAL_SECONDS,
        check,
    )
    .unwrap();
    assert_eq!(next_day.status, BackgroundStatus::Checked);
    assert_eq!(next_day.notify, None);
    assert_eq!(calls.get(), 2);
}

#[test]
fn an_unreachable_check_records_the_attempt_and_keeps_the_found_update() {
    let directory = tempfile::tempdir().unwrap();
    run_background_check_with(directory.path(), &request(None), 1_000_000, |_| {
        Ok(UpdateCheck::Available(stable("9.9.9")))
    })
    .unwrap();
    let offline = run_background_check_with(
        directory.path(),
        &request(None),
        1_000_000 + INTERVAL_SECONDS,
        |_| Err(UpdateCheckError::Unavailable),
    )
    .unwrap();
    assert_eq!(offline.status, BackgroundStatus::Unavailable);
    let record = read_record(directory.path());
    assert_eq!(record.checked_at, 1_000_000);
    assert_eq!(record.attempted_at, 1_000_000 + INTERVAL_SECONDS);
    assert_eq!(record.available, Some(stable("9.9.9")));
}

#[test]
fn switching_the_channel_rechecks_at_once_and_drops_the_old_channel_s_update() {
    let directory = tempfile::tempdir().unwrap();
    run_background_check_with(directory.path(), &request(None), 1_000_000, |_| {
        Ok(UpdateCheck::Available(stable("9.9.9")))
    })
    .unwrap();
    // 一分钟后换到测试版、又连不上：照样立刻尝试，稳定版找到的版本不再显示。
    let offline =
        run_background_check_with(directory.path(), &request(Some("beta")), 1_000_060, |_| {
            Err(UpdateCheckError::Unavailable)
        })
        .unwrap();
    assert_eq!(offline.status, BackgroundStatus::Unavailable);
    let record = read_record(directory.path());
    assert_eq!(record.channel, UpdateChannel::Beta);
    assert_eq!(record.checked_at, 0);
    assert_eq!(record.available, None);
    assert_eq!(record.notified_version.as_deref(), Some("9.9.9"));
    assert_eq!(
        pending_update(directory.path(), &request(Some("beta"))).unwrap(),
        None
    );
    // 测试版通道里的检查也看到了 9.9.9 这个正式版：同一个构建不再提醒。
    let beta = run_background_check_with(
        directory.path(),
        &request(Some("beta")),
        1_000_060 + RETRY_SECONDS,
        |seen| {
            assert_eq!(seen.channel.as_deref(), Some("beta"));
            Ok(UpdateCheck::Available(stable("9.9.9")))
        },
    )
    .unwrap();
    assert_eq!(beta.status, BackgroundStatus::Checked);
    assert_eq!(beta.notify, None);
}

#[test]
fn windows7_records_every_channel_as_stable() {
    let directory = tempfile::tempdir().unwrap();
    let request = UpdateCheckRequest {
        platform: "windows7".into(),
        current_version: "1.0.0".into(),
        edition: None,
        arch: Some("x64".into()),
        channel: Some("nightly".into()),
    };
    run_background_check_with(directory.path(), &request, 1_000_000, |_| {
        Ok(UpdateCheck::None)
    })
    .unwrap();
    assert_eq!(read_record(directory.path()).channel, UpdateChannel::Stable);
    // 通道没变（都是稳定版），一小时后不再查。
    let later =
        run_background_check_with(directory.path(), &request, 1_003_600, |_| panic!("not due"))
            .unwrap();
    assert_eq!(later.status, BackgroundStatus::NotDue);
}

#[test]
fn a_bad_request_or_relative_directory_is_refused_without_touching_disk() {
    let directory = tempfile::tempdir().unwrap();
    let bad_channel = request(Some("canary"));
    assert_eq!(
        run_background_check_with(directory.path(), &bad_channel, 1, |_| panic!("no check")),
        Err(BackgroundError::Invalid)
    );
    assert_eq!(
        pending_update(directory.path(), &bad_channel),
        Err(BackgroundError::Invalid)
    );
    let relative = Path::new("relative/state");
    assert_eq!(
        run_background_check_with(relative, &request(None), 1, |_| panic!("no check")),
        Err(BackgroundError::Invalid)
    );
    let bad_version = UpdateCheckRequest {
        current_version: "unknown".into(),
        ..request(None)
    };
    assert_eq!(
        pending_update(directory.path(), &bad_version),
        Err(BackgroundError::Invalid)
    );
    assert!(!directory.path().join(RECORD_FILE).exists());
    // 没有记录时没有要显示的版本。
    assert_eq!(pending_update(directory.path(), &request(None)), Ok(None));
}

#[test]
fn another_process_checking_skips_this_one() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(directory.path()).unwrap();
    let private = crate::file_lock::open_private_directory(directory.path()).unwrap();
    let lock =
        crate::file_lock::open_private_lock_file_at(&private, OsStr::new(LOCK_FILE)).unwrap();
    let _held = crate::file_lock::ScopedFileLock::exclusive(lock).unwrap();
    let skipped = run_background_check_with(directory.path(), &request(None), 1, |_| {
        panic!("another process holds the lock")
    })
    .unwrap();
    assert_eq!(skipped.status, BackgroundStatus::NotDue);
}

#[test]
fn an_outcome_serialises_as_hosts_read_it() {
    let outcome = BackgroundOutcome {
        status: BackgroundStatus::NotDue,
        notify: None,
    };
    assert_eq!(
        serde_json::to_value(outcome).unwrap(),
        serde_json::json!({ "status": "not_due", "notify": null })
    );
}
