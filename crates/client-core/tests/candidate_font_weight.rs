use msime_client_core::preferences::{CandidateFontWeight, Preferences, PreferencesStore};

#[test]
fn absent_weight_reads_regular_and_regular_is_not_written() {
    // 出厂值是常规字重，等于这个字段出现之前的画法；默认值不写进文档，旧版本照常读取。
    let original = Preferences::default();
    assert_eq!(original.candidate_font_weight, CandidateFontWeight::Regular);
    let json = serde_json::to_value(&original).unwrap();
    assert!(json.get("candidate_font_weight").is_none());
    let loaded: Preferences = serde_json::from_value(json).unwrap();
    assert_eq!(loaded.candidate_font_weight, CandidateFontWeight::Regular);
}

#[test]
fn chosen_weight_round_trips_through_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    for weight in [
        CandidateFontWeight::Medium,
        CandidateFontWeight::Semibold,
        CandidateFontWeight::Bold,
        CandidateFontWeight::Regular,
    ] {
        let revision = store.load().unwrap().revision;
        let preferences = Preferences {
            candidate_font_weight: weight,
            ..Preferences::default()
        };
        store.save(revision, preferences).unwrap();
        assert_eq!(
            store.load().unwrap().preferences.candidate_font_weight,
            weight
        );
    }
    let document: serde_json::Value = serde_json::to_value(Preferences {
        candidate_font_weight: CandidateFontWeight::Semibold,
        ..Preferences::default()
    })
    .unwrap();
    assert_eq!(document["candidate_font_weight"], "semibold");
}

#[test]
fn unknown_weight_is_refused_on_submission() {
    let mut submission = serde_json::to_value(Preferences::default()).unwrap();
    submission["candidate_font_weight"] = "heavy".into();
    assert!(Preferences::from_submission(submission.clone()).is_err());
    submission["candidate_font_weight"] = 700.into();
    assert!(Preferences::from_submission(submission).is_err());
}

#[test]
fn weights_map_to_opentype_values_and_preedit_stays_at_least_semibold() {
    let table = [
        (CandidateFontWeight::Regular, 400, 600),
        (CandidateFontWeight::Medium, 500, 600),
        (CandidateFontWeight::Semibold, 600, 600),
        (CandidateFontWeight::Bold, 700, 700),
    ];
    for (weight, body, preedit) in table {
        assert_eq!(weight.value(), body);
        assert_eq!(weight.preedit_value(), preedit);
    }
}
