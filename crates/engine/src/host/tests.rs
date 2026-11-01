//! The port of the removed C++ bridge's `crates/engine-bridge/src/tests.rs` (tests-inventory.md §4.2) against the Rust facade, plus the two annotation tests of `engine-bridge/src/dictionary_stage/tests.rs` that exercise the host snapshot rule (fixtures written straight into the generation instead of staged), and unit tests of the bridge-local helpers.

use std::path::Path;

use rusqlite::Connection;

use super::glosses::{candidate_gloss_display, candidate_gloss_key};
use super::*;
use crate::types::{CandidateSource, SchemeType, WordItem};
use crate::{
    CommandTableEntry, DictionaryStateRecord, HelpcodeKeymap, MentionEntry, QuickPhraseEntry,
    SentenceAssociationOptions,
};

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
        first_page_rows: crate::types::DEFAULT_FIRST_PAGE_ROWS,
        scheme: 0,
        enabled_schemes: crate::types::SchemeSet::ALL,
        shuangpin_profile: 0,
        shuangpin_custom_profile: None,
        shuangpin_preedit_uses_raw: true,
        force_cantonese_shuangpin: false,
        single_character_only: false,
        caret_edit_suffix: false,
        learning: false,
        learn_english_on_enter: true,
        next_word_prediction: false,
        autocorrect_transposition: true,
        autocorrect_neighbor: true,
        fuzzy_pinyin_rules: 0,
        wubi_mixed_pinyin: false,
        wubi_z_wildcard: false,
        wubi_profile: 0,
        helpcode: false,
        strict_quanpin_helpcode: false,
        show_helpcode: true,
        shuangpin_helpcode_modes: crate::types::ShuangpinHelpcodeModes::default(),
        helpcode_schema: "ziranma".into(),
        helpcode_trigger_key: None,
        chinese_punctuation: true,
        paired_punctuation: true,
        punctuation_lock: 0,
        corner_quotes: false,
        hollow_brackets: false,
        enumeration_comma_backslash: true,
        enumeration_comma_slash: false,
        frequency_mode: "promote".into(),
        frequency_trigger_count: 1,
        frequency_linear_step: 1,
        mixed_candidates: true,
        mixed_english: true,
        english_minimum_prefix: 5,
        english_first_page_count: 1,
        mixed_emoji: false,
        mixed_kaomoji: false,
        mixed_symbols: false,
        mixed_japanese: false,
        multilingual_languages: Vec::new(),
        local_unicode: true,
        local_date_time: true,
        local_date_time_inline: true,
        local_date_time_menu: true,
        local_quick_phrase: true,
        local_emoji: true,
        local_kaomoji: true,
        local_super_jianpin: true,
        local_temporary_english: true,
        local_temporary_japanese: true,
        local_expression: false,
        local_command: false,
        local_mention: false,
        local_chaizi: false,
        local_symbol: true,
        local_shift_letter_entry: true,
        chaizi_traditional_output: false,
        command_table: Vec::new(),
        plugin_commands: Vec::new(),
        mention_entries: Vec::new(),
        quick_phrase_table: Vec::new(),
        quick_phrase_candidates: false,
        quick_phrase_frequency: true,
        helpcode_table: None,
        helpcode_fallback_resources: None,
        sentence_association: SentenceAssociationOptions {
            word_lattice: true,
            neural_keyboard: false,
            show_next_on_duplicate: false,
            ..Default::default()
        },
        rescoring_context: String::new(),
        sentence_alternatives: true,
        vietnamese_input_method: 0,
        vietnamese_tone_style: 0,
        korean_layout: 0,
        cantonese_dictionary: String::new(),
        zhuyin_dictionary: String::new(),
        stroke_dictionary: String::new(),
        japanese_dictionary: String::new(),
        wubi_dictionary: String::new(),
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
fn quanpin_host_preedit_uses_segmentation_without_changing_editing_or_commit() {
    for (raw, display) in [
        (b"nihao".as_slice(), "ni'hao"),
        (b"ni'hao".as_slice(), "ni'hao"),
        (b"nihao'".as_slice(), "ni'hao'"),
        (b"sahnghao".as_slice(), "sahng'hao"),
        (b"sahnghao'".as_slice(), "sahng'hao'"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut options = helpcode_fixture(root.path(), "", "");
        options.helpcode = false;
        let mut session = Session::new(&options).unwrap();
        type_text(&mut session, raw);
        let view = session.snapshot().unwrap();
        assert_eq!(view.preedit, display);
        assert_eq!(view.editing_text.as_bytes(), raw);
        assert_eq!(view.caret_position, raw.len());
        session.set_caret(Some(2));
        let moved = session.snapshot().unwrap();
        assert_eq!(moved.preedit, display);
        assert_eq!(moved.editing_text, view.editing_text);
        assert_eq!(moved.caret_position, 2);
        session.command(Command::MoveEnd).unwrap();
        assert_eq!(
            session
                .command(Command::CommitRaw)
                .unwrap()
                .commit
                .as_bytes(),
            raw
        );
    }
}

#[test]
fn quanpin_host_preedit_keeps_literal_and_local_modes_unsegmented() {
    let root = tempfile::tempdir().unwrap();
    let options = options(root.path());
    let mut session = Session::new(&options).unwrap();
    session.set_dedicated_english(true).unwrap();
    type_text(&mut session, b"nihao");
    assert_eq!(session.snapshot().unwrap().preedit, "nihao");
    session.command(Command::Cancel).unwrap();
    session.set_dedicated_english(false).unwrap();
    for raw in [b"Ynihao".as_slice(), b"https://example.test".as_slice()] {
        for byte in raw {
            assert!(
                session
                    .character(*byte, byte.is_ascii_uppercase())
                    .unwrap()
                    .handled
            );
        }
        let view = session.snapshot().unwrap();
        assert_eq!(view.preedit.as_bytes(), raw);
        session.command(Command::Cancel).unwrap();
    }
}

#[test]
fn quanpin_host_preedit_preserves_case_and_shuangpin_display_preference() {
    let root = tempfile::tempdir().unwrap();
    let mut options = options(root.path());
    let mut session = Session::new(&options).unwrap();
    for byte in b"niHao" {
        assert!(
            session
                .character(*byte, byte.is_ascii_uppercase())
                .unwrap()
                .handled
        );
    }
    assert_eq!(session.snapshot().unwrap().preedit, "niHao");
    assert_eq!(session.snapshot().unwrap().editing_text, "niHao");
    drop(session);
    options.scheme = SchemeType::Shuangpin as u8;
    for (raw_display, expected) in [(true, "nihc"), (false, "ni'hao")] {
        options.shuangpin_preedit_uses_raw = raw_display;
        let mut session = Session::new(&options).unwrap();
        type_text(&mut session, b"nihc");
        assert_eq!(session.snapshot().unwrap().preedit, expected);
        assert_eq!(session.snapshot().unwrap().editing_text, "nihc");
    }
}

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
    let packaged = resources.path().join("msime-english.db");
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
    Connection::open(resources.path().join("msime-english.db"))
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
    Connection::open(resources.path().join("msime-english.db"))
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

/// 释义来源在查询线程里按路径加文件身份缓存，省掉每个按键重新打开词库；同一线程上改写自定义译文、删掉学到的释义库、换掉发布词库，下一次查询都要看到新文件，结果与每次重新打开相同。替换的文件都比原来长，免得粗粒度的修改时间和复用的 inode 让新旧文件看起来一样。
#[test]
fn cached_glosses_follow_replaced_and_removed_files() {
    let resources = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let packaged = resources.path().join("msime-english.db");
    Connection::open(&packaged)
        .unwrap()
        .execute_batch(&format!(
            "{ENGLISH_SCHEMA} INSERT INTO zh_en_glosses VALUES('测试','packaged gloss');"
        ))
        .unwrap();
    let resources_path = resources.path().to_str().unwrap();
    let user_path = user.path().to_str().unwrap();
    let candidates = vec![("测试".into(), 0)];
    let glosses = || candidate_glosses_with_user(resources_path, user_path, &candidates).unwrap();
    assert_eq!(glosses(), vec!["packaged gloss"]);
    assert_eq!(glosses(), vec!["packaged gloss"]);

    let custom = user.path().join("custom_translations.txt");
    std::fs::write(&custom, "测试\thand written gloss\n").unwrap();
    assert_eq!(glosses(), vec!["hand written gloss"]);
    std::fs::write(
        &custom,
        "# edited in Settings\n测试\tedited hand written gloss\n",
    )
    .unwrap();
    assert_eq!(glosses(), vec!["edited hand written gloss"]);
    std::fs::remove_file(&custom).unwrap();
    assert_eq!(glosses(), vec!["packaged gloss"]);

    assert!(save_candidate_gloss(
        user_path,
        true,
        "测试",
        "learned gloss"
    ));
    assert_eq!(glosses(), vec!["learned gloss"]);
    assert!(save_candidate_gloss(
        user_path,
        true,
        "测试",
        "relearned gloss"
    ));
    assert_eq!(glosses(), vec!["relearned gloss"]);
    std::fs::remove_file(user.path().join("translation-glosses.db")).unwrap();
    assert_eq!(glosses(), vec!["packaged gloss"]);

    std::fs::remove_file(&packaged).unwrap();
    Connection::open(&packaged)
        .unwrap()
        .execute_batch(&format!(
            "{ENGLISH_SCHEMA} INSERT INTO zh_en_glosses VALUES('测试','replaced gloss');
             CREATE TABLE padding(value BLOB); INSERT INTO padding VALUES(zeroblob(65536));"
        ))
        .unwrap();
    assert_eq!(glosses(), vec!["replaced gloss"]);
    assert_eq!(
        candidate_glosses(resources_path, &candidates).unwrap(),
        vec!["replaced gloss"]
    );
    std::fs::remove_file(&packaged).unwrap();
    assert!(candidate_glosses(resources_path, &candidates).is_err());
}

/// 离线释义库的连接同样按文件身份缓存：换成另一种语言的文件、再删掉，下一次查询给出和重新打开一样的结果。
#[test]
fn cached_offline_target_glosses_follow_replaced_and_removed_files() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("zh-fr.db");
    offline_gloss_database(&path, "fr", 1);
    let candidates = vec![("猫".into(), 0)];
    let glosses = || candidate_target_glosses(path.to_str().unwrap(), "fr", &candidates);
    assert_eq!(glosses().unwrap(), vec!["chat, chatte; félin"]);
    assert_eq!(glosses().unwrap(), vec!["chat, chatte; félin"]);

    std::fs::remove_file(&path).unwrap();
    offline_gloss_database(&path, "ja", 1);
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TABLE padding(value BLOB); INSERT INTO padding VALUES(zeroblob(65536));",
        )
        .unwrap();
    assert_eq!(
        glosses().unwrap_err().to_string(),
        "Offline gloss dictionary language mismatch"
    );
    assert_eq!(
        candidate_target_glosses(path.to_str().unwrap(), "ja", &candidates).unwrap(),
        vec!["chat, chatte; félin"]
    );

    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        glosses().unwrap_err().to_string(),
        "Offline gloss dictionary unavailable"
    );
    assert!(!path.exists(), "a read-only open must not create the file");
}

/// 键盘收起、内存吃紧时宿主释放调用线程的释义缓存；之后的查询重新打开词库、重新读自定义译文，结果与释放前相同，没打开过任何缓存时释放也不出错。
#[test]
fn released_gloss_caches_reopen_on_the_next_query() {
    release_gloss_caches();
    let resources = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    Connection::open(resources.path().join("msime-english.db"))
        .unwrap()
        .execute_batch(&format!(
            "{ENGLISH_SCHEMA} INSERT INTO zh_en_glosses VALUES('测试','packaged gloss');"
        ))
        .unwrap();
    std::fs::write(
        user.path().join("custom_translations.txt"),
        "你好\thand written gloss\n",
    )
    .unwrap();
    let target = resources.path().join("zh-fr.db");
    offline_gloss_database(&target, "fr", 1);
    let resources_path = resources.path().to_str().unwrap();
    let user_path = user.path().to_str().unwrap();
    let candidates = vec![("测试".into(), 0), ("你好".into(), 0)];
    let glosses = || candidate_glosses_with_user(resources_path, user_path, &candidates).unwrap();
    let target_candidates = vec![("猫".into(), 0)];
    let target_glosses =
        || candidate_target_glosses(target.to_str().unwrap(), "fr", &target_candidates).unwrap();
    assert_eq!(glosses(), vec!["packaged gloss", "hand written gloss"]);
    assert_eq!(target_glosses(), vec!["chat, chatte; félin"]);

    release_gloss_caches();
    assert_eq!(glosses(), vec!["packaged gloss", "hand written gloss"]);
    assert_eq!(target_glosses(), vec!["chat, chatte; félin"]);
}

#[cfg(unix)]
#[test]
fn learned_glosses_reject_a_symlinked_database() {
    use std::os::unix::fs::symlink;

    let resources = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    Connection::open(resources.path().join("msime-english.db"))
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
    Connection::open(resources.path().join("msime-english.db"))
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

fn pronunciation_database(path: &Path, kind: &str, version: i32) {
    let database = Connection::open(path).unwrap();
    database
        .execute_batch(&format!(
            "CREATE TABLE en_phonetics(word TEXT PRIMARY KEY, phonetic TEXT NOT NULL) WITHOUT ROWID;
             CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
             INSERT INTO meta VALUES('kind', '{kind}');
             INSERT INTO en_phonetics VALUES('love', 'lʌv');
             INSERT INTO en_phonetics VALUES('sky', 'skaɪ');
             PRAGMA user_version = {version};"
        ))
        .unwrap();
}

#[test]
fn english_phonetics_answer_known_words_in_order() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("en-phonetic.db");
    pronunciation_database(&path, "en_phonetic", 1);
    let words = vec!["sky".to_owned(), "unknown".to_owned(), "love".to_owned()];
    assert_eq!(
        english_phonetics(path.to_str().unwrap(), &words).unwrap(),
        vec!["skaɪ", "", "lʌv"]
    );
    assert!(english_phonetics(path.to_str().unwrap(), &[])
        .unwrap()
        .is_empty());
}

#[test]
fn english_phonetics_refuse_another_kind_version_or_a_missing_file() {
    let directory = tempfile::tempdir().unwrap();
    let words = vec!["love".to_owned()];
    let gloss = directory.path().join("gloss.db");
    pronunciation_database(&gloss, "zh_gloss", 1);
    assert!(english_phonetics(gloss.to_str().unwrap(), &words).is_err());
    let future = directory.path().join("future.db");
    pronunciation_database(&future, "en_phonetic", 2);
    assert!(english_phonetics(future.to_str().unwrap(), &words).is_err());
    let missing = directory.path().join("missing.db");
    assert!(english_phonetics(missing.to_str().unwrap(), &words).is_err());
    assert!(english_phonetics("", &words).is_err());
    assert!(
        !missing.exists(),
        "a read-only open must not create the file"
    );
}

#[test]
fn target_phonetics_read_their_own_language_table_only() {
    let directory = tempfile::tempdir().unwrap();
    let french = directory.path().join("fr-phonetic.db");
    Connection::open(&french)
        .unwrap()
        .execute_batch(
            "CREATE TABLE fr_phonetics(word TEXT PRIMARY KEY, phonetic TEXT NOT NULL) WITHOUT ROWID;
             CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
             INSERT INTO meta VALUES('kind', 'fr_phonetic');
             INSERT INTO fr_phonetics VALUES('chat', '/ʃa/');
             INSERT INTO fr_phonetics VALUES('pomme de terre', '/pɔm də tɛʁ/');
             PRAGMA user_version = 1;",
        )
        .unwrap();
    let path = french.to_str().unwrap();
    let words = vec![
        "pomme de terre".to_owned(),
        "chien".to_owned(),
        "chat".to_owned(),
    ];
    assert_eq!(
        phonetics(path, "fr", &words).unwrap(),
        vec!["/pɔm də tɛʁ/", "", "/ʃa/"]
    );
    // 法语表被当成西语或英文表来查时拒绝，而不是照读。
    assert!(phonetics(path, "es", &words).is_err());
    assert!(english_phonetics(path, &words).is_err());
    // 表名由语言拼出，所以只接受已知的语言代码。
    assert!(phonetics(path, "fr_phonetics; --", &words).is_err());
    assert!(phonetics(path, "", &words).is_err());
}

#[cfg(unix)]
#[test]
fn english_phonetics_rejects_a_linked_database() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let external = outside.path().join("en-phonetic.db");
    pronunciation_database(&external, "en_phonetic", 1);
    let linked = directory.path().join("en-phonetic.db");
    symlink(&external, &linked).unwrap();

    assert!(english_phonetics(linked.to_str().unwrap(), &["love".to_owned()]).is_err());
}

#[cfg(any(unix, windows))]
#[test]
fn english_phonetics_rejects_a_hard_linked_database() {
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let external = outside.path().join("en-phonetic.db");
    pronunciation_database(&external, "en_phonetic", 1);
    let linked = directory.path().join("en-phonetic.db");
    std::fs::hard_link(&external, &linked).unwrap();
    // 别的用户也能写的目录里，多链接文件可能是他们放的；Windows 上多链接一律拒绝。
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
    }

    assert!(english_phonetics(linked.to_str().unwrap(), &["love".to_owned()]).is_err());
}

/// 发音库是只读的随包资源；Nix 的 store 去重把它合并成硬链接后，所在目录只有属主能写，照常查询（#6386）。
#[cfg(unix)]
#[test]
fn english_phonetics_reads_a_hard_linked_database_in_a_closed_directory() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    // 只有属主能写；临时目录的权限取决于 umask。
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let original = directory.path().join("en-phonetic.db");
    pronunciation_database(&original, "en_phonetic", 1);
    std::fs::hard_link(&original, directory.path().join("deduplicated.db")).unwrap();

    assert!(english_phonetics(original.to_str().unwrap(), &["love".to_owned()]).is_ok());
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
    Connection::open(resources.join("msime-pinyin.db"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',100);
             CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
             CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);",
        )
        .unwrap();
    Connection::open(resources.join("msime-english.db"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);
             CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY,chinese_gloss TEXT);
             CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY,english TEXT);
             INSERT INTO english_words VALUES('word','word',100);",
        )
        .unwrap();
    std::fs::copy(
        resources.join("msime-pinyin.db"),
        dictionaries.join("msime-pinyin.db"),
    )
    .unwrap();
    std::fs::copy(
        resources.join("msime-english.db"),
        dictionaries.join("msime-english.db"),
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
    Connection::open(dictionaries.join("msime-pinyin.db"))
        .unwrap()
        .execute("UPDATE tbl_2_n SET weight=1", [])
        .unwrap();

    reset_learned_data(&value).unwrap();

    let weight: i64 = Connection::open(dictionaries.join("msime-pinyin.db"))
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
    for name in ["msime-pinyin.db", "msime-english.db"] {
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
    // 日文混输是实验选项，产品默认关。
    assert!(!prepared.mixed_japanese);
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
        "wubi86",
    ] {
        value.helpcode_schema = schema.into();
        for enabled in [false, true] {
            value.helpcode = enabled;
            let mut session = Session::new(&value).unwrap();
            type_text(&mut session, b"ni");
            assert!(session.character(b'H', true).unwrap().handled);
            let snapshot = session.snapshot().unwrap();
            assert_eq!(snapshot.preedit, if enabled { "ni'H" } else { "niH" });
            assert_eq!(snapshot.editing_text, "niH");
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
    value.korean_layout = 2;
    assert_eq!(message(&value), "Unsupported Korean layout");
    value.korean_layout = 1;
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

/// 小鹤的键位，zh 换到 a，ing 换到 `;`，零声母改用 o 引导。
fn custom_table() -> crate::types::ShuangpinCustomTable {
    let pairs = |entries: &[(&str, &str)]| -> Vec<(String, String)> {
        entries
            .iter()
            .map(|(unit, key)| ((*unit).to_owned(), (*key).to_owned()))
            .collect()
    };
    crate::types::ShuangpinCustomTable {
        initials: pairs(&[("zh", "a"), ("ch", "i"), ("sh", "u")]),
        finals: pairs(&[
            ("iu", "q"),
            ("ei", "w"),
            ("e", "e"),
            ("uan", "r"),
            ("ue", "t"),
            ("ve", "t"),
            ("un", "y"),
            ("u", "u"),
            ("i", "i"),
            ("uo", "o"),
            ("o", "o"),
            ("ie", "p"),
            ("a", "a"),
            ("ong", "s"),
            ("iong", "s"),
            ("ai", "d"),
            ("en", "f"),
            ("eng", "g"),
            ("ang", "h"),
            ("an", "j"),
            ("uai", "k"),
            ("ing", ";"),
            ("uang", "l"),
            ("iang", "l"),
            ("ou", "z"),
            ("ua", "x"),
            ("ia", "x"),
            ("ao", "c"),
            ("ui", "v"),
            ("v", "v"),
            ("in", "b"),
            ("iao", "n"),
            ("ian", "m"),
        ]),
        zero_initials: pairs(&[
            ("a", "oa"),
            ("ai", "od"),
            ("an", "oj"),
            ("ang", "oh"),
            ("ao", "oc"),
            ("e", "oe"),
            ("ei", "ow"),
            ("en", "of"),
            ("eng", "og"),
            ("er", "or"),
            ("o", "oo"),
            ("ou", "oz"),
        ]),
    }
}

#[test]
fn custom_profile_runs_the_users_table() {
    let dir = tempfile::tempdir().unwrap();
    let mut options = options(dir.path());
    options.scheme = 1;
    options.shuangpin_profile = 4;
    options.shuangpin_custom_profile = Some(custom_table());
    options.shuangpin_preedit_uses_raw = false;
    let mut session = Session::new(&options).unwrap();
    type_text(&mut session, b"ahx;oa");
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.shuangpin_profile, "custom");
    // `;` 上有韵母，宿主要把它当字母键送进来。
    assert!(snapshot.microsoft_shuangpin);
    assert_eq!(snapshot.editing_text, "ahx;oa");
    assert_eq!(snapshot.segment_raw_boundaries, vec![0, 2, 4, 6]);
    assert_eq!(snapshot.preedit, "zhang'xing'a");
    // 偶数位置上的 `;` 不是一个音节的第二个键，不收。
    let mut session = Session::new(&options).unwrap();
    type_text(&mut session, b"ah;");
    assert_eq!(session.snapshot().unwrap().editing_text, "ah");
}

/// 韵母都不在 `;` 上、只有零声母编码用 `;` 做第二个键时，会话照样收 `;`，并告诉宿主把它当字母键送进来。
#[test]
fn custom_profile_accepts_semicolon_used_only_by_a_zero_initial_code() {
    let dir = tempfile::tempdir().unwrap();
    let mut table = custom_table();
    for (unit, key) in &mut table.finals {
        if unit == "ing" {
            *key = "k".to_owned();
        }
    }
    for (unit, key) in &mut table.zero_initials {
        if unit == "a" {
            *key = "o;".to_owned();
        }
    }
    let mut options = options(dir.path());
    options.scheme = 1;
    options.shuangpin_profile = 4;
    options.shuangpin_custom_profile = Some(table);
    options.shuangpin_preedit_uses_raw = false;
    let mut session = Session::new(&options).unwrap();
    type_text(&mut session, b"xko;");
    let snapshot = session.snapshot().unwrap();
    assert!(snapshot.microsoft_shuangpin);
    assert_eq!(snapshot.editing_text, "xko;");
    assert_eq!(snapshot.preedit, "xing'a");
}

#[test]
fn custom_profile_needs_a_valid_table_only_when_shuangpin_is_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let mut options = options(dir.path());
    options.shuangpin_profile = 4;
    let message = |value: &EngineOptions| Session::new(value).err().unwrap().to_string();
    assert_eq!(
        message(&options),
        "Invalid custom shuangpin profile: no table"
    );
    let mut table = custom_table();
    table.finals.retain(|(unit, _)| unit != "iong");
    options.shuangpin_custom_profile = Some(table);
    assert_eq!(
        message(&options),
        "Invalid custom shuangpin profile: finals unit \"iong\" has no key"
    );
    // 没有双拼的会话用不上这份设置，按小鹤建起来。
    options.enabled_schemes = crate::types::SchemeSet::of(&[crate::types::SchemeType::Quanpin]);
    let session = Session::new(&options).unwrap();
    assert_eq!(session.snapshot().unwrap().shuangpin_profile, "xiaohe");
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

/// 评测工具靠它把全拼用例编成双拼按键：按名字取内置方案，`custom` 和不认识的名字没有表可编。
#[test]
fn encode_shuangpin_uses_the_named_builtin_profile() {
    assert_eq!(
        encode_shuangpin("ni'hao", "xiaohe").as_deref(),
        Some("ni'hc")
    );
    assert_eq!(
        encode_shuangpin("a'ming", "microsoft").as_deref(),
        Some("oa'm;")
    );
    for profile in ["custom", "", "quanpin", "xiaohe-v2"] {
        assert_eq!(encode_shuangpin("ni'hao", profile), None, "{profile}");
    }
    assert_eq!(encode_shuangpin("ni'bad", "xiaohe"), None);
}

/// 音节表与方案无关：首道双拼编不出 `den`（`en` 和 `ia` 同在 `k` 上，解回 `dia`），`den` 仍是音节。
#[test]
fn shuangpin_syllables_do_not_depend_on_the_profile() {
    for syllable in ["de", "den", "a", "na", "zhuang", "yo"] {
        assert!(is_shuangpin_syllable(syllable), "{syllable}");
    }
    for piece in ["", "d", "zhei", "ni'hao", "xyz"] {
        assert!(!is_shuangpin_syllable(piece), "{piece}");
    }
    assert_eq!(encode_shuangpin("den", "shoudao"), None);
    assert_eq!(encode_shuangpin("den", "xiaohe").as_deref(), Some("df"));
}

/// 内置方案里只有微软双拼用 `;` 键；`custom` 和不认识的名字没有表，答不了。
#[test]
fn shuangpin_uses_semicolon_follows_the_builtin_table() {
    for (profile, expected) in [
        ("xiaohe", false),
        ("ziranma", false),
        ("shoudao", false),
        ("microsoft", true),
        ("smart_abc", false),
    ] {
        assert_eq!(
            shuangpin_uses_semicolon(profile),
            Some(expected),
            "{profile}"
        );
    }
    for profile in ["custom", "", "quanpin"] {
        assert_eq!(shuangpin_uses_semicolon(profile), None, "{profile}");
    }
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

/// 换上的本地墙钟就是日期时间模式读的那一个：评测靠它钉住行内日期时间行，同一份输入在哪天跑都打出同样的候选。
#[test]
fn a_replaced_clock_is_what_the_date_time_mode_reads() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = Session::new(&options(dir.path())).unwrap();
    session.set_clock(crate::Clock {
        steady: Box::new(std::time::Instant::now),
        local: std::sync::Arc::new(|| crate::LocalDateTime {
            year: 2026,
            month: 1,
            day: 5,
            weekday: 1,
            hour: 10,
            minute: 0,
            second: 0,
        }),
    });
    assert!(session.character(b'T', true).unwrap().handled);
    type_text(&mut session, b"sj");
    let snapshot = session.snapshot().unwrap();
    assert!(
        snapshot
            .candidates
            .iter()
            .any(|candidate| candidate == "2026-01-05 10:00:00"),
        "{:?}",
        snapshot.candidates
    );
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

/// 14 键：快照的 `nine_key` 只表示九键，`key_grid` 说出是哪种网格；组码只从 `grid_key` 进来，硬件字母照常走全拼。
#[test]
fn real_engine_reports_the_fourteen_key_grid_apart_from_nine_key() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = Session::new(&options(dir.path())).unwrap();
    assert_eq!(session.snapshot().unwrap().key_grid, None);
    session
        .set_key_grid(Some(crate::KeyGrid::FourteenKey))
        .unwrap();
    let snapshot = session.snapshot().unwrap();
    assert!(!snapshot.nine_key);
    assert_eq!(snapshot.key_grid, Some(crate::KeyGrid::FourteenKey));
    assert!(session.character(b'q', false).unwrap().handled);
    assert_eq!(session.snapshot().unwrap().editing_text, "q");
    assert!(!session.grid_key(b'n').unwrap().handled);
    session.command(Command::Cancel).unwrap();
    assert!(session.grid_key(b'n').unwrap().handled);
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.editing_text, "b");
    assert!(!snapshot.nine_key_spellings.is_empty());
    session.command(Command::Cancel).unwrap();

    session.set_nine_key_enabled(true).unwrap();
    let snapshot = session.snapshot().unwrap();
    assert!(snapshot.nine_key);
    assert_eq!(snapshot.key_grid, Some(crate::KeyGrid::NineKey));
    assert!(!session.grid_key(b'n').unwrap().handled);
    session.set_key_grid(None).unwrap();
    let snapshot = session.snapshot().unwrap();
    assert!(!snapshot.nine_key);
    assert_eq!(snapshot.key_grid, None);
}

/// 宿主拿 `shuangpin_profile` 画键面、算键盘外观的缓存键，九宫格组字途中它也得是会话跑的方案名；核心快照这时留空，门面不能照抄。
#[test]
fn snapshot_names_the_shuangpin_profile_while_a_grid_composes() {
    let dir = tempfile::tempdir().unwrap();
    let mut options = options(dir.path());
    options.scheme = 1;
    options.shuangpin_profile = crate::ShuangpinProfileKind::Ziranma as u8;
    let mut session = Session::new(&options).unwrap();
    let table =
        crate::resolve_fourteen_key_shuangpin(crate::ShuangpinProfileKind::Ziranma, None).unwrap();
    session
        .set_key_grid(Some(crate::KeyGrid::FourteenKeyShuangpin(table)))
        .unwrap();
    assert_eq!(session.snapshot().unwrap().shuangpin_profile, "ziranma");
    assert!(session.grid_key(b'n').unwrap().handled);
    let snapshot = session.snapshot().unwrap();
    assert!(snapshot.grid_composing);
    assert_eq!(snapshot.shuangpin_profile, "ziranma");
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

/// 末尾单个 `n` 在宿主画出的读音里还是字母（#6997）：只打一个 `n` 不会立刻变成ん，`nn` 退格一次能看到变化；`reading` 仍是确定时上屏的读音。
#[test]
fn japanese_display_reading_keeps_a_trailing_lone_n() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.scheme = 3;
    let mut session = Session::new(&value).unwrap();
    let readings = |session: &Session| {
        let snapshot = session.snapshot().unwrap();
        (snapshot.display_reading, snapshot.reading)
    };
    assert!(session.character(b'n', false).unwrap().handled);
    assert_eq!(readings(&session), ("n".to_owned(), "ん".to_owned()));
    assert!(session.character(b'n', false).unwrap().handled);
    assert_eq!(readings(&session), ("ん".to_owned(), "ん".to_owned()));
    assert!(session.command(Command::Backspace).unwrap().handled);
    assert_eq!(readings(&session), ("n".to_owned(), "ん".to_owned()));
    assert!(session.command(Command::Backspace).unwrap().handled);
    assert_eq!(readings(&session), (String::new(), String::new()));
    type_text(&mut session, b"kan");
    assert_eq!(readings(&session), ("かn".to_owned(), "かん".to_owned()));
    let committed = session.command(Command::CommitReading).unwrap();
    assert!(committed.has_commit);
    assert_eq!(committed.commit, "かん");

    // 其他方案画的读音不变。
    let mut value = options(dir.path());
    value.scheme = 4;
    let mut korean = Session::new(&value).unwrap();
    type_text(&mut korean, b"gks");
    let snapshot = korean.snapshot().unwrap();
    assert!(!snapshot.reading.is_empty());
    assert_eq!(snapshot.display_reading, snapshot.reading);
}

/// 录制器的单词条日文模型（读音 かな，词 甲），与 golden 场景 `ri_japanese_model_a` 同一份字节。
const JAPANESE_MODEL_KANA: &str = "MSJPDT1\u{0}\u{1}\u{0}\u{0}\u{0}\u{1}\u{0}\u{0}\u{0}\u{1}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}8\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}L\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}N\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\t\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{6}\u{0}\u{6}\u{0}\u{0}\u{0}\u{3}\u{0}\u{0}\u{0}\u{0}\u{0}\u{1}\u{0}\u{0}\u{0}\u{0}\u{0}かな甲";

fn japanese_candidates(value: &EngineOptions) -> Vec<String> {
    let mut session = Session::new(value).unwrap();
    type_text(&mut session, b"kana");
    session.snapshot().unwrap().candidates
}

/// `japanese_dictionary` 指向资源目录之外的模型时读那一份；为空时仍读资源目录里的 `msime-japanese.dat`，两处都没有就只给假名行。
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
    assert!(!Path::new(&value.dictionaries)
        .join("msime-english.db")
        .exists());

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
    let error = Session::new(&value).err().expect("no msime-cantonese.db");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
    );
    value.cantonese_dictionary = dir.path().join("missing.db").to_str().unwrap().to_owned();
    let error = Session::new(&value)
        .err()
        .expect("missing msime-cantonese.db");
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
    value.scheme = 10;
    let error = Session::new(&value).err().expect("scheme ten");
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
    let error = Session::new(&value).err().expect("no msime-zhuyin.db");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
    );
    value.zhuyin_dictionary = dir.path().join("missing.db").to_str().unwrap().to_owned();
    let error = Session::new(&value).err().expect("missing msime-zhuyin.db");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
    );
    assert!(!dir.path().join("missing.db").exists());
}

#[test]
fn stroke_is_unavailable_without_its_dictionary() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.scheme = 9;
    let error = Session::new(&value).err().expect("no msime-stroke.db");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
    );
    value.stroke_dictionary = dir.path().join("missing.db").to_str().unwrap().to_owned();
    let error = Session::new(&value).err().expect("missing msime-stroke.db");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
    );
    assert!(!dir.path().join("missing.db").exists());
}

// 笔画的快照：`preedit` 与 `reading` 是笔画字形，`editing_text` 与光标是键入的 ASCII 字母；Enter 上屏字母串，两条 CommitRaw 都不学习。
#[test]
fn stroke_snapshot_draws_glyphs_and_edits_letters() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("msime-stroke.db");
    crate::stroke::fixture::build(&path);
    let mut value = options(dir.path());
    value.scheme = 9;
    value.stroke_dictionary = path.to_str().unwrap().to_owned();
    let mut session = Session::new(&value).unwrap();
    type_text(&mut session, b"hsx");
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.scheme, 9);
    assert_eq!(snapshot.preedit, "一丨＊");
    assert_eq!(snapshot.reading, "一丨＊");
    assert_eq!(snapshot.editing_text, "hsx");
    assert_eq!(snapshot.caret_position, 3);
    assert!(snapshot.segment_raw_boundaries.is_empty());
    assert!(!snapshot.candidate_list_open);
    assert_eq!(snapshot.candidates, ["土"]);
    assert_eq!(snapshot.candidate_codes, ["hsx"]);

    let result = session.command(Command::CommitRaw).unwrap();
    assert_eq!((result.handled, result.commit.as_str()), (true, "hsx"));
    type_text(&mut session, b"pn");
    let result = session.command(Command::CommitRawWithoutLearning).unwrap();
    assert_eq!((result.handled, result.commit.as_str()), (true, "pn"));
    assert!(session.snapshot().unwrap().reading.is_empty());
}

#[test]
fn zhuyin_command_sixteen_opens_a_list_whose_selection_commits_nothing() {
    use crate::language_dictionary::{FORMAT_VERSION, METADATA_FORMAT_VERSION, SCHEMA};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("msime-zhuyin.db");
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

// 注音九键经宿主接口开启：数字拼音节，`nine_key_spellings` 给出候选读音，越界下标 unhandled，合法下标钉读音。
#[test]
fn zhuyin_nine_key_exposes_reading_choices() {
    use crate::language_dictionary::{FORMAT_VERSION, METADATA_FORMAT_VERSION, SCHEMA};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("msime-zhuyin.db");
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
            "INSERT INTO syllables VALUES ('ㄋㄧˇ'),('ㄌㄧˇ');\
             INSERT INTO entries VALUES ('ㄋㄧˇ','你',1000),('ㄌㄧˇ','李',1200);",
        )
        .unwrap();
    drop(connection);
    let mut value = options(dir.path());
    value.scheme = 6;
    value.zhuyin_dictionary = path.to_str().unwrap().to_owned();
    let mut session = Session::new(&value).unwrap();
    session.set_nine_key_enabled(true).unwrap();
    assert!(session.snapshot().unwrap().nine_key);
    type_text(&mut session, b"28c");
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.reading, "李");
    assert_eq!(snapshot.editing_text, "28c");
    assert_eq!(snapshot.nine_key_spellings, ["ㄌㄧˇ", "ㄋㄧˇ"]);
    assert!(
        !session
            .choose_nine_key_spelling(snapshot.nine_key_spellings.len())
            .unwrap()
            .handled
    );
    assert!(session.choose_nine_key_spelling(1).unwrap().handled);
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.reading, "你");
    assert!(snapshot.nine_key_spellings.is_empty());
    let result = session.command(Command::CommitRaw).unwrap();
    assert_eq!((result.handled, result.commit.as_str()), (true, "你"));
    session.set_nine_key_enabled(false).unwrap();
    assert!(!session.snapshot().unwrap().nine_key);
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
    let learned: String = Connection::open(Path::new(&value.dictionaries).join("msime-english.db"))
        .unwrap()
        .query_row(
            "SELECT display FROM english_words WHERE word='hello'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(learned, "hello");
}

// 随包的 msime-english.db 按 Google unigram 计数给权重，拼音词库用的是自己的尺度，英文权重说明不了中文权重的高低（#1040、#1063 两次按权重提英文都撤回了）。所以输入能切成完整音节时（这里的 `ni`），混输从不让英文领先拼音的首选：不凭随包权重，不因为上屏过，也不因为置顶过；置顶只在英文词之间调顺序。英文只在拼音拆不开、英文完全匹配整段输入，或者用户把它固定在第一位时才领先，那条规则不比较权重，见 `CandidateQueries::mixed`。
#[test]
fn mixed_english_never_outranks_chinese_on_complete_pinyin() {
    const ENGLISH: u8 = 4;
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.learning = true;
    value.english_minimum_prefix = 2;
    for directory in [&value.resources, &value.dictionaries] {
        let directory = Path::new(directory);
        Connection::open(directory.join("msime-pinyin.db"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_1_n VALUES('ni','n','你',2000);
                 INSERT INTO tbl_1_n VALUES('ni','n','倪',1000);
                 CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                 CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);",
            )
            .unwrap();
        Connection::open(directory.join("msime-english.db"))
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
    let database = Path::new(&value.dictionaries).join("msime-english.db");
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
    let database = Path::new(&value.dictionaries).join("msime-english.db");
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

/// 九宫格组字时回车上屏的不是用户逐个打出的字母：九键是数字，14 键是结束组字后的首选（没有候选时是组码 `bugao`），都不学成英文词，也不报学不进去。
#[test]
fn grid_raw_commit_is_not_learned_as_an_english_word() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let mut session = Session::new(&value).unwrap();
    session.set_nine_key_enabled(true).unwrap();
    type_text(&mut session, b"64426");
    let result = session.command(Command::CommitRaw).unwrap();
    assert_eq!(result.commit, "64426");
    assert_eq!(result.diagnostic, "");

    session
        .set_key_grid(Some(crate::KeyGrid::FourteenKey))
        .unwrap();
    for letter in b"nihao" {
        assert!(session.grid_key(*letter).unwrap().handled);
    }
    let result = session.command(Command::CommitRaw).unwrap();
    assert!(result.has_commit);
    assert_eq!(result.diagnostic, "");
    assert!(session.snapshot().unwrap().editing_text.is_empty());
    assert_eq!(english_word_count(&value, &result.commit), 0);
    assert_eq!(english_word_count(&value, "bugao"), 0);
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
    // 回车上屏的是所见预编辑：临时英文的入口字母 `Y` 算文本（#7616）；进词库的词仍不带它。
    assert_eq!(result.commit, "Yrustacean");
    assert_eq!(result.diagnostic, "");
    assert_eq!(english_word_count(&value, "rustacean"), 1);
}

/// 网址不是英文单词：网址模式下 Enter 上屏整段网址，不写进英文词库。五笔 `http` 加 `s` 进入网址模式，预编辑 `https` 全是字母，没有排除时会被当作英文单词学进词库。
#[test]
fn url_enter_is_not_learned_as_english() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.scheme = 2;
    let mut session = Session::new(&value).unwrap();
    type_text(&mut session, b"https");
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.local_mode, "url");
    assert_eq!(snapshot.preedit, "https");
    let result = session.command(Command::CommitRaw).unwrap();
    assert!(result.has_commit);
    assert_eq!(result.commit, "https");
    assert_eq!(result.diagnostic, "");
    assert_eq!(english_word_count(&value, "https"), 0);
}

/// 网址模式下 Esc 丢弃网址，不上屏，回到没有本地模式的状态。
#[test]
fn url_escape_discards_without_commit() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let mut session = Session::new(&value).unwrap();
    type_text(&mut session, b"www");
    assert!(session.punctuation(b'.').unwrap().handled);
    type_text(&mut session, b"a");
    assert_eq!(session.snapshot().unwrap().local_mode, "url");
    let result = session.command(Command::Cancel).unwrap();
    assert!(result.handled);
    assert!(!result.has_commit, "{:?}", result.commit);
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.local_mode, "none");
    assert!(snapshot.preedit.is_empty());
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
    // 回车上屏所见预编辑，入口字母 `R` 算文本（#7616）；词库里学的仍是不带标记、带 `r` 前缀的词（bridge.cpp:1349-1357）。
    assert_eq!(result.commit, "Rkk");
    assert_eq!(result.diagnostic, "");
    assert_eq!(english_word_count(&value, "rkk"), 1);
    assert_eq!(english_word_count(&value, "kk"), 0);
}

/// Windows `input.enter_learns_english_word` 出厂为开：回车上屏输入的英文词写进英文词库；
/// 关掉后回车照常上屏，只是不学词。
#[test]
fn enter_learns_english_word_follows_the_preference() {
    for enabled in [false, true] {
        for (route, typed, committed, learned) in [
            ("dedicated", "hello", "hello", "hello"),
            ("incomplete_pinyin", "xyz", "xyz", "xyz"),
            ("temporary_english", "Yrustacean", "Yrustacean", "rustacean"),
            ("temporary_japanese", "Rkk", "Rkk", "rkk"),
            ("complete_pinyin", "ni", "ni", "ni"),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut value = options(dir.path());
            value.learn_english_on_enter = enabled;
            let mut session = Session::new(&value).unwrap();
            session.set_dedicated_english(route == "dedicated").unwrap();
            for byte in typed.bytes() {
                assert!(
                    session
                        .character(byte, byte.is_ascii_uppercase())
                        .unwrap()
                        .handled
                );
            }
            let result = session.command(Command::CommitRaw).unwrap();
            assert!(result.handled && result.has_commit, "{route}");
            assert_eq!(result.commit, committed, "{route}");
            assert_eq!(result.diagnostic, "", "{route}");
            assert_eq!(
                english_word_count(&value, learned),
                i64::from(enabled && route != "complete_pinyin"),
                "{route}"
            );
        }
    }
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
    value.single_character_only = true;
    value.learning = true;
    value.autocorrect_transposition = false;
    value.autocorrect_neighbor = true;
    value.fuzzy_pinyin_rules = u32::MAX;
    value.wubi_mixed_pinyin = true;
    value.helpcode = true;
    value.strict_quanpin_helpcode = true;
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
    value.mixed_symbols = true;
    value.mixed_japanese = true;
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
    assert!(mapped.single_character_only);
    assert!(mapped.learning);
    assert_eq!(mapped.autocorrect_types, crate::autocorrect_type::NEIGHBOR);
    assert_eq!(mapped.fuzzy_pinyin.rules, crate::fuzzy_rule::ALL);
    assert!(mapped.wubi.mixed_pinyin);
    assert!(mapped.helpcode);
    assert!(mapped.strict_quanpin_helpcode);
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
    assert!(mapped.expressive.symbol_candidates);
    assert!(mapped.japanese_mixing);
    assert!(!mapped.local_modes.date_time && !mapped.local_modes.temporary_japanese);
    assert!(mapped.local_modes.unicode && mapped.local_modes.temporary_english);
    assert!(mapped.sentence_association.neural_keyboard);
    assert_eq!(mapped.rescoring_context, "上文");
    assert!(!mapped.sentence_alternatives);
    assert!(mapped.personal_context);
    assert_eq!(mapped.enabled_schemes, crate::SchemeSet::ALL);
}

/// 引擎库只有一套默认值（`SessionOptions::new`：辅助码关、表名 `wubi86`、学习开），门面不从它继承：宿主给的辅助码、学习开关原样交给会话，个人上下文按产品一直的取值打开。
#[test]
fn session_options_do_not_inherit_the_library_defaults() {
    let library = crate::SessionOptions::new(crate::RuntimePaths::default());
    assert!(!library.helpcode);
    assert_eq!(library.helpcode_schema, "wubi86");
    assert!(library.learning && library.personal_context);

    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.helpcode = true;
    value.helpcode_schema = "ziranma".into();
    value.learning = false;
    let mapped = super::options::session_options(&value).unwrap();
    assert!(mapped.helpcode);
    assert_eq!(mapped.helpcode_schema, "ziranma");
    assert!(!mapped.learning);
    assert!(mapped.personal_context);
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

#[cfg(unix)]
#[test]
fn translation_sidecar_publication_keeps_open_readers_on_complete_old_content() {
    use std::io::Read;

    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let target = Path::new(&value.dictionaries).join("custom_translations.txt");
    let resource = Path::new(&value.resources).join("custom_translations.txt");
    let user = Path::new(&value.user_data).join("custom_translations.txt");
    std::fs::write(&resource, b"fixture\told\n").unwrap();
    super::options::prepare_translation_sidecar(&value).unwrap();
    let mut previous = std::fs::File::open(&target).unwrap();

    std::fs::write(&user, b"fixture\tnew\n").unwrap();
    super::options::prepare_translation_sidecar(&value).unwrap();

    let mut old_contents = String::new();
    previous.read_to_string(&mut old_contents).unwrap();
    assert_eq!(old_contents, "fixture\told\n");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "fixture\tnew\n");
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
fn translation_sidecar_writer_does_not_follow_a_replaced_target() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = dir.path().join("custom_translations.txt");
    let external = outside.path().join("target.txt");
    std::fs::write(&external, "keep\n").unwrap();
    symlink(&external, &target).unwrap();

    assert!(super::options::write_private_file_no_follow(&target, b"overwrite\n").is_err());
    assert_eq!(std::fs::read_to_string(external).unwrap(), "keep\n");
}

#[cfg(any(unix, windows))]
#[test]
fn translation_sidecar_writer_rejects_a_hard_linked_target() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = dir.path().join("custom_translations.txt");
    let external = outside.path().join("target.txt");
    std::fs::write(&external, "keep\n").unwrap();
    std::fs::hard_link(&external, &target).unwrap();

    assert!(super::options::write_private_file_no_follow(&target, b"overwrite\n").is_err());
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
    Connection::open(dir.path().join("msime-english.db"))
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
    // 日文混输行的汉字是日文，不当中文查释义。
    assert_eq!(candidate_gloss_key("私", 13), None);
    // 脚本插件的行不交给任何释义来源，汉字和英文都一样。
    assert_eq!(candidate_gloss_key("天气", 14), None);
    assert_eq!(candidate_gloss_key("hello", 14), None);
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
    Connection::open(dir.path().join("msime-others.db"))
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
    Connection::open(Path::new(&value.dictionaries).join("msime-pinyin.db"))
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
pub(super) fn helpcode_fixture(root: &Path, extra_sql: &str, helpcodes: &str) -> EngineOptions {
    let mut options = options(root);
    for directory in [&options.resources, &options.dictionaries] {
        let directory = Path::new(directory);
        Connection::open(directory.join("msime-pinyin.db"))
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
        crate::ensure_english_schema(&directory.join("msime-english.db")).unwrap();
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
fn missing_builtin_helpcode_uses_host_fallback_for_annotations_and_filtering() {
    let root = tempfile::tempdir().unwrap();
    let mut options = helpcode_fixture(root.path(), "", "");
    std::fs::remove_file(
        Path::new(&options.resources).join("helpcodes/zrm_helpcode_big_unique.txt"),
    )
    .unwrap();
    let fallback = root.path().join("bundle");
    std::fs::create_dir_all(fallback.join("helpcodes")).unwrap();
    for (file, table) in [
        ("zrm_helpcode_big_unique.txt", "你=ab\n好=cd\n拟=ef\n"),
        ("xiaohe_helpcode.txt", "你=gh\n好=ij\n拟=kl\n"),
    ] {
        std::fs::write(fallback.join("helpcodes").join(file), table).unwrap();
    }
    options.helpcode_fallback_resources = Some(fallback.to_str().unwrap().to_owned());
    let mut session = Session::new(&options).unwrap();
    type_text(&mut session, b"nihao");
    assert!(session
        .snapshot()
        .unwrap()
        .candidate_annotations
        .iter()
        .any(|a| a.to_ascii_uppercase().contains("AC")));
    session.character(b'E', true).unwrap();
    let view = session.snapshot().unwrap();
    assert_eq!(view.candidates[0], "拟好");

    session.command(Command::Cancel).unwrap();
    session
        .set_helpcode_table(Some(std::sync::Arc::new(
            crate::helpcode::HelpcodeKeymap::default(),
        )))
        .unwrap();
    session.set_helpcode_table(None).unwrap();
    type_text(&mut session, b"nihao");
    session.character(b'E', true).unwrap();
    assert_eq!(session.snapshot().unwrap().candidates[0], "拟好");

    // 重建和方案切换均保留宿主回退目录。
    options.helpcode_schema = "xiaohe".into();
    let mut session = Session::new(&options).unwrap();
    type_text(&mut session, b"nihao");
    assert!(session
        .snapshot()
        .unwrap()
        .candidate_annotations
        .iter()
        .any(|a| a.to_ascii_uppercase().contains("GI")));
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
fn wubi_reverse_codes_are_shown_only_in_the_wubi_scheme() {
    let extra = "INSERT INTO wubi86 VALUES('wqvb','你好',300);";

    let quanpin_root = tempfile::tempdir().unwrap();
    let mut quanpin = helpcode_fixture(quanpin_root.path(), extra, "");
    quanpin.show_helpcode = false;
    let mut session = Session::new(&quanpin).unwrap();
    type_text(&mut session, b"nihao");
    let view = session.snapshot().unwrap();
    let index = view
        .candidates
        .iter()
        .position(|word| word == "你好")
        .unwrap();
    // 全拼方案不附五笔反查码。
    assert!(!view.candidate_annotations[index].contains("wqvb"));

    let mixed_root = tempfile::tempdir().unwrap();
    let mut mixed = helpcode_fixture(mixed_root.path(), extra, "");
    mixed.scheme = SchemeType::Wubi as u8;
    mixed.wubi_mixed_pinyin = true;
    let mut session = Session::new(&mixed).unwrap();
    type_text(&mut session, b"nihao");
    let view = session.snapshot().unwrap();
    let index = view
        .candidates
        .iter()
        .position(|word| word == "你好")
        .unwrap();
    assert!(view.candidate_annotations[index].contains("wqvb"));

    let native_root = tempfile::tempdir().unwrap();
    let mut native = helpcode_fixture(native_root.path(), extra, "");
    native.scheme = SchemeType::Wubi as u8;
    let mut session = Session::new(&native).unwrap();
    type_text(&mut session, b"wq");
    let view = session.snapshot().unwrap();
    let index = view
        .candidates
        .iter()
        .position(|word| word == "你好")
        .unwrap();
    assert!(view.candidate_annotations[index].contains("wqvb"));
}

#[test]
fn wubi_reverse_codes_follow_the_selected_profile() {
    let root = tempfile::tempdir().unwrap();
    let mut options = helpcode_fixture(
        root.path(),
        "CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER);
         INSERT INTO wubi86 VALUES('wqvb','你好',300);
         INSERT INTO wubi98 VALUES('abcd','你好',300);",
        "",
    );
    options.wubi_profile = 1;
    // 反查只在五笔方案里显示，所以用五笔混输拼音来看拼音候选按所选版本反查。
    options.scheme = SchemeType::Wubi as u8;
    options.wubi_mixed_pinyin = true;
    let mut session = Session::new(&options).unwrap();
    type_text(&mut session, b"nihao");
    let view = session.snapshot().unwrap();
    let index = view
        .candidates
        .iter()
        .position(|word| word == "你好")
        .unwrap();
    assert!(view.candidate_annotations[index].contains("abcd"));
    assert!(!view.candidate_annotations[index].contains("wqvb"));
}

#[test]
fn wubi_reverse_codes_are_looked_up_only_in_the_wubi_scheme() {
    // 宿主只在五笔方案里显示反查编码，所以全拼方案的刷新连查都不查；五笔混输拼音照查。
    let root = tempfile::tempdir().unwrap();
    let mut options = helpcode_fixture(
        root.path(),
        "INSERT INTO wubi86 VALUES('wqvb','你好',300);",
        "",
    );
    options.show_helpcode = false;
    for (scheme, mixed, expected) in [
        (SchemeType::Quanpin, false, None),
        (SchemeType::Wubi, true, Some("wqvb")),
    ] {
        options.scheme = scheme as u8;
        options.wubi_mixed_pinyin = mixed;
        let mut session =
            crate::session::Session::new(super::options::session_options(&options).unwrap())
                .unwrap();
        for byte in b"nihao" {
            session.character(*byte, false);
        }
        assert!(session
            .snapshot()
            .candidates
            .iter()
            .any(|item| item.word == "你好"));
        assert_eq!(session.candidate_wubi_code("你好"), expected, "{scheme:?}");
    }
}

#[test]
fn wubi_reverse_code_fast_path_requires_the_display_word_identity() {
    let root = tempfile::tempdir().unwrap();
    let mut options = helpcode_fixture(
        root.path(),
        "INSERT INTO wubi86 VALUES('wqvb','你好',300),('wqbb','父子',200);",
        "",
    );
    options.scheme = SchemeType::Wubi as u8;
    options.show_helpcode = false;
    let mut session = Session::new(&options).unwrap();
    type_text(&mut session, b"wq");
    let snapshot = session.snapshot().unwrap();
    assert!(!snapshot.candidates.is_empty());
    for (index, candidate) in snapshot.candidates.iter().enumerate() {
        assert_eq!(
            session.candidate_wubi_code_for_display(index, candidate),
            snapshot
                .candidate_annotations
                .get(index)
                .and_then(|annotation| annotation.split_whitespace().next()),
            "同座候选应复用已有反查码"
        );
        assert!(
            session
                .candidate_wubi_code_for_display(index, "另一个词")
                .is_none(),
            "词面不一致时不得读取该座位的反查码"
        );
    }
    if snapshot.candidates.len() > 1 {
        let other = &snapshot.candidates[1];
        assert_eq!(
            session.candidate_wubi_code_for_display(0, other),
            snapshot.candidate_annotations[1].split_whitespace().next(),
            "座位错位时应回退到按词查找"
        );
    }
}

#[test]
fn wubi_reverse_code_fast_path_requires_full_candidate_identity() {
    let root = tempfile::tempdir().unwrap();
    let mut options = helpcode_fixture(
        root.path(),
        "INSERT INTO wubi86 VALUES('wqvb','你好',300);",
        "",
    );
    options.scheme = SchemeType::Wubi as u8;
    options.show_helpcode = false;
    let mut session = Session::new(&options).unwrap();
    type_text(&mut session, b"wq");
    let snapshot = session.snapshot().unwrap();
    let index = snapshot
        .candidates
        .iter()
        .position(|candidate| candidate == "你好")
        .unwrap();
    let different_identity = WordItem::new(
        "different",
        "你好",
        0,
        CandidateSource::Database,
        "different",
    );

    assert_eq!(
        session.candidate_wubi_code_for_display_item(index, &different_identity),
        None,
        "同词面但候选身份不同，不得复用该座位的反查码"
    );
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
        Connection::open(Path::new(directory).join("msime-pinyin.db"))
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
        let main = Connection::open(Path::new(directory).join("msime-pinyin.db")).unwrap();
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
        Connection::open(Path::new(directory).join("msime-pinyin.db"))
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

/// local_database.cpp:34-38,63-66 opened msime's generation dictionary once per local-mode query, so nothing held it after the sessions were gone. A reset or snapshot restore replaces `msime-pinyin.db` at the same path once every session is dropped (Windows needs the handle closed to rename it), and the next session must read the new file.
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
    let main = Path::new(&value.dictionaries).join("msime-pinyin.db");
    dictionary(
        &Path::new(&value.resources).join("msime-pinyin.db"),
        "旧短语",
    );
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
    use crate::user_dictionary::journal::{cache_generation, thread_holds_journal};
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.learning = true;
    for directory in [&value.resources, &value.dictionaries] {
        Connection::open(Path::new(directory).join("msime-pinyin.db"))
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
    let generation = cache_generation();
    type_text(&mut session, b"nihao");
    let snapshot = session.snapshot().unwrap();
    let index = snapshot
        .candidates
        .iter()
        .position(|word| word == "拟好")
        .unwrap();
    session.select(index).unwrap();
    // Another test closing every journal meanwhile takes the parked connection away.
    assert!(thread_holds_journal() || cache_generation() != generation);
    drop(session);
    assert!(!thread_holds_journal());
}

/// Hosts keep their sessions in a thread-local map, so a session can be dropped while its thread exits, after that thread's cached journal is already destroyed. Its drop still flushes and releases without touching the destroyed cache; it used to abort the process (`cannot access a Thread Local Storage value during or after destruction`).
#[test]
fn a_session_dropped_while_its_thread_exits_does_not_abort() {
    use crate::user_dictionary::journal::{cache_generation, thread_holds_journal};
    thread_local! {
        static HELD: std::cell::RefCell<Option<Session>> = const { std::cell::RefCell::new(None) };
    }
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.learning = true;
    for directory in [&value.resources, &value.dictionaries] {
        Connection::open(Path::new(directory).join("msime-pinyin.db"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',200),('ni''hao','nh','拟好',100);",
            )
            .unwrap();
    }
    let journal = Path::new(&value.user_data).join("msime_user.db");
    std::thread::spawn(move || {
        let generation = cache_generation();
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
        // Another test closing every journal meanwhile takes the parked connection away.
        assert!(thread_holds_journal() || cache_generation() != generation);
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

pub(super) fn generated_mode_options(root: &Path) -> EngineOptions {
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
        phone: String::new(),
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
    for name in ["msime-pinyin.db", "msime-english.db"] {
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
            phone: String::new(),
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
    // 内置的农历命令 `yinli` 也以 y 开头，排在指令表之后；它的文字随真实时钟变，只核对触发词。
    let view = session.snapshot().unwrap();
    assert_eq!(view.candidates[0], "哟");
    assert_eq!(view.candidate_codes, ["yo", "yinli"]);
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
fn chaizi_rows_show_readings_and_never_a_helpcode() {
    let root = tempfile::tempdir().unwrap();
    // 𢰩（才 子）在 Unihan 里没有读音，𢱆 读 zī；辅助码表给了两者编码，拆字行也不能拿辅助码补上注释或替换读音。
    let mut options = helpcode_fixture(root.path(), "", "𢰩=ab\n𢱆=ab\n焱=cd\n");
    assert!(!options.local_chaizi);
    options.local_chaizi = true;
    let mut session = Session::new(&options).unwrap();
    assert!(session.character(b'C', true).unwrap().handled);
    type_text(&mut session, b"caizi");
    let view = session.snapshot().unwrap();
    assert_eq!(view.local_mode, "chaizi");
    assert_eq!(view.candidates[..2], ["𢰩".to_owned(), "𢱆".to_owned()]);
    assert_eq!(
        view.candidate_annotations[..2],
        ["".to_owned(), "zī".to_owned()]
    );
    session.command(Command::Cancel).unwrap();
    session.character(b'C', true).unwrap();
    type_text(&mut session, b"huohuohuo");
    let view = session.snapshot().unwrap();
    assert_eq!(view.candidates[0], "焱");
    assert_eq!(view.candidate_annotations[0], "yàn");
}

#[test]
fn chaizi_options_map_into_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    let off = super::options::session_options(&value).unwrap();
    assert!(!off.local_modes.chaizi && !off.chaizi_traditional_output);
    value.local_chaizi = true;
    value.chaizi_traditional_output = true;
    let on = super::options::session_options(&value).unwrap();
    assert!(on.local_modes.chaizi && on.chaizi_traditional_output);
}

#[test]
fn only_generated_modes_are_left_out_of_typing_statistics() {
    for mode in ["expression", "command", "mention"] {
        assert!(!local_mode_counts_as_typing(mode), "{mode}");
    }
    // 拆字上屏的字是用户挑出来的，和 E、J 一样计入打字统计。
    for mode in [
        "none",
        "unicode",
        "date_time",
        "temporary_english",
        "chaizi",
        "unknown",
        "",
    ] {
        assert!(local_mode_counts_as_typing(mode), "{mode}");
    }
}

/// 只有日文、越南文或藏文的方案集合不读 msime-pinyin.db：资源目录里只有 `msime-english.db` 也能准备代次、建会话、打字，代次里始终没有 msime-pinyin.db。个人词库只收英文词，拼音和快捷短语直接说明没有中文词库；重置和快照导入都只换回、回放英文词库。
#[test]
fn schemes_without_the_main_dictionary_run_on_english_alone() {
    let root = tempfile::tempdir().unwrap();
    let resources = root.path().join("resources");
    std::fs::create_dir_all(&resources).unwrap();
    Connection::open(resources.join("msime-english.db"))
        .unwrap()
        .execute_batch(&format!(
            "{ENGLISH_SCHEMA} INSERT INTO english_words VALUES('word','word',100);"
        ))
        .unwrap();
    for (content_id, scheme, code, input, preedit) in [
        ("japanese", SchemeType::JapaneseRomaji, 3, &b"ka"[..], ""),
        (
            "vietnamese",
            SchemeType::Vietnamese,
            7,
            &b"tieengs"[..],
            "tiếng",
        ),
        ("tibetan", SchemeType::Tibetan, 8, &b"bod"[..], "བོད"),
    ] {
        let set = crate::types::SchemeSet::of(&[scheme]);
        let user = root.path().join(content_id).join("user");
        let mut prepared = prepare_options_for(
            resources.to_str().unwrap(),
            user.to_str().unwrap(),
            root.path().join(content_id).join("cache").to_str().unwrap(),
            content_id,
            set,
        )
        .unwrap();
        assert_eq!(prepared.enabled_schemes, set);
        let dictionaries = Path::new(&prepared.dictionaries).to_owned();
        assert!(dictionaries.join("msime-english.db").is_file());
        assert!(!dictionaries.join("msime-pinyin.db").exists());
        // 集合外的方案（缺省的全拼）建不了会话；本版本的方案照常。
        assert!(Session::new(&prepared).is_err());
        prepared.scheme = code;
        let mut session = Session::new(&prepared).unwrap();
        type_text(&mut session, input);
        let snapshot = session.snapshot().unwrap();
        assert_eq!(snapshot.scheme, code, "{content_id}");
        if scheme == SchemeType::JapaneseRomaji {
            assert_eq!(snapshot.reading, "か");
        } else {
            assert_eq!(snapshot.preedit, preedit, "{content_id}");
        }
        drop(session);
        assert!(!dictionaries.join("msime-pinyin.db").exists());

        let entry = |kind, key: &str, value: &str| DictionaryEntry {
            kind,
            key: key.into(),
            value: value.into(),
            weight: 10,
        };
        dictionary_edit(
            &prepared,
            None,
            Some(&entry(DictionaryKind::English, "zzfixture", "Zzfixture")),
            "add-english",
        )
        .unwrap();
        for kind in [DictionaryKind::Pinyin, DictionaryKind::QuickPhrase] {
            let key = if kind == DictionaryKind::Pinyin {
                "ni'hao"
            } else {
                "dh"
            };
            assert_eq!(
                dictionary_edit(&prepared, None, Some(&entry(kind, key, "你好")), "")
                    .unwrap_err()
                    .to_string(),
                "This input method has no Chinese dictionary; only English words can be edited"
            );
        }
        let listed = dictionary_entries(&prepared, 0, 10).unwrap();
        assert_eq!(listed.entries.len(), 1);
        assert_eq!(listed.entries[0].kind, DictionaryKind::English);
        assert!(
            dictionary_table_entries(&prepared, DictionaryKind::Pinyin, "nihao", 0, 10)
                .unwrap()
                .entries
                .is_empty()
        );

        // 快照导入：拼音行收进日志但不回放，英文行回放进新代次的英文词库。
        let generation = root.path().join(content_id).join("snapshot");
        let staged = stage_dictionary_state(
            &prepared,
            generation.to_str().unwrap(),
            "restored",
            10,
            [
                DictionaryStateRecord::Entry {
                    kind: DictionaryKind::Pinyin,
                    key: "ni'hao".into(),
                    value: "你好".into(),
                    weight: 50,
                    display: String::new(),
                    deleted: false,
                    user_inserted: true,
                },
                DictionaryStateRecord::Entry {
                    kind: DictionaryKind::English,
                    key: "zzrestored".into(),
                    value: "Zzrestored".into(),
                    weight: 60,
                    display: "Zzrestored".into(),
                    deleted: false,
                    user_inserted: true,
                },
            ]
            .into_iter()
            .map(Ok),
        )
        .unwrap();
        let staged_dictionaries = Path::new(&staged.dictionaries);
        assert!(!staged_dictionaries.join("msime-pinyin.db").exists());
        let restored: i64 = Connection::open(staged_dictionaries.join("msime-english.db"))
            .unwrap()
            .query_row(
                "SELECT weight FROM english_words WHERE word='zzrestored' AND display='Zzrestored'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(restored, 60);
        let listed = dictionary_entries(&staged, 0, 10).unwrap();
        assert_eq!(
            listed
                .entries
                .iter()
                .map(|entry| entry.key.as_str())
                .collect::<Vec<_>>(),
            ["zzrestored"]
        );

        reset_learned_data(&prepared).unwrap();
        assert!(dictionary_entries(&prepared, 0, 10)
            .unwrap()
            .entries
            .is_empty());
        assert!(!dictionaries.join("msime-pinyin.db").exists());
    }
}

/// `z` 临时拼音反查的码表：全拼有词组和单字，五笔 86 与 98 各有编码。
const PINYIN_LOOKUP_SQL: &str = "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
     INSERT INTO tbl_1_n VALUES('ni','n','你',100);
     CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
     INSERT INTO tbl_1_h VALUES('hao','h','好',100);
     INSERT INTO wubi86 VALUES('wqvb','你好',300),('wq','你',200),('wqiy','你',100),('vbg','好',100);
     CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER);
     INSERT INTO wubi98 VALUES('abcd','你好',300);";

fn pinyin_lookup_options(root: &Path) -> EngineOptions {
    let mut options = helpcode_fixture(root, PINYIN_LOOKUP_SQL, "");
    options.scheme = SchemeType::Wubi as u8;
    options.wubi_mixed_pinyin = false;
    options.show_helpcode = false;
    options
}

fn annotation_of(view: &EngineSnapshot, word: &str) -> String {
    let index = view
        .candidates
        .iter()
        .position(|candidate| candidate == word)
        .unwrap_or_else(|| panic!("{word} missing from {:?}", view.candidates));
    view.candidate_annotations[index].clone()
}

#[test]
fn z_opens_a_pinyin_lookup_annotated_with_wubi_codes() {
    let root = tempfile::tempdir().unwrap();
    let options = pinyin_lookup_options(root.path());
    let mut session = Session::new(&options).unwrap();
    for character in b"znihao" {
        assert!(session.character(*character, false).unwrap().handled);
    }
    let view = session.snapshot().unwrap();
    assert_eq!(view.preedit, "znihao");
    assert_eq!(view.local_mode, "none");
    // 反查只出拼音候选，不查五笔码表。
    assert!(view.answered_by_pinyin_fallback);
    assert!(!view.wubi_unique_four_code);
    assert!(annotation_of(&view, "你好").contains("wqvb"));
    // 单字显示全码，不是简码。
    let single = annotation_of(&view, "你");
    assert!(single.contains("wqiy"), "{single}");
    assert!(!single.split(' ').any(|code| code == "wq"), "{single}");
}

#[test]
fn the_pinyin_lookup_follows_the_wubi_profile() {
    let root = tempfile::tempdir().unwrap();
    let mut options = pinyin_lookup_options(root.path());
    options.wubi_profile = 1;
    let mut session = Session::new(&options).unwrap();
    type_text(&mut session, b"znihao");
    let view = session.snapshot().unwrap();
    let annotation = annotation_of(&view, "你好");
    assert!(annotation.contains("abcd"), "{annotation}");
    assert!(!annotation.contains("wqvb"), "{annotation}");
}

#[test]
fn the_pinyin_lookup_is_entered_and_left_without_touching_wubi_codes() {
    let root = tempfile::tempdir().unwrap();
    let options = pinyin_lookup_options(root.path());
    let mut session = Session::new(&options).unwrap();

    // 只打 `z`：进入反查，还没有拼音，也就没有候选。
    assert!(session.character(b'z', false).unwrap().handled);
    let view = session.snapshot().unwrap();
    assert_eq!(view.preedit, "z");
    assert!(view.candidates.is_empty());
    // 退格删掉 `z` 就离开反查。
    assert!(session.command(Command::Backspace).unwrap().handled);
    assert_eq!(session.snapshot().unwrap().preedit, "");

    // 单独的 `z` 回车什么也不上屏；没有候选时选词上屏的是拼音，不带 `z`。
    type_text(&mut session, b"z");
    assert_eq!(session.command(Command::CommitRaw).unwrap().commit, "");
    assert_eq!(session.snapshot().unwrap().preedit, "");
    type_text(&mut session, b"zq");
    assert!(session.snapshot().unwrap().candidates.is_empty());
    assert_eq!(session.finish(0).unwrap().commit, "q");
    assert_eq!(session.snapshot().unwrap().preedit, "");

    // 离开后照常是五笔码。
    type_text(&mut session, b"wqvb");
    let view = session.snapshot().unwrap();
    assert_eq!(view.candidates.first().map(String::as_str), Some("你好"));
    assert!(!view.answered_by_pinyin_fallback);
    session.command(Command::Cancel).unwrap();

    // 取消同样结束反查，下一个码不受影响；五笔码中间的 `z` 仍交回宿主。
    type_text(&mut session, b"zni");
    session.command(Command::Cancel).unwrap();
    type_text(&mut session, b"wq");
    assert!(!session.character(b'z', false).unwrap().handled);
    assert_eq!(session.snapshot().unwrap().preedit, "wq");
}

#[test]
fn a_mixed_pinyin_session_has_no_z_lookup() {
    let root = tempfile::tempdir().unwrap();
    let mut options = pinyin_lookup_options(root.path());
    options.wubi_mixed_pinyin = true;
    let mut session = Session::new(&options).unwrap();
    type_text(&mut session, b"znihao");
    // 混输里 `z` 是拼音字母：`znihao` 不是一个拼音，没有 `你好`。
    let view = session.snapshot().unwrap();
    assert!(!view.candidates.iter().any(|word| word == "你好"));
}

#[test]
fn committing_from_the_pinyin_lookup_keeps_the_rest_in_the_lookup() {
    let root = tempfile::tempdir().unwrap();
    let options = pinyin_lookup_options(root.path());
    let mut session = Session::new(&options).unwrap();
    type_text(&mut session, b"znihao");
    let view = session.snapshot().unwrap();
    let index = view
        .candidates
        .iter()
        .position(|word| word == "你")
        .unwrap();
    let result = session.select(index).unwrap();
    assert_eq!(result.commit, "你");
    // 剩下的 `hao` 仍是反查，不会被当成五笔码 `hao`。
    let view = session.snapshot().unwrap();
    assert_eq!(view.preedit, "zhao");
    assert!(annotation_of(&view, "好").contains("vbg"));
    let index = view
        .candidates
        .iter()
        .position(|word| word == "好")
        .unwrap();
    assert_eq!(session.select(index).unwrap().commit, "好");
    assert_eq!(session.snapshot().unwrap().preedit, "");

    // 回车上屏的是拼音本身，引导键 `z` 不上屏。
    type_text(&mut session, b"zhello");
    let result = session.command(Command::CommitRaw).unwrap();
    assert_eq!(result.commit, "hello");
}

#[test]
fn the_pinyin_lookup_learns_nothing() {
    let journal_rows = |options: &EngineOptions| -> i64 {
        let path = Path::new(&options.user_data).join("msime_user.db");
        if !path.exists() {
            return 0;
        }
        Connection::open(path)
            .unwrap()
            .query_row(
                "SELECT count(*) FROM user_dictionary_operations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
    };
    let pick_second = |options: &EngineOptions| {
        let mut session = Session::new(options).unwrap();
        type_text(&mut session, b"znihao");
        let view = session.snapshot().unwrap();
        let index = view
            .candidates
            .iter()
            .position(|word| word == "拟好")
            .unwrap();
        assert!(index > 0, "{:?}", view.candidates);
        assert_eq!(session.select(index).unwrap().commit, "拟好");
    };

    let root = tempfile::tempdir().unwrap();
    let mut options = pinyin_lookup_options(root.path());
    options.learning = true;
    pick_second(&options);
    pick_second(&options);
    assert_eq!(journal_rows(&options), 0);
    // 五笔码照旧，拼音的排序也没被反查里的选择改动。
    let mut session = Session::new(&options).unwrap();
    type_text(&mut session, b"wqvb");
    assert_eq!(session.snapshot().unwrap().candidates, vec!["你好"]);
    session.command(Command::Cancel).unwrap();
    type_text(&mut session, b"znihao");
    let view = session.snapshot().unwrap();
    let first = view
        .candidates
        .iter()
        .position(|word| word == "你好")
        .unwrap();
    let second = view
        .candidates
        .iter()
        .position(|word| word == "拟好")
        .unwrap();
    assert!(first < second, "{:?}", view.candidates);

    // 对照：混输里同样的选择会学习，说明上面的零确实是反查没有学，而不是测不到。
    let control_root = tempfile::tempdir().unwrap();
    let mut control = pinyin_lookup_options(control_root.path());
    control.learning = true;
    control.wubi_mixed_pinyin = true;
    let mut session = Session::new(&control).unwrap();
    type_text(&mut session, b"nihao");
    let view = session.snapshot().unwrap();
    let index = view
        .candidates
        .iter()
        .position(|word| word == "拟好")
        .unwrap();
    session.select(index).unwrap();
    drop(session);
    assert!(journal_rows(&control) > 0);
}

#[test]
fn a_multi_pick_in_the_pinyin_lookup_creates_no_phrase() {
    // `你号` 不在码表里：分两次选出它时，打开学习的会话会把它当新词写进拼音词库。
    let phrase_options = |root: &Path, mixed_pinyin: bool| {
        let mut options = pinyin_lookup_options(root);
        for directory in [&options.resources, &options.dictionaries] {
            Connection::open(Path::new(directory).join("msime-pinyin.db"))
                .unwrap()
                .execute("INSERT INTO tbl_1_h VALUES('hao','h','号',50)", [])
                .unwrap();
        }
        options.learning = true;
        options.wubi_mixed_pinyin = mixed_pinyin;
        options
    };
    let pick_in_two = |options: &EngineOptions, typed: &[u8]| {
        let mut session = Session::new(options).unwrap();
        type_text(&mut session, typed);
        for word in ["你", "号"] {
            let view = session.snapshot().unwrap();
            let index = view
                .candidates
                .iter()
                .position(|candidate| candidate == word)
                .unwrap_or_else(|| panic!("{word} missing from {:?}", view.candidates));
            assert_eq!(session.select(index).unwrap().commit, word);
        }
        assert_eq!(session.snapshot().unwrap().preedit, "");
    };
    let created_rows = |options: &EngineOptions| -> i64 {
        let path = Path::new(&options.user_data).join("msime_user.db");
        if !path.exists() {
            return 0;
        }
        Connection::open(path)
            .unwrap()
            .query_row(
                "SELECT count(*) FROM user_dictionary_operations WHERE value='你号' AND user_inserted=1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
    };

    let root = tempfile::tempdir().unwrap();
    let options = phrase_options(root.path(), false);
    pick_in_two(&options, b"znihao");
    assert_eq!(created_rows(&options), 0);

    // 对照：混输里同样分两次选，会造出 `你号`，说明上面的零是反查没有造词，而不是这条路径走不到。
    let control_root = tempfile::tempdir().unwrap();
    let control = phrase_options(control_root.path(), true);
    pick_in_two(&control, b"nihao");
    assert_eq!(created_rows(&control), 1);
}

#[test]
fn hidden_helpcode_snapshot_does_not_recompute_annotations() {
    let root = tempfile::tempdir().unwrap();
    let mut options = helpcode_fixture(root.path(), "", "你=ab\n好=cd\n拟=ef\n");
    options.show_helpcode = false;
    let mut session = Session::new(&options).unwrap();
    type_text(&mut session, b"nihao");
    let (snapshot, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.snapshot().unwrap());
    eprintln!("隐藏辅助码快照分配: {allocations}");
    // 邮箱入口 `@` 及追加的字面符号各占一次分配。
    assert!(
        allocations <= 34,
        "隐藏辅助码重复构造分配了 {allocations} 次"
    );
    assert!(!snapshot.candidates.is_empty());
}

/// #7616：只按了入口键就回车时，上屏的是入口字母本身，去掉标记后没有正文可学——以前会报「英文词学不了」（`Y`），临时日文还会把补出来的 `r` 写进英文词库。
#[test]
fn a_bare_entry_letter_raw_commit_learns_nothing() {
    for (letter, mode, word) in [
        (b'Y', "temporary_english", "y"),
        (b'R', "temporary_japanese", "r"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let value = options(dir.path());
        let mut session = Session::new(&value).unwrap();
        assert!(session.character(letter, true).unwrap().handled);
        assert_eq!(session.snapshot().unwrap().local_mode, mode);
        let result = session.command(Command::CommitRaw).unwrap();
        assert_eq!(result.commit, (letter as char).to_string());
        assert_eq!(result.diagnostic, "");
        assert_eq!(english_word_count(&value, word), 0);
    }
}

#[test]
fn resource_dictionary_browse_distinguishes_missing_files_and_preserves_model_paging() {
    use super::dictionary_lookup::{dictionary_resource_entries, ResourceDictionaryKind::*};
    let root = tempfile::tempdir().unwrap();
    let options = options(root.path());
    for kind in [Japanese, Cantonese, Zhuyin, Stroke] {
        let page = dictionary_resource_entries(&options, kind, "", 0, 2).unwrap();
        assert!(!page.available);
        assert!(!page.has_more);
        assert!(page.entries.is_empty());
    }
    let korean = dictionary_resource_entries(&options, KoreanHanja, "", 0, 2).unwrap();
    assert!(korean.available && korean.has_more);
    assert_eq!(korean.entries.len(), 2);
    let bytes = crate::japanese::decoder::test_model::bytes(
        &[
            ("!", "!", 0, 0, 1),
            ("-", "-", 0, 0, 2),
            ("かな", "仮名", 0, 0, 10),
            ("きごう", "※", 0, 0, 1),
            ("にほん", "日本", 0, 0, -10),
            ("にほんご", "日本語", 0, 0, 20),
            ("ー", "ー", 0, 0, 1),
        ],
        1,
        &[0],
    );
    let path = Path::new(&options.resources).join("msime-japanese.dat");
    std::fs::write(&path, &bytes).unwrap();
    for (offset, expected, has_more) in [(0, "仮名", true), (1, "日本", true), (2, "日本語", false)]
    {
        let page = dictionary_resource_entries(&options, Japanese, "", offset, 1).unwrap();
        assert!(page.available);
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].value, expected);
        assert_eq!(page.has_more, has_more);
    }
    let empty = dictionary_resource_entries(&options, Japanese, "※", 0, 1).unwrap();
    assert!(empty.available && empty.entries.is_empty() && !empty.has_more);
    let page = dictionary_resource_entries(&options, Japanese, "日本", 0, 1).unwrap();
    assert!(page.available && page.has_more);
    assert_eq!(page.entries[0].value, "日本");
    let page = dictionary_resource_entries(&options, Japanese, "にほん", 1, 1).unwrap();
    assert!(!page.has_more);
    assert_eq!(page.entries[0].value, "日本語");
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    assert!(dictionary_resource_entries(&options, Japanese, "", 0, 1001).is_err());
    assert!(dictionary_resource_entries(&options, Japanese, "", 1_000_001, 1).is_err());
}

fn multilingual_fixture(root: &Path) -> EngineOptions {
    let mut value = helpcode_fixture(root, "", "");
    value.helpcode = false;
    value.mixed_english = false;
    value.multilingual_languages = vec![
        crate::multilingual::Language::Chinese,
        crate::multilingual::Language::English,
        crate::multilingual::Language::Japanese,
    ];
    Connection::open(Path::new(&value.dictionaries).join("msime-english.db"))
        .unwrap()
        .execute_batch("INSERT INTO english_words VALUES('hello','hello',100);")
        .unwrap();
    std::fs::write(
        Path::new(&value.resources).join("msime-japanese.dat"),
        crate::japanese::decoder::test_model::bytes(
            &[("おはよう", "おはよう", 0, 0, 500)],
            1,
            &[0],
        ),
    )
    .unwrap();
    value
}

#[test]
fn host_multilingual_options_map_the_selected_languages() {
    let root = tempfile::tempdir().unwrap();
    let value = multilingual_fixture(root.path());
    assert_eq!(
        options::session_options(&value)
            .unwrap()
            .multilingual_languages,
        value.multilingual_languages
    );
}

#[test]
fn host_multilingual_snapshot_keeps_the_raw_identity_and_language_ranges() {
    use crate::multilingual::Language;
    let root = tempfile::tempdir().unwrap();
    let value = multilingual_fixture(root.path());
    let mut session = Session::new(&value).unwrap();
    type_text(&mut session, b"hellonihaoohayou");
    let snapshot = session.snapshot().unwrap();
    let index = snapshot
        .candidate_sources
        .iter()
        .position(|source| *source == CandidateSource::Multilingual as u8)
        .expect("宿主应保留连续混合行");
    assert_eq!(snapshot.candidates[index], "hello你好おはよう");
    let metadata = snapshot
        .multilingual_conversion
        .expect("宿主快照应保留语言边界");
    assert_eq!(metadata.raw_input, "hellonihaoohayou");
    assert_eq!(metadata.raw_input, snapshot.editing_text);
    assert_eq!(
        metadata
            .conversion
            .segments
            .iter()
            .map(|segment| (
                segment.language,
                segment.input.clone(),
                segment.text.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            (Language::English, 0..5, "hello"),
            (Language::Chinese, 5..10, "你好"),
            (Language::Japanese, 10..16, "おはよう"),
        ]
    );
    assert_eq!(session.select(index).unwrap().commit, "hello你好おはよう");
    assert!(session
        .snapshot()
        .unwrap()
        .multilingual_conversion
        .is_none());
    type_text(&mut session, b"nihao");
    assert!(session
        .snapshot()
        .unwrap()
        .multilingual_conversion
        .is_none());
    session.command(Command::Cancel).unwrap();
    assert!(session
        .snapshot()
        .unwrap()
        .multilingual_conversion
        .is_none());
}

/// The iOS keyboard destroys its session whenever it goes away and builds one on the next appearance; the personal context model survives that only if the session records the journal as it lets go. Not on Windows, where a store that only read the journal records nothing rather than keep a probe connection open.
#[cfg(not(windows))]
#[test]
fn dropping_a_session_records_its_journal_for_the_next_one() {
    let root = tempfile::tempdir().unwrap();
    let options = helpcode_fixture(root.path(), "", "");
    let journal = super::options::runtime_paths(&options).user(crate::assets::USER_JOURNAL);
    let connection = Connection::open(&journal).unwrap();
    crate::user_dictionary::journal::ensure_schema(&connection).unwrap();
    connection
        .execute_batch("INSERT INTO personal_bigram(previous,word,count) VALUES('你','好',2);")
        .unwrap();
    drop(connection);
    let store = crate::user_dictionary::ngram_store::PersonalNgramStore::for_journal(&journal);
    drop(Session::new(&options).unwrap());
    assert!(store.settled_for_tests());
}
