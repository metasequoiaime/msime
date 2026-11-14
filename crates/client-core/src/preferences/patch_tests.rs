use super::*;

#[test]
fn patch_merges_nested_fields_and_replaces_arrays() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let first = store
        .apply_patch(
            0,
            &serde_json::json!({"quanpin": {"autocorrect_neighbor": false}, "fuzzy_pinyin": {"rules": ["z-zh", "n-l"]}}),
        )
        .unwrap();
    assert!(!first.preferences.quanpin.autocorrect_neighbor);
    assert!(first.preferences.quanpin.autocorrect_transposition);
    assert_eq!(first.preferences.fuzzy_pinyin.rules.len(), 2);
    let second = store
        .apply_patch(
            first.revision,
            &serde_json::json!({"fuzzy_pinyin": {"rules": ["f-h"]}}),
        )
        .unwrap();
    assert_eq!(second.preferences.fuzzy_pinyin.rules.len(), 1);
    assert_eq!(store.load().unwrap(), second);
}

#[test]
fn patch_null_removes_a_field_and_rejects_invalid_documents() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let first = store
        .apply_patch(0, &serde_json::json!({"caret_edit_suffix": true}))
        .unwrap();
    let reset = store
        .apply_patch(
            first.revision,
            &serde_json::json!({"caret_edit_suffix": null}),
        )
        .unwrap();
    assert_eq!(
        reset.preferences.caret_edit_suffix,
        Preferences::default().caret_edit_suffix
    );
    let bytes = fs::read(store.path()).unwrap();
    for patch in [
        serde_json::json!({"candidate_page_size": 0}),
        serde_json::json!({"candidate_page_size": "invalid"}),
        serde_json::json!({"fuzzy_pinyin": {"rules": ["future-rule"]}}),
        serde_json::json!({"synthetic_unknown": true}),
        serde_json::json!({"candidate_page_size": null}),
        serde_json::json!(null),
    ] {
        assert!(store.apply_patch(reset.revision, &patch).is_err());
        assert_eq!(fs::read(store.path()).unwrap(), bytes);
    }
}

#[test]
fn patch_conflict_preserves_other_writes_and_existing_unknown_fields() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let mut document = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    document["revision"] = 4.into();
    document["preferences"]["multilingual_input"] = serde_json::json!({
        "enabled": true, "languages": ["chinese", "japanese"],
        "future_policy": {"synthetic": true}
    });
    fs::write(store.path(), serde_json::to_vec(&document).unwrap()).unwrap();
    let saved = store
        .apply_patch(4, &serde_json::json!({"candidate_page_size": 7}))
        .unwrap();
    assert!(matches!(
        store.apply_patch(4, &serde_json::json!({"candidate_page_size": 8})),
        Err(PreferencesError::Conflict)
    ));
    assert_eq!(store.load().unwrap(), saved);
    let written: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(
        written["preferences"]["multilingual_input"]["future_policy"],
        document["preferences"]["multilingual_input"]["future_policy"]
    );
}

#[test]
fn patch_array_replacement_removes_future_elements_even_when_known_values_match() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let mut document = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    document["revision"] = 3.into();
    document["preferences"]["fuzzy_pinyin"]["rules"] = serde_json::json!(["z-zh", "future-rule"]);
    fs::write(store.path(), serde_json::to_vec(&document).unwrap()).unwrap();
    let saved = store
        .apply_patch(3, &serde_json::json!({"fuzzy_pinyin": {"rules": ["z-zh"]}}))
        .unwrap();
    assert_eq!(saved.revision, 4);
    let written: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(
        written["preferences"]["fuzzy_pinyin"]["rules"],
        serde_json::json!(["z-zh"])
    );
}

#[test]
fn patch_rejects_nested_typos_and_changes_to_existing_unknown_fields() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let mut document = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    document["revision"] = 4.into();
    document["preferences"]["quanpin"]["synthetic_future"] = serde_json::json!({"enabled": true});
    fs::write(store.path(), serde_json::to_vec(&document).unwrap()).unwrap();
    let bytes = fs::read(store.path()).unwrap();
    for patch in [
        serde_json::json!({"quanpin": {"synthetic_typo": false}}),
        serde_json::json!({"quanpin": {"synthetic_future": null}}),
        serde_json::json!({"quanpin": {"synthetic_future": {"enabled": false}}}),
    ] {
        assert!(matches!(
            store.apply_patch(4, &patch),
            Err(PreferencesError::InvalidPatch)
        ));
        assert_eq!(fs::read(store.path()).unwrap(), bytes);
    }
    let saved = store
        .apply_patch(
            4,
            &serde_json::json!({"quanpin": {"autocorrect_neighbor": false}}),
        )
        .unwrap();
    assert!(!saved.preferences.quanpin.autocorrect_neighbor);
    let written: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(
        written["preferences"]["quanpin"]["synthetic_future"],
        document["preferences"]["quanpin"]["synthetic_future"]
    );
}

#[test]
fn patch_no_op_preserves_revision_and_future_array_elements() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let mut document = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    document["revision"] = 3.into();
    document["preferences"]["fuzzy_pinyin"]["rules"] = serde_json::json!(["z-zh", "future-rule"]);
    fs::write(store.path(), serde_json::to_vec(&document).unwrap()).unwrap();
    let bytes = fs::read(store.path()).unwrap();
    let saved = store.apply_patch(3, &serde_json::json!({})).unwrap();
    assert_eq!(saved.revision, 3);
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
    store
        .apply_patch(3, &serde_json::json!({"candidate_page_size": 7}))
        .unwrap();
    let written: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(
        written["preferences"]["fuzzy_pinyin"]["rules"],
        document["preferences"]["fuzzy_pinyin"]["rules"]
    );
}

#[test]
fn concurrent_patches_with_the_same_revision_have_one_winner() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let revision = store
        .apply_patch(0, &serde_json::json!({}))
        .unwrap()
        .revision;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let results = std::thread::scope(|scope| {
        let workers: Vec<_> = [7, 8]
            .into_iter()
            .map(|size| {
                let barrier = barrier.clone();
                let path = directory.path();
                scope.spawn(move || {
                    barrier.wait();
                    PreferencesStore::new(path)
                        .apply_patch(revision, &serde_json::json!({"candidate_page_size": size}))
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(PreferencesError::Conflict)))
            .count(),
        1
    );
    let winner = results.into_iter().find_map(Result::ok).unwrap();
    assert_eq!(store.load().unwrap(), winner);
    assert_eq!(winner.revision, revision + 1);
}
