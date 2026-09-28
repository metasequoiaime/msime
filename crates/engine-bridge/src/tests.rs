//! Unit tests for the parent module, in their own file because the module
//! is large enough that mixing them with the implementation obscured both.
//! Same `mod tests` as before, so `use super::*` still names the parent.

use super::*;

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
    assert!(candidate_glosses_with_user(resources_path, user_path, &candidates).is_err());
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
    let db = rusqlite::Connection::open(&packaged).unwrap();
    db.execute_batch(
        "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);
            CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY,chinese_gloss TEXT);
            CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY,english_gloss TEXT);
            INSERT INTO en_zh_glosses VALUES('missing','发布释义');
            INSERT INTO zh_en_glosses VALUES('测试','packaged gloss');",
    )
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

#[test]
fn hand_written_glosses_outrank_learned_and_packaged_ones() {
    // The user's own file wins, and nothing was pinning that. It arrives by a route worth writing
    // down: translation-glosses.db sits in the user directory the settings page writes
    // custom_translations.txt to, and EnglishDictionary opened without an explicit translations path
    // reads its sidecar from beside the database - so the learned store carries the hand-written
    // entries too, and query_*_gloss answers from them before touching anything else.
    //
    // That makes the precedence an emergent property of where two files happen to live. Moving either
    // one, or giving the learned store an explicit translations path, would silently drop the user's
    // glosses to the bottom. This test is what would notice.
    let resources = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let database = rusqlite::Connection::open(resources.path().join("english.db")).unwrap();
    database
        .execute_batch(
            "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);
                 CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY,chinese_gloss TEXT);
                 CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY,english_gloss TEXT);
                 INSERT INTO zh_en_glosses VALUES('测试','packaged gloss');",
        )
        .unwrap();
    let resources_path = resources.path().to_str().unwrap();
    let user_path = user.path().to_str().unwrap();
    let candidates = vec![("测试".into(), 0)];

    // Packaged only, to begin with.
    assert_eq!(
        candidate_glosses_with_user(resources_path, user_path, &candidates).unwrap(),
        vec!["packaged gloss"]
    );

    // A gloss learned from the network outranks the packaged one, which this already guaranteed.
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

    // What the user wrote outranks both. Anything else means an automatic answer silently replacing
    // the one they asked for by hand.
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

#[test]
fn unsafe_learned_glosses_fall_back_to_packaged_values() {
    let resources = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let database = rusqlite::Connection::open(resources.path().join("english.db")).unwrap();
    database
        .execute_batch(
            "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);
                 CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY,chinese_gloss TEXT);
                 CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY,english_gloss TEXT);
                 INSERT INTO zh_en_glosses VALUES('测试','packaged gloss');",
        )
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

fn offline_gloss_database(path: &std::path::Path, language: &str, version: i32) {
    let database = rusqlite::Connection::open(path).unwrap();
    database
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
    let renamed = directory.path().join("zh-ja.db");
    offline_gloss_database(&renamed, "fr", 1);
    assert!(candidate_target_glosses(renamed.to_str().unwrap(), "ja", &candidates).is_err());
    let future = directory.path().join("zh-de.db");
    offline_gloss_database(&future, "de", 2);
    assert!(candidate_target_glosses(future.to_str().unwrap(), "de", &candidates).is_err());
    let missing = directory.path().join("zh-ko.db");
    assert!(candidate_target_glosses(missing.to_str().unwrap(), "ko", &candidates).is_err());
    assert!(
        !missing.exists(),
        "a read-only open must not create the file"
    );
    let damaged = directory.path().join("zh-es.db");
    std::fs::write(&damaged, "synthetic damaged database").unwrap();
    assert!(candidate_target_glosses(damaged.to_str().unwrap(), "es", &candidates).is_err());
}

pub(super) fn options(root: &std::path::Path) -> EngineOptions {
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
        shuangpin_profile: 0,
        shuangpin_preedit_uses_raw: true,
        learning: false,
        autocorrect_transposition: true,
        autocorrect_neighbor: true,
        fuzzy_pinyin_rules: 0,
        wubi_mixed_pinyin: false,
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
        sentence_alternatives: true,
    }
}

#[test]
fn reset_learned_data_restores_packaged_dictionaries_and_clears_journal() {
    // Both an ASCII root and one carrying Chinese characters. On Windows a
    // narrow conversion of the second either mangles it or throws, and the
    // reset derives temporary, backup and SQLite sidecar names from these
    // paths - a throw would abort it after it had already published files.
    for component in ["ascii", "陆傲天"] {
        reset_learned_data_under_root(component);
    }
}

fn reset_learned_data_under_root(component: &str) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join(component);
    std::fs::create_dir_all(&root).unwrap();
    let root = root.as_path();
    let value = options(root);
    let resources = std::path::Path::new(&value.resources);
    let dictionaries = std::path::Path::new(&value.dictionaries);
    let main_fixture = "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
                            INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',100);\
                            CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
                            CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);";
    rusqlite::Connection::open(resources.join("msime.db"))
        .unwrap()
        .execute_batch(main_fixture)
        .unwrap();
    rusqlite::Connection::open(resources.join("english.db"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);\
                 CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY,chinese_gloss TEXT);\
                 CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY,english TEXT);\
                 INSERT INTO english_words VALUES('word','word',100);",
        )
        .unwrap();
    std::fs::copy(resources.join("msime.db"), dictionaries.join("msime.db")).unwrap();
    std::fs::copy(
        resources.join("english.db"),
        dictionaries.join("english.db"),
    )
    .unwrap();
    let journal = std::path::Path::new(&value.user_data).join("msime_user.db");
    rusqlite::Connection::open(&journal)
        .unwrap()
        .execute_batch(
            "CREATE TABLE user_dictionary_operations(dictionary TEXT,key TEXT,value TEXT,operation TEXT,weight INTEGER,display TEXT,user_inserted INTEGER);\
                 CREATE TABLE personal_dictionary_receipts(request_id TEXT PRIMARY KEY,payload TEXT);\
                 CREATE TABLE candidate_selection_state(context_key TEXT,entry_key TEXT,value TEXT,selection_count INTEGER);\
                 CREATE TABLE fixed_candidate_positions(context_key TEXT,entry_key TEXT,value TEXT,position INTEGER);\
                 INSERT INTO user_dictionary_operations VALUES('pinyin','ni''hao','你好','upsert',1,'',1);\
                 INSERT INTO candidate_selection_state VALUES('ni''hao','ni''hao','你好',7);",
        )
        .unwrap();
    rusqlite::Connection::open(dictionaries.join("msime.db"))
        .unwrap()
        .execute("UPDATE tbl_2_n SET weight=1", [])
        .unwrap();

    reset_learned_data(&value).unwrap();

    let database = rusqlite::Connection::open(dictionaries.join("msime.db")).unwrap();
    let weight: i64 = database
        .query_row(
            "SELECT weight FROM tbl_2_n WHERE key='ni''hao'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(weight, 100);
    let journal = rusqlite::Connection::open(journal).unwrap();
    let operations: i64 = journal
        .query_row(
            "SELECT count(*) FROM user_dictionary_operations",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let selections: i64 = journal
        .query_row(
            "SELECT count(*) FROM candidate_selection_state",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(operations, 0);
    assert_eq!(selections, 0);
}

#[test]
fn prepared_options_disable_quanpin_autocorrect_by_default() {
    let root = tempfile::tempdir().unwrap();
    let resources = root.path().join("resources");
    std::fs::create_dir_all(&resources).unwrap();
    for name in ["msime.db", "english.db"] {
        rusqlite::Connection::open(resources.join(name)).unwrap();
    }
    let prepared = super::prepare_options(
        resources.to_str().unwrap(),
        root.path().join("user").to_str().unwrap(),
        root.path().join("cache").to_str().unwrap(),
        "synthetic-defaults",
    )
    .unwrap();
    assert!(!prepared.autocorrect_transposition);
    assert!(!prepared.autocorrect_neighbor);
    assert_eq!(prepared.english_minimum_prefix, 5);
}
#[test]
fn dictionary_revision_uses_real_journal_and_rejects_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let before = super::dictionary_state_revision(&value).unwrap();
    assert_eq!(before.len(), 64);
    assert_eq!(before, super::dictionary_state_revision(&value).unwrap());
    let journal = std::path::Path::new(&value.user_data).join("msime_user.db");
    assert!(!journal.exists());
    std::fs::write(&journal, b"synthetic invalid database").unwrap();
    assert!(super::dictionary_state_revision(&value).is_err());
    assert_eq!(
        std::fs::read(&journal).unwrap(),
        b"synthetic invalid database"
    );
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
            session.character(b'n', false).unwrap();
            session.character(b'i', false).unwrap();
            assert_eq!(session.character(b'H', true).unwrap().handled, enabled);
        }
    }
    value.helpcode_schema = "unknown".into();
    assert!(Session::new(&value).is_err());
}

/// The jiajia table this repository carries is in the shape the Engine parses.
///
/// Five helpcode tables arrive inside the locked Engine archive and cannot rot independently of
/// it. This one does not: it lives in `resources/helpcodes/` and is injected by an overlay, so
/// it is the one that can go missing, be truncated by a bad merge, or be saved in an encoding
/// the Engine reads as nothing. The test above would not notice any of that - it points
/// `resources` at an empty directory, so it shows the scheme is registered and accepted and
/// would pass with no table at all.
///
/// What it checks is the Engine's own parse rule from `HelpcodeUtils::load_helpcode_keymap`:
/// split at the first `=`, take two characters after it, keep the entry only when both are
/// `a`-`z`. An entry this rejects is silently absent at runtime rather than an error, which is
/// why counting them here is worth doing.
///
/// Reading the codes the Engine would read is as far as this level goes: filtering candidates
/// needs a real dictionary, and these tests run against empty directories.
#[test]
fn the_carried_jiajia_table_parses_the_way_the_engine_reads_it() {
    let table = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../resources/helpcodes/jiajia_helpcode.txt");
    let text = std::fs::read_to_string(&table)
        .unwrap_or_else(|error| panic!("{}: {error}", table.display()));

    let mut keymap = std::collections::BTreeMap::new();
    let mut rejected = Vec::new();
    for line in text.lines() {
        let Some(position) = line.find('=') else {
            rejected.push(line);
            continue;
        };
        let (character, code) = line.split_at(position);
        let code: String = code[1..].chars().take(2).collect();
        if position == 0 || code.len() != 2 || !code.bytes().all(|byte| byte.is_ascii_lowercase()) {
            rejected.push(line);
            continue;
        }
        keymap.insert(character.to_owned(), code);
    }

    assert!(
        rejected.is_empty(),
        "the Engine drops these silently: {:?}",
        &rejected[..rejected.len().min(5)]
    );
    // Codes quoted in resources/helpcodes/NOTICE.md as coming from the alignment.
    for (character, code) in [("好", "nz"), ("你", "de"), ("中", "ks"), ("国", "ky")] {
        assert_eq!(keymap.get(character).map(String::as_str), Some(code));
    }
    // The notice records 7968 entries; a table that lost a chunk still parses.
    assert_eq!(keymap.len(), 7968);
}

#[test]
fn invalid_options_return_errors_instead_of_unwinding_into_rust() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.scheme = 255;
    assert!(Session::new(&value).is_err());
    value.scheme = 0;
    value.shuangpin_profile = 255;
    assert!(Session::new(&value).is_err());
    value.shuangpin_profile = 0;
    value.resources = "relative".into();
    assert!(Session::new(&value).is_err());
}
#[test]
fn microsoft_profile_accepts_semicolon_as_an_ing_final() {
    let dir = tempfile::tempdir().unwrap();
    for profile in 0..4 {
        let mut options = options(dir.path());
        options.scheme = 1;
        options.shuangpin_profile = profile;
        let mut session = Session::new(&options).unwrap();
        session.character(b'b', false).unwrap();
        session.character(b';', false).unwrap();
        assert_eq!(
            session.snapshot().unwrap().editing_text,
            if profile == 3 { "b;" } else { "b" }
        );
    }
}

#[test]
fn microsoft_profile_keeps_trailing_semicolon_in_segment_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    for (input, expected) in [
        ("nihkb;", vec![0, 2, 4, 6]),
        // `cb` is a valid Microsoft shuangpin pair and must win over
        // pairing the final `b` with the trailing semicolon.
        ("nihcb;", vec![0, 2, 3, 5, 6]),
    ] {
        let mut options = options(dir.path());
        options.scheme = 1;
        options.shuangpin_profile = 3;
        let mut session = Session::new(&options).unwrap();
        for character in input.bytes() {
            session.character(character, false).unwrap();
        }
        assert_eq!(session.snapshot().unwrap().segment_raw_boundaries, expected);
    }
}

/// A keyboard face labels its letter keys with the units they carry. The hints
/// have to come from the profile the session is running, which is why this asks
/// the session for its profile name instead of assuming the option index and the
/// name agree.
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
            // " / " separates initials from finals, so it appears at most once; units
            // on the same side are separated by a space.
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

/// Xiaohe keeps two finals on K, and a host-side copy of the keymap listed only
/// one of them, so the key that types `guai` carried no sign of it. The hints are
/// read out of the Engine now; this holds that specific key to both units.
#[test]
fn shuangpin_key_hints_keep_every_unit_a_key_carries() {
    let hints: std::collections::HashMap<String, String> = shuangpin_key_hints("xiaohe")
        .into_iter()
        .map(|entry| (entry.key, entry.hint))
        .collect();
    assert_eq!(hints.get("K").map(String::as_str), Some("ing uai"));
    assert_eq!(hints.get("V").map(String::as_str), Some("zh / ui ü"));
}

/// Labelling the keys with a scheme the session is not running is worse than
/// labelling nothing, so an unrecognised name yields no hints at all instead of
/// falling back to the default profile the Engine's own lookup returns.
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
        session.character(b'y', false).unwrap();
        session.character(b'o', false).unwrap();
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

#[test]
fn commit_raw_applies_windows_english_learning_policy() {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let mut session = Session::new(&value).unwrap();
    session.set_dedicated_english(true).unwrap();
    for character in b"hello" {
        assert!(session.character(*character, false).unwrap().handled);
    }
    assert_eq!(session.command(Command::CommitRaw).unwrap().commit, "hello");
    let database =
        rusqlite::Connection::open(std::path::Path::new(&value.dictionaries).join("english.db"))
            .unwrap();
    let learned: String = database
        .query_row(
            "SELECT display FROM english_words WHERE word='hello'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(learned, "hello");
}

// The shipped english.db weighs its words by Google unigram counts (dancing 14310606) while the pinyin tables use their own scale (单词 225415), so an English weight says nothing about a Chinese one. Mixed input therefore never seats an English word ahead of the leading Chinese candidate: not on its shipped weight, not after it is committed, not after it is pinned. Pinning only reorders it among the English words. The fixture gives every English word a weight above the Chinese ones, as the shipped dictionaries do.
#[test]
fn mixed_english_never_takes_the_first_seat_from_chinese() {
    const ENGLISH: u8 = 4;
    let dir = tempfile::tempdir().unwrap();
    let mut value = options(dir.path());
    value.learning = true;
    value.english_minimum_prefix = 2;
    for directory in [&value.resources, &value.dictionaries] {
        let directory = std::path::Path::new(directory);
        rusqlite::Connection::open(directory.join("msime.db"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
                 INSERT INTO tbl_1_n VALUES('ni','n','你',2000);\
                 INSERT INTO tbl_1_n VALUES('ni','n','倪',1000);\
                 CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
                 CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);",
            )
            .unwrap();
        rusqlite::Connection::open(directory.join("english.db"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);\
                 CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY,chinese_gloss TEXT);\
                 CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY,english_gloss TEXT);\
                 INSERT INTO english_words VALUES('nimbus','Nimbus',50000);\
                 INSERT INTO english_words VALUES('ninja','Ninja',30000);",
            )
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

    // Unlearned: the heaviest English word takes the leading English seat behind 你.
    let snapshot = typed(&mut session);
    assert_eq!(
        snapshot.candidates[1], "Nimbus",
        "{:?}",
        snapshot.candidates
    );

    // Committing the leading English word keeps it behind 你.
    session.select(index_of(&snapshot, "Nimbus")).unwrap();
    typed(&mut session);

    // Pinning an English word is an ordering among the English words too.
    let snapshot = typed(&mut session);
    let ninja = index_of(&snapshot, "Ninja");
    assert!(session.pin_candidate(ninja).unwrap().handled);
    let snapshot = typed(&mut session);
    assert_eq!(snapshot.candidates[1], "Ninja", "{:?}", snapshot.candidates);
}

#[test]
fn raw_commit_without_learning_leaves_the_english_dictionary_alone() {
    for dedicated in [true, false] {
        raw_commit_without_learning_case(dedicated);
    }
}

fn raw_commit_without_learning_case(dedicated: bool) {
    let dir = tempfile::tempdir().unwrap();
    let value = options(dir.path());
    let mut session = Session::new(&value).unwrap();
    session.set_dedicated_english(dedicated).unwrap();
    for character in b"hello" {
        assert!(session.character(*character, false).unwrap().handled);
    }
    assert_eq!(
        session
            .command(Command::CommitRawWithoutLearning)
            .unwrap()
            .commit,
        "hello"
    );
    let database = std::path::Path::new(&value.dictionaries).join("english.db");
    if database.exists() {
        let database = rusqlite::Connection::open(database).unwrap();
        let learned: i64 = database
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='english_words'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let learned = if learned == 0 {
            0
        } else {
            database
                .query_row(
                    "SELECT COUNT(*) FROM english_words WHERE word='hello'",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
        };
        assert_eq!(learned, 0);
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
    let database = std::path::Path::new(&value.dictionaries).join("english.db");
    if database.exists() {
        let database = rusqlite::Connection::open(database).unwrap();
        let count: i64 = database
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='english_words'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }
}
