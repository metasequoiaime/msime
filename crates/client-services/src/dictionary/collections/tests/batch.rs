use super::*;

#[test]
fn batch_disable_and_enable_keeps_unselected_and_preexisting_words() {
    let mut fixture = Fixture::new();
    let shared = english("shared");
    let owned = english("owned");
    let preexisting = english("original");
    fixture
        .dictionary
        .insert(preexisting.identity(), preexisting.weight);
    let first = create_with(
        &fixture,
        "甲",
        &[shared.clone(), owned.clone(), preexisting.clone()],
    );
    let second = create_with(&fixture, "乙", std::slice::from_ref(&owned));
    let third = create_with(&fixture, "丙", std::slice::from_ref(&shared));
    fixture.drain();
    let selected = vec![first.clone(), second.clone(), first];
    fixture.store.set_enabled_many(&selected, false).unwrap();
    fixture.drain();
    assert!(!fixture.has(&owned));
    assert!(fixture.has(&shared));
    assert!(fixture.has(&preexisting));
    let view = fixture.store.load().unwrap();
    assert!(
        view.collections
            .iter()
            .find(|item| item.id.to_string() == third)
            .unwrap()
            .enabled
    );
    fixture.store.set_enabled_many(&selected, true).unwrap();
    fixture.drain();
    assert!(fixture.has(&owned));
    assert_eq!(fixture.dictionary.len(), 3);
}

#[test]
fn batch_delete_removes_selected_files_and_preserves_shared_word_owner() {
    let mut fixture = Fixture::new();
    let shared = english("shared");
    let owned = english("owned");
    let first = create_with(&fixture, "甲", &[shared.clone(), owned.clone()]);
    let second = create_with(&fixture, "乙", std::slice::from_ref(&owned));
    let remaining = create_with(&fixture, "丙", std::slice::from_ref(&shared));
    fixture.drain();
    fixture
        .store
        .delete_many(&[first.clone(), second.clone(), first.clone()])
        .unwrap();
    fixture.drain();
    assert!(fixture.has(&shared));
    assert!(!fixture.has(&owned));
    let view = fixture.store.load().unwrap();
    assert_eq!(view.collections.len(), 1);
    assert_eq!(view.collections[0].id.to_string(), remaining);
    assert!(!fixture
        .store
        .directory
        .join(format!("{first}.json"))
        .exists());
    assert!(!fixture
        .store
        .directory
        .join(format!("{second}.json"))
        .exists());
}

#[test]
fn invalid_batch_target_does_not_change_the_index_or_queue() {
    let fixture = Fixture::new();
    let first = create_with(&fixture, "甲", &[english("one")]);
    let before = fixture.store.load().unwrap();
    let requests = fixture.requests();
    for ids in [
        vec![],
        vec![first.clone(), "builtin:pinyin".into()],
        vec![first.clone(), Uuid::new_v4().to_string()],
        vec![first; MAX_COLLECTIONS + 1],
    ] {
        assert!(fixture.store.set_enabled_many(&ids, false).is_err());
        assert!(fixture.store.delete_many(&ids).is_err());
        assert_eq!(fixture.store.load().unwrap(), before);
        assert_eq!(fixture.requests(), requests);
    }
}

#[test]
fn existing_group_import_deduplicates_and_keeps_disabled_state() {
    let fixture = Fixture::new();
    let id = create_with(&fixture, "自定义", &[english("one")]);
    fixture.store.set_enabled(&id, false).unwrap();
    let view = fixture
        .store
        .import(ImportRequest {
            id: Some(&id),
            name: None,
            kind: PersonalWordKind::English,
            format: "standard",
            text: "ONE\tone\nTWO\ttwo\nTWO\ttwo\n",
            readings: None,
        })
        .unwrap();
    let report = view.import.unwrap();
    assert_eq!((report.imported, report.duplicates), (1, 2));
    assert_eq!(view.collections[0].name, "自定义");
    assert_eq!(view.collections[0].entry_count, 2);
    assert!(!view.collections[0].enabled);
    assert_eq!(fixture.store.load().unwrap().collections, view.collections);
}

#[test]
fn batch_actions_use_the_shared_wire_contract() {
    let fixture = Fixture::new();
    let id = create_with(&fixture, "甲", &[]);
    for document in [
        serde_json::json!({"operation":"set_enabled_many", "ids":[id], "enabled":false}),
        serde_json::json!({"operation":"delete_many", "ids":[id]}),
    ] {
        fixture
            .store
            .perform(serde_json::from_value(document).unwrap(), None)
            .unwrap();
    }
    assert!(fixture.store.load().unwrap().collections.is_empty());
}

#[test]
fn scel_import_cannot_lose_its_source_in_a_custom_group() {
    let fixture = Fixture::new();
    let view = fixture
        .store
        .create("自定义", PersonalWordKind::Pinyin)
        .unwrap();
    let id = view.collections[0].id.to_string();
    let before = fixture.store.load().unwrap();
    let error = fixture
        .store
        .import_text(ImportRequest {
            id: Some(&id),
            name: None,
            kind: PersonalWordKind::Pinyin,
            format: SCEL_FORMAT,
            text: "测试\tce'shi\n",
            readings: None,
        })
        .unwrap_err();
    assert_eq!(error.code(), "collections_source_conflict");
    assert_eq!(fixture.store.load().unwrap(), before);
}
