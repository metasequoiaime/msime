#[test]
fn snapshot_module_is_present() {
    assert_eq!(super::HANDLE_LIMIT, 8);
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
    activation_case(false, false, 123);
    activation_case(true, false, 123);
}

#[test]
fn activation_rejects_live_session_before_swapping() {
    // The registry is process-global; use a distinct fixture handle so this
    // test can run in parallel with the successful activation cases.
    activation_case(false, true, 125);
    activation_case(true, true, 125);
}

#[test]
fn discard_does_not_require_maintenance_lock_for_live_paths() {
    use super::*;
    use msime_client_core::dictionary::access::DictionaryAccess;
    use msime_engine_bridge::EngineOptions;
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
        shuangpin_profile: 0,
        shuangpin_preedit_uses_raw: true,
        learning: false,
        autocorrect_transposition: true,
        autocorrect_neighbor: true,
        fuzzy_pinyin_rules: 0,
        wubi_mixed_pinyin: false,
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
        sentence_association: msime_engine_bridge::SentenceAssociationOptions {
            word_lattice: true,
            google: true,
            neural_desktop: false,
            neural_keyboard: false,
            show_next_on_duplicate: false,
        },
        rescoring_context: String::new(),
        sentence_alternatives: true,
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

fn activation_case(nested_dictionaries: bool, hold_session: bool, handle: u64) {
    use super::*;
    use msime_client_core::dictionary::access::DictionaryAccess;
    use msime_engine_bridge::EngineOptions;
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
        shuangpin_profile: 0,
        shuangpin_preedit_uses_raw: true,
        learning: false,
        autocorrect_transposition: true,
        autocorrect_neighbor: true,
        fuzzy_pinyin_rules: 0,
        wubi_mixed_pinyin: false,
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
        sentence_alternatives: true,
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
    assert_eq!(fs::read(outside_backup.path().join("sentinel")).unwrap(), b"keep");
    fs::remove_file(&linked_backup).unwrap();
    // Other tests spawn processes concurrently, and a fork can briefly inherit the dropped session's locked file description before close-on-exec runs, so maintenance access may read busy for a moment. Same allowance as the access lock's own test.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let activated = loop {
        match activate(handle, &expected) {
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
    for name in ["user", "cache", dictionaries] {
        assert_eq!(fs::read(active.join(name).join("marker")).unwrap(), b"new");
    }
    assert_eq!(
        super::activation_receipt(&active_options)
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
