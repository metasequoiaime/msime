use super::*;

#[test]
fn cantonese_simplified_output_learns_the_dictionary_spelling() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let cantonese = cantonese_dictionary(fixture.path());
    let configure = cantonese_learning_options(&cantonese, FrequencyAdjustmentMode::Pin);
    let mut session = fixture.session_with(|options| {
        configure(options);
        options.cantonese_output_conversion =
            Some(crate::TextConversion(synthetic_cantonese_simplification));
    });
    type_text(&mut session, "hou");
    assert_eq!(words(&session), ["好", "号"]);
    let result = session.select(1);
    assert_eq!(result.commit.as_deref(), Some("号"));
    assert_eq!(result.diagnostic, None);
    drop(session);
    crate::flush_personal_learning();

    let rows = cantonese_journal_rows(&fixture);
    assert_eq!(rows.len(), 1);
    assert_eq!((&*rows[0].0, &*rows[0].1, rows[0].3), ("hou", "號", false));
    let mut simplified = fixture.session_with(|options| {
        configure(options);
        options.cantonese_output_conversion =
            Some(crate::TextConversion(synthetic_cantonese_simplification));
    });
    type_text(&mut simplified, "hou");
    assert_eq!(words(&simplified), ["号", "好"]);
    let mut traditional = fixture.session_with(&configure);
    type_text(&mut traditional, "hou");
    assert_eq!(words(&traditional), ["號", "好"]);
}

#[test]
fn cantonese_simplified_pieces_keep_the_dictionary_spelling_in_new_words() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let cantonese = cantonese_dictionary(fixture.path());
    Connection::open(&cantonese)
        .unwrap()
        .execute_batch("UPDATE entries SET weight=6000 WHERE text='妳';")
        .unwrap();
    let configure = cantonese_learning_options(&cantonese, FrequencyAdjustmentMode::Disabled);
    let mut session = fixture.session_with(|options| {
        configure(options);
        options.cantonese_output_conversion =
            Some(crate::TextConversion(synthetic_cantonese_simplification));
    });
    type_text(&mut session, "neihou");
    assert_eq!(words(&session), ["你好", "你"]);
    assert_eq!(session.select(1).commit.as_deref(), Some("你"));
    assert_eq!(words(&session), ["好", "号"]);
    assert_eq!(session.select(1).commit.as_deref(), Some("号"));
    drop(session);
    crate::flush_personal_learning();

    let rows = cantonese_journal_rows(&fixture);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (&*rows[0].0, &*rows[0].1, rows[0].3),
        ("nei hou", "妳號", true)
    );
    let mut traditional = fixture.session_with(&configure);
    type_text(&mut traditional, "neihou");
    assert_eq!(words(&traditional)[0], "妳號");
}

#[test]
fn cantonese_simplified_composed_sentence_keeps_the_dictionary_spelling() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let cantonese = cantonese_dictionary(fixture.path());
    Connection::open(&cantonese)
        .unwrap()
        .execute_batch("UPDATE entries SET weight=5000 WHERE text='號';")
        .unwrap();
    let configure = cantonese_learning_options(&cantonese, FrequencyAdjustmentMode::Pin);
    let mut session = fixture.session_with(|options| {
        configure(options);
        options.cantonese_output_conversion =
            Some(crate::TextConversion(synthetic_cantonese_simplification));
    });
    type_text(&mut session, "ngo'hou");
    assert_eq!(words(&session)[0], "我号");
    assert_eq!(session.select(0).commit.as_deref(), Some("我号"));
    drop(session);
    crate::flush_personal_learning();
    let rows = cantonese_journal_rows(&fixture);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (&*rows[0].0, &*rows[0].1, rows[0].3),
        ("ngo hou", "我號", true)
    );
}
