use super::*;
use crate::assets;
use crate::japanese::decoder::test_model;
use crate::quanpin::fixture::Fixture;
use rusqlite::Connection;

const ALL: &[Language] = &[Language::Chinese, Language::English, Language::Japanese];

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture
        .insert("ni'hao", "你好", 1000)
        .insert("ri", "日", 100)
        .insert("ri'wen", "日文", 10000)
        .insert("wen'shi", "问世", 100)
        .insert("shi", "是", 100)
        .insert("o", "哦", 100)
        .insert("ha", "哈", 100)
        .insert("you", "有", 100)
        .insert("wo", "我", 100)
        .insert("men", "们", 100);
    let english = fixture.paths.dictionary(assets::ENGLISH_DICTIONARY);
    crate::ensure_english_schema(&english).unwrap();
    Connection::open(english)
        .unwrap()
        .execute_batch("INSERT INTO english_words VALUES('hello','hello',100),('women','women',100),('men','men',100);")
        .unwrap();
    std::fs::write(
        fixture.paths.resource(assets::JAPANESE_MODEL),
        test_model::bytes(
            &[
                ("ありがとう", "ありがとう", 0, 0, 500),
                ("おはよう", "おはよう", 0, 0, 500),
            ],
            1,
            &[0],
        ),
    )
    .unwrap();
    fixture
}

fn text(conversion: &Conversion) -> String {
    conversion
        .segments
        .iter()
        .map(|span| span.text.as_str())
        .collect()
}

#[test]
fn real_dictionary_providers_preserve_two_changes_and_raw_ranges() {
    let fixture = fixture();
    let mut composer = DictionaryComposer::new(&fixture.paths, ALL);
    let converted = composer
        .convert("hellonihaoohayou")
        .expect("三种所选语言应完整覆盖");
    assert_eq!(text(&converted), "hello你好おはよう");
    assert!(!converted.chinese_complete);
    assert_eq!(
        converted
            .segments
            .iter()
            .map(|span| (span.language, span.input.clone()))
            .collect::<Vec<_>>(),
        [
            (Language::English, 0..5),
            (Language::Chinese, 5..10),
            (Language::Japanese, 10..16)
        ]
    );
    assert_eq!(
        text(&composer.convert("hellonihaoarigatou").unwrap()),
        "hello你好ありがとう"
    );
}

#[test]
fn merged_chinese_span_uses_the_existing_lattice_ranking() {
    let fixture = fixture();
    let converted = DictionaryComposer::new(&fixture.paths, ALL)
        .convert("helloriwenshiohayou")
        .expect("中间中文片段应完整转换");
    assert_eq!(text(&converted), "hello日文是おはよう");
}

#[test]
fn chinese_interpretation_is_preserved_for_an_ambiguous_mixed_alternative() {
    let fixture = fixture();
    let converted = DictionaryComposer::new(&fixture.paths, ALL)
        .convert("nihaoohayou")
        .expect("歧义混合应作为备选保留");
    assert_eq!(text(&converted), "你好おはよう");
    assert!(converted.chinese_complete);
}

#[test]
fn unselected_languages_and_missing_models_do_not_fill_unknown_tails() {
    let fixture = fixture();
    let mut composer =
        DictionaryComposer::new(&fixture.paths, &[Language::Chinese, Language::English]);
    let chinese_tail = composer.convert("hellonihaoohayou").unwrap();
    assert_eq!(text(&chinese_tail), "hello你好哦哈有");
    assert!(chinese_tail
        .segments
        .iter()
        .all(|span| span.language != Language::Japanese));
    assert!(composer.convert("hellonihaoarigatou").is_none());
    assert_eq!(text(&composer.convert("hellonihao").unwrap()), "hello你好");
    std::fs::remove_file(fixture.paths.resource(assets::JAPANESE_MODEL)).unwrap();
    let mut missing = DictionaryComposer::new(&fixture.paths, ALL);
    assert!(missing.convert("hellonihaoarigatou").is_none());
    assert_eq!(text(&missing.convert("hellonihao").unwrap()), "hello你好");
}

#[test]
fn appends_deletions_and_mid_input_edits_match_fresh_decodes() {
    let fixture = fixture();
    let mut composer = DictionaryComposer::new(&fixture.paths, ALL);
    for input in [
        "h",
        "hello",
        "hellonihao",
        "hellonihaoohayou",
        "hellonihao",
        "helloriwenshi",
        "helloriwenshiohayou",
        "nihaoohayou",
        "",
        "hellonihao",
    ] {
        let expected = DictionaryComposer::new(&fixture.paths, ALL).convert(input);
        assert_eq!(
            composer.convert(input),
            expected,
            "编辑后不能沿用失效边：{input}"
        );
    }
    assert!(composer.convert("hellonihao").is_some());
}

#[test]
fn clearing_after_a_dictionary_revision_requeries_cached_spans() {
    let fixture = fixture();
    let mut composer = DictionaryComposer::new(&fixture.paths, ALL);
    assert_eq!(text(&composer.convert("hellonihao").unwrap()), "hello你好");
    Connection::open(fixture.paths.dictionary(assets::ENGLISH_DICTIONARY))
        .unwrap()
        .execute(
            "UPDATE english_words SET display='greeting' WHERE word='hello'",
            [],
        )
        .unwrap();
    composer.clear();
    assert_eq!(
        text(&composer.convert("hellonihao").unwrap()),
        "greeting你好"
    );
}

#[test]
fn oversized_complete_word_cannot_disappear_and_expose_a_mixed_path() {
    let fixture = fixture();
    Connection::open(fixture.paths.dictionary(assets::ENGLISH_DICTIONARY))
        .unwrap()
        .execute(
            "INSERT INTO english_words VALUES('hellonihao',?1,100)",
            ["x".repeat(257)],
        )
        .unwrap();
    let mut composer = DictionaryComposer::new(&fixture.paths, ALL);
    assert!(composer.convert("hellonihao").is_none());
    assert!(composer.convert("hellonihao").is_none());
    assert!(composer.convert("helloriwenshi").is_some());
}

#[test]
fn whole_words_invalid_input_and_oversized_surfaces_do_not_make_mixed_rows() {
    let fixture = fixture();
    let mut composer = DictionaryComposer::new(&fixture.paths, ALL);
    for raw in [
        "women",
        "hello",
        "Hello你好",
        "hellonihao?",
        &"a".repeat(97),
    ] {
        assert!(composer.convert(raw).is_none());
    }
    Connection::open(fixture.paths.dictionary(assets::ENGLISH_DICTIONARY))
        .unwrap()
        .execute(
            "UPDATE english_words SET display=?1 WHERE word='hello'",
            ["x".repeat(257)],
        )
        .unwrap();
    composer.clear();
    assert!(composer.convert("hellonihao").is_none());
}
