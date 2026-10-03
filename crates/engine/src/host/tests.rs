//! The port of the removed C++ bridge's `crates/engine-bridge/src/tests.rs` (tests-inventory.md §4.2) against the Rust facade, plus the two annotation tests of `engine-bridge/src/dictionary_stage/tests.rs` that exercise the host snapshot rule (fixtures written straight into the generation instead of staged), and unit tests of the bridge-local helpers.

use std::path::Path;

use rusqlite::Connection;

use super::glosses::{candidate_gloss_display, candidate_gloss_key};
use super::*;

fn options(root: &Path) -> EngineOptions {
    let path = |name| {
        let path = root.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path.to_str().unwrap().to_owned()
    };
    EngineOptions {
        resources: path("resources"),
        user_data: path("user"),
        cache: path("cache"),
        dictionaries: path("dictionaries"),
        scheme: 0,
        enabled_schemes: crate::types::SchemeSet::ALL,
        shuangpin_profile: 0,
        shuangpin_preedit_uses_raw: true,
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
        english_minimum_prefix: 5,
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
        sentence_association: SentenceAssociationOptions {
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
        japanese_dictionary: String::new(),
    }
}

fn type_text(session: &mut Session, text: &[u8]) {
    for character in text {
        session.character(*character, false).unwrap();
    }
}

const ENGLISH_SCHEMA: &str = "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);
    CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY,chinese_gloss TEXT);
    CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY,english_gloss TEXT);";

#[test]
fn normalizes_full_pinyin_using_the_expected_word_length() {
    assert_eq!(normalize_full_pinyin("xian", 1), "xian");
    assert_eq!(normalize_full_pinyin("xian", 2), "xi'an");
    assert_eq!(
        normalize_full_pinyin("a'ba'la'ti'ya'yun'hai", 7),
        "a'ba'la'ti'ya'yun'hai"
    );
    assert_eq!(normalize_full_pinyin("xian", 3), "");
    assert_eq!(normalize_full_pinyin("ni'hao'", 2), "");
}

#[test]
fn validates_and_normalizes_personal_dictionary_entries() {
    let normalized = dictionary_validate(&DictionaryEntry {
        kind: DictionaryKind::Pinyin,
        key: "NI HAO".into(),
        value: "拟好".into(),
        weight: 100_000,
    })
    .unwrap();
    assert_eq!(normalized.key, "ni'hao");
    let english = dictionary_validate(&DictionaryEntry {
        kind: DictionaryKind::English,
        key: "dont".into(),
        value: "don't".into(),
        weight: 100_000,
    })
    .unwrap();
    assert_eq!(english.key, "dont");
    assert_eq!(english.value, "don't");
    assert!(dictionary_validate(&DictionaryEntry {
        kind: DictionaryKind::English,
        key: "wrong_code".into(),
        value: "Word".into(),
        weight: 100_000,
    })
    .is_err());
}

#[test]
fn learned_glosses_survive_unavailable_packaged_dictionary() {
    let resources = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let resources_path = resources.path().to_str().unwrap();
    let user_path = user.path().to_str().unwrap();
    let candidates = vec![
        ("测试".into(), 0),
        ("Synthetic".into(), 4),
        ("Missing".into(), 4),
    ];
    let error = candidate_glosses_with_user(resources_path, user_path, &candidates).unwrap_err();
    assert_eq!(error.to_string(), "Candidate gloss dictionary unavailable");
    assert!(save_candidate_gloss(
        user_path,
        true,
        "测试",
        "synthetic gloss"
    ));
    assert!(save_candidate_gloss(
        user_path,
        false,
        "synthetic",
        "合成释义"
    ));
    let expected = vec![
        "synthetic gloss".to_owned(),
        "合成释义".to_owned(),
        String::new(),
    ];
    assert_eq!(
        candidate_glosses_with_user(resources_path, user_path, &candidates).unwrap(),
        expected
    );
    let packaged = resources.path().join("english.db");
    std::fs::write(&packaged, "synthetic damaged database").unwrap();
    assert_eq!(
        candidate_glosses_with_user(resources_path, user_path, &candidates).unwrap(),
        expected
    );
    assert!(candidate_glosses(resources_path, &candidates).is_err());
    std::fs::remove_file(&packaged).unwrap();
    Connection::open(&packaged)
        .unwrap()
        .execute_batch(&format!(
            "{ENGLISH_SCHEMA}
            INSERT INTO en_zh_glosses VALUES('missing','发布释义');
            INSERT INTO zh_en_glosses VALUES('测试','packaged gloss');"
        ))
        .unwrap();
    assert_eq!(
        candidate_glosses_with_user(resources_path, user_path, &candidates).unwrap(),
        vec!["synthetic gloss", "合成释义", "发布释义"]
    );
    std::fs::write(
        user.path().join("translation-glosses.db"),
        "synthetic damaged database",
    )
    .unwrap();
    assert_eq!(
        candidate_glosses_with_user(resources_path, user_path, &candidates).unwrap(),
        vec!["packaged gloss", "", "发布释义"]
    );
}

// The user's own file wins through an emergent route: translation-glosses.db sits in the directory the settings page writes custom_translations.txt to, and the learned store reads its sidecar from beside itself. Moving either file, or giving the learned store an explicit translations path, would silently drop the user's glosses to the bottom; this test is what would notice.
#[test]
fn hand_written_glosses_outrank_learned_and_packaged_ones() {
    let resources = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    Connection::open(resources.path().join("english.db"))
        .unwrap()
        .execute_batch(&format!(
            "{ENGLISH_SCHEMA} INSERT INTO zh_en_glosses VALUES('测试','packaged gloss');"
        ))
        .unwrap();
    let resources_path = resources.path().to_str().unwrap();
    let user_path = user.path().to_str().unwrap();
    let candidates = vec![("测试".into(), 0)];

    assert_eq!(
        candidate_glosses_with_user(resources_path, user_path, &candidates).unwrap(),
        vec!["packaged gloss"]
    );
    assert!(save_candidate_gloss(
        user_path,
        true,
        "测试",
        "learned gloss"
    ));
    assert_eq!(
        candidate_glosses_with_user(resources_path, user_path, &candidates).unwrap(),
        vec!["learned gloss"]
    );
    std::fs::write(
        user.path().join("custom_translations.txt"),
        "测试\thand written gloss\n",
    )
    .unwrap();
    assert_eq!(
        candidate_glosses_with_user(resources_path, user_path, &candidates).unwrap(),
        vec!["hand written gloss"]
    );
}

/// The user's own `custom_translations.txt` outranks every automatic gloss even before any online gloss was saved, which is when `translation-glosses.db` first appears; the file Settings writes must not wait for that store.
#[test]
fn hand_written_glosses_apply_without_a_learned_store() {
    let resources = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    Connection::open(resources.path().join("english.db"))
        .unwrap()
        .execute_batch(&format!(
            "{ENGLISH_SCHEMA} INSERT INTO zh_en_glosses VALUES('测试','packaged gloss');
             INSERT INTO en_zh_glosses VALUES('hello','打招呼');"
        ))
        .unwrap();
    std::fs::write(
        user.path().join("custom_translations.txt"),
        "测试\thand written gloss\nhello\t你好\n",
    )
    .unwrap();
    let resources_path = resources.path().to_str().unwrap();
    let user_path = user.path().to_str().unwrap();
    let candidates = vec![("测试".into(), 0), ("hello".into(), 4)];
    assert!(!user.path().join("translation-glosses.db").exists());
    assert_eq!(
        candidate_glosses_with_user(resources_path, user_path, &candidates).unwrap(),
        vec!["hand written gloss", "你好"]
    );
    // The macOS learned-gloss path asks for the user overlay alone.
    assert_eq!(
        candidate_glosses_with_user("", user_path, &candidates).unwrap(),
        vec!["hand written gloss", "你好"]
    );
    assert!(!user.path().join("translation-glosses.db").exists());
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(
        candidate_glosses_with_user("", empty.path().to_str().unwrap(), &candidates)
            .unwrap_err()
            .to_string(),
        crate::diagnostics::CANDIDATE_GLOSS_UNAVAILABLE
    );
}

#[cfg(unix)]
#[test]
fn learned_glosses_reject_a_symlinked_database() {
    use std::os::unix::fs::symlink;

    let resources = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    Connection::open(resources.path().join("english.db"))
        .unwrap()
        .execute_batch(&format!(
            "{ENGLISH_SCHEMA} INSERT INTO zh_en_glosses VALUES('测试','packaged gloss');"
        ))
        .unwrap();
    assert!(save_candidate_gloss(
        external.path().to_str().unwrap(),
        true,
        "测试",
        "external gloss",
    ));
    symlink(
        external.path().join("translation-glosses.db"),
        user.path().join("translation-glosses.db"),
    )
    .unwrap();

    let candidates = vec![("测试".into(), 0)];
    assert_eq!(
        candidate_glosses_with_user(
            resources.path().to_str().unwrap(),
            user.path().to_str().unwrap(),
            &candidates,
        )
        .unwrap(),
        vec!["packaged gloss"]
    );
}

#[cfg(unix)]
#[test]
fn saving_learned_glosses_rejects_a_symlinked_database() {
    use std::os::unix::fs::symlink;

    let user = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let external = outside.path().join("external.db");
    symlink(&external, user.path().join("translation-glosses.db")).unwrap();

    assert!(!save_candidate_gloss(
        user.path().to_str().unwrap(),
        true,
        "测试",
        "external gloss",
    ));
    assert!(!external.exists());
}

#[test]
fn unsafe_learned_glosses_fall_back_to_packaged_values() {
    let resources = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    Connection::open(resources.path().join("english.db"))
        .unwrap()
        .execute_batch(&format!(
            "{ENGLISH_SCHEMA} INSERT INTO zh_en_glosses VALUES('测试','packaged gloss');"
        ))
        .unwrap();
    let resources_path = resources.path().to_str().unwrap();
    let user_path = user.path().to_str().unwrap();
    let candidates = vec![("测试".into(), 0)];

    for codepoint in (1..=0x1f).chain(0x7f..=0x9f) {
        let control = char::from_u32(codepoint).unwrap();
        if matches!(control, '\t' | '\n' | '\r') {
            continue;
        }
        assert!(save_candidate_gloss(
            user_path,
            true,
            "测试",
            &format!("before{control}after"),
        ));
        assert_eq!(
            candidate_glosses_with_user(resources_path, user_path, &candidates).unwrap(),
            vec!["packaged gloss"]
        );
    }
    for whitespace in ['\t', '\n', '\r'] {
        assert!(save_candidate_gloss(
            user_path,
            true,
            "测试",
            &format!("learned{whitespace}gloss"),
        ));
        assert_eq!(
            candidate_glosses_with_user(resources_path, user_path, &candidates).unwrap(),
            vec!["learned gloss"]
        );
    }
}

fn offline_gloss_database(path: &Path, language: &str, version: i32) {
    Connection::open(path)
        .unwrap()
        .execute_batch(&format!(
            "CREATE TABLE zh_glosses(chinese TEXT PRIMARY KEY, gloss TEXT NOT NULL, source TEXT NOT NULL) WITHOUT ROWID;
             CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
             INSERT INTO meta VALUES('target_language', '{language}');
             INSERT INTO zh_glosses VALUES('猫', 'chat, chatte; félin', 'cat; cat');
             INSERT INTO zh_glosses VALUES('天', 'jour; ciel; firmament', 'day; sky; heaven');
             PRAGMA user_version = {version};"
        ))
        .unwrap();
}

#[test]
fn offline_target_glosses_answer_chinese_candidates_only() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("zh-fr.db");
    offline_gloss_database(&path, "fr", 1);
    let path = path.to_str().unwrap();
    let candidates = vec![
        ("猫".into(), 0),
        ("天".into(), 0),
        ("狗".into(), 0),
        ("cat".into(), 4),
        ("猫".into(), 6),
    ];
    // A third sense is cut by the same display rule as the English glosses; Latin candidates and emoji sources get nothing.
    assert_eq!(
        candidate_target_glosses(path, "fr", &candidates).unwrap(),
        vec!["chat, chatte; félin", "jour; ciel", "", "", ""]
    );
}

#[test]
fn offline_target_glosses_refuse_a_file_for_another_language_or_version() {
    let directory = tempfile::tempdir().unwrap();
    let candidates = vec![("猫".into(), 0)];
    let message = |path: &Path, language| {
        candidate_target_glosses(path.to_str().unwrap(), language, &candidates)
            .unwrap_err()
            .to_string()
    };
    let renamed = directory.path().join("zh-ja.db");
    offline_gloss_database(&renamed, "fr", 1);
    assert_eq!(
        message(&renamed, "ja"),
        "Offline gloss dictionary language mismatch"
    );
    let future = directory.path().join("zh-de.db");
    offline_gloss_database(&future, "de", 2);
    assert_eq!(
        message(&future, "de"),
        "Offline gloss dictionary version unsupported"
    );
    let missing = directory.path().join("zh-ko.db");
    assert_eq!(
        message(&missing, "ko"),
        "Offline gloss dictionary unavailable"
    );
    assert!(
        !missing.exists(),
        "a read-only open must not create the file"
    );
    let damaged = directory.path().join("zh-es.db");
    std::fs::write(&damaged, "synthetic damaged database").unwrap();
    assert!(candidate_target_glosses(damaged.to_str().unwrap(), "es", &candidates).is_err());
}

#[test]
fn reset_learned_data_restores_packaged_dictionaries_and_clears_journal() {
    // Both an ASCII root and one carrying Chinese characters: the reset derives temporary, backup and SQLite sidecar names from these paths, and on Windows a narrow conversion of the second either mangles it or throws after files are already published.
    for component in ["ascii", "陆傲天"] {
        reset_learned_data_under_root(component);
    }
}

fn reset_learned_data_under_root(component: &str) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join(component);
    std::fs::create_dir_all(&root).unwrap();
    let value = options(&root);
    let resources = Path::new(&value.resources);
    let dictionaries = Path::new(&value.dictionaries);
    Connection::open(resources.join("msime.db"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',100);
             CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
             CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);",
        )
        .unwrap();
    Connection::open(resources.join("english.db"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);
             CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY,chinese_gloss TEXT);
             CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY,english TEXT);
             INSERT INTO english_words VALUES('word','word',100);",
        )
        .unwrap();
    std::fs::copy(resources.join("msime.db"), dictionaries.join("msime.db")).unwrap();
    std::fs::copy(
        resources.join("english.db"),
        dictionaries.join("english.db"),
    )
    .unwrap();
    let journal = Path::new(&value.user_data).join("msime_user.db");
    Connection::open(&journal)
        .unwrap()
        .execute_batch(
            "CREATE TABLE user_dictionary_operations(dictionary TEXT,key TEXT,value TEXT,operation TEXT,weight INTEGER,display TEXT,user_inserted INTEGER);
             CREATE TABLE personal_dictionary_receipts(request_id TEXT PRIMARY KEY,payload TEXT);
             CREATE TABLE candidate_selection_state(context_key TEXT,entry_key TEXT,value TEXT,selection_count INTEGER);
             CREATE TABLE fixed_candidate_positions(context_key TEXT,entry_key TEXT,value TEXT,position INTEGER);
             INSERT INTO user_dictionary_operations VALUES('pinyin','ni''hao','你好','upsert',1,'',1);
             INSERT INTO candidate_selection_state VALUES('ni''hao','ni''hao','你好',7);",
        )
        .unwrap();
    Connection::open(dictionaries.join("msime.db"))
        .unwrap()
        .execute("UPDATE tbl_2_n SET weight=1", [])
        .unwrap();

    reset_learned_data(&value).unwrap();

    let weight: i64 = Connection::open(dictionaries.join("msime.db"))
        .unwrap()
        .query_row(
            "SELECT weight FROM tbl_2_n WHERE key='ni''hao'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(weight, 100);
    let journal = Connection::open(journal).unwrap();
    let count = |table: &str| -> i64 {
        journal
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    };
    assert_eq!(count("user_dictionary_operations"), 0);
    assert_eq!(count("candidate_selection_state"), 0);
}

#[test]
fn reset_refuses_to_reset_the_packaged_bundle_in_place() {
    let root = tempfile::tempdir().unwrap();
    let mut value = options(root.path());
    value.dictionaries = value.resources.clone();
    assert_eq!(
        reset_learned_data(&value).unwrap_err().to_string(),
        "Cannot reset packaged dictionaries in place"
    );
    let value = options(root.path());
    assert_eq!(
        reset_learned_data(&value).unwrap_err().to_string(),
        "Packaged dictionary is unavailable"
    );
}

#[test]
fn prepared_options_disable_quanpin_autocorrect_by_default() {
    let root = tempfile::tempdir().unwrap();
    let resources = root.path().join("resources");
    std::fs::create_dir_all(&resources).unwrap();
    for name in ["msime.db", "english.db"] {
        Connection::open(resources.join(name)).unwrap();
    }
    let prepared = prepare_options(
        resources.to_str().unwrap(),
        root.path().join("user").to_str().unwrap(),
        root.path().join("cache").to_str().unwrap(),
        "synthetic-defaults",
    )
    .unwrap();
    assert!(!prepared.autocorrect_transposition);
    assert!(!prepared.autocorrect_neighbor);
    assert_eq!(prepared.english_minimum_prefix, 5);
    assert_eq!(
        Path::new(&prepared.dictionaries),
        root.path()
            .join("user")
            .join("dictionaries")
            .join("synthetic-defaults")
    );
    // The defaults must build a session as they stand.
    Session::new(&prepared).unwrap();
}

#[test]
fn dictionary_revision_uses_real_journal_and_rejects_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let before = dictionary_state_revision(&value).unwrap();
    assert_eq!(before.len(), 64);
    assert_eq!(before, dictionary_state_revision(&value).unwrap());
    let journal = Path::new(&value.user_data).join("msime_user.db");
    assert!(!journal.exists());
    std::fs::write(&journal, b"synthetic invalid database").unwrap();
    assert!(dictionary_state_revision(&value).is_err());
    assert_eq!(
        std::fs::read(&journal).unwrap(),
        b"synthetic invalid database"
    );
}

#[test]
fn staging_rejects_a_zero_limit_and_reports_transport_failures() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let generation = dir.path().join("generation");
    let generation = generation.to_str().unwrap();
    let error =
        stage_dictionary_state(&value, generation, "zero", 0, std::iter::empty()).unwrap_err();
    assert_eq!(error.to_string(), "Invalid snapshot record limit");
    let error = stage_dictionary_state(
        &value,
        generation,
        "broken",
        10,
        std::iter::once(Err(SnapshotReadError)),
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "Snapshot record stream failed");
    assert!(!Path::new(generation).join("broken").exists());
}

#[test]
fn helpcode_settings_reach_the_real_engine() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    for schema in [
        "lantian",
        "ziranma",
        "shouyou2_0",
        "shouyouplus",
        "xiaohe",
        "jiajia",
    ] {
        value.helpcode_schema = schema.into();
        for enabled in [false, true] {
            value.helpcode = enabled;
            let mut session = Session::new(&value).unwrap();
            type_text(&mut session, b"ni");
            assert_eq!(session.character(b'H', true).unwrap().handled, enabled);
        }
    }
    value.helpcode_schema = "unknown".into();
    assert!(Session::new(&value).is_err());
}

#[test]
fn custom_helpcode_table_is_loaded_by_the_engine_session() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    let custom = Path::new(&value.resources).join("helpcodes").join("custom");
    std::fs::create_dir_all(&custom).unwrap();
    std::fs::write(
        custom.join("synthetic.txt"),
        "\u{feff}# name: Synthetic\r\n# name_en: Synthetic\r\n你=ab\r\n",
    )
    .unwrap();
    value.helpcode = true;
    value.helpcode_schema = "custom/synthetic".into();

    let mut session = Session::new(&value).unwrap();
    type_text(&mut session, b"ni");
    assert!(session.character(b'A', true).unwrap().handled);
}

/// 辅助码表插件坏掉后宿主用 `None` 退回方案原来的表；那张表是已被删掉的 `custom/<stem>` 时装上空表，不让宿主的每次聚焦都失败。
#[test]
fn a_missing_fallback_schema_gives_an_empty_helpcode_table() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    let custom = Path::new(&value.resources).join("helpcodes").join("custom");
    std::fs::create_dir_all(&custom).unwrap();
    std::fs::write(custom.join("synthetic.txt"), "你=ab\n").unwrap();
    value.helpcode = true;
    value.helpcode_schema = "custom/synthetic".into();
    value.helpcode_table = Some(std::sync::Arc::new(HelpcodeKeymap::from_codes(
        [("你".to_owned(), "cd".to_owned())].into_iter().collect(),
    )));
    let mut session = Session::new(&value).unwrap();
    std::fs::remove_file(custom.join("synthetic.txt")).unwrap();

    session
        .set_helpcode_table(None)
        .expect("a missing fallback schema failed the replacement");
    type_text(&mut session, b"ni");
    let view = session.snapshot().unwrap();
    assert!(
        view.candidate_annotations.iter().all(String::is_empty),
        "{:?}",
        view.candidate_annotations
    );
    // 回退仍然缺失时再换一次也一样。
    session.set_helpcode_table(None).unwrap();
}

/// The jiajia table this repository carries is injected rather than shipped inside a locked archive, so it is the one that can go missing, be truncated by a bad merge or be saved in an encoding the engine reads as nothing. An entry the parser rejects is silently absent at runtime, which is why this loads it through the engine's own loader and counts.
#[test]
fn the_carried_jiajia_table_parses_the_way_the_engine_reads_it() {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let text = std::fs::read_to_string(resources.join("helpcodes/jiajia_helpcode.txt")).unwrap();
    let keymap = crate::helpcode::load_helpcode_keymap(&resources, "jiajia").unwrap();
    // Codes quoted in resources/helpcodes/NOTICE.md as coming from the alignment.
    for (character, code) in [("好", "nz"), ("你", "de"), ("中", "ks"), ("国", "ky")] {
        assert_eq!(keymap.code(character), Some(code));
    }
    // The notice records 7968 entries; one per line, so a line the parser dropped shows as a shortfall.
    assert_eq!(keymap.len(), 7968);
    assert_eq!(text.lines().filter(|line| !line.is_empty()).count(), 7968);
}

#[test]
fn invalid_options_return_errors_instead_of_unwinding_into_rust() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    let message = |value: &EngineOptions| Session::new(value).err().unwrap().to_string();
    value.scheme = 255;
    assert_eq!(message(&value), "Unsupported input scheme");
    value.scheme = 0;
    value.shuangpin_profile = 255;
    assert_eq!(message(&value), "Unsupported shuangpin profile");
    value.shuangpin_profile = 0;
    value.wubi_profile = 2;
    assert_eq!(message(&value), "Unsupported wubi profile");
    value.wubi_profile = 1;
    value.frequency_mode = "sometimes".into();
    assert_eq!(message(&value), "Unsupported frequency mode");
    value.frequency_mode = "promote".into();
    value.vietnamese_input_method = 2;
    assert_eq!(message(&value), "Unsupported Vietnamese input method");
    value.vietnamese_input_method = 1;
    value.vietnamese_tone_style = 2;
    assert_eq!(message(&value), "Unsupported Vietnamese tone style");
    value.vietnamese_tone_style = 1;
    assert!(Session::new(&value).is_ok());
    value.resources = "relative".into();
    assert_eq!(message(&value), "Runtime directories must be absolute");
}

#[test]
fn microsoft_profile_accepts_semicolon_as_an_ing_final() {
    let dir = tempfile::tempdir().unwrap();
    for profile in 0..4 {
        let mut options = options(dir.path());
        options.scheme = 1;
        options.shuangpin_profile = profile;
        let mut session = Session::new(&options).unwrap();
        type_text(&mut session, b"b;");
        let snapshot = session.snapshot().unwrap();
        assert_eq!(snapshot.editing_text, if profile == 3 { "b;" } else { "b" });
        assert_eq!(snapshot.microsoft_shuangpin, profile == 3);
    }
}

#[test]
fn microsoft_profile_keeps_trailing_semicolon_in_segment_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    for (input, expected) in [
        (&b"nihkb;"[..], vec![0, 2, 4, 6]),
        // `cb` is a valid Microsoft shuangpin pair and must win over pairing the final `b` with the trailing semicolon.
        (&b"nihcb;"[..], vec![0, 2, 3, 5, 6]),
    ] {
        let mut options = options(dir.path());
        options.scheme = 1;
        options.shuangpin_profile = 3;
        let mut session = Session::new(&options).unwrap();
        type_text(&mut session, input);
        assert_eq!(session.snapshot().unwrap().segment_raw_boundaries, expected);
    }
}

/// A keyboard face labels its letter keys with the units they carry, read from the profile the session runs: this asks the session for its profile name instead of assuming the option index and the name agree.
#[test]
fn shuangpin_key_hints_describe_the_profile_the_session_runs() {
    let dir = tempfile::tempdir().unwrap();
    let mut seen: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for profile in 0..4 {
        let mut options = options(dir.path());
        options.scheme = 1;
        options.shuangpin_profile = profile;
        let session = Session::new(&options).unwrap();
        let name = session.snapshot().unwrap().shuangpin_profile;
        let hints: Vec<(String, String)> = shuangpin_key_hints(&name)
            .into_iter()
            .map(|entry| (entry.key, entry.hint))
            .collect();
        assert!(
            hints.len() >= 26,
            "{name} labelled only {} keys",
            hints.len()
        );
        for (key, hint) in &hints {
            assert!(
                key.len() == 1 && ("A"..="Z").contains(&key.as_str()) || key == ";",
                "{name} produced a hint for {key:?}, which is not a letter key"
            );
            assert!(!hint.is_empty(), "{name} key {key} carries an empty hint");
            // " / " separates initials from finals, so it appears at most once.
            assert!(
                hint.matches(" / ").count() <= 1,
                "{name} key {key} hint {hint:?} reads as more than two sides"
            );
        }
        seen.push((name, hints));
    }
    for (index, (name, hints)) in seen.iter().enumerate() {
        for (other_name, other_hints) in seen.iter().skip(index + 1) {
            assert_ne!(
                hints, other_hints,
                "{name} and {other_name} produced the same key face"
            );
        }
    }
}

/// Xiaohe keeps two finals on K, and a host-side copy of the keymap once listed only one of them.
#[test]
fn shuangpin_key_hints_keep_every_unit_a_key_carries() {
    let hints: std::collections::HashMap<String, String> = shuangpin_key_hints("xiaohe")
        .into_iter()
        .map(|entry| (entry.key, entry.hint))
        .collect();
    assert_eq!(hints.get("K").map(String::as_str), Some("ing uai"));
    assert_eq!(hints.get("V").map(String::as_str), Some("zh / ui ü"));
}

#[test]
fn shuangpin_key_hints_reject_an_unknown_profile() {
    assert!(shuangpin_key_hints("").is_empty());
    assert!(shuangpin_key_hints("xiaohe-v2").is_empty());
    assert!(shuangpin_key_hints("quanpin").is_empty());
}

#[test]
fn all_shuangpin_profiles_accept_yo_as_one_syllable() {
    let dir = tempfile::tempdir().unwrap();
    for profile in 0..4 {
        let mut options = options(dir.path());
        options.scheme = 1;
        options.shuangpin_profile = profile;
        let mut session = Session::new(&options).unwrap();
        type_text(&mut session, b"yo");
        let snapshot = session.snapshot().unwrap();
        assert_eq!(snapshot.editing_text, "yo");
        assert_eq!(snapshot.preedit, "yo", "yo was split: {snapshot:?}");
    }
}

#[test]
fn cloud_query_preserves_manual_quanpin_segmentation() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = Session::new(&options(dir.path())).unwrap();
    for character in b"qi'e'huan" {
        assert!(session.character(*character, false).unwrap().handled);
    }
    let query = session.online_query().unwrap();
    assert!(query.available);
    assert_eq!(query.query_text, "qi'e'huan");
    // A source other than cloud or AI, an unavailable query, and a scheme no session issues are refused before the engine sees them.
    assert!(!session.apply_online_candidate(&query, "企鹅换", 2).unwrap());
    let unavailable = OnlineQuerySnapshot {
        available: false,
        ..query.clone()
    };
    assert!(!session
        .apply_online_candidate(&unavailable, "企鹅换", 0)
        .unwrap());
    let foreign = OnlineQuerySnapshot {
        scheme: 200,
        ..query.clone()
    };
    assert!(!session
        .apply_online_candidates(&foreign, &["企鹅换".to_owned()], 1)
        .unwrap());
    session.command(Command::Cancel).unwrap();
    assert_eq!(
        session.online_query().unwrap(),
        OnlineQuerySnapshot::default()
    );
}

#[test]
fn real_engine_handles_unicode_mode_without_a_dictionary_bundle() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = Session::new(&options(dir.path())).unwrap();
    assert!(session.character(b'U', true).unwrap().handled);
    for character in b"4e2d" {
        assert!(session.character(*character, false).unwrap().handled);
    }
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.local_mode, "unicode");
    assert!(snapshot
        .candidates
        .iter()
        .any(|candidate| candidate == "中"));
    let result = session.select(0).unwrap();
    assert!(result.has_commit);
    assert_eq!(result.commit, "中");
    assert!(session.snapshot().unwrap().preedit.is_empty());
    assert_eq!(session.snapshot().unwrap().local_mode, "none");
}

#[test]
fn real_engine_exposes_nine_key_mode_and_spelling_choices() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = Session::new(&options(dir.path())).unwrap();
    assert!(!session.snapshot().unwrap().nine_key);
    assert!(!session.character(b'6', false).unwrap().handled);
    session.set_nine_key_enabled(true).unwrap();
    assert!(session.snapshot().unwrap().nine_key);
    assert!(session.character(b'6', false).unwrap().handled);
    let snapshot = session.snapshot().unwrap();
    assert!(!snapshot.nine_key_spellings.is_empty());
    assert!(
        !session
            .choose_nine_key_spelling(snapshot.nine_key_spellings.len())
            .unwrap()
            .handled
    );
    assert!(session.choose_nine_key_spelling(0).unwrap().handled);
    session.command(Command::Cancel).unwrap();
    session.set_nine_key_enabled(false).unwrap();
    assert!(!session.snapshot().unwrap().nine_key);
}

#[test]
fn real_engine_cycles_the_last_japanese_kana_variant() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.scheme = 3;
    let mut session = Session::new(&value).unwrap();
    for character in b"ka" {
        assert!(session.character(*character, false).unwrap().handled);
    }
    assert_eq!(session.snapshot().unwrap().reading, "か");
    assert!(session.command(Command::CycleKanaVariant).unwrap().handled);
    assert_eq!(session.snapshot().unwrap().reading, "が");
    assert!(session.command(Command::CycleKanaVariant).unwrap().handled);
    assert_eq!(session.snapshot().unwrap().reading, "か");
}

/// 录制器的单词条日文模型（读音 かな，词 甲），与 golden 场景 `ri_japanese_model_a` 同一份字节。
const JAPANESE_MODEL_KANA: &str = "MSJPDT1\u{0}\u{1}\u{0}\u{0}\u{0}\u{1}\u{0}\u{0}\u{0}\u{1}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}8\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}L\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}N\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\t\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{6}\u{0}\u{6}\u{0}\u{0}\u{0}\u{3}\u{0}\u{0}\u{0}\u{0}\u{0}\u{1}\u{0}\u{0}\u{0}\u{0}\u{0}かな甲";

fn japanese_candidates(value: &EngineOptions) -> Vec<String> {
    let mut session = Session::new(value).unwrap();
    type_text(&mut session, b"kana");
    session.snapshot().unwrap().candidates
}

/// `japanese_dictionary` 指向资源目录之外的模型时读那一份；为空时仍读资源目录里的 `dict_japanese.dat`，两处都没有就只给假名行。
#[test]
fn the_japanese_model_path_overrides_the_resource_copy() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.scheme = 3;

    let empty = japanese_candidates(&value);
    assert!(empty.iter().any(|word| word == "かな"));
    assert!(!empty.iter().any(|word| word == "甲"));

    let downloaded = dir.path().join("packs").join(crate::assets::JAPANESE_MODEL);
    std::fs::create_dir_all(downloaded.parent().unwrap()).unwrap();
    std::fs::write(&downloaded, JAPANESE_MODEL_KANA).unwrap();
    value.japanese_dictionary = downloaded.to_str().unwrap().to_owned();
    assert!(japanese_candidates(&value).iter().any(|word| word == "甲"));

    let other = tempfile::tempdir().unwrap();
    let mut value = options(other.path());
    value.scheme = 3;
    std::fs::write(
        Path::new(&value.resources).join(crate::assets::JAPANESE_MODEL),
        JAPANESE_MODEL_KANA,
    )
    .unwrap();
    assert!(japanese_candidates(&value).iter().any(|word| word == "甲"));
}

#[test]
fn real_engine_composes_korean_with_the_hangul_as_its_reading() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.scheme = 4;
    let mut session = Session::new(&value).unwrap();
    let mut committed = String::new();
    for character in b"gksrmf" {
        let result = session.character(*character, false).unwrap();
        assert!(result.handled);
        committed.push_str(&result.commit);
    }
    assert_eq!(committed, "한");
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.scheme, 4);
    assert_eq!(snapshot.preedit, "글");
    assert_eq!(snapshot.reading, "글");
    assert_eq!(snapshot.editing_text, "rmf");
    assert_eq!(snapshot.caret_position, 3);
    assert!(snapshot.candidates.is_empty() && snapshot.candidate_sources.is_empty());
    assert!(snapshot.segment_raw_boundaries.is_empty());
    assert!(!session.online_query().unwrap().available);

    // Enter commits the syllable and leaves the key to the host; nothing is learned as an English word.
    let result = session.command(Command::CommitRaw).unwrap();
    assert!(!result.handled);
    assert!(result.has_commit);
    assert_eq!(result.commit, "글");
    assert_eq!(result.diagnostic, "");
    assert!(!Path::new(&value.dictionaries).join("english.db").exists());

    // The raw commit without learning behaves the same, and punctuation stays ASCII.
    session.character(b'r', false).unwrap();
    let result = session.command(Command::CommitRawWithoutLearning).unwrap();
    assert_eq!((result.handled, result.commit.as_str()), (false, "ㄱ"));
    session.character(b'r', false).unwrap();
    session.character(b'k', false).unwrap();
    let result = session.punctuation(b',').unwrap();
    assert_eq!((result.handled, result.commit.as_str()), (true, "가,"));
}

#[test]
fn the_korean_hanja_list_reports_itself_open() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.scheme = 4;
    let mut session = Session::new(&value).unwrap();
    type_text(&mut session, b"gks");
    let snapshot = session.snapshot().unwrap();
    assert!(!snapshot.candidate_list_open);
    assert!(snapshot.candidates.is_empty());

    assert!(session.command(Command::ConvertHanja).unwrap().handled);
    let snapshot = session.snapshot().unwrap();
    assert!(snapshot.candidate_list_open);
    assert!(snapshot
        .candidates
        .iter()
        .any(|candidate| candidate == "韓"));

    // The same command closes it again.
    assert!(session.command(Command::ConvertHanja).unwrap().handled);
    let snapshot = session.snapshot().unwrap();
    assert!(!snapshot.candidate_list_open);
    assert!(snapshot.candidates.is_empty());
}

#[test]
fn schemes_without_an_openable_list_never_report_one_open() {
    let dir = tempfile::tempdir().unwrap();
    for scheme in 0..4 {
        let mut value = options(dir.path());
        value.scheme = scheme;
        let mut session = Session::new(&value).unwrap();
        type_text(&mut session, b"ka");
        assert!(!session.command(Command::ConvertHanja).unwrap().handled);
        assert!(!session.snapshot().unwrap().candidate_list_open);
    }
}

#[test]
fn cantonese_is_unavailable_without_its_dictionary() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.scheme = 5;
    let error = Session::new(&value).err().expect("no cantonese.db");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
    );
    value.cantonese_dictionary = dir.path().join("missing.db").to_str().unwrap().to_owned();
    let error = Session::new(&value).err().expect("missing cantonese.db");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
    );
    assert!(!dir.path().join("missing.db").exists());
}

#[test]
fn scheme_eight_is_tibetan_and_needs_no_dictionary() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.scheme = 8;
    let mut session = Session::new(&value).unwrap();
    type_text(&mut session, b"bod");
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.preedit, "བོད");
    assert_eq!(snapshot.scheme, 8);
    let space = session.command(Command::CommitCandidate).unwrap();
    assert!(space.handled);
    assert_eq!(space.commit, "བོད་");
    value.scheme = 9;
    let error = Session::new(&value).err().expect("scheme nine");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::UNSUPPORTED_INPUT_SCHEME
    );
}

#[test]
fn zhuyin_is_unavailable_without_its_dictionary() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.scheme = 6;
    let error = Session::new(&value).err().expect("no zhuyin.db");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
    );
    value.zhuyin_dictionary = dir.path().join("missing.db").to_str().unwrap().to_owned();
    let error = Session::new(&value).err().expect("missing zhuyin.db");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
    );
    assert!(!dir.path().join("missing.db").exists());
}

#[test]
fn zhuyin_command_sixteen_opens_a_list_whose_selection_commits_nothing() {
    use crate::language_dictionary::{FORMAT_VERSION, METADATA_FORMAT_VERSION, SCHEMA};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("zhuyin.db");
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch(SCHEMA).unwrap();
    connection
        .execute(
            "INSERT INTO metadata VALUES (?1, ?2)",
            (METADATA_FORMAT_VERSION, FORMAT_VERSION.to_string()),
        )
        .unwrap();
    connection
        .execute_batch(
            "INSERT INTO syllables VALUES ('ㄋㄧˇ');\
             INSERT INTO entries VALUES ('ㄋㄧˇ','你',1000),('ㄋㄧˇ','妳',300);",
        )
        .unwrap();
    drop(connection);
    let mut value = options(dir.path());
    value.scheme = 6;
    value.zhuyin_dictionary = path.to_str().unwrap().to_owned();
    let mut session = Session::new(&value).unwrap();
    type_text(&mut session, b"su3");
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.preedit, "你");
    assert_eq!(snapshot.reading, "你");
    assert_eq!(snapshot.editing_text, "su3");
    assert!(!snapshot.candidate_list_open);

    assert!(session.command(Command::ConvertHanja).unwrap().handled);
    let snapshot = session.snapshot().unwrap();
    assert!(snapshot.candidate_list_open);
    assert_eq!(snapshot.candidates, ["你", "妳"]);

    let result = session.select(1).unwrap();
    assert!(result.handled);
    assert!(!result.has_commit);
    let snapshot = session.snapshot().unwrap();
    assert!(!snapshot.candidate_list_open);
    assert_eq!(snapshot.preedit, "妳");

    let result = session.command(Command::CommitRaw).unwrap();
    assert_eq!((result.handled, result.commit.as_str()), (true, "妳"));
}

#[test]
fn commit_raw_applies_windows_english_learning_policy() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let mut session = Session::new(&value).unwrap();
    session.set_dedicated_english(true).unwrap();
    for character in b"hello" {
        assert!(session.character(*character, false).unwrap().handled);
    }
    let result = session.command(Command::CommitRaw).unwrap();
    assert_eq!(result.commit, "hello");
    assert_eq!(result.diagnostic, "");
    let learned: String = Connection::open(Path::new(&value.dictionaries).join("english.db"))
        .unwrap()
        .query_row(
            "SELECT display FROM english_words WHERE word='hello'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(learned, "hello");
}

// The shipped english.db weighs its words by Google unigram counts while the pinyin tables use their own scale, so an English weight says nothing about a Chinese one. Mixed input therefore never seats an English word ahead of the leading Chinese candidate: not on its shipped weight, not after it is committed, not after it is pinned. Pinning only reorders it among the English words.
#[test]
fn mixed_english_never_takes_the_first_seat_from_chinese() {
    const ENGLISH: u8 = 4;
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.learning = true;
    value.english_minimum_prefix = 2;
    for directory in [&value.resources, &value.dictionaries] {
        let directory = Path::new(directory);
        Connection::open(directory.join("msime.db"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_1_n VALUES('ni','n','你',2000);
                 INSERT INTO tbl_1_n VALUES('ni','n','倪',1000);
                 CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                 CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);",
            )
            .unwrap();
        Connection::open(directory.join("english.db"))
            .unwrap()
            .execute_batch(&format!(
                "{ENGLISH_SCHEMA}
                 INSERT INTO english_words VALUES('nimbus','Nimbus',50000);
                 INSERT INTO english_words VALUES('ninja','Ninja',30000);"
            ))
            .unwrap();
    }
    let mut session = Session::new(&value).unwrap();
    let typed = |session: &mut Session| {
        session.command(Command::Cancel).unwrap();
        for character in b"ni" {
            assert!(session.character(*character, false).unwrap().handled);
        }
        let snapshot = session.snapshot().unwrap();
        assert_eq!(
            snapshot.candidate_sources.first(),
            Some(&0),
            "an English word took the first seat: {:?}",
            snapshot.candidates
        );
        assert_eq!(snapshot.candidates[0], "你", "{:?}", snapshot.candidates);
        snapshot
    };
    let index_of = |snapshot: &EngineSnapshot, word: &str| {
        snapshot
            .candidates
            .iter()
            .zip(&snapshot.candidate_sources)
            .position(|(candidate, source)| candidate == word && *source == ENGLISH)
            .unwrap_or_else(|| panic!("{word} is not offered: {:?}", snapshot.candidates))
    };

    let snapshot = typed(&mut session);
    assert_eq!(
        snapshot.candidates[1], "Nimbus",
        "{:?}",
        snapshot.candidates
    );
    session.select(index_of(&snapshot, "Nimbus")).unwrap();
    typed(&mut session);
    let snapshot = typed(&mut session);
    let ninja = index_of(&snapshot, "Ninja");
    assert!(session.pin_candidate(ninja).unwrap().handled);
    let snapshot = typed(&mut session);
    assert_eq!(snapshot.candidates[1], "Ninja", "{:?}", snapshot.candidates);
}

fn english_word_count(value: &EngineOptions, word: &str) -> i64 {
    let database = Path::new(&value.dictionaries).join("english.db");
    if !database.exists() {
        return 0;
    }
    let database = Connection::open(database).unwrap();
    let tables: i64 = database
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='english_words'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    if tables == 0 {
        return 0;
    }
    database
        .query_row(
            "SELECT COUNT(*) FROM english_words WHERE word=?1",
            [word],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn raw_commit_without_learning_leaves_the_english_dictionary_alone() {
    for dedicated in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let value = options(dir.path());
        let mut session = Session::new(&value).unwrap();
        session.set_dedicated_english(dedicated).unwrap();
        for character in b"hello" {
            assert!(session.character(*character, false).unwrap().handled);
        }
        let result = session.command(Command::CommitRawWithoutLearning).unwrap();
        assert!(result.handled && result.has_commit);
        assert_eq!(result.commit, "hello");
        assert!(session.snapshot().unwrap().preedit.is_empty());
        assert_eq!(english_word_count(&value, "hello"), 0);
    }
}

#[test]
fn complete_pinyin_raw_commit_does_not_learn_as_english() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let mut session = Session::new(&value).unwrap();
    for character in b"ni" {
        assert!(session.character(*character, false).unwrap().handled);
    }
    assert_eq!(session.command(Command::CommitRaw).unwrap().commit, "ni");
    let database = Path::new(&value.dictionaries).join("english.db");
    if database.exists() {
        let count: i64 = Connection::open(database)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='english_words'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }
}

#[test]
fn incomplete_pinyin_raw_commit_is_learned_as_an_english_word() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let mut session = Session::new(&value).unwrap();
    type_text(&mut session, b"xyz");
    let result = session.command(Command::CommitRaw).unwrap();
    assert_eq!(result.commit, "xyz");
    assert_eq!(result.diagnostic, "");
    assert_eq!(english_word_count(&value, "xyz"), 1);
}

/// bridge.cpp:1332-1359: Enter in any local mode learns the committed letters as an English word. Local modes are entered only from the pinyin schemes, whose segmentation is empty while one is active, so the incomplete-pinyin rule would learn the word too; this pins the outcome the two rules share.
#[test]
fn local_mode_raw_commit_is_learned_as_an_english_word() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let mut session = Session::new(&value).unwrap();
    assert!(session.character(b'Y', true).unwrap().handled);
    type_text(&mut session, b"rustacean");
    assert_eq!(session.snapshot().unwrap().local_mode, "temporary_english");
    let result = session.command(Command::CommitRaw).unwrap();
    assert!(result.has_commit);
    assert_eq!(result.commit, "rustacean");
    assert_eq!(result.diagnostic, "");
    assert_eq!(english_word_count(&value, "rustacean"), 1);
}

/// bridge.cpp:1345-1348: in temporary Japanese the word is learned with its `R` trigger put back in front, so the letters typed in that mode stay apart from the same letters typed as English.
#[test]
fn temporary_japanese_raw_commit_learns_with_the_r_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let mut session = Session::new(&value).unwrap();
    assert!(session.character(b'R', true).unwrap().handled);
    type_text(&mut session, b"kk");
    assert_eq!(session.snapshot().unwrap().local_mode, "temporary_japanese");
    let result = session.command(Command::CommitRaw).unwrap();
    assert!(result.has_commit);
    assert!(!result.commit.starts_with('R'), "{:?}", result.commit);
    assert!(
        result.commit.bytes().all(|byte| byte.is_ascii_alphabetic()),
        "{:?}",
        result.commit
    );
    assert_eq!(result.diagnostic, "");
    let learned = format!("r{}", result.commit).to_ascii_lowercase();
    assert_eq!(english_word_count(&value, &learned), 1);
    assert_eq!(english_word_count(&value, &result.commit), 0);
}

/// bridge.cpp:1349-1357: a local-mode commit that is not an English word (here the Unicode mode's hex digits) is still committed; the failed learning is reported beside it rather than undoing it.
#[test]
fn unlearnable_local_mode_raw_commit_reports_the_diagnostic_but_still_commits() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let mut session = Session::new(&value).unwrap();
    assert!(session.character(b'U', true).unwrap().handled);
    type_text(&mut session, b"4e2d");
    let result = session.command(Command::CommitRaw).unwrap();
    assert!(result.handled && result.has_commit);
    assert!(!result.commit.is_empty());
    assert_eq!(
        result.diagnostic,
        crate::diagnostics::ENGLISH_WORD_NOT_LEARNED
    );
    assert_eq!(result.diagnostic, "English word could not be learned.");
    assert!(session.snapshot().unwrap().preedit.is_empty());
}

#[test]
fn host_argument_checks_use_the_bridge_errors() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = Session::new(&options(dir.path())).unwrap();
    let message = |error: crate::EngineError| error.to_string();
    assert_eq!(
        message(session.character(200, false).unwrap_err()),
        "Engine character must be ASCII"
    );
    assert_eq!(
        message(session.punctuation(128).unwrap_err()),
        "Engine punctuation must be ASCII"
    );
    assert_eq!(
        message(
            session
                .balance_paired_punctuation_after_auto_close(0xE3)
                .unwrap_err()
        ),
        "Paired punctuation opening must be ASCII"
    );
    for position in [0, 6] {
        assert_eq!(
            message(session.fix_candidate_position(0, position).unwrap_err()),
            "Invalid candidate position"
        );
    }
    assert_eq!(
        message(session.set_punctuation_lock(3).unwrap_err()),
        "Invalid punctuation lock"
    );
    session.set_punctuation_lock(2).unwrap();
}

#[test]
fn session_options_map_every_host_field() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.scheme = 1;
    value.shuangpin_profile = 2;
    value.shuangpin_preedit_uses_raw = false;
    value.learning = true;
    value.autocorrect_transposition = false;
    value.autocorrect_neighbor = true;
    value.fuzzy_pinyin_rules = u32::MAX;
    value.wubi_mixed_pinyin = true;
    value.helpcode = true;
    value.helpcode_schema = "xiaohe".into();
    value.chinese_punctuation = false;
    value.paired_punctuation = false;
    value.punctuation_lock = 2;
    value.frequency_mode = "linear".into();
    value.frequency_trigger_count = 3;
    value.frequency_linear_step = 4;
    value.mixed_english = false;
    value.english_minimum_prefix = 7;
    value.mixed_emoji = true;
    value.mixed_kaomoji = true;
    value.local_date_time = false;
    value.local_temporary_japanese = false;
    value.sentence_association.neural_keyboard = true;
    value.rescoring_context = "上文".into();
    value.sentence_alternatives = false;
    value.vietnamese_input_method = 1;
    value.vietnamese_tone_style = 1;
    let mapped = super::options::session_options(&value).unwrap();
    assert_eq!(mapped.paths.dictionaries, Path::new(&value.dictionaries));
    assert_eq!(
        mapped.vietnamese_input_method,
        crate::vietnamese::InputMethod::Vni
    );
    assert_eq!(
        mapped.vietnamese_tone_style,
        crate::vietnamese::ToneStyle::Classic
    );
    assert_eq!(mapped.scheme, crate::SchemeType::Shuangpin);
    assert_eq!(
        mapped.shuangpin_profile,
        crate::ShuangpinProfileKind::Shoudao
    );
    assert!(!mapped.shuangpin_preedit_uses_raw);
    assert!(mapped.learning);
    assert_eq!(mapped.autocorrect_types, crate::autocorrect_type::NEIGHBOR);
    assert_eq!(mapped.fuzzy_pinyin.rules, crate::fuzzy_rule::ALL);
    assert!(mapped.wubi.mixed_pinyin);
    assert!(mapped.helpcode);
    assert_eq!(mapped.helpcode_schema, "xiaohe");
    assert!(!mapped.chinese_punctuation);
    assert!(!mapped.paired_punctuation);
    assert_eq!(mapped.punctuation_lock, 2);
    assert_eq!(
        mapped.frequency.mode,
        crate::FrequencyAdjustmentMode::Linear
    );
    assert_eq!(mapped.frequency.trigger_count, 3);
    assert_eq!(mapped.frequency.linear_step, 4);
    assert!(!mapped.english.mixed_candidates);
    assert_eq!(mapped.english.minimum_prefix, 7);
    assert!(mapped.expressive.emoji_candidates && mapped.expressive.kaomoji_candidates);
    assert!(!mapped.local_modes.date_time && !mapped.local_modes.temporary_japanese);
    assert!(mapped.local_modes.unicode && mapped.local_modes.temporary_english);
    assert!(mapped.sentence_association.neural_keyboard);
    assert_eq!(mapped.rescoring_context, "上文");
    assert!(!mapped.sentence_alternatives);
    assert!(mapped.personal_context);
    assert_eq!(mapped.enabled_schemes, crate::SchemeSet::ALL);
}

/// `enabled_schemes` 原样交给会话。双拼不在其中时双拼键位不校验，不合法的值按小鹤处理；双拼在其中时照旧报错。
#[test]
fn session_options_skip_the_shuangpin_profile_without_shuangpin() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.scheme = crate::SchemeType::Wubi as u8;
    value.shuangpin_profile = 200;
    assert_eq!(
        super::options::session_options(&value)
            .unwrap_err()
            .to_string(),
        "Unsupported shuangpin profile"
    );

    value.enabled_schemes = crate::SchemeSet::of(&[crate::SchemeType::Wubi]);
    let mapped = super::options::session_options(&value).unwrap();
    assert_eq!(mapped.enabled_schemes, value.enabled_schemes);
    assert_eq!(
        mapped.shuangpin_profile,
        crate::ShuangpinProfileKind::Xiaohe
    );
    let session = Session::new(&value).unwrap();
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.scheme, crate::SchemeType::Wubi as u8);
    assert_eq!(snapshot.shuangpin_profile, "xiaohe");
    assert!(!snapshot.microsoft_shuangpin);

    value.scheme = crate::SchemeType::Quanpin as u8;
    assert_eq!(
        Session::new(&value).err().unwrap().to_string(),
        "Input scheme is not enabled"
    );
}

#[test]
fn translation_sidecar_prefers_the_user_file_and_is_removed_without_one() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let target = Path::new(&value.dictionaries).join("custom_translations.txt");
    let user = Path::new(&value.user_data).join("custom_translations.txt");
    let resource = Path::new(&value.resources).join("custom_translations.txt");
    std::fs::write(&resource, "天\tpackaged\n").unwrap();
    super::options::prepare_translation_sidecar(&value).unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "天\tpackaged\n");
    std::fs::write(&user, "天\tmine\n").unwrap();
    super::options::prepare_translation_sidecar(&value).unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "天\tmine\n");
    std::fs::remove_file(&user).unwrap();
    std::fs::remove_file(&resource).unwrap();
    Session::new(&value).unwrap();
    assert!(!target.exists());
}

#[test]
fn translation_sidecar_copy_rejects_an_oversized_source() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let resource = Path::new(&value.resources).join("custom_translations.txt");
    std::fs::File::create(&resource)
        .unwrap()
        .set_len(1024 * 1024 + 1)
        .unwrap();

    assert_eq!(
        super::options::prepare_translation_sidecar(&value)
            .unwrap_err()
            .to_string(),
        "Unable to prepare custom translation sidecar"
    );
    assert!(!Path::new(&value.dictionaries)
        .join("custom_translations.txt")
        .exists());
}

#[cfg(unix)]
#[test]
fn translation_sidecar_copy_rejects_a_symlinked_user_file() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let target = Path::new(&value.dictionaries).join("custom_translations.txt");
    let user = Path::new(&value.user_data).join("custom_translations.txt");
    let resource = Path::new(&value.resources).join("custom_translations.txt");
    std::fs::write(&resource, "天\tpackaged\n").unwrap();
    std::fs::write(
        outside.path().join("custom_translations.txt"),
        "天\texternal\n",
    )
    .unwrap();
    symlink(outside.path().join("custom_translations.txt"), &user).unwrap();

    super::options::prepare_translation_sidecar(&value).unwrap();
    assert_eq!(std::fs::read_to_string(target).unwrap(), "天\tpackaged\n");
}

#[cfg(unix)]
#[test]
fn translation_sidecar_copy_rejects_a_symlinked_target() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let target = Path::new(&value.dictionaries).join("custom_translations.txt");
    let user = Path::new(&value.user_data).join("custom_translations.txt");
    let external = outside.path().join("target.txt");
    std::fs::write(&user, "天\tuser\n").unwrap();
    std::fs::write(&external, "keep\n").unwrap();
    symlink(&external, &target).unwrap();

    assert_eq!(
        super::options::prepare_translation_sidecar(&value)
            .unwrap_err()
            .to_string(),
        "Unable to prepare custom translation sidecar"
    );
    assert_eq!(std::fs::read_to_string(external).unwrap(), "keep\n");
}

#[cfg(unix)]
#[test]
fn translation_sidecar_copy_rejects_a_symlinked_target_parent() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let dictionaries = Path::new(&value.dictionaries).to_path_buf();
    std::fs::remove_dir(&dictionaries).unwrap();
    symlink(outside.path(), &dictionaries).unwrap();
    std::fs::write(
        Path::new(&value.resources).join("custom_translations.txt"),
        "天\tpackaged\n",
    )
    .unwrap();

    assert_eq!(
        super::options::prepare_translation_sidecar(&value)
            .unwrap_err()
            .to_string(),
        "Unable to prepare custom translation sidecar"
    );
    assert!(!outside.path().join("custom_translations.txt").exists());
}

#[test]
fn english_completions_validate_and_lowercase_the_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let resources = dir.path().to_str().unwrap();
    let message = |prefix: &str, limit| {
        english_completions(resources, prefix, limit)
            .unwrap_err()
            .to_string()
    };
    assert_eq!(message("he", 0), "Invalid English completion limit");
    assert_eq!(message("he", 33), "Invalid English completion limit");
    assert_eq!(message("he1", 5), "Invalid English completion prefix");
    assert_eq!(
        english_completions(resources, "", 5).unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(message("he", 5), "English dictionary unavailable");
    Connection::open(dir.path().join("english.db"))
        .unwrap()
        .execute_batch(&format!(
            "{ENGLISH_SCHEMA}
             INSERT INTO english_words VALUES('hello','Hello',10);
             INSERT INTO english_words VALUES('help','help',20);
             INSERT INTO english_words VALUES('world','world',30);"
        ))
        .unwrap();
    assert_eq!(
        english_completions(resources, "HEL", 32).unwrap(),
        vec!["help", "Hello"]
    );
    assert_eq!(
        english_completions(resources, "hel", 1).unwrap(),
        vec!["help"]
    );
}

#[test]
fn gloss_keys_follow_the_bridge_rules() {
    assert_eq!(
        candidate_gloss_key("Don't Stop-Me", 4),
        Some(("don't stop-me".to_owned(), false))
    );
    assert_eq!(
        candidate_gloss_key("天气", 0),
        Some(("天气".to_owned(), true))
    );
    // Mixed text with a Han character is looked up verbatim as Chinese.
    assert_eq!(
        candidate_gloss_key("A股", 0),
        Some(("A股".to_owned(), true))
    );
    assert_eq!(candidate_gloss_key("天气", 6), None);
    assert_eq!(candidate_gloss_key("天气", 7), None);
    assert_eq!(candidate_gloss_key("- '", 4), None);
    assert_eq!(candidate_gloss_key("123", 0), None);
    assert_eq!(candidate_gloss_key("〇", 0), None);
}

#[test]
fn gloss_display_keeps_two_senses_and_withholds_control_characters() {
    assert_eq!(candidate_gloss_display("a; b; c"), "a; b");
    let joined = candidate_gloss_display("alpha;beta");
    assert_eq!(joined, "alpha; beta");
    assert_eq!(joined.capacity(), joined.len());
    assert_eq!(candidate_gloss_display("甲；乙;丙"), "甲; 乙");
    assert_eq!(
        candidate_gloss_display(";; first ;  ; second"),
        "first; second"
    );
    assert_eq!(
        candidate_gloss_display("  spaced \t out\r\nwords  "),
        "spaced out words"
    );
    assert_eq!(candidate_gloss_display("only"), "only");
    assert_eq!(candidate_gloss_display(""), "");
    assert_eq!(candidate_gloss_display("bad\u{7}bell"), "");
    assert_eq!(candidate_gloss_display("bad\u{85}next"), "");
    // A control character in the third sense is cut before the check sees it.
    assert_eq!(candidate_gloss_display("a; b; \u{1}"), "a; b");
}

#[test]
fn emoji_catalog_wrappers_page_through_others_db() {
    let dir = tempfile::tempdir().unwrap();
    let resources = dir.path().to_str().unwrap();
    Connection::open(dir.path().join("others.db"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE emoji(emoji TEXT, category TEXT, keywords TEXT, pinyin TEXT, sort_order INTEGER);
             INSERT INTO emoji VALUES('😀','smileys','grin','xiao',1),('😀','smileys','grin','xiao',2),('🐱','animals','cat','mao',3);
             CREATE TABLE kaomoji_catalog(kaomoji TEXT, keywords TEXT, sort_order INTEGER);
             INSERT INTO kaomoji_catalog VALUES('(^_^)','smile',1);
             CREATE TABLE symbol_catalog(symbol TEXT, category TEXT, parent_category TEXT, keywords TEXT, sort_order INTEGER);
             INSERT INTO symbol_catalog VALUES('→','arrows','math','right',1);",
        )
        .unwrap();
    let page = emoji_catalog_filtered_page(resources, "", "", "", 0, 10, "").unwrap();
    assert_eq!(
        page.iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        ["😀", "🐱"]
    );
    let slice = emoji_catalog_slice(resources, "", "", "", 0, 2, "").unwrap();
    assert_eq!(slice.items.len(), 2);
    assert_eq!(slice.next_offset, 2);
    assert!(!slice.complete);
    assert_eq!(
        emoji_catalog_slice(resources, "", "", "", 0, 0, "")
            .unwrap_err()
            .to_string(),
        "Invalid emoji catalog page"
    );
    assert_eq!(
        emoji_catalog_groups(resources, "").unwrap(),
        ["smileys", "animals"]
    );
    assert_eq!(emoji_catalog_groups(resources, "kaomoji").unwrap(), ["All"]);
    let groups = emoji_symbol_groups(resources).unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(
        (groups[0].parent.as_str(), groups[0].title.as_str()),
        ("math", "arrows")
    );
    let missing = tempfile::tempdir().unwrap();
    assert_eq!(
        emoji_symbol_groups(missing.path().to_str().unwrap())
            .unwrap_err()
            .to_string(),
        "Emoji catalog unavailable"
    );
}

#[test]
fn hanzi_to_pinyin_reads_the_generation_dictionary() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    Connection::open(Path::new(&value.dictionaries).join("msime.db"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',100);",
        )
        .unwrap();
    assert_eq!(hanzi_to_pinyin(&value, "你好"), "ni'hao");
    assert_eq!(hanzi_to_pinyin(&value, "hello"), "");
}

/// ni'hao 你好 100 / 拟好 80 plus the english schema v3 (`dictionary_stage/tests.rs:6-30`), written into the resources and the generation alike instead of staged.
fn helpcode_fixture(root: &Path, extra_sql: &str, helpcodes: &str) -> EngineOptions {
    let mut options = options(root);
    for directory in [&options.resources, &options.dictionaries] {
        let directory = Path::new(directory);
        Connection::open(directory.join("msime.db"))
            .unwrap()
            .execute_batch(&format!(
                "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',100),('ni''hao','nh','拟好',80);
                 CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                 CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);
                 CREATE INDEX idx_quick_parases_key_weight ON quick_parases(key,weight DESC);
                 {extra_sql}"
            ))
            .unwrap();
        crate::ensure_english_schema(&directory.join("english.db")).unwrap();
    }
    let directory = Path::new(&options.resources).join("helpcodes");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("zrm_helpcode_big_unique.txt"), helpcodes).unwrap();
    options.helpcode = true;
    options
}

/// helpcode_utils.cpp:57-67: a table that exists but cannot be read gives an empty keymap, so the host still gets a session, only without helpcode annotations.
#[test]
fn an_unreadable_helpcode_table_still_creates_the_session() {
    let root = tempfile::tempdir().unwrap();
    let options = helpcode_fixture(root.path(), "", "");
    let table = Path::new(&options.resources).join("helpcodes/zrm_helpcode_big_unique.txt");
    std::fs::remove_file(&table).unwrap();
    std::fs::create_dir(&table).unwrap();
    let mut session = Session::new(&options).expect("an unreadable table failed the session");
    type_text(&mut session, b"nihao");
    let view = session.snapshot().unwrap();
    assert!(!view.candidates.is_empty());
    assert!(
        view.candidate_annotations.iter().all(String::is_empty),
        "{:?}",
        view.candidate_annotations
    );
}

#[test]
fn helpcode_display_toggle_keeps_candidates_and_filtering_enabled() {
    let root = tempfile::tempdir().unwrap();
    let mut options = helpcode_fixture(root.path(), "", "你=ab\n好=cd\n拟=ef\n");
    let mut candidates = None;
    for visible in [true, false, true] {
        options.show_helpcode = visible;
        let mut session = Session::new(&options).unwrap();
        type_text(&mut session, b"nihao");
        let view = session.snapshot().unwrap();
        assert!(!view.candidates.is_empty());
        if let Some(previous) = &candidates {
            assert_eq!(&view.candidates, previous);
        }
        candidates = Some(view.candidates.clone());
        assert_eq!(
            view.candidate_annotations
                .iter()
                .any(|value| !value.is_empty()),
            visible,
            "{:?}",
            view.candidate_annotations
        );
        assert!(session.character(b'A', true).unwrap().handled);
    }
}

#[test]
fn hiding_helpcode_restores_correction_annotations() {
    let root = tempfile::tempdir().unwrap();
    let mut options = helpcode_fixture(
        root.path(),
        "CREATE TABLE tbl_2_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);
         INSERT INTO tbl_2_s VALUES('shang''hao','sh','上好',100);",
        "上=ab\n好=cd\n",
    );
    for visible in [true, false, true] {
        options.show_helpcode = visible;
        let mut session = Session::new(&options).unwrap();
        // Same transposition the reference's pinyin correction tests use.
        type_text(&mut session, b"sahnghao");
        let view = session.snapshot().unwrap();
        let index = view
            .candidates
            .iter()
            .position(|text| text == "上好")
            .unwrap_or_else(|| panic!("{:?}", view.candidates));
        let annotation = &view.candidate_annotations[index];
        assert!(view.candidate_corrected[index]);
        if visible {
            assert!(!annotation.is_empty());
            assert_ne!(annotation, "sahnghao");
        } else {
            assert_eq!(annotation, "sahnghao");
        }
    }
}

#[test]
fn snapshot_vectors_stay_parallel_to_the_candidates() {
    let root = tempfile::tempdir().unwrap();
    let options = helpcode_fixture(root.path(), "", "你=ab\n好=cd\n拟=ef\n");
    let mut session = Session::new(&options).unwrap();
    type_text(&mut session, b"nihao");
    let view = session.snapshot().unwrap();
    let count = view.candidates.len();
    assert!(count > 0);
    for length in [
        view.candidate_codes.len(),
        view.candidate_annotations.len(),
        view.candidate_sources.len(),
        view.candidate_positions.len(),
        view.candidate_corrected.len(),
        view.candidate_answers_key.len(),
    ] {
        assert_eq!(length, count);
    }
    assert_eq!(view.local_mode, "none");
    assert_eq!(view.scheme, 0);
    assert_eq!(view.reading, "");
    assert_eq!(view.shuangpin_profile, "xiaohe");
    assert!(!view.microsoft_shuangpin);
}

/// 98 五笔会话读 `wubi98`，选词的学习记录归入 `wubi98`，不碰 `wubi86`。
#[test]
fn a_wubi98_session_reads_and_learns_into_wubi98() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.scheme = 2;
    value.wubi_profile = 1;
    value.learning = true;
    for directory in [&value.resources, &value.dictionaries] {
        Connection::open(Path::new(directory).join("msime.db"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                 INSERT INTO wubi86 VALUES('kg','甲',300);
                 CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER);
                 INSERT INTO wubi98 VALUES('kg','乙',300),('kg','丙',100);
                 CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);",
            )
            .unwrap();
    }
    let mut session = Session::new(&value).unwrap();
    type_text(&mut session, b"kg");
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.candidates, vec!["乙", "丙"]);
    assert!(session.select(1).unwrap().has_commit);
    drop(session);
    let journal = Connection::open(Path::new(&value.user_data).join("msime_user.db")).unwrap();
    let count = |dictionary: &str| -> i64 {
        journal
            .query_row(
                "SELECT count(*) FROM user_dictionary_operations WHERE dictionary=?1",
                [dictionary],
                |row| row.get(0),
            )
            .unwrap()
    };
    assert_eq!(count("wubi98"), 1);
    assert_eq!(count("wubi"), 0);
}

// The C++ wrote the queued personal context from `atexit`; here the dropped session writes it, so a host that quits within the ~2 s flush delay of its last pick keeps it.
#[test]
fn dropping_a_session_writes_its_queued_personal_context() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.learning = true;
    let rows = [
        ("ni", "甲", 300),
        ("ni", "丙", 100),
        ("hao", "子", 300),
        ("hao", "寅", 100),
    ];
    for directory in [&value.resources, &value.dictionaries] {
        let main = Connection::open(Path::new(directory).join("msime.db")).unwrap();
        main.execute_batch(
            "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
             CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);",
        )
        .unwrap();
        for (key, word, weight) in rows {
            let table = crate::user_dictionary::journal::pinyin_table(key).unwrap();
            main.execute_batch(&format!(
                "CREATE TABLE IF NOT EXISTS \"{table}\"(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
            ))
            .unwrap();
            main.execute(
                &format!("INSERT INTO \"{table}\" VALUES(?1,?2,?3,?4)"),
                (key, &key[..1], word, weight),
            )
            .unwrap();
        }
    }
    let mut session = Session::new(&value).unwrap();
    for (typed, word) in [(&b"ni"[..], "丙"), (&b"hao"[..], "寅")] {
        type_text(&mut session, typed);
        let snapshot = session.snapshot().unwrap();
        let index = snapshot
            .candidates
            .iter()
            .position(|candidate| candidate == word)
            .unwrap_or_else(|| panic!("{word} is not offered: {:?}", snapshot.candidates));
        assert!(session.select(index).unwrap().has_commit);
    }
    drop(session);
    let count: i64 = Connection::open(Path::new(&value.user_data).join("msime_user.db"))
        .unwrap()
        .query_row(
            "SELECT count FROM personal_bigram WHERE previous='丙' AND word='寅'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);
}

/// test_personal_context_input_session.cpp:816-827: a journal that cannot be written keeps the commit and reports the personal context, without any of the input text. Frequency learning is off so its own diagnostic cannot take the slot first.
#[test]
fn a_failed_personal_context_write_keeps_the_commit_with_a_diagnostic() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.learning = true;
    value.frequency_mode = "disabled".into();
    for directory in [&value.resources, &value.dictionaries] {
        Connection::open(Path::new(directory).join("msime.db"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_1_n VALUES('ni','n','甲',300),('ni','n','丙',100);
                 CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                 CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);",
            )
            .unwrap();
    }
    std::fs::create_dir(Path::new(&value.user_data).join("msime_user.db")).unwrap();
    let mut session = Session::new(&value).unwrap();
    type_text(&mut session, b"ni");
    let snapshot = session.snapshot().unwrap();
    let index = snapshot
        .candidates
        .iter()
        .position(|candidate| candidate == "甲")
        .unwrap_or_else(|| panic!("甲 is not offered: {:?}", snapshot.candidates));
    let result = session.select(index).unwrap();
    assert!(result.handled && result.has_commit);
    assert_eq!(result.commit, "甲");
    assert_eq!(
        result.diagnostic,
        crate::diagnostics::PERSONAL_CONTEXT_NOT_PERSISTED
    );
    assert!(!result.diagnostic.contains('甲') && !result.diagnostic.contains("ni"));
}

/// local_database.cpp:34-38,63-66 opened msime's generation dictionary once per local-mode query, so nothing held it after the sessions were gone. A reset or snapshot restore replaces `msime.db` at the same path once every session is dropped (Windows needs the handle closed to rename it), and the next session must read the new file.
#[test]
fn local_mode_reads_follow_a_dictionary_replaced_after_the_sessions_are_gone() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let dictionary = |path: &Path, phrase: &str| {
        Connection::open(path)
            .unwrap()
            .execute_batch(&format!(
                "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                 CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);
                 INSERT INTO quick_parases VALUES('kx','{phrase}',10);"
            ))
            .unwrap();
    };
    let main = Path::new(&value.dictionaries).join("msime.db");
    dictionary(&Path::new(&value.resources).join("msime.db"), "旧短语");
    dictionary(&main, "旧短语");
    let phrases = || {
        let mut session = Session::new(&value).unwrap();
        assert!(session.character(b'K', true).unwrap().handled);
        type_text(&mut session, b"kx");
        let snapshot = session.snapshot().unwrap();
        assert_eq!(snapshot.local_mode, "quick_phrase");
        snapshot.candidates
    };
    assert!(phrases().contains(&"旧短语".to_owned()));
    let staged = dir.path().join("replacement.db");
    dictionary(&staged, "新短语");
    std::fs::rename(&staged, &main).unwrap();
    let candidates = phrases();
    assert!(
        candidates.contains(&"新短语".to_owned()) && !candidates.contains(&"旧短语".to_owned()),
        "the replaced dictionary is still read: {candidates:?}"
    );
}

/// user_dictionary_journal.cpp:445-452: the reference opened msime's journal per call, so no thread kept it open. A one-shot call releases its thread's cached journal on return, a dropped session releases it too, and a live session keeps it for the keystroke path.
#[test]
fn journal_handles_are_released_when_a_thread_is_done_with_them() {
    use crate::user_dictionary::journal::thread_holds_journal;
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.learning = true;
    for directory in [&value.resources, &value.dictionaries] {
        Connection::open(Path::new(directory).join("msime.db"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',200),('ni''hao','nh','拟好',100);",
            )
            .unwrap();
    }

    // A pool thread editing the dictionary keeps nothing open afterwards, whatever the call returned.
    let edit = value.clone();
    std::thread::spawn(move || {
        let entry = DictionaryEntry {
            kind: DictionaryKind::Pinyin,
            key: "ni'hao".into(),
            value: "你蒿".into(),
            weight: 100_000,
        };
        dictionary_edit(&edit, None, Some(&entry), "release-test").unwrap();
        assert!(!thread_holds_journal());
        assert!(dictionary_state_revision(&edit).is_ok());
        assert!(!thread_holds_journal());
    })
    .join()
    .unwrap();

    let mut session = Session::new(&value).unwrap();
    type_text(&mut session, b"nihao");
    let snapshot = session.snapshot().unwrap();
    let index = snapshot
        .candidates
        .iter()
        .position(|word| word == "拟好")
        .unwrap();
    session.select(index).unwrap();
    assert!(thread_holds_journal());
    drop(session);
    assert!(!thread_holds_journal());
}

/// Hosts keep their sessions in a thread-local map, so a session can be dropped while its thread exits, after that thread's cached journal is already destroyed. Its drop still flushes and releases without touching the destroyed cache; it used to abort the process (`cannot access a Thread Local Storage value during or after destruction`).
#[test]
fn a_session_dropped_while_its_thread_exits_does_not_abort() {
    use crate::user_dictionary::journal::thread_holds_journal;
    thread_local! {
        static HELD: std::cell::RefCell<Option<Session>> = const { std::cell::RefCell::new(None) };
    }
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.learning = true;
    for directory in [&value.resources, &value.dictionaries] {
        Connection::open(Path::new(directory).join("msime.db"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',200),('ni''hao','nh','拟好',100);",
            )
            .unwrap();
    }
    let journal = Path::new(&value.user_data).join("msime_user.db");
    std::thread::spawn(move || {
        // The map is registered before the journal cache, so thread exit destroys the cache first.
        HELD.with(|held| {
            *held.borrow_mut() = Some(Session::new(&value).unwrap());
            let mut held = held.borrow_mut();
            let session = held.as_mut().unwrap();
            type_text(session, b"nihao");
            let index = session
                .snapshot()
                .unwrap()
                .candidates
                .iter()
                .position(|word| word == "拟好")
                .unwrap();
            session.select(index).unwrap();
        });
        assert!(thread_holds_journal());
    })
    .join()
    .unwrap();
    // The drop during thread exit wrote the queued pick, not merely survived.
    let written: i64 = Connection::open(&journal)
        .unwrap()
        .query_row(
            "SELECT count(*) FROM personal_bigram WHERE previous=char(1) AND word='拟好'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(written, 1);
}

fn generated_mode_options(root: &Path) -> EngineOptions {
    let mut value = options(root);
    value.local_expression = true;
    value.local_command = true;
    value.local_mention = true;
    value.command_table = vec![CommandTableEntry {
        trigger: "hi".into(),
        title: "问候".into(),
        template: "你好".into(),
    }];
    value.mention_entries = vec![MentionEntry {
        text: "Alice".into(),
        key: String::new(),
    }];
    value.quick_phrase_table = vec![QuickPhraseEntry {
        key: "dh".into(),
        text: "电话".into(),
    }];
    value
}

#[test]
fn generated_local_modes_map_through_the_options() {
    let dir = tempfile::tempdir().unwrap();
    let resources = dir.path().join("resources");
    std::fs::create_dir_all(&resources).unwrap();
    for name in ["msime.db", "english.db"] {
        Connection::open(resources.join(name)).unwrap();
    }
    let defaults = prepare_options(
        resources.to_str().unwrap(),
        dir.path().join("user").to_str().unwrap(),
        dir.path().join("cache").to_str().unwrap(),
        "generated-modes",
    )
    .unwrap();
    assert!(!defaults.local_expression && !defaults.local_command && !defaults.local_mention);
    assert!(defaults.command_table.is_empty() && defaults.mention_entries.is_empty());
    assert!(defaults.quick_phrase_table.is_empty());
    let value = generated_mode_options(dir.path());
    let mapped = super::options::session_options(&value).unwrap();
    assert!(
        mapped.local_modes.expression && mapped.local_modes.command && mapped.local_modes.mention
    );
    assert_eq!(mapped.command_table, value.command_table);
    assert_eq!(mapped.mention_entries, value.mention_entries);
    assert_eq!(mapped.quick_phrase_table, value.quick_phrase_table);
    let off = super::options::session_options(&options(dir.path())).unwrap();
    assert!(!off.local_modes.expression && !off.local_modes.command && !off.local_modes.mention);
}

#[test]
fn generated_local_modes_publish_their_spelling_symbols_and_rows() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = Session::new(&generated_mode_options(dir.path())).unwrap();
    assert_eq!(session.snapshot().unwrap().spelling_symbols, "/@");
    assert!(session.character(b'V', true).unwrap().handled);
    type_text(&mut session, b"2*3");
    let view = session.snapshot().unwrap();
    assert_eq!(view.local_mode, "expression");
    assert_eq!(view.spelling_symbols, "0123456789+-*/.()%^");
    assert_eq!(view.candidates[0], "6");
    assert!(!session.online_query().unwrap().available);
    session.command(Command::Cancel).unwrap();

    assert!(session.character(b'/', false).unwrap().handled);
    type_text(&mut session, b"h");
    let view = session.snapshot().unwrap();
    assert_eq!(view.local_mode, "command");
    assert_eq!(view.candidates, ["你好"]);
    assert_eq!(view.candidate_codes, ["hi"]);
    assert_eq!(view.candidate_annotations, ["问候"]);
    session.command(Command::Cancel).unwrap();

    assert!(session.character(b'@', false).unwrap().handled);
    type_text(&mut session, b"al");
    assert_eq!(session.snapshot().unwrap().candidates, ["Alice"]);
    session
        .set_mention_entries(&[MentionEntry {
            text: "Alan".into(),
            key: String::new(),
        }])
        .unwrap();
    assert_eq!(session.snapshot().unwrap().candidates, ["Alan"]);
    session.command(Command::Cancel).unwrap();
    session
        .set_command_table(&[CommandTableEntry {
            trigger: "yo".into(),
            title: "招呼".into(),
            template: "哟".into(),
        }])
        .unwrap();
    session.character(b'/', false).unwrap();
    type_text(&mut session, b"y");
    assert_eq!(session.snapshot().unwrap().candidates, ["哟"]);
}

/// Enter in the expression, command and mention modes commits what was typed, and unlike the other local modes learns none of it as an English word: arithmetic, a trigger or a mention key is not a word the user spelled.
#[test]
fn generated_local_mode_raw_commits_are_not_learned() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = generated_mode_options(dir.path());
    value.learning = true;
    let mut session = Session::new(&value).unwrap();
    for (entry, shift, input, expected) in [
        (b'V', true, &b"1+"[..], "V1+"),
        (b'/', false, &b"xyz"[..], "/xyz"),
        (b'@', false, &b"bob"[..], "@bob"),
    ] {
        assert!(session.character(entry, shift).unwrap().handled);
        type_text(&mut session, input);
        let result = session.command(Command::CommitRaw).unwrap();
        assert_eq!(result.commit, expected);
        assert_eq!(result.diagnostic, "");
        for word in [expected, &expected[1..]] {
            assert_eq!(english_word_count(&value, word), 0, "{word}");
        }
    }
}

#[test]
fn generated_rows_carry_no_helpcode() {
    let root = tempfile::tempdir().unwrap();
    let mut options = helpcode_fixture(root.path(), "", "一=ab\n二=cd\n三=ef\n");
    options.local_expression = true;
    let mut session = Session::new(&options).unwrap();
    session.character(b'V', true).unwrap();
    type_text(&mut session, b"123");
    let view = session.snapshot().unwrap();
    assert_eq!(view.candidates[0], "一百二十三");
    assert!(
        view.candidate_annotations.iter().all(String::is_empty),
        "{:?}",
        view.candidate_annotations
    );
}

#[test]
fn only_generated_modes_are_left_out_of_typing_statistics() {
    for mode in ["expression", "command", "mention"] {
        assert!(!local_mode_counts_as_typing(mode), "{mode}");
    }
    for mode in [
        "none",
        "unicode",
        "date_time",
        "temporary_english",
        "unknown",
        "",
    ] {
        assert!(local_mode_counts_as_typing(mode), "{mode}");
    }
}
