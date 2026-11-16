//! 在真实存储边界注入中断，重建服务后检查偏好、密文事件和私有恢复日志。
use super::*;

const PENDING: &str = "settings-sync-pending";

fn page_size(settings: &PreferencesStore) -> u32 {
    settings
        .load()
        .unwrap()
        .preferences
        .candidate_page_size
        .into()
}

fn event_count(drive: &Drive) -> usize {
    drive
        .0
        .lock()
        .unwrap()
        .files
        .keys()
        .filter(|path| path.starts_with("msime-settings/v1/e-"))
        .count()
}

/// 发布期间让基线写入失败；偏好写回和事件追加仍使用生产实现。
fn interrupt_state_commit(service: &SettingsWebDav, drive: &Drive) -> Vec<u8> {
    let path = service.directory.join(STATE_FILE);
    let before = std::fs::read(&path).unwrap();
    drive.0.lock().unwrap().append_hook = Some(Box::new(move || {
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
    }));
    before
}

fn restore_state(service: &SettingsWebDav, before: &[u8]) {
    std::fs::remove_dir(service.directory.join(STATE_FILE)).unwrap();
    service.storage().write_private(STATE_FILE, before).unwrap();
}

#[test]
fn a_lost_move_response_is_recovered_without_republishing_or_undoing_a_remote_reset() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let (laptop, phone, laptop_settings, phone_settings) = paired(root.path(), &drive);
    let punctuation = phone_settings
        .load()
        .unwrap()
        .preferences
        .chinese_punctuation;
    edit(&laptop_settings, |settings| {
        settings.candidate_page_size = 9
    });
    edit(&phone_settings, |settings| {
        settings.chinese_punctuation = !punctuation
    });
    phone.run(&phone_settings).unwrap();
    let before = laptop_settings.load().unwrap();
    drive.0.lock().unwrap().lose_append_response = true;
    assert_eq!(
        laptop.run(&laptop_settings).unwrap_err(),
        SettingsWebDavError::Relay(RelayError::Unavailable)
    );
    assert_eq!(laptop_settings.load().unwrap(), before);
    assert!(laptop.status().unwrap().pending_write);

    phone.run(&phone_settings).unwrap();
    edit(&phone_settings, |settings| {
        settings.chinese_punctuation = punctuation
    });
    phone.run(&phone_settings).unwrap();
    let events = event_count(&drive);
    let restarted = service(root.path(), &drive, "laptop");
    assert!(!restarted.run(&laptop_settings).unwrap().uploaded);
    assert_eq!(event_count(&drive), events);
    assert_eq!(page_size(&laptop_settings), 9);
    assert_eq!(
        laptop_settings
            .load()
            .unwrap()
            .preferences
            .chinese_punctuation,
        punctuation
    );
    assert!(!restarted.status().unwrap().pending_write);
}

#[test]
fn a_saved_download_is_not_replayed_as_a_local_edit_after_state_commit_failure() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let (laptop, phone, laptop_settings, phone_settings) = paired(root.path(), &drive);
    let punctuation = phone_settings
        .load()
        .unwrap()
        .preferences
        .chinese_punctuation;
    edit(&laptop_settings, |settings| {
        settings.candidate_page_size = 9;
        settings.voice_input.asr_token = "synthetic-recovery-private-token".into();
    });
    edit(&phone_settings, |settings| {
        settings.chinese_punctuation = !punctuation
    });
    phone.run(&phone_settings).unwrap();
    let state = interrupt_state_commit(&laptop, &drive);
    assert_eq!(
        laptop.run(&laptop_settings).unwrap_err(),
        SettingsWebDavError::Storage
    );
    assert_eq!(
        laptop_settings
            .load()
            .unwrap()
            .preferences
            .chinese_punctuation,
        !punctuation
    );
    let pending = std::fs::read(laptop.directory.join(PENDING)).unwrap();
    let text = String::from_utf8(pending).unwrap();
    assert!(!text.contains("synthetic-recovery-private-token"));
    assert!(!text.contains("synthetic-password"));
    restore_state(&laptop, &state);
    let requests = drive.0.lock().unwrap().requests.len();
    assert!(laptop.status().unwrap().pending_write);
    assert_eq!(drive.0.lock().unwrap().requests.len(), requests);

    phone.run(&phone_settings).unwrap();
    edit(&phone_settings, |settings| {
        settings.chinese_punctuation = punctuation
    });
    phone.run(&phone_settings).unwrap();
    let events = event_count(&drive);
    let restarted = service(root.path(), &drive, "laptop");
    assert!(!restarted.run(&laptop_settings).unwrap().uploaded);
    assert_eq!(event_count(&drive), events);
    let result = laptop_settings.load().unwrap().preferences;
    assert_eq!(result.candidate_page_size, 9);
    assert_eq!(result.chinese_punctuation, punctuation);
    assert_eq!(
        result.voice_input.asr_token,
        "synthetic-recovery-private-token"
    );
    assert!(!restarted.status().unwrap().pending_write);
}

#[test]
fn ambiguous_edits_after_an_interrupted_download_keep_all_versions_for_a_choice() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let (laptop, phone, laptop_settings, phone_settings) = paired(root.path(), &drive);
    edit(&laptop_settings, |settings| {
        settings.candidate_page_size = 9
    });
    edit(&phone_settings, |settings| {
        settings.chinese_punctuation = !settings.chinese_punctuation
    });
    phone.run(&phone_settings).unwrap();
    let state = interrupt_state_commit(&laptop, &drive);
    assert_eq!(
        laptop.run(&laptop_settings).unwrap_err(),
        SettingsWebDavError::Storage
    );
    restore_state(&laptop, &state);
    edit(&laptop_settings, |settings| {
        settings.candidate_page_size = 8
    });
    let before = laptop_settings.load().unwrap();
    let files = drive.0.lock().unwrap().files.clone();
    let pending = std::fs::read(laptop.directory.join(PENDING)).unwrap();
    let restarted = service(root.path(), &drive, "laptop");
    assert_eq!(
        restarted.run(&laptop_settings).unwrap_err(),
        SettingsWebDavError::RecoveryConflict
    );
    assert_eq!(laptop_settings.load().unwrap(), before);
    assert_eq!(drive.0.lock().unwrap().files, files);
    assert_eq!(
        std::fs::read(laptop.directory.join(PENDING)).unwrap(),
        pending
    );
}

#[test]
fn a_concurrent_local_save_is_preserved_and_can_be_published_on_retry() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let (laptop, phone, laptop_settings, phone_settings) = paired(root.path(), &drive);
    edit(&laptop_settings, |settings| {
        settings.candidate_page_size = 9
    });
    edit(&phone_settings, |settings| {
        settings.chinese_punctuation = !settings.chinese_punctuation
    });
    let punctuation = phone_settings
        .load()
        .unwrap()
        .preferences
        .chinese_punctuation;
    phone.run(&phone_settings).unwrap();
    let directory = root.path().to_path_buf();
    drive.0.lock().unwrap().append_hook = Some(Box::new(move || {
        edit(&preferences(&directory, "laptop"), |settings| {
            settings.candidate_page_size = 8
        });
    }));
    assert_eq!(
        laptop.run(&laptop_settings).unwrap_err(),
        SettingsWebDavError::Conflict
    );
    assert_eq!(page_size(&laptop_settings), 8);
    assert!(!laptop.status().unwrap().pending_write);
    let retry = laptop.run(&laptop_settings).unwrap();
    assert!(retry.uploaded && retry.downloaded);
    assert_eq!(page_size(&laptop_settings), 8);
    assert_eq!(
        laptop_settings
            .load()
            .unwrap()
            .preferences
            .chinese_punctuation,
        punctuation
    );
    phone.run(&phone_settings).unwrap();
    assert_eq!(page_size(&phone_settings), 8);
}

#[test]
fn recovery_does_not_forget_a_confirmed_event_when_the_drive_rolls_back() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let (laptop, phone, laptop_settings, phone_settings) = paired(root.path(), &drive);
    edit(&laptop_settings, |settings| {
        settings.candidate_page_size = 9
    });
    edit(&phone_settings, |settings| {
        settings.chinese_punctuation = !settings.chinese_punctuation
    });
    phone.run(&phone_settings).unwrap();
    let state = interrupt_state_commit(&laptop, &drive);
    assert_eq!(
        laptop.run(&laptop_settings).unwrap_err(),
        SettingsWebDavError::Storage
    );
    restore_state(&laptop, &state);
    drive
        .0
        .lock()
        .unwrap()
        .files
        .remove("msime-settings/v1/e-000000000002.json");
    let before = laptop_settings.load().unwrap();
    let restarted = service(root.path(), &drive, "laptop");
    assert_eq!(
        restarted.run(&laptop_settings).unwrap_err(),
        SettingsWebDavError::Relay(RelayError::Rollback)
    );
    assert_eq!(laptop_settings.load().unwrap(), before);
    assert!(restarted.status().unwrap().pending_write);
}

#[test]
fn a_committed_round_with_a_leftover_journal_does_not_roll_back_later_edits() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let (laptop, phone, laptop_settings, phone_settings) = paired(root.path(), &drive);
    edit(&laptop_settings, |settings| {
        settings.candidate_page_size = 9
    });
    edit(&phone_settings, |settings| {
        settings.chinese_punctuation = !settings.chinese_punctuation
    });
    phone.run(&phone_settings).unwrap();
    let state = interrupt_state_commit(&laptop, &drive);
    assert_eq!(
        laptop.run(&laptop_settings).unwrap_err(),
        SettingsWebDavError::Storage
    );
    let pending = std::fs::read(laptop.directory.join(PENDING)).unwrap();
    restore_state(&laptop, &state);
    laptop.run(&laptop_settings).unwrap();
    laptop.storage().write_private(PENDING, &pending).unwrap();
    edit(&laptop_settings, |settings| {
        settings.candidate_page_size = 8
    });
    let restarted = service(root.path(), &drive, "laptop");
    assert!(restarted.run(&laptop_settings).unwrap().uploaded);
    assert_eq!(page_size(&laptop_settings), 8);
    assert!(!restarted.status().unwrap().pending_write);
}

#[test]
fn malformed_or_future_recovery_records_never_change_preferences_or_events() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let (laptop, _, laptop_settings, _) = paired(root.path(), &drive);
    edit(&laptop_settings, |settings| {
        settings.candidate_page_size = 9
    });
    drive.0.lock().unwrap().lose_append_response = true;
    assert_eq!(
        laptop.run(&laptop_settings).unwrap_err(),
        SettingsWebDavError::Relay(RelayError::Unavailable)
    );
    let valid = std::fs::read(laptop.directory.join(PENDING)).unwrap();
    let mut future: serde_json::Value = serde_json::from_slice(&valid).unwrap();
    future["version"] = 2.into();
    let mut unknown: serde_json::Value = serde_json::from_slice(&valid).unwrap();
    unknown["unknown"] = true.into();
    let before = laptop_settings.load().unwrap();
    let files = drive.0.lock().unwrap().files.clone();
    for (bytes, expected) in [
        (b"invalid".to_vec(), SettingsWebDavError::Storage),
        (
            serde_json::to_vec(&future).unwrap(),
            SettingsWebDavError::Protocol,
        ),
        (
            serde_json::to_vec(&unknown).unwrap(),
            SettingsWebDavError::Storage,
        ),
        (
            vec![b' '; 4 * 1024 * 1024 + 1],
            SettingsWebDavError::Storage,
        ),
    ] {
        laptop.storage().write_private(PENDING, &bytes).unwrap();
        assert_eq!(laptop.run(&laptop_settings).unwrap_err(), expected);
        assert_eq!(laptop_settings.load().unwrap(), before);
        assert_eq!(drive.0.lock().unwrap().files, files);
    }
}

#[test]
fn a_failed_intent_write_stops_before_uploading_or_applying_a_download() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let (laptop, phone, laptop_settings, phone_settings) = paired(root.path(), &drive);
    edit(&laptop_settings, |settings| {
        settings.candidate_page_size = 9
    });
    edit(&phone_settings, |settings| {
        settings.chinese_punctuation = !settings.chinese_punctuation
    });
    phone.run(&phone_settings).unwrap();
    std::fs::create_dir(laptop.directory.join(PENDING)).unwrap();
    let before = laptop_settings.load().unwrap();
    let files = drive.0.lock().unwrap().files.clone();
    assert_eq!(
        laptop.run(&laptop_settings).unwrap_err(),
        SettingsWebDavError::Storage
    );
    assert_eq!(laptop_settings.load().unwrap(), before);
    assert_eq!(drive.0.lock().unwrap().files, files);
}

#[test]
fn a_published_round_without_a_download_keeps_edits_made_before_state_commit() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let laptop = service(root.path(), &drive, "laptop");
    let settings = preferences(root.path(), "laptop");
    laptop.run(&settings).unwrap();
    edit(&settings, |settings| settings.candidate_page_size = 9);
    let path = laptop.directory.join(STATE_FILE);
    let previous = std::fs::read(&path).unwrap();
    let directory = root.path().to_path_buf();
    drive.0.lock().unwrap().append_hook = Some(Box::new(move || {
        edit(&preferences(&directory, "laptop"), |settings| {
            settings.candidate_page_size = 8
        });
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
    }));
    assert_eq!(
        laptop.run(&settings).unwrap_err(),
        SettingsWebDavError::Storage
    );
    assert_eq!(page_size(&settings), 8);
    restore_state(&laptop, &previous);
    let restarted = service(root.path(), &drive, "laptop");
    assert!(restarted.run(&settings).unwrap().uploaded);
    assert_eq!(page_size(&settings), 8);
    assert!(!restarted.status().unwrap().pending_write);
}
