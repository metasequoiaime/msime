use msime_client_core::preferences::{Preferences, PreferencesStore};

#[test]
fn next_word_prediction_defaults_off_and_round_trips_independently_of_sentence_candidates() {
    let defaults = Preferences::default();
    assert!(!defaults.next_word_prediction);
    let mut document = serde_json::to_value(&defaults).unwrap();
    assert!(document.get("next_word_prediction").is_none());
    document["next_word_prediction"] = serde_json::Value::Bool(true);
    let enabled: Preferences = serde_json::from_value(document).unwrap();
    assert!(enabled.next_word_prediction);
    assert_eq!(enabled.sentence_association, defaults.sentence_association);
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let mut snapshot = store.load().unwrap();
    snapshot.preferences.next_word_prediction = true;
    store.save(snapshot.revision, snapshot.preferences).unwrap();
    assert!(store.load().unwrap().preferences.next_word_prediction);
}
