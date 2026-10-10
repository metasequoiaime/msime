#[test]
fn snapshot_module_is_present() {
    assert_eq!(super::HANDLE_LIMIT, 8);
}

#[cfg(unix)]
#[test]
fn activation_receipt_does_not_follow_a_fixed_temporary_symlink() {
    use std::fs;
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let temporary = root
        .path()
        .join(format!("{}.tmp", super::ACTIVATION_RECEIPT_NAME));
    let outside_file = outside.path().join("receipt");
    fs::write(&outside_file, b"keep me").unwrap();
    symlink(&outside_file, &temporary).unwrap();
    let path = root.path().join(super::ACTIVATION_RECEIPT_NAME);

    super::write_activation_receipt_at(&path, "10000000-0000-4000-8000-000000000001").unwrap();
    assert_eq!(fs::read(outside_file).unwrap(), b"keep me");
    assert_eq!(
        fs::read(path).unwrap(),
        b"10000000-0000-4000-8000-000000000001"
    );
}

#[cfg(unix)]
#[test]
fn activation_receipt_rejects_a_symlinked_receipt() {
    use msime_client_core::preferences::Preferences;
    use std::fs;
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let user = root.path().join("user");
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir(&user).unwrap();
    let outside_file = outside.path().join("receipt");
    fs::write(&outside_file, b"10000000-0000-4000-8000-000000000001").unwrap();
    symlink(&outside_file, user.join(super::ACTIVATION_RECEIPT_NAME)).unwrap();
    let options: super::HostOptions = serde_json::from_value(serde_json::json!({
        "api_version": 1,
        "resources": root.path().join("resources"),
        "user_data": user,
        "cache": root.path().join("cache"),
        "dictionaries": root.path().join("dictionaries"),
        "preferences": Preferences::default(),
    }))
    .unwrap();

    assert_eq!(
        super::activation_receipt(&options.into_engine_options()),
        Err("snapshot activation receipt unavailable")
    );
}

#[cfg(unix)]
#[test]
fn activation_receipt_rejects_a_symlinked_parent() {
    use msime_path_trust::untrusted_symlink as symlink;
    use std::fs;

    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let linked = root.path().join("linked");
    symlink(outside.path(), &linked).unwrap();
    let path = linked.join(super::ACTIVATION_RECEIPT_NAME);
    let outside_file = outside.path().join(super::ACTIVATION_RECEIPT_NAME);
    fs::write(&outside_file, b"keep me").unwrap();

    let result = super::write_activation_receipt_at(&path, "10000000-0000-4000-8000-000000000001");

    assert!(result.is_err());
    assert_eq!(fs::read(outside_file).unwrap(), b"keep me");
}

#[cfg(unix)]
#[test]
fn snapshot_file_open_does_not_follow_a_leaf_symlink() {
    use std::fs;
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("snapshot.ndjson");
    fs::write(&outside_file, b"synthetic snapshot").unwrap();
    let linked = root.path().join("snapshot.ndjson");
    symlink(&outside_file, &linked).unwrap();

    assert!(super::open_snapshot_file(&linked).is_err());
    assert_eq!(fs::read(outside_file).unwrap(), b"synthetic snapshot");
}

#[test]
fn queue_state_can_be_polled_while_an_engine_session_holds_shared_access() {
    use msime_client_core::dictionary::access::DictionaryAccess;
    use msime_client_core::preferences::Preferences;
    use std::fs;

    let root = tempfile::tempdir().unwrap();
    for name in ["resources", "user", "cache", "dictionaries"] {
        fs::create_dir(root.path().join(name)).unwrap();
    }
    let path = |name: &str| root.path().join(name).to_string_lossy().into_owned();
    let options: super::HostOptions = serde_json::from_value(serde_json::json!({
        "api_version": 1,
        "resources": path("resources"),
        "user_data": path("user"),
        "cache": path("cache"),
        "dictionaries": path("dictionaries"),
        "preferences": Preferences::default(),
    }))
    .unwrap();
    let _session =
        DictionaryAccess::try_session(&root.path().join("user"), &root.path().join("dictionaries"))
            .unwrap()
            .unwrap();
    let queue_root = tempfile::tempdir_in(root.path()).unwrap();
    let queue = super::snapshot_queue(queue_root.path().to_str().unwrap()).unwrap();

    let state = super::snapshot_queue_state(&queue, options, false).unwrap();

    assert!(state["request"].is_null());
    assert!(state["localVersion"].as_str().is_some());
}

#[test]
fn inspection_requires_the_complete_counted_snapshot_envelope() {
    use sha2::{Digest, Sha256};
    use std::fs;

    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("snapshot.ndjson");
    let lines = [
        r#"{"type":"header","format":"msime-dictionary-snapshot","version":1,"revision":7}"#,
        r#"{"type":"entry","data":{"id":"fixture","kind":"quick","code":"test","word":"合成","weight":1,"revision":1,"updated_at":"2026-09-01T00:00:00Z"}}"#,
        r#"{"type":"overlay","deleted":false,"data":{"id":"fixture","kind":"quick","code":"test","word":"合成","weight":1,"revision":1,"updated_at":"2026-09-01T00:00:00Z"}}"#,
    ];
    let body = format!("{}\n", lines.join("\n"));
    let digest = hex::encode(Sha256::digest(body.as_bytes()));
    let footer = format!(
        r#"{{"type":"footer","records":{},"sha256":"{}"}}"#,
        lines.len(),
        digest
    );
    let complete = format!("{body}{footer}\n");
    fs::write(&file, &complete).unwrap();
    let metadata = super::inspect_snapshot(&file).unwrap();
    assert_eq!(metadata.cloud_revision, 7);
    assert_eq!(metadata.records, 3);
    assert_eq!(metadata.entries, 1);
    assert_eq!(metadata.overlays, 1);
    assert_eq!(metadata.engine_records, 1);
    assert_eq!(metadata.bytes, complete.len() as u64);
    assert_eq!(
        metadata.file_sha256,
        hex::encode(Sha256::digest(complete.as_bytes()))
    );
    let staged = super::SnapshotFileRecords::open(&file)
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(staged.len(), 1, "only the overlay is an Engine record");
    assert!(staged[0].is_ok());

    fs::write(&file, body.as_bytes()).unwrap();
    assert!(super::inspect_snapshot(&file).is_err());
    fs::write(&file, complete.replace("\"records\":3", "\"records\":2")).unwrap();
    assert!(super::inspect_snapshot(&file).is_err());
    fs::write(&file, complete.replacen("合成", "篡改", 1)).unwrap();
    assert!(super::inspect_snapshot(&file).is_err());
    fs::write(&file, format!("{complete}{{}}\n")).unwrap();
    assert!(super::inspect_snapshot(&file).is_err());

    let malformed_lines = [
        lines[0].to_owned(),
        lines[1].to_owned(),
        lines[2].replace("\"weight\":1", "\"weight\":2"),
    ];
    let malformed_body = format!("{}\n", malformed_lines.join("\n"));
    let malformed_digest = hex::encode(Sha256::digest(malformed_body.as_bytes()));
    let malformed = format!(
        "{malformed_body}{{\"type\":\"footer\",\"records\":3,\"sha256\":\"{malformed_digest}\"}}\n"
    );
    fs::write(&file, malformed).unwrap();
    assert!(super::inspect_snapshot(&file).is_err());
}

#[cfg(unix)]
#[test]
fn inspection_rejects_a_snapshot_below_a_symlinked_parent() {
    use msime_path_trust::untrusted_symlink as symlink;
    use sha2::{Digest, Sha256};
    use std::fs;

    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let body = concat!(
        r#"{"type":"header","format":"msime-dictionary-snapshot","version":1,"revision":7}"#,
        "\n"
    );
    let digest = hex::encode(Sha256::digest(body.as_bytes()));
    let snapshot = format!("{body}{{\"type\":\"footer\",\"records\":1,\"sha256\":\"{digest}\"}}\n");
    fs::write(outside.path().join("snapshot.ndjson"), snapshot).unwrap();
    symlink(outside.path(), root.path().join("linked")).unwrap();

    let path = root.path().join("linked/snapshot.ndjson");
    assert!(super::inspect_snapshot(&path).is_err());
}

#[cfg(unix)]
#[test]
fn snapshot_publication_rejects_a_symlinked_parent() {
    use msime_path_trust::untrusted_symlink as symlink;
    use std::fs;

    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let linked = root.path().join("linked");
    symlink(outside.path(), &linked).unwrap();
    let destination = linked.join("snapshot.ndjson");
    let outside_file = outside.path().join("snapshot.ndjson");
    fs::write(&outside_file, b"keep me").unwrap();

    let result = super::publish_snapshot(&destination, b"synthetic snapshot");

    assert!(result.is_err());
    assert_eq!(fs::read(outside_file).unwrap(), b"keep me");
}

#[test]
fn restore_reinspects_the_exact_file_before_upload() {
    use msime_client_core::account::AccountDictionarySnapshotRestore;
    use sha2::{Digest, Sha256};
    use std::fs;

    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("snapshot.ndjson");
    let header =
        r#"{"type":"header","format":"msime-dictionary-snapshot","version":1,"revision":7}"#;
    let body = format!("{header}\n");
    let body_sha256 = hex::encode(Sha256::digest(body.as_bytes()));
    let complete =
        format!("{body}{{\"type\":\"footer\",\"records\":1,\"sha256\":\"{body_sha256}\"}}\n");
    fs::write(&file, &complete).unwrap();
    let file_sha256 = hex::encode(Sha256::digest(complete.as_bytes()));
    let request = super::RestoreRequest {
        revision: 11,
        expected_sha256: file_sha256.clone(),
        access_token: "a".repeat(64),
    };
    let mut uploaded = false;

    let result = super::restore_snapshot_with(request, &file, |path, revision, token| {
        uploaded = true;
        assert_eq!(path, file);
        assert_eq!(revision, 11);
        assert_eq!(token, "a".repeat(64));
        Ok(AccountDictionarySnapshotRestore {
            revision: 12,
            reset: true,
        })
    })
    .unwrap();
    assert!(uploaded);
    assert_eq!(result, serde_json::json!({"revision": 12, "reset": true}));

    let changed = complete.replace("\"revision\":7", "\"revision\":8");
    fs::write(&file, changed).unwrap();
    let mut called = false;
    let request = super::RestoreRequest {
        revision: 11,
        expected_sha256: file_sha256,
        access_token: "a".repeat(64),
    };
    assert_eq!(
        super::restore_snapshot_with(request, &file, |_, _, _| {
            called = true;
            unreachable!()
        }),
        Err("account_invalid".to_owned())
    );
    assert!(!called);
}

#[test]
fn restore_maps_account_errors_without_exposing_snapshot_data() {
    use msime_client_core::account::AccountError;
    use sha2::{Digest, Sha256};
    use std::fs;

    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("snapshot.ndjson");
    let header =
        r#"{"type":"header","format":"msime-dictionary-snapshot","version":1,"revision":0}"#;
    let body = format!("{header}\n");
    let body_sha256 = hex::encode(Sha256::digest(body.as_bytes()));
    let complete =
        format!("{body}{{\"type\":\"footer\",\"records\":1,\"sha256\":\"{body_sha256}\"}}\n");
    fs::write(&file, &complete).unwrap();
    let request = super::RestoreRequest {
        revision: 4,
        expected_sha256: hex::encode(Sha256::digest(complete.as_bytes())),
        access_token: "b".repeat(64),
    };

    let result: Result<serde_json::Value, String> =
        super::restore_snapshot_with(request, &file, |_, _, _| Err(AccountError::Conflict));
    assert_eq!(result, Err("account_conflict".to_owned()));
}

#[test]
fn activation_swaps_all_state_roots_and_consumes_handle() {
    activation_case(false, false, 123, false);
    activation_case(true, false, 123, false);
}

#[test]
fn activation_rejects_live_session_before_swapping() {
    // The registry is process-global; use a distinct fixture handle so this
    // test can run in parallel with the successful activation cases.
    activation_case(false, true, 125, false);
    activation_case(true, true, 125, false);
}

#[cfg(unix)]
#[test]
fn activation_keeps_a_replaced_live_root_out_of_the_swap() {
    activation_case(false, false, 127, true);
}

#[test]
fn discard_does_not_require_maintenance_lock_for_live_paths() {
    use super::*;
    use msime_client_core::dictionary::access::DictionaryAccess;
    use msime_engine::host::EngineOptions;
    use std::fs;

    let root = tempfile::tempdir().unwrap();
    let user = root.path().join("user");
    let dictionaries = root.path().join("dictionaries");
    fs::create_dir_all(&user).unwrap();
    fs::create_dir_all(&dictionaries).unwrap();
    let session_access = DictionaryAccess::try_session(&user, &dictionaries)
        .unwrap()
        .unwrap();
    let directory = tempfile::tempdir_in(root.path()).unwrap();
    let options = EngineOptions {
        resources: root.path().join("resources").to_string_lossy().into_owned(),
        user_data: user.to_string_lossy().into_owned(),
        cache: root.path().join("cache").to_string_lossy().into_owned(),
        dictionaries: dictionaries.to_string_lossy().into_owned(),
        scheme: 0,
        enabled_schemes: msime_engine::SchemeSet::ALL,
        shuangpin_profile: 0,
        shuangpin_custom_profile: None,
        shuangpin_preedit_uses_raw: true,
        single_character_only: false,
        learning: false,
        autocorrect_transposition: true,
        autocorrect_neighbor: true,
        fuzzy_pinyin_rules: 0,
        wubi_mixed_pinyin: false,
        wubi_profile: 0,
        helpcode: false,
        show_helpcode: true,
        helpcode_schema: "ziranma".into(),
        chinese_punctuation: true,
        paired_punctuation: true,
        punctuation_lock: 0,
        frequency_mode: "promote".into(),
        frequency_trigger_count: 1,
        frequency_linear_step: 1,
        mixed_english: true,
        english_minimum_prefix: 2,
        mixed_emoji: false,
        mixed_kaomoji: false,
        local_unicode: true,
        local_date_time: true,
        local_quick_phrase: true,
        local_emoji: true,
        local_kaomoji: true,
        local_super_jianpin: true,
        local_temporary_english: true,
        local_temporary_japanese: true,
        local_expression: false,
        local_command: false,
        local_mention: false,
        command_table: Vec::new(),
        mention_entries: Vec::new(),
        quick_phrase_table: Vec::new(),
        helpcode_table: None,
        sentence_association: msime_engine::host::SentenceAssociationOptions {
            word_lattice: true,
            neural_keyboard: false,
            show_next_on_duplicate: false,
        },
        rescoring_context: String::new(),
        sentence_alternatives: true,
        vietnamese_input_method: 0,
        vietnamese_tone_style: 0,
        cantonese_dictionary: String::new(),
        zhuyin_dictionary: String::new(),
        stroke_dictionary: String::new(),
        japanese_dictionary: String::new(),
    };
    registry().lock().unwrap().insert(
        456,
        Prepared {
            directory,
            active_options: options.clone(),
            options,
            source_version: String::new(),
        },
    );
    assert_eq!(
        discard(456).unwrap(),
        serde_json::json!({"discarded": true})
    );
    assert!(!registry().lock().unwrap().contains_key(&456));
    drop(session_access);
}

fn activation_case(
    nested_dictionaries: bool,
    hold_session: bool,
    handle: u64,
    replace_live_user: bool,
) {
    use super::*;
    use msime_client_core::dictionary::access::DictionaryAccess;
    use msime_engine::host::EngineOptions;
    use std::fs;
    use std::path::Path;

    let root = tempfile::tempdir().unwrap();
    let active = root.path().join("active");
    let staged = root.path().join("staged");
    let dictionaries = if nested_dictionaries {
        "user/dictionaries/generation"
    } else {
        "dictionaries"
    };
    for name in ["resources", "user", "cache", dictionaries] {
        fs::create_dir_all(active.join(name)).unwrap();
        fs::create_dir_all(staged.join(name)).unwrap();
        fs::write(active.join(name).join("marker"), b"old").unwrap();
        fs::write(staged.join(name).join("marker"), b"new").unwrap();
    }
    let activation_id = "00112233-4455-6677-8899-aabbccddeeff";
    fs::write(
        staged.join("user").join(super::ACTIVATION_RECEIPT_NAME),
        activation_id,
    )
    .unwrap();
    let make = |base: &Path| EngineOptions {
        resources: base.join("resources").to_str().unwrap().into(),
        user_data: base.join("user").to_str().unwrap().into(),
        cache: base.join("cache").to_str().unwrap().into(),
        dictionaries: base.join(dictionaries).to_str().unwrap().into(),
        scheme: 0,
        enabled_schemes: msime_engine::SchemeSet::ALL,
        shuangpin_profile: 0,
        shuangpin_custom_profile: None,
        shuangpin_preedit_uses_raw: true,
        single_character_only: false,
        learning: false,
        autocorrect_transposition: true,
        autocorrect_neighbor: true,
        fuzzy_pinyin_rules: 0,
        wubi_mixed_pinyin: false,
        wubi_profile: 0,
        helpcode: false,
        show_helpcode: true,
        helpcode_schema: "ziranma".into(),
        chinese_punctuation: true,
        paired_punctuation: true,
        punctuation_lock: 0,
        frequency_mode: "promote".into(),
        frequency_trigger_count: 1,
        frequency_linear_step: 1,
        mixed_english: true,
        english_minimum_prefix: 2,
        mixed_emoji: false,
        mixed_kaomoji: false,
        local_unicode: true,
        local_date_time: true,
        local_quick_phrase: true,
        local_emoji: true,
        local_kaomoji: true,
        local_super_jianpin: true,
        local_temporary_english: true,
        local_temporary_japanese: true,
        local_expression: false,
        local_command: false,
        local_mention: false,
        command_table: Vec::new(),
        mention_entries: Vec::new(),
        quick_phrase_table: Vec::new(),
        helpcode_table: None,
        sentence_alternatives: true,
        vietnamese_input_method: 0,
        vietnamese_tone_style: 0,
        cantonese_dictionary: String::new(),
        zhuyin_dictionary: String::new(),
        stroke_dictionary: String::new(),
        japanese_dictionary: String::new(),
        sentence_association: msime_engine::host::SentenceAssociationOptions {
            word_lattice: true,
            neural_keyboard: false,
            show_next_on_duplicate: false,
        },
        rescoring_context: String::new(),
    };
    let active_options = make(&active);
    let staged_options = make(&staged);
    let expected = super::version_without_access(&active_options).unwrap();
    let directory = tempfile::tempdir_in(root.path()).unwrap();
    if !hold_session {
        // The replacement generation can be syntactically well-formed while
        // still being rejected by the Engine.  Probe that failure before any
        // state root is moved and keep the old generation readable.
        let translation_source = staged.join("user/custom_translations.txt");
        let translation_target = staged.join(dictionaries).join("custom_translations.txt");
        fs::write(&translation_source, b"fixture").unwrap();
        fs::create_dir(&translation_target).unwrap();
        let unusable_handle = super::NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        registry().lock().unwrap().insert(
            unusable_handle,
            Prepared {
                directory: tempfile::tempdir_in(root.path()).unwrap(),
                active_options: active_options.clone(),
                options: staged_options.clone(),
                source_version: expected.clone(),
            },
        );
        assert!(activate(unusable_handle, &expected).is_err());
        assert!(registry().lock().unwrap().contains_key(&unusable_handle));
        for name in ["user", "cache", dictionaries] {
            assert_eq!(fs::read(active.join(name).join("marker")).unwrap(), b"old");
        }
        discard(unusable_handle).unwrap();
        fs::remove_file(translation_source).unwrap();
        fs::remove_dir(translation_target).unwrap();
    }
    registry().lock().unwrap().insert(
        handle,
        Prepared {
            directory,
            active_options: active_options.clone(),
            options: staged_options,
            source_version: expected.clone(),
        },
    );
    let session_access = if hold_session {
        Some(
            DictionaryAccess::try_session(
                Path::new(&active_options.user_data),
                Path::new(&active_options.dictionaries),
            )
            .unwrap()
            .unwrap(),
        )
    } else {
        None
    };
    if let Some(session_access) = session_access {
        assert!(activate(handle, &expected).is_err());
        for name in ["user", "cache", dictionaries] {
            assert_eq!(fs::read(active.join(name).join("marker")).unwrap(), b"old");
        }
        drop(session_access);
    }
    let wrong = "0".repeat(64);
    assert!(activate(handle, &wrong).is_err());
    for name in ["user", "cache", dictionaries] {
        assert_eq!(fs::read(active.join(name).join("marker")).unwrap(), b"old");
    }
    fs::remove_dir_all(staged.join("cache")).unwrap();
    assert!(activate(handle, &expected).is_err());
    for name in ["user", "cache", dictionaries] {
        assert_eq!(fs::read(active.join(name).join("marker")).unwrap(), b"old");
    }
    let backup_path = |path: &Path| {
        path.with_file_name(format!(
            "{}.msime-snapshot-old-{handle}",
            path.file_name().unwrap().to_string_lossy(),
        ))
    };
    for path in [
        backup_path(&active.join("user")),
        backup_path(&active.join("cache")),
        backup_path(&active.join(dictionaries)),
    ] {
        assert!(!path.exists());
    }
    fs::create_dir_all(staged.join("cache")).unwrap();
    fs::write(staged.join("cache").join("marker"), b"new").unwrap();
    let outside_backup = tempfile::tempdir_in(root.path()).unwrap();
    fs::write(outside_backup.path().join("sentinel"), b"keep").unwrap();
    let linked_backup = backup_path(&active.join("user"));
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside_backup.path(), &linked_backup).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(outside_backup.path(), &linked_backup).unwrap();
    assert!(activate(handle, &expected).is_err());
    assert_eq!(
        fs::read(outside_backup.path().join("sentinel")).unwrap(),
        b"keep"
    );
    fs::remove_file(&linked_backup).unwrap();
    // Other tests spawn processes concurrently, and a fork can briefly inherit the dropped session's locked file description before close-on-exec runs, so maintenance access may read busy for a moment. Same allowance as the access lock's own test.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let moved_user = active.with_file_name("moved-user");
    let mut injected = false;
    let activated = loop {
        let result = if replace_live_user && !injected {
            injected = true;
            activate_with_hook(handle, &expected, || {
                fs::rename(active.join("user"), &moved_user).unwrap();
                fs::create_dir(active.join("user")).unwrap();
                fs::write(active.join("user").join("marker"), b"attacker").unwrap();
            })
        } else {
            activate(handle, &expected)
        };
        match result {
            Err("snapshot access busy") => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "released dictionary lock remained busy"
                );
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            result => break result.unwrap(),
        }
    };
    assert_eq!(activated, serde_json::json!({"activated": true}));
    for name in ["cache", dictionaries] {
        assert_eq!(fs::read(active.join(name).join("marker")).unwrap(), b"new");
    }
    if replace_live_user {
        assert_eq!(
            fs::read(active.join("user").join("marker")).unwrap(),
            b"attacker"
        );
        assert_eq!(fs::read(moved_user.join("marker")).unwrap(), b"new");
    } else {
        assert_eq!(
            fs::read(active.join("user").join("marker")).unwrap(),
            b"new"
        );
    }
    let receipt_options = if replace_live_user {
        let mut options = active_options.clone();
        options.user_data = moved_user.to_str().unwrap().into();
        options
    } else {
        active_options.clone()
    };
    assert_eq!(
        super::activation_receipt(&receipt_options)
            .unwrap()
            .as_deref(),
        Some(activation_id)
    );
    assert!(!registry().lock().unwrap().contains_key(&handle));
    assert!(activate(handle, &expected).is_err());
}

#[test]
fn a_backup_that_still_holds_the_user_s_data_survives_the_cleanup() {
    use std::fs;

    // Rollback puts the original contents back with renames that are best effort. One of those
    // failing is precisely the case where the backup is the only remaining copy, and the cleanup
    // that follows used to be `remove_dir_all` - it would have deleted the user's dictionaries on
    // the way out of a failure they had already survived.
    let root = tempfile::tempdir().unwrap();

    let recovered = root.path().join("rollback-emptied-this");
    fs::create_dir_all(&recovered).unwrap();
    super::discard_recovered_backup(&recovered);
    assert!(!recovered.exists(), "an emptied backup is cleaned up");

    let stranded = root.path().join("rollback-could-not-empty-this");
    fs::create_dir_all(&stranded).unwrap();
    fs::write(stranded.join("user.db"), b"the only copy").unwrap();
    super::discard_recovered_backup(&stranded);
    assert_eq!(
        fs::read(stranded.join("user.db")).unwrap(),
        b"the only copy",
        "a backup with anything left in it is kept, whatever it costs in space"
    );
}

/// Activation replaces `msime_user.db` at the same path, as `reset_learned_data` does, and must close the process's cached journal and personal-context connections first, as reset does (reset.rs). Otherwise the personal-context store keeps writing into the replaced, deleted journal and serving its counts, so what a new session learns after the restore is lost.
#[test]
fn activation_reopens_the_personal_context_store_on_the_restored_journal() {
    use super::*;
    use msime_engine::host::{EngineOptions, Session};
    use std::fs;
    use std::path::Path;

    let root = tempfile::tempdir().unwrap();
    let active = root.path().join("active");
    let staged = root.path().join("staged");
    let fixture = "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                   INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',200),('ni''hao','nh','拟好',100);
                   CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                   CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);";
    for base in [&active, &staged] {
        for name in ["resources", "user", "cache", "dictionaries"] {
            fs::create_dir_all(base.join(name)).unwrap();
        }
        for name in ["resources", "dictionaries"] {
            rusqlite::Connection::open(base.join(name).join("msime-pinyin.db"))
                .unwrap()
                .execute_batch(fixture)
                .unwrap();
        }
    }
    let make = |base: &Path| EngineOptions {
        resources: base.join("resources").to_str().unwrap().into(),
        user_data: base.join("user").to_str().unwrap().into(),
        cache: base.join("cache").to_str().unwrap().into(),
        dictionaries: base.join("dictionaries").to_str().unwrap().into(),
        scheme: 0,
        enabled_schemes: msime_engine::SchemeSet::ALL,
        shuangpin_profile: 0,
        shuangpin_custom_profile: None,
        shuangpin_preedit_uses_raw: true,
        single_character_only: false,
        learning: true,
        autocorrect_transposition: true,
        autocorrect_neighbor: true,
        fuzzy_pinyin_rules: 0,
        wubi_mixed_pinyin: false,
        wubi_profile: 0,
        helpcode: false,
        show_helpcode: true,
        helpcode_schema: "ziranma".into(),
        chinese_punctuation: true,
        paired_punctuation: true,
        punctuation_lock: 0,
        frequency_mode: "disabled".into(),
        frequency_trigger_count: 1,
        frequency_linear_step: 1,
        mixed_english: false,
        english_minimum_prefix: 2,
        mixed_emoji: false,
        mixed_kaomoji: false,
        local_unicode: true,
        local_date_time: true,
        local_quick_phrase: true,
        local_emoji: true,
        local_kaomoji: true,
        local_super_jianpin: true,
        local_temporary_english: true,
        local_temporary_japanese: true,
        local_expression: false,
        local_command: false,
        local_mention: false,
        command_table: Vec::new(),
        mention_entries: Vec::new(),
        quick_phrase_table: Vec::new(),
        helpcode_table: None,
        sentence_alternatives: true,
        vietnamese_input_method: 0,
        vietnamese_tone_style: 0,
        cantonese_dictionary: String::new(),
        zhuyin_dictionary: String::new(),
        stroke_dictionary: String::new(),
        japanese_dictionary: String::new(),
        sentence_association: msime_engine::host::SentenceAssociationOptions {
            word_lattice: true,
            neural_keyboard: false,
            show_next_on_duplicate: false,
        },
        rescoring_context: String::new(),
    };
    let active_options = make(&active);
    let journal = active.join("user").join("msime_user.db");
    let pick = |word: &str| {
        let mut session = Session::new(&active_options).unwrap();
        for byte in b"nihao" {
            session.character(*byte, false).unwrap();
        }
        let snapshot = session.snapshot().unwrap();
        let index = snapshot
            .candidates
            .iter()
            .position(|candidate| candidate == word)
            .unwrap_or_else(|| panic!("{word} is not offered: {:?}", snapshot.candidates));
        assert!(session.select(index).unwrap().has_commit);
        // Dropping the session writes its queued context.
    };
    let learned = |word: &str| -> i64 {
        rusqlite::Connection::open(&journal)
            .unwrap()
            .query_row(
                "SELECT count(*) FROM personal_bigram WHERE previous=char(1) AND word=?1",
                [word],
                |row| row.get(0),
            )
            .unwrap_or(0)
    };
    pick("拟好");
    assert_eq!(learned("拟好"), 1);

    let handle = 131;
    let expected = super::version_without_access(&active_options).unwrap();
    registry().lock().unwrap().insert(
        handle,
        Prepared {
            directory: tempfile::tempdir_in(root.path()).unwrap(),
            active_options: active_options.clone(),
            options: make(&staged),
            source_version: expected.clone(),
        },
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match activate(handle, &expected) {
            Err("snapshot access busy") if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            result => {
                result.unwrap();
                break;
            }
        }
    }
    assert!(!journal.exists(), "the restored state had no journal");

    pick("你好");
    assert_eq!(learned("你好"), 1, "learning after the restore was lost");
    assert_eq!(learned("拟好"), 0, "the pre-restore context came back");
}

/// 快照准备与 `prepare_host_configuration` 用同一条发货规则：给出按需清单时，不含日文词典的资源目录通过校验；不给时照旧拒绝。
#[test]
fn snapshot_preparation_accepts_resources_shipped_without_the_on_demand_pair() {
    use super::*;
    use msime_client_core::resources::ON_DEMAND_JAPANESE_ARTIFACTS;
    use std::fs;

    let root = tempfile::tempdir().unwrap();
    let resources = root.path().join("resources");
    let specification = crate::tests::synthetic_desktop_lock(&resources);
    fs::remove_file(resources.join("msime-japanese.dat")).unwrap();
    fs::remove_file(resources.join("msime-mozc_dictionary_oss_README.txt")).unwrap();
    fs::remove_file(resources.join("msime-mozc_LICENSE.txt")).unwrap();
    for name in ["user", "cache", "dictionaries", "staging"] {
        fs::create_dir_all(root.path().join(name)).unwrap();
    }
    let document = serde_json::json!({
        "api_version": 1,
        "resources": resources,
        "user_data": root.path().join("user"),
        "cache": root.path().join("cache"),
        "dictionaries": root.path().join("dictionaries"),
        "preferences": msime_client_core::preferences::Preferences::default(),
    });
    let request = || -> PrepareRequest {
        let options: HostOptions = serde_json::from_value(document.clone()).unwrap();
        let expected_version = version(&options.clone().into_engine_options()).unwrap();
        PrepareRequest {
            options,
            staging_root: root.path().join("staging").to_str().unwrap().into(),
            expected_version,
            records: 0,
            activation_id: None,
        }
    };
    let rejected = |on_demand: &[&str]| {
        matches!(
            prepare(request(), &specification, on_demand, std::iter::empty()),
            Err("snapshot resources rejected")
        )
    };
    assert!(rejected(&[]));
    assert!(!rejected(&ON_DEMAND_JAPANESE_ARTIFACTS));
}

/// 输入记录（#5659）用例共用的一台「设备」：随包词库只有空表，日志里有一个用户自己的词，以及一条学习调权、一条删除记录、一个固定位置和一条选词计数。
struct LearningDevice {
    root: tempfile::TempDir,
    host: serde_json::Value,
    options: msime_engine::host::EngineOptions,
}

impl LearningDevice {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        for name in ["resources", "user", "cache", "dictionaries"] {
            std::fs::create_dir_all(root.path().join(name)).unwrap();
        }
        for name in ["resources", "dictionaries"] {
            rusqlite::Connection::open(root.path().join(name).join("msime-pinyin.db"))
                .unwrap()
                .execute_batch(
                    "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                     CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                     CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);
                     CREATE INDEX idx_quick_parases_key_weight ON quick_parases(key,weight DESC);",
                )
                .unwrap();
            msime_engine::ensure_english_schema(&root.path().join(name).join("msime-english.db"))
                .unwrap();
        }
        let host = serde_json::json!({
            "api_version": 1,
            "resources": root.path().join("resources"),
            "user_data": root.path().join("user"),
            "cache": root.path().join("cache"),
            "dictionaries": root.path().join("dictionaries"),
            "preferences": crate::Preferences::default(),
            "preferences_directory": root.path(),
        });
        let options = super::parse_options(host.to_string().as_bytes()).unwrap();
        Self {
            root,
            host,
            options,
        }
    }

    fn with_learning() -> Self {
        let device = Self::new();
        msime_engine::host::dictionary_edit(
            &device.options,
            None,
            Some(&msime_engine::host::DictionaryEntry {
                kind: msime_engine::host::DictionaryKind::Pinyin,
                key: "ni'hao".into(),
                value: "你好".into(),
                weight: 100,
            }),
            "seed-own-word",
        )
        .unwrap();
        device.sql(
            "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted) VALUES('pinyin','ni''hao','拟好','upsert',200,'',0),('pinyin','ni''hao','泥好','delete',0,'',1);
             INSERT INTO fixed_candidate_positions(context_key,entry_key,value,position) VALUES('ni''hao','ni''hao','拟好',2);
             INSERT INTO candidate_selection_state(context_key,entry_key,value,selection_count) VALUES('ni''hao','ni''hao','你好',7);",
        );
        device
    }

    fn journal(&self) -> std::path::PathBuf {
        self.root.path().join("user").join("msime_user.db")
    }

    fn sql(&self, statements: &str) {
        rusqlite::Connection::open(self.journal())
            .unwrap()
            .execute_batch(statements)
            .unwrap();
    }

    fn number(&self, query: &str) -> Option<i64> {
        use rusqlite::OptionalExtension;
        rusqlite::Connection::open(self.journal())
            .unwrap()
            .query_row(query, [], |row| row.get(0))
            .optional()
            .unwrap()
    }

    fn request(&self, action: serde_json::Value) -> Result<serde_json::Value, String> {
        crate::dictionary::dictionary_request_json(
            serde_json::json!({"options": self.host, "action": action})
                .to_string()
                .as_bytes(),
        )
    }

    fn merge_pending(&self) -> Result<serde_json::Value, String> {
        self.request(serde_json::json!({"operation": "merge_pending_learning"}))
    }

    fn export(&self, name: &str, action: serde_json::Value) -> std::path::PathBuf {
        let destination = self.root.path().join(name);
        let mut action = action;
        action["operation"] = serde_json::json!("export_snapshot");
        action["destination"] = serde_json::json!(destination);
        self.request(action).unwrap();
        destination
    }
}

/// 加这个参数之前的导出算法原样抄在这里，作为云同步快照不变的基准。
fn legacy_snapshot(rows: &[(&str, &str, &str, i64)], updated_at: &str) -> Vec<u8> {
    use serde_json::json;
    use sha2::{Digest, Sha256};
    let mut body = Vec::new();
    let mut push = |line: serde_json::Value| {
        body.extend_from_slice(line.to_string().as_bytes());
        body.push(b'\n');
    };
    push(json!({
        "type": "header",
        "format": "msime-dictionary-snapshot",
        "version": 1,
        "revision": 1,
    }));
    let id = |kind: &str, code: &str, word: &str| {
        let mut digest = Sha256::new();
        digest.update(kind.as_bytes());
        digest.update(b"\t");
        digest.update(code.as_bytes());
        digest.update(b"\t");
        digest.update(word.as_bytes());
        hex::encode(&digest.finalize()[..16])
    };
    for (kind, code, word, weight) in rows {
        push(json!({"type": "entry", "data": {
            "id": id(kind, code, word),
            "kind": kind,
            "code": code,
            "word": word,
            "weight": weight,
            "revision": 1,
            "updated_at": updated_at,
        }}));
    }
    for (kind, code, word, weight) in rows {
        push(json!({"type": "overlay", "deleted": false, "data": {
            "id": id(kind, code, word),
            "kind": kind,
            "code": code,
            "word": word,
            "weight": weight,
            "revision": 1,
            "updated_at": updated_at,
            "user_inserted": true,
        }}));
    }
    let records = 1 + rows.len() * 2;
    let checksum = hex::encode(Sha256::digest(&body));
    body.extend_from_slice(
        json!({"type": "footer", "records": records, "sha256": checksum})
            .to_string()
            .as_bytes(),
    );
    body.push(b'\n');
    body
}

#[test]
fn an_export_without_learning_is_byte_for_byte_the_cloud_snapshot() {
    let device = LearningDevice::with_learning();
    for (name, action) in [
        ("omitted.ndjson", serde_json::json!({})),
        (
            "explicit.ndjson",
            serde_json::json!({"include_learning": false}),
        ),
    ] {
        let path = device.export(name, action);
        let bytes = std::fs::read(&path).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        let entry: serde_json::Value = serde_json::from_str(text.lines().nth(1).unwrap()).unwrap();
        let updated_at = entry["data"]["updated_at"].as_str().unwrap();
        assert_eq!(
            bytes,
            legacy_snapshot(&[("pinyin", "ni'hao", "你好", 100)], updated_at),
            "{name}"
        );
    }
    let exported = device
        .request(serde_json::json!({"operation": "export_snapshot", "destination": device.root.path().join("again.ndjson")}))
        .unwrap();
    assert!(exported.get("learning").is_none(), "{exported}");
}

#[test]
fn a_learning_export_carries_the_journal_and_restages_to_the_same_revision() {
    let device = LearningDevice::with_learning();
    let destination = device.root.path().join("learning.ndjson");
    let exported = device
        .request(serde_json::json!({"operation": "export_snapshot", "destination": destination, "include_learning": true}))
        .unwrap();
    assert_eq!(exported["entries"], 1, "{exported}");
    assert_eq!(exported["overlays"], 3, "{exported}");
    assert_eq!(exported["positions"], 1, "{exported}");
    assert_eq!(exported["selections"], 1, "{exported}");
    assert_eq!(exported["learning"], 4, "{exported}");
    assert_eq!(exported["learning_skipped"], 0, "{exported}");
    let metadata = super::inspect_snapshot(&destination).unwrap();
    let text = std::fs::read_to_string(&destination).unwrap();
    assert!(text.contains("\"user_inserted\":false"), "{text}");
    assert!(text.contains("\"deleted\":true"), "{text}");

    // 整份激活走的路：把快照读成记录、另建一代，日志与导出的那台一模一样。
    let generation = device.root.path().join("restaged");
    let staged = msime_engine::host::stage_dictionary_state(
        &device.options,
        generation.to_str().unwrap(),
        "fixture",
        metadata.engine_records,
        super::SnapshotFileRecords::open(&destination).unwrap(),
    )
    .unwrap();
    assert_eq!(
        msime_engine::host::dictionary_state_revision(&staged).unwrap(),
        msime_engine::host::dictionary_state_revision(&device.options).unwrap()
    );
    assert_eq!(
        device
            .request(serde_json::json!({"operation": "learning_count"}))
            .unwrap()["count"],
        4
    );
}

/// 快照格式装不下的输入记录跳过，选词计数截到 10，导出照样通过自检。
#[test]
fn a_learning_export_leaves_out_what_the_format_cannot_carry() {
    let device = LearningDevice::with_learning();
    device.sql(
        "UPDATE candidate_selection_state SET selection_count=12;
         INSERT INTO fixed_candidate_positions(context_key,entry_key,value,position) VALUES('a'||char(9)||'b','ni''hao','你好',1);",
    );
    let destination = device.root.path().join("learning.ndjson");
    let exported = device
        .request(serde_json::json!({"operation": "export_snapshot", "destination": destination, "include_learning": true}))
        .unwrap();
    assert_eq!(exported["learning"], 4, "{exported}");
    assert_eq!(exported["learning_skipped"], 1, "{exported}");
    let text = std::fs::read_to_string(&destination).unwrap();
    assert!(text.contains("\"count\":10"), "{text}");
}

#[test]
fn queued_learning_merges_when_idle_keeping_local_rows_and_the_larger_count() {
    let source = LearningDevice::with_learning();
    let backup = source.export(
        "backup.ndjson",
        serde_json::json!({"include_learning": true}),
    );

    // 新设备上已经学过一些：拟好有自己的调权，位置 2 固定了拟蒿，你好选过 9 次。
    let device = LearningDevice::new();
    device.sql(
        "CREATE TABLE IF NOT EXISTS user_dictionary_operations(dictionary TEXT NOT NULL,key TEXT NOT NULL,value TEXT NOT NULL,operation TEXT NOT NULL CHECK(operation IN ('upsert','delete')),weight INTEGER NOT NULL DEFAULT 0,display TEXT NOT NULL DEFAULT '',user_inserted INTEGER NOT NULL DEFAULT 0,updated_at INTEGER NOT NULL DEFAULT(unixepoch()),PRIMARY KEY(dictionary,key,value));
         CREATE TABLE IF NOT EXISTS candidate_selection_state(context_key TEXT NOT NULL,entry_key TEXT NOT NULL,value TEXT NOT NULL,selection_count INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(context_key,entry_key,value));
         CREATE TABLE IF NOT EXISTS fixed_candidate_positions(context_key TEXT NOT NULL,entry_key TEXT NOT NULL,value TEXT NOT NULL,position INTEGER NOT NULL CHECK(position BETWEEN 1 AND 5),PRIMARY KEY(context_key,entry_key,value),UNIQUE(context_key,position));
         INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted) VALUES('pinyin','ni''hao','拟好','upsert',50,'',0);
         INSERT INTO fixed_candidate_positions(context_key,entry_key,value,position) VALUES('ni''hao','ni''hao','拟蒿',2);
         INSERT INTO candidate_selection_state(context_key,entry_key,value,selection_count) VALUES('ni''hao','ni''hao','你好',9);",
    );
    let queued = device
        .request(serde_json::json!({"operation": "queue_learning_merge", "source": backup}))
        .unwrap();
    assert_eq!(queued, serde_json::json!({"queued": true, "learning": 4}));
    let pending = device.root.path().join(super::PENDING_LEARNING_NAME);
    let metadata = super::inspect_snapshot(&pending).unwrap();
    assert_eq!(
        (
            metadata.entries,
            metadata.overlays,
            metadata.positions,
            metadata.selections
        ),
        (0, 2, 1, 1)
    );

    // 还有会话开着时拿不到独占访问，文件留着下次再试。
    let session = msime_client_core::dictionary::access::DictionaryAccess::try_session(
        &device.root.path().join("user"),
        &device.root.path().join("dictionaries"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        device.merge_pending().unwrap_err(),
        "dictionary maintenance busy"
    );
    assert!(pending.exists());
    drop(session);

    // 建会话前的个人词库同步不碰它：一份大备份不能拖慢恢复后第一次弹出键盘。
    let synced =
        crate::dictionary::personal_dictionary_sync_json(device.host.to_string().as_bytes())
            .unwrap();
    assert!(synced.get("learning_merged").is_none(), "{synced}");
    assert!(pending.exists());

    assert_eq!(
        device.merge_pending().unwrap(),
        serde_json::json!({"merged": true, "entries": 1, "positions": 0, "selections": 0, "kept": 3, "skipped": 0})
    );
    assert!(!pending.exists());
    assert!(!device
        .root
        .path()
        .join(super::CLAIMED_LEARNING_NAME)
        .exists());
    assert_eq!(
        device.number("SELECT weight FROM user_dictionary_operations WHERE value='拟好'"),
        Some(50)
    );
    assert_eq!(
        device.number("SELECT user_inserted FROM user_dictionary_operations WHERE value='泥好' AND operation='delete'"),
        Some(1)
    );
    // 用户自己的词走个人词库队列，不在待合并的文件里。
    assert_eq!(
        device.number("SELECT count(*) FROM user_dictionary_operations WHERE value='你好'"),
        Some(0)
    );
    assert_eq!(
        device.number("SELECT count(*) FROM fixed_candidate_positions WHERE value='拟好'"),
        Some(0)
    );
    assert_eq!(
        device.number("SELECT selection_count FROM candidate_selection_state WHERE value='你好'"),
        Some(9)
    );
    assert_eq!(
        device.merge_pending().unwrap(),
        serde_json::json!({"merged": false})
    );
}

/// 合并期间设置页又排了一份：只删认领的那份，新排的留到下一次。
#[test]
fn a_queue_made_while_a_claimed_merge_runs_survives_it() {
    let source = LearningDevice::with_learning();
    let first = source.export(
        "first.ndjson",
        serde_json::json!({"include_learning": true}),
    );
    source.sql("INSERT INTO candidate_selection_state(context_key,entry_key,value,selection_count) VALUES('ni','ni','泥',3);");
    let second = source.export(
        "second.ndjson",
        serde_json::json!({"include_learning": true}),
    );

    let device = LearningDevice::new();
    let pending = device.root.path().join(super::PENDING_LEARNING_NAME);
    let claimed = device.root.path().join(super::CLAIMED_LEARNING_NAME);
    let queue = |backup: &std::path::Path| {
        device
            .request(serde_json::json!({"operation": "queue_learning_merge", "source": backup}))
            .unwrap()
    };
    queue(&first);
    // 键盘已经认领了第一份、正在合并时，用户又从另一份备份恢复。
    std::fs::rename(&pending, &claimed).unwrap();
    assert_eq!(queue(&second)["learning"], 5);

    assert_eq!(device.merge_pending().unwrap()["selections"], 1);
    assert!(!claimed.exists());
    assert!(pending.exists(), "the newer queue must survive");
    assert_eq!(
        device.number("SELECT count(*) FROM candidate_selection_state WHERE value='泥'"),
        Some(0)
    );

    assert_eq!(device.merge_pending().unwrap()["selections"], 1);
    assert!(!pending.exists());
    assert_eq!(
        device.number("SELECT selection_count FROM candidate_selection_state WHERE value='泥'"),
        Some(3)
    );
}

/// 写库一直出错时保留文件重试，连续三次后放弃；格式不对的文件直接删掉。
#[test]
fn a_failing_merge_is_retried_then_abandoned() {
    let source = LearningDevice::with_learning();
    let backup = source.export(
        "backup.ndjson",
        serde_json::json!({"include_learning": true}),
    );
    let device = LearningDevice::new();
    // 模拟磁盘满一类的写库错误：往拼音表里插行一律失败。
    rusqlite::Connection::open(device.root.path().join("dictionaries").join("msime-pinyin.db"))
        .unwrap()
        .execute_batch("CREATE TRIGGER synthetic_failure BEFORE INSERT ON tbl_2_n BEGIN SELECT RAISE(ABORT,'synthetic write failure'); END;")
        .unwrap();
    device
        .request(serde_json::json!({"operation": "queue_learning_merge", "source": backup}))
        .unwrap();
    let claimed = device.root.path().join(super::CLAIMED_LEARNING_NAME);
    for _ in 0..2 {
        assert_eq!(
            device.merge_pending().unwrap_err(),
            "learning merge rejected"
        );
        assert!(claimed.exists());
    }
    // 同一事务里的选词计数也没有写进去。
    assert_eq!(
        device.number("SELECT count(*) FROM candidate_selection_state"),
        Some(0)
    );
    assert_eq!(
        device.merge_pending().unwrap_err(),
        "learning merge abandoned"
    );
    assert!(!claimed.exists());
    assert!(!device
        .root
        .path()
        .join(super::LEARNING_ATTEMPTS_NAME)
        .exists());
    assert_eq!(
        device.merge_pending().unwrap(),
        serde_json::json!({"merged": false})
    );

    std::fs::write(
        device.root.path().join(super::PENDING_LEARNING_NAME),
        b"not a snapshot\n",
    )
    .unwrap();
    assert_eq!(
        device.merge_pending().unwrap_err(),
        "invalid snapshot document"
    );
    assert!(!claimed.exists());
}

/// 日志整体读不出来（这里是一行不是 UTF-8 的学习调权）时备份照样导出词，告诉宿主输入记录没带上。
#[test]
fn a_learning_export_falls_back_to_words_when_the_journal_cannot_be_read() {
    let device = LearningDevice::with_learning();
    device.sql(
        "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted) VALUES('pinyin','ni''hao',CAST(x'ff' AS TEXT),'upsert',90,'',0);",
    );
    let destination = device.root.path().join("learning.ndjson");
    let exported = device
        .request(serde_json::json!({"operation": "export_snapshot", "destination": destination, "include_learning": true}))
        .unwrap();
    assert_eq!(exported["entries"], 1, "{exported}");
    assert_eq!(exported["learning"], 0, "{exported}");
    assert_eq!(
        exported["learning_error"], "dictionary read rejected",
        "{exported}"
    );
    super::inspect_snapshot(&destination).unwrap();
}

/// 旧版本导出的备份（快照里只有词）照样能恢复：没有输入记录可排，什么也不写。
#[test]
fn a_backup_without_learning_queues_nothing() {
    let source = LearningDevice::with_learning();
    let backup = source.export("legacy.ndjson", serde_json::json!({}));
    let device = LearningDevice::new();
    assert_eq!(
        device
            .request(serde_json::json!({"operation": "queue_learning_merge", "source": backup}))
            .unwrap(),
        serde_json::json!({"queued": false, "learning": 0})
    );
    assert!(!device
        .root
        .path()
        .join(super::PENDING_LEARNING_NAME)
        .exists());
    assert_eq!(
        device
            .request(serde_json::json!({"operation": "learning_count"}))
            .unwrap()["count"],
        0
    );
    assert!(device
        .request(
            serde_json::json!({"operation": "queue_learning_merge", "source": "relative.ndjson"})
        )
        .is_err());
}

/// 输入习惯（#5659）：导出、校验、排队，键盘空闲时随输入记录一起合并，计数取大、置顶保留本机；计数也让整份激活的判断知道本机学过东西。
#[test]
fn learning_habits_export_queue_and_merge_when_idle() {
    let source = LearningDevice::with_learning();
    source.sql(
        "INSERT INTO personal_bigram(previous,word,count) VALUES(char(1),'我',4),('我','想',2);
         INSERT INTO pick_transitions(previous_key,previous_value,key,value,count,updated_at) VALUES('wo','我','xiang','想',3,100);
         INSERT INTO pinyin_typo_counts(typed,intended,accepted,updated_at) VALUES('jai','jia',5,100);
         INSERT INTO pinned_candidates(context_key,value,updated_at) VALUES('ni','你',100),('hao','好',100);",
    );
    let file = source.root.path().join("habits.ndjson");
    let exported = source
        .request(serde_json::json!({"operation": "export_habits", "destination": file}))
        .unwrap();
    assert_eq!(exported["habits"], 6);
    assert_eq!(exported["skipped"], 0);
    assert_eq!(
        source
            .request(serde_json::json!({"operation": "inspect_habits", "source": file}))
            .unwrap(),
        serde_json::json!({"habits": 6})
    );
    assert_eq!(
        source
            .request(serde_json::json!({"operation": "learning_count"}))
            .unwrap()["habits"],
        6
    );

    let device = LearningDevice::new();
    assert_eq!(
        device
            .request(serde_json::json!({"operation": "learning_count"}))
            .unwrap(),
        serde_json::json!({"count": 0, "habits": 0})
    );
    device.sql(
        "CREATE TABLE IF NOT EXISTS personal_bigram(previous TEXT NOT NULL,word TEXT NOT NULL,count INTEGER NOT NULL CHECK(count>0),PRIMARY KEY(previous,word)) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS pinned_candidates(context_key TEXT PRIMARY KEY,value TEXT NOT NULL,updated_at INTEGER NOT NULL DEFAULT(unixepoch()));
         INSERT INTO personal_bigram(previous,word,count) VALUES('我','想',5);
         INSERT INTO pinned_candidates(context_key,value,updated_at) VALUES('ni','泥',200);",
    );
    assert_eq!(
        device
            .request(serde_json::json!({"operation": "queue_habits_merge", "source": file}))
            .unwrap(),
        serde_json::json!({"queued": true, "habits": 6})
    );
    let pending = device.root.path().join(super::habits::HABITS_FILES.pending);
    assert!(pending.exists());

    let session = msime_client_core::dictionary::access::DictionaryAccess::try_session(
        &device.root.path().join("user"),
        &device.root.path().join("dictionaries"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        device.merge_pending().unwrap(),
        serde_json::json!({"merged": false, "habits": {"merged": false, "error": "dictionary maintenance busy"}})
    );
    assert!(pending.exists());
    drop(session);

    assert_eq!(
        device.merge_pending().unwrap(),
        serde_json::json!({"merged": false, "habits": {"merged": true, "written": 4, "kept": 2, "trimmed": 0}})
    );
    assert!(!pending.exists());
    assert_eq!(
        device.number("SELECT count FROM personal_bigram WHERE previous='我' AND word='想'"),
        Some(5)
    );
    assert_eq!(
        device.number("SELECT count FROM personal_bigram WHERE previous=char(1) AND word='我'"),
        Some(4)
    );
    assert_eq!(
        device
            .number("SELECT count(*) FROM pinned_candidates WHERE context_key='ni' AND value='泥'"),
        Some(1)
    );
    assert_eq!(
        device.number("SELECT accepted FROM pinyin_typo_counts WHERE typed='jai'"),
        Some(5)
    );
    assert_eq!(
        device.merge_pending().unwrap(),
        serde_json::json!({"merged": false})
    );
}

/// 坏掉的输入习惯文件在排队时就被拒，什么也不写；已经排下却坏了的文件在合并时直接删掉。
#[test]
fn a_damaged_habits_file_is_refused_and_a_damaged_queue_is_dropped() {
    let source = LearningDevice::new();
    source.sql(
        "CREATE TABLE IF NOT EXISTS personal_bigram(previous TEXT NOT NULL,word TEXT NOT NULL,count INTEGER NOT NULL CHECK(count>0),PRIMARY KEY(previous,word)) WITHOUT ROWID;
         INSERT INTO personal_bigram(previous,word,count) VALUES('我','想',2);",
    );
    let file = source.root.path().join("habits.ndjson");
    source
        .request(serde_json::json!({"operation": "export_habits", "destination": file}))
        .unwrap();
    let text = std::fs::read_to_string(&file)
        .unwrap()
        .replace("\"count\":2", "\"count\":3");
    std::fs::write(&file, text).unwrap();

    let device = LearningDevice::new();
    assert_eq!(
        device
            .request(serde_json::json!({"operation": "queue_habits_merge", "source": file}))
            .unwrap_err(),
        "invalid habits document"
    );
    let pending = device.root.path().join(super::habits::HABITS_FILES.pending);
    assert!(!pending.exists());

    std::fs::write(&pending, b"not habits\n").unwrap();
    assert_eq!(
        device.merge_pending().unwrap(),
        serde_json::json!({"merged": false, "habits": {"merged": false, "error": "invalid habits document"}})
    );
    assert!(!pending.exists());
    assert!(!device
        .root
        .path()
        .join(super::habits::HABITS_FILES.claimed)
        .exists());
}

/// 恢复前的只读校验：完整的快照返回元数据，截断的报错。
#[test]
fn a_snapshot_is_inspected_without_side_effects() {
    let source = LearningDevice::with_learning();
    let backup = source.export(
        "backup.ndjson",
        serde_json::json!({"include_learning": true}),
    );
    let inspected = source
        .request(serde_json::json!({"operation": "inspect_snapshot", "source": backup}))
        .unwrap();
    assert_eq!(inspected["entries"], 1);
    let bytes = std::fs::read(&backup).unwrap();
    std::fs::write(&backup, &bytes[..bytes.len() / 2]).unwrap();
    assert!(source
        .request(serde_json::json!({"operation": "inspect_snapshot", "source": backup}))
        .is_err());
}
