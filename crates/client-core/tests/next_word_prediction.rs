use msime_client_core::preferences::{NextWordPredictionButtons, Preferences, PreferencesStore};

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

#[test]
fn next_word_prediction_buttons_default_to_expand_only_and_stay_out_of_the_document() {
    let defaults = Preferences::default();
    assert_eq!(
        defaults.next_word_prediction_buttons,
        NextWordPredictionButtons::ExpandOnly
    );
    let document = serde_json::to_value(&defaults).unwrap();
    assert!(document.get("next_word_prediction_buttons").is_none());
    // 没有这个键的旧文档按现有样式读。
    let legacy: Preferences = serde_json::from_value(document).unwrap();
    assert_eq!(
        legacy.next_word_prediction_buttons,
        NextWordPredictionButtons::ExpandOnly
    );
}

#[test]
fn next_word_prediction_buttons_round_trip_every_style() {
    for (style, wire) in [
        (NextWordPredictionButtons::ExpandOnly, "expand_only"),
        (
            NextWordPredictionButtons::CloseAndExpand,
            "close_and_expand",
        ),
        (NextWordPredictionButtons::CloseOnly, "close_only"),
    ] {
        assert_eq!(style.as_str(), wire);
        assert_eq!(NextWordPredictionButtons::parse(wire), Some(style));
        assert_eq!(
            serde_json::from_value::<NextWordPredictionButtons>(wire.into()).unwrap(),
            style
        );
        let directory = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(directory.path());
        let mut snapshot = store.load().unwrap();
        snapshot.preferences.next_word_prediction_buttons = style;
        store.save(snapshot.revision, snapshot.preferences).unwrap();
        assert_eq!(
            store
                .load()
                .unwrap()
                .preferences
                .next_word_prediction_buttons,
            style
        );
    }
    let mut document = serde_json::to_value(Preferences::default()).unwrap();
    document["next_word_prediction_buttons"] = "close_only".into();
    let written =
        serde_json::to_value(serde_json::from_value::<Preferences>(document).unwrap()).unwrap();
    assert_eq!(written["next_word_prediction_buttons"], "close_only");
}

#[test]
fn next_word_prediction_buttons_read_unknown_styles_as_default_but_reject_other_types() {
    let mut document = serde_json::to_value(Preferences::default()).unwrap();
    document["next_word_prediction_buttons"] = "swipe_to_close".into();
    let unknown: Preferences = serde_json::from_value(document.clone()).unwrap();
    assert_eq!(
        unknown.next_word_prediction_buttons,
        NextWordPredictionButtons::ExpandOnly
    );
    assert_eq!(NextWordPredictionButtons::parse("swipe_to_close"), None);
    document["next_word_prediction_buttons"] = serde_json::Value::Bool(true);
    assert!(serde_json::from_value::<Preferences>(document).is_err());
}
