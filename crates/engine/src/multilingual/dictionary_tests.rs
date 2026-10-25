use super::*;
use crate::assets;
use crate::japanese::decoder::test_model;
use crate::quanpin::fixture::Fixture;
use rusqlite::Connection;

const ALL: &[Language] = &[Language::Chinese, Language::English, Language::Japanese];

fn cold_fixture() -> Fixture {
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

fn fixture() -> Fixture {
    let fixture = cold_fixture();
    assert!(crate::japanese::decoder::JapaneseDictionary::shared(
        &fixture.paths.resource(assets::JAPANESE_MODEL)
    )
    .is_some());
    fixture
}

fn text(conversion: &Conversion) -> String {
    conversion
        .segments
        .iter()
        .map(|span| span.text.as_str())
        .collect()
}

fn input_session(fixture: &Fixture, languages: &[Language]) -> crate::Session {
    let mut options = crate::SessionOptions::new(fixture.paths.clone());
    options.helpcode = false;
    options.english.mixed_candidates = false;
    options.multilingual_languages = languages.to_vec();
    crate::Session::new(options).unwrap()
}

fn type_input(session: &mut crate::Session, raw: &str) {
    for byte in raw.bytes() {
        let result = session.character(byte, false);
        assert!(result.handled && result.commit.is_none());
    }
}

#[test]
fn session_multilingual_selection_finishes_without_writing_a_cross_language_phrase() {
    let fixture = fixture();
    let mut session = input_session(&fixture, ALL);
    type_input(&mut session, "hellonihaoohayou");
    let snapshot = session.snapshot();
    let seat = snapshot
        .candidates
        .iter()
        .position(|row| row.source as u8 == 18)
        .expect("连续混合路径应进入候选");
    assert_eq!(snapshot.candidates[seat].word, "hello你好おはよう");
    assert!(snapshot.candidate_answers_key[seat]);
    let before = std::fs::read(fixture.journal()).ok();
    let selected = session.select(seat);
    assert_eq!(selected.commit.as_deref(), Some("hello你好おはよう"));
    assert!(session.snapshot().editing_text.is_empty());
    crate::user_dictionary::ngram_store::flush_all();
    assert_eq!(std::fs::read(fixture.journal()).ok(), before);
    type_input(&mut session, "nihao");
    assert_eq!(session.snapshot().candidates[0].word, "你好");
}

#[test]
fn session_multilingual_keeps_a_complete_chinese_candidate_before_the_mixed_alternative() {
    let fixture = fixture();
    let mut session = input_session(&fixture, ALL);
    type_input(&mut session, "nihaoohayou");
    let snapshot = session.snapshot();
    let seat = snapshot
        .candidates
        .iter()
        .position(|row| row.source as u8 == 18)
        .expect("歧义混合应保留备选");
    assert!(seat > 0);
    assert_eq!(snapshot.candidates[0].word, "你好哦哈有");
    assert_eq!(snapshot.candidates[seat].word, "你好おはよう");
    assert!(snapshot.candidate_answers_key[0]);
}

#[test]
fn session_multilingual_disabled_and_unselected_providers_do_not_add_the_same_row() {
    let fixture = fixture();
    let mut disabled = input_session(&fixture, &[]);
    type_input(&mut disabled, "hellonihaoarigatou");
    assert!(disabled
        .snapshot()
        .candidate_sources
        .iter()
        .all(|source| *source as u8 != 18));
    let mut chinese_english = input_session(&fixture, &[Language::Chinese, Language::English]);
    type_input(&mut chinese_english, "hellonihaoarigatou");
    assert!(chinese_english
        .snapshot()
        .candidate_sources
        .iter()
        .all(|source| *source as u8 != 18));
}

#[test]
fn session_multilingual_reads_the_explicit_downloaded_japanese_model() {
    let fixture = fixture();
    let model = fixture.paths.cache.join("synthetic-japanese.dat");
    std::fs::rename(fixture.paths.resource(assets::JAPANESE_MODEL), &model).unwrap();
    assert!(crate::japanese::decoder::JapaneseDictionary::shared(&model).is_some());
    let mut options = crate::SessionOptions::new(fixture.paths.clone());
    options.helpcode = false;
    options.multilingual_languages = ALL.to_vec();
    options.japanese_dictionary = model;
    let mut session = crate::Session::new(options).unwrap();
    type_input(&mut session, "hellonihaoarigatou");
    assert!(session
        .snapshot()
        .candidates
        .iter()
        .any(|row| { row.source as u8 == 18 && row.word == "hello你好ありがとう" }));
}

#[test]
fn session_multilingual_caret_prefix_does_not_consume_the_unselected_tail() {
    let fixture = fixture();
    let mut session = input_session(&fixture, ALL);
    type_input(&mut session, "hellonihao");
    assert!(session
        .snapshot()
        .candidate_sources
        .iter()
        .any(|source| *source as u8 == 18));
    session.command(crate::Command::MoveLeft);
    assert!(session
        .snapshot()
        .candidate_sources
        .iter()
        .all(|source| *source as u8 != 18));
    assert_eq!(session.snapshot().editing_text, "hellonihao");
    session.command(crate::Command::MoveEnd);
    assert!(session
        .snapshot()
        .candidate_sources
        .iter()
        .any(|source| *source as u8 == 18));
}

#[test]
fn session_multilingual_snapshot_keeps_language_ranges_and_clears_them_on_cancel() {
    let fixture = fixture();
    let mut session = input_session(&fixture, ALL);
    type_input(&mut session, "hellonihaoohayou");
    let conversion = session
        .snapshot()
        .multilingual_conversion
        .expect("快照应保留分段");
    assert_eq!(text(&conversion), "hello你好おはよう");
    assert_eq!(
        conversion
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
    session.command(crate::Command::Cancel);
    assert!(session.snapshot().multilingual_conversion.is_none());
    type_input(&mut session, "nihao");
    assert!(session.snapshot().multilingual_conversion.is_none());
}

#[test]
fn session_multilingual_does_not_shift_a_user_fixed_candidate_slot() {
    let fixture = fixture();
    let mut session = input_session(&fixture, ALL);
    type_input(&mut session, "nihaohello");
    let seat = session
        .snapshot()
        .candidates
        .iter()
        .position(|row| row.source.is_dictionary() && row.word == "你好")
        .expect("应保留可固定的中文前缀");
    let fixed = session.fix_position(seat, 1);
    assert!(fixed.handled && fixed.diagnostic.is_none());
    let snapshot = session.snapshot();
    assert_eq!(snapshot.candidates[0].word, "你好");
    assert_eq!(snapshot.candidates[0].fixed_position, 1);
    assert!(snapshot.candidates.iter().any(|row| row.source as u8 == 18));
}

fn quick_phrase_session(
    fixture: &Fixture,
    raw: &str,
    mode: crate::FrequencyAdjustmentMode,
) -> crate::Session {
    let mut options = crate::SessionOptions::new(fixture.paths.clone());
    options.helpcode = false;
    options.english.mixed_candidates = false;
    options.multilingual_languages = ALL.to_vec();
    options.quick_phrase_candidates = true;
    options.quick_phrase_frequency = true;
    options.quick_phrase_table = vec![crate::types::QuickPhraseEntry {
        key: raw.into(),
        text: "合成快捷短语".into(),
    }];
    options.frequency = crate::FrequencyAdjustmentOptions {
        mode,
        trigger_count: 1,
        linear_step: 1,
    };
    crate::Session::new(options).unwrap()
}

#[test]
fn session_multilingual_selection_does_not_bypass_a_preceding_quick_phrase_group() {
    let fixture = fixture();
    let raw = "nihaoohayou";
    let frequency = crate::FrequencyAdjustmentOptions {
        mode: crate::FrequencyAdjustmentMode::Pin,
        trigger_count: 1,
        linear_step: 1,
    };
    crate::user_dictionary::ranking::learn_quick_phrase_selection(
        &fixture.paths.dictionary(assets::MAIN_DICTIONARY),
        &fixture.journal(),
        raw,
        "合成快捷短语",
        true,
        1,
        &frequency,
    )
    .unwrap();
    let mut session = quick_phrase_session(&fixture, raw, frequency.mode);
    type_input(&mut session, raw);
    let snapshot = session.snapshot();
    let group = snapshot
        .candidates
        .iter()
        .position(|row| row.source == crate::CandidateSource::QuickPhrase)
        .unwrap();
    let mixed = snapshot
        .candidates
        .iter()
        .position(|row| row.source == crate::CandidateSource::Multilingual)
        .unwrap();
    assert!(group < mixed, "回归必须覆盖越过短语组的选择");
    let before = std::fs::read(fixture.journal()).unwrap();
    assert_eq!(
        session.select(mixed).commit.as_deref(),
        Some("你好おはよう")
    );
    crate::user_dictionary::ngram_store::flush_all();
    assert_eq!(
        crate::user_dictionary::ranking::quick_phrase_slot(&fixture.journal(), raw),
        Some(0)
    );
    assert_eq!(std::fs::read(fixture.journal()).unwrap(), before);
}

#[test]
fn session_multilingual_row_does_not_count_toward_quick_phrase_promotion() {
    let fixture = fixture();
    let raw = "nihaohello";
    let mut session = quick_phrase_session(&fixture, raw, crate::FrequencyAdjustmentMode::Linear);
    type_input(&mut session, raw);
    let snapshot = session.snapshot();
    let group = snapshot
        .candidates
        .iter()
        .position(|row| row.source == crate::CandidateSource::QuickPhrase)
        .unwrap();
    let mixed = snapshot
        .candidates
        .iter()
        .position(|row| row.source == crate::CandidateSource::Multilingual)
        .unwrap();
    assert!(mixed < group, "回归必须覆盖短语组前的混合行");
    let ordinary = snapshot.candidates[..group]
        .iter()
        .filter(|row| row.source.is_dictionary() || row.source.is_generated_or_fallback())
        .count();
    assert!(ordinary > 0);
    assert_eq!(group, ordinary + 1, "夹具仅包含普通候选与一行混合候选");
    assert_eq!(
        session.select(group).commit.as_deref(),
        Some("合成快捷短语")
    );
    assert_eq!(
        crate::user_dictionary::ranking::quick_phrase_slot(&fixture.journal(), raw),
        Some(ordinary - 1)
    );
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

#[test]
fn background_japanese_loading_never_blocks_multilingual_keystrokes() {
    use crate::japanese::decoder::{JapaneseDictionary, SHARED_CACHE_TEST_LOCK};
    use std::sync::mpsc;
    use std::time::Duration;

    let _cache = SHARED_CACHE_TEST_LOCK.lock().unwrap();
    let fixture = cold_fixture();
    let model = fixture.paths.resource(assets::JAPANESE_MODEL);
    let pause = JapaneseDictionary::pause_next_load(&model);
    let mut composer = DictionaryComposer::new(&fixture.paths, ALL);
    composer.warm_up_in_background();
    pause.wait();
    let (sender, receiver) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let chinese_english = composer.convert("hellonihao");
        let pending_japanese = composer.convert("hellonihaoohayou");
        sender.send((chinese_english, pending_japanese)).unwrap();
        composer
    });
    // 加载线程被屏障挂起；接收限时只负责发现阻塞，放行后总会归还工作线程。
    let early = receiver.recv_timeout(Duration::from_secs(1));
    pause.wait();
    let mut composer = worker.join().unwrap();
    let (chinese_english, pending_japanese) = early.expect("按键不得等待日文模型加载");
    assert_eq!(text(&chinese_english.unwrap()), "hello你好");
    assert!(pending_japanese.is_none_or(|conversion| conversion
        .segments
        .iter()
        .all(|segment| segment.language != Language::Japanese)));
    assert!(JapaneseDictionary::shared(&model).is_some());
    // 模型就绪后，即使输入串没变，也必须重查先前未命中的日文跨度。
    assert_eq!(
        text(&composer.convert("hellonihaoohayou").unwrap()),
        "hello你好おはよう"
    );
}

#[test]
fn simultaneous_japanese_mixing_never_waits_for_the_background_model() {
    use crate::japanese::decoder::{JapaneseDictionary, SHARED_CACHE_TEST_LOCK};
    use std::sync::mpsc;
    use std::time::Duration;

    let _cache = SHARED_CACHE_TEST_LOCK.lock().unwrap();
    let fixture = cold_fixture();
    let model = fixture.paths.resource(assets::JAPANESE_MODEL);
    let pause = JapaneseDictionary::pause_next_load(&model);
    let mut options = crate::SessionOptions::new(fixture.paths.clone());
    options.helpcode = false;
    options.multilingual_languages = ALL.to_vec();
    options.japanese_mixing = true;
    let (ready_sender, ready_receiver) = mpsc::channel();
    let (go_sender, go_receiver) = mpsc::channel();
    let (done_sender, done_receiver) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let mut session = crate::Session::new(options).unwrap();
        ready_sender.send(()).unwrap();
        go_receiver.recv().unwrap();
        type_input(&mut session, "nihao");
        done_sender
            .send(session.snapshot().candidates[0].word.clone())
            .unwrap();
    });
    ready_receiver.recv().unwrap();
    pause.wait();
    go_sender.send(()).unwrap();
    let early = done_receiver.recv_timeout(Duration::from_secs(1));
    pause.wait();
    worker.join().unwrap();
    assert_eq!(
        early.expect("连续输入与日文混输并用不得等待模型加载"),
        "你好"
    );
}
