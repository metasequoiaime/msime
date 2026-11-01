//! Unit tests for the parent module, in their own file because the module
//! is large enough that mixing them with the implementation obscured both.
//! Same `mod tests` as before, so `use super::*` still names the parent.

use super::*;

fn import_engine_options() -> msime_engine::host::EngineOptions {
    msime_engine::host::EngineOptions {
        resources: String::new(),
        user_data: String::new(),
        cache: String::new(),
        dictionaries: String::new(),
        first_page_rows: 6,
        scheme: 0,
        enabled_schemes: msime_engine::SchemeSet::ALL,
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
        shuangpin_helpcode_modes: msime_engine::ShuangpinHelpcodeModes::default(),
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
        english_minimum_prefix: 2,
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
        sentence_association: msime_engine::SentenceAssociationOptions {
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

fn quick_phrase(code: &str, text: &str) -> QuickPhrase {
    QuickPhrase {
        code: code.into(),
        text: text.into(),
    }
}

#[test]
#[cfg(not(target_os = "android"))]
fn typed_quick_phrase_edits_find_rows_by_code_and_text_and_list_only_user_phrases() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().to_str().unwrap();
    std::fs::create_dir(directory.path().join("user")).unwrap();
    // The smallest dictionary the Engine opens, as the host tests build it.
    let fixture = "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                       CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                       CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);
                       CREATE INDEX idx_quick_parases_key_weight ON quick_parases(key,weight DESC);";
    for name in ["resources", "dictionaries"] {
        let path = directory.path().join(name);
        std::fs::create_dir(&path).unwrap();
        rusqlite::Connection::open(path.join("msime-pinyin.db"))
            .unwrap()
            .execute_batch(fixture)
            .unwrap();
    }
    let mut document = json!({
        "api_version": 1,
        "resources": format!("{root}/resources"),
        "user_data": format!("{root}/user"),
        "cache": format!("{root}/cache"),
        "dictionaries": format!("{root}/dictionaries"),
        "preferences": msime_client_core::preferences::Preferences::default(),
        "preferences_directory": root,
    });
    // The Linux desktop publishes the skin catalog into the same document.
    document["candidate_skin_catalog"] = json!([]);
    let options = DictionaryOptions::from_host_document(document.clone()).unwrap();
    assert_eq!(options.user_data(), format!("{root}/user"));
    let mut unknown = document.clone();
    unknown["surprise"] = json!(true);
    assert!(DictionaryOptions::from_host_document(unknown).is_err());

    let list = |prefix: &str, offset: usize, limit: usize| {
        user_quick_phrases(&options, prefix, offset, limit).unwrap()
    };
    assert!(list("", 0, 10).phrases.is_empty());

    edit_user_quick_phrase(
        &options,
        &QuickPhraseEdit::Add(quick_phrase("DZ", "合成地址")),
        "t-add-1",
    )
    .unwrap();
    edit_user_quick_phrase(
        &options,
        &QuickPhraseEdit::Add(quick_phrase("sj", "合成手机")),
        "t-add-2",
    )
    .unwrap();
    // Other dictionaries share the store and must never be listed.
    let wubi = msime_engine::host::DictionaryEntry {
        kind: DictionaryKind::Wubi,
        key: "dz".into(),
        value: "合成".into(),
        weight: 10,
    };
    edit_personal_dictionary(&options.0, None, Some(&wubi), "t-add-3").unwrap();

    assert_eq!(
        edit_user_quick_phrase(
            &options,
            &QuickPhraseEdit::Add(quick_phrase("dz", "合成地址")),
            "t-add-4",
        )
        .unwrap_err(),
        "quick phrase already exists"
    );

    let all = list("", 0, 10);
    assert_eq!(all.phrases.len(), 2, "{:?}", all.phrases);
    assert!(!all.has_more);
    // The code is folded the way the Engine stores it.
    assert!(all.phrases.contains(&quick_phrase("dz", "合成地址")));
    assert_eq!(
        list("s", 0, 10).phrases,
        vec![quick_phrase("sj", "合成手机")]
    );
    let first = list("", 0, 1);
    assert_eq!(first.phrases.len(), 1);
    assert!(first.has_more);
    let second = list("", 1, 1);
    assert_eq!(second.phrases.len(), 1);
    assert!(!second.has_more);
    assert_ne!(first.phrases, second.phrases);

    edit_user_quick_phrase(
        &options,
        &QuickPhraseEdit::Replace {
            previous: quick_phrase("DZ", "合成地址"),
            replacement: quick_phrase("dz", "合成新地址"),
        },
        "t-replace-1",
    )
    .unwrap();
    assert_eq!(
        list("dz", 0, 10).phrases,
        vec![quick_phrase("dz", "合成新地址")]
    );

    edit_user_quick_phrase(
        &options,
        &QuickPhraseEdit::Remove(quick_phrase("sj", "合成手机")),
        "t-remove-1",
    )
    .unwrap();
    assert!(list("sj", 0, 10).phrases.is_empty());
    for edit in [
        QuickPhraseEdit::Remove(quick_phrase("sj", "合成手机")),
        QuickPhraseEdit::Replace {
            previous: quick_phrase("dz", "合成地址"),
            replacement: quick_phrase("dz", "合成别的"),
        },
    ] {
        assert_eq!(
            edit_user_quick_phrase(&options, &edit, "t-missing").unwrap_err(),
            "quick phrase not found"
        );
    }
    // The host's bounds still apply to a typed caller.
    assert!(edit_user_quick_phrase(
        &options,
        &QuickPhraseEdit::Add(quick_phrase("has space", "合成")),
        "t-invalid",
    )
    .unwrap_err()
    .starts_with(crate::INVALID_DICTIONARY_ENTRY));
    assert!(user_quick_phrases(&options, "", 0, 0).is_err());
}

/// A data root holding the smallest dictionary the Engine opens, with `bundled` statements run on the working dictionary.
fn word_fixture(directory: &Path, bundled: &str) -> DictionaryOptions {
    let root = directory.to_str().unwrap();
    std::fs::create_dir(directory.join("user")).unwrap();
    // Pinyin rows live in one table per syllable count and initial; these are the ones the tests type.
    let fixture = "CREATE TABLE tbl_2_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                       CREATE TABLE tbl_2_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                       CREATE TABLE tbl_3_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                       CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                       CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);
                       CREATE INDEX idx_quick_parases_key_weight ON quick_parases(key,weight DESC);";
    for name in ["resources", "dictionaries"] {
        let path = directory.join(name);
        std::fs::create_dir(&path).unwrap();
        let connection = rusqlite::Connection::open(path.join("msime-pinyin.db")).unwrap();
        connection.execute_batch(fixture).unwrap();
        connection.execute_batch(bundled).unwrap();
    }
    DictionaryOptions::from_host_document(json!({
        "api_version": 1,
        "resources": format!("{root}/resources"),
        "user_data": format!("{root}/user"),
        "cache": format!("{root}/cache"),
        "dictionaries": format!("{root}/dictionaries"),
        "preferences": msime_client_core::preferences::Preferences::default(),
        "preferences_directory": root,
    }))
    .unwrap()
}

fn new_word(code: Option<&str>, word: &str) -> NewWord {
    NewWord {
        code: code.map(str::to_owned),
        word: word.into(),
        weight: None,
    }
}

#[test]
#[cfg(not(target_os = "android"))]
fn typed_word_edits_find_user_and_bundled_rows_by_code_and_word() {
    let directory = tempfile::tempdir().unwrap();
    let options = word_fixture(
        directory.path(),
        "INSERT INTO wubi86 VALUES('aaaa','合成工',500);
             INSERT INTO tbl_2_c VALUES('ce''shi','cs','测试',100);",
    );
    let list = |kind: WordKind, prefix: &str, bundled: bool| {
        dictionary_words(&options, kind, prefix, bundled, 0, 100)
            .unwrap()
            .words
    };
    let edit = |edit: WordEdit, id: &str| edit_dictionary_word(&options, &edit, id);
    assert!(list(WordKind::Pinyin, "", false).is_empty());

    // An unseparated pinyin code is cut into the syllables the Engine stores.
    edit(
        WordEdit::Add(WordKind::Pinyin, new_word(Some("HeCheng"), "合成")),
        "w-add-1",
    )
    .unwrap();
    let added = Word {
        kind: WordKind::Pinyin,
        code: "he'cheng".into(),
        word: "合成".into(),
        weight: NEW_WORD_WEIGHT,
        bundled: false,
    };
    assert_eq!(list(WordKind::Pinyin, "", false), vec![added.clone()]);
    assert_eq!(list(WordKind::Pinyin, "hech", false), vec![added.clone()]);
    assert!(list(WordKind::Wubi, "", false).is_empty());
    for code in ["hecheng", "he'cheng", "he cheng"] {
        assert_eq!(
            edit(
                WordEdit::Add(WordKind::Pinyin, new_word(Some(code), "合成")),
                "w-add-2"
            )
            .unwrap_err(),
            "word already exists"
        );
    }

    // The weight found in the table is the one the Engine compares, so a re-weight lands.
    edit(
        WordEdit::SetWeight {
            kind: WordKind::Pinyin,
            code: "hecheng".into(),
            word: "合成".into(),
            weight: 50,
        },
        "w-weight-1",
    )
    .unwrap();
    assert_eq!(list(WordKind::Pinyin, "", false)[0].weight, 50);

    // A bundled row is found with the bundled rows included, and can be re-weighted and removed.
    assert!(list(WordKind::Wubi, "aaaa", false).is_empty());
    assert_eq!(
        list(WordKind::Wubi, "aaaa", true),
        vec![Word {
            kind: WordKind::Wubi,
            code: "aaaa".into(),
            word: "合成工".into(),
            weight: 500,
            bundled: true,
        }]
    );
    edit(
        WordEdit::SetWeight {
            kind: WordKind::Wubi,
            code: "aaaa".into(),
            word: "合成工".into(),
            weight: 800,
        },
        "w-weight-2",
    )
    .unwrap();
    assert_eq!(list(WordKind::Wubi, "aaaa", true)[0].weight, 800);
    edit(
        WordEdit::Remove {
            kind: WordKind::Wubi,
            code: "aaaa".into(),
            word: "合成工".into(),
        },
        "w-remove-1",
    )
    .unwrap();
    assert!(list(WordKind::Wubi, "aaaa", true).is_empty());

    let remove = WordEdit::Remove {
        kind: WordKind::Pinyin,
        code: "he'cheng".into(),
        word: "合成".into(),
    };
    edit(remove.clone(), "w-remove-2").unwrap();
    assert!(list(WordKind::Pinyin, "", false).is_empty());
    assert_eq!(edit(remove, "w-remove-3").unwrap_err(), "word not found");
    assert!(edit(
        WordEdit::Add(WordKind::Wubi, new_word(Some("abcde"), "合成")),
        "w-bad"
    )
    .unwrap_err()
    .starts_with(crate::INVALID_DICTIONARY_ENTRY));
}

#[test]
#[cfg(not(target_os = "android"))]
fn a_word_import_skips_what_is_there_names_what_is_refused_and_replays_cleanly() {
    let directory = tempfile::tempdir().unwrap();
    let options = word_fixture(
        directory.path(),
        "INSERT INTO tbl_2_c VALUES('ce''shi','cs','测试',100);",
    );
    let words = [
        new_word(Some("hecheng"), "合成"),
        // Bundled already, found by its reading.
        new_word(None, "测试"),
        new_word(Some("has space!"), "合成"),
        // No reading for a word the dictionary does not have.
        new_word(None, "鑫"),
        new_word(Some("hechengci"), "合成词"),
    ];
    let outcome = import_dictionary_words(&options, WordKind::Pinyin, &words, "import-1").unwrap();
    assert_eq!(outcome.added, 2);
    assert_eq!(outcome.existing, 1);
    assert_eq!(
        outcome
            .rejected
            .iter()
            .map(|(index, _)| *index)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert!(outcome
        .rejected
        .iter()
        .all(|(_, reason)| reason.starts_with(crate::INVALID_DICTIONARY_ENTRY)));
    let replay = import_dictionary_words(&options, WordKind::Pinyin, &words, "import-1").unwrap();
    assert_eq!((replay.added, replay.existing), (0, 3));
    assert_eq!(
        dictionary_words(&options, WordKind::Pinyin, "", false, 0, 100)
            .unwrap()
            .words
            .len(),
        2
    );
    assert!(import_dictionary_words(&options, WordKind::Pinyin, &[], "import-2").is_err());
    assert!(import_dictionary_words(&options, WordKind::Pinyin, &words, "bad id").is_err());
}

/// Every file under `directory` with what it holds: a database by its rows, since opening a session commits to the file without changing a row, and anything else by its bytes.
fn tree(directory: &Path) -> Vec<(std::path::PathBuf, Vec<String>)> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(tree(&path));
        } else if path.extension().is_some_and(|extension| extension == "db") {
            let connection = rusqlite::Connection::open(&path).unwrap();
            let mut rows = Vec::new();
            let tables: Vec<String> = connection
                .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            for table in tables {
                // Sorted rather than ordered by rowid: the personal n-gram tables are WITHOUT ROWID.
                let mut statement = connection
                    .prepare(&format!("SELECT * FROM \"{table}\""))
                    .unwrap();
                let columns = statement.column_count();
                let mut query = statement.query([]).unwrap();
                while let Some(row) = query.next().unwrap() {
                    let values: Vec<rusqlite::types::Value> =
                        (0..columns).map(|index| row.get(index).unwrap()).collect();
                    rows.push(format!("{table} {values:?}"));
                }
            }
            // Sorted here rather than by rowid: personal learning keeps some tables WITHOUT ROWID, and only the rows a file holds matter, not their order. Each row carries its table's name, so this also keeps the tables apart.
            rows.sort();
            files.push((path, rows));
        } else {
            let bytes = std::fs::read(&path).unwrap();
            files.push((path, vec![format!("{bytes:?}")]));
        }
    }
    files.sort();
    files
}

#[test]
#[cfg(not(target_os = "android"))]
fn a_lookup_names_where_each_candidate_came_from_and_leaves_the_user_data_alone() {
    let directory = tempfile::tempdir().unwrap();
    let options = word_fixture(
        directory.path(),
        "INSERT INTO wubi86 VALUES('aaaa','合成工',500);
             INSERT INTO tbl_2_c VALUES('ce''shi','cs','测试',100);",
    );
    edit_dictionary_word(
        &options,
        &WordEdit::Add(
            WordKind::Wubi,
            NewWord {
                code: Some("aaaa".into()),
                word: "合成字".into(),
                weight: Some(900),
            },
        ),
        "lookup-1",
    )
    .unwrap();
    let before = tree(&directory.path().join("user"));

    let candidates = lookup_candidates(&options, Some(LookupScheme::Wubi), "aaaa", 10).unwrap();
    let find = |text: &str| {
        candidates
            .iter()
            .find(|candidate| candidate.text == text)
            .map(|candidate| (candidate.origin, candidate.weight))
            .unwrap_or_else(|| panic!("{text} missing from {candidates:?}"))
    };
    assert_eq!(find("合成字"), (CandidateOrigin::UserWord, Some(900)));
    assert_eq!(find("合成工"), (CandidateOrigin::Dictionary, Some(500)));
    // The user's own scheme, quanpin by default.
    let candidates = lookup_candidates(&options, None, "ceshi", 1).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].text, "测试");
    assert_eq!(candidates[0].origin, CandidateOrigin::Dictionary);
    assert_eq!(candidates[0].weight, Some(100));

    for (scheme, code) in [
        (None, ""),
        (None, "Ceshi"),
        (None, "ce;shi"),
        (Some(LookupScheme::Wubi), "a;"),
    ] {
        assert!(
            lookup_candidates(&options, scheme, code, 5).is_err(),
            "{code}"
        );
    }
    assert!(lookup_candidates(&options, None, &"a".repeat(65), 5).is_err());
    assert!(lookup_candidates(&options, None, "ceshi", 0).is_err());
    assert!(lookup_candidates(&options, None, "ceshi", MAX_LOOKUP_CANDIDATES + 1).is_err());
    assert_eq!(tree(&directory.path().join("user")), before);
}

/// 日文混输开着时，查询会话照用户的设置混进日文转换，来源报成 `Japanese`；关着时没有这一行。
#[test]
#[cfg(not(target_os = "android"))]
fn a_lookup_keeps_the_users_japanese_mixing() {
    let directory = tempfile::tempdir().unwrap();
    let plain = word_fixture(
        directory.path(),
        "INSERT INTO tbl_2_c VALUES('ce''shi','cs','测试',100);",
    );
    std::fs::write(
        directory.path().join("resources/msime-japanese.dat"),
        crate::tests::single_lemma_japanese_model("おはよう", "おはよう", 500),
    )
    .unwrap();
    let root = directory.path().to_str().unwrap();
    let mut preferences = msime_client_core::preferences::Preferences::default();
    preferences.mixed_input.japanese = true;
    let mixing = DictionaryOptions::from_host_document(json!({
        "api_version": 1,
        "resources": format!("{root}/resources"),
        "user_data": format!("{root}/user"),
        "cache": format!("{root}/cache"),
        "dictionaries": format!("{root}/dictionaries"),
        "preferences": preferences,
        "preferences_directory": root,
    }))
    .unwrap();

    let candidates = lookup_candidates(&mixing, None, "ohayou", 10).unwrap();
    assert_eq!(
        candidates.first(),
        Some(&LookupCandidate {
            text: "おはよう".into(),
            code: "ohayou".into(),
            origin: CandidateOrigin::Japanese,
            weight: None,
        }),
        "{candidates:?}"
    );
    assert_eq!(
        candidates
            .iter()
            .filter(|candidate| candidate.origin == CandidateOrigin::Japanese)
            .count(),
        1
    );
    assert!(lookup_candidates(&plain, None, "ohayou", 10)
        .unwrap()
        .iter()
        .all(|candidate| candidate.origin != CandidateOrigin::Japanese));
}

/// 五笔四码唯一的词在第四键就自动上屏了，查询仍要把它报告成这串编码的唯一候选，而不是空列表。
#[test]
#[cfg(not(target_os = "android"))]
fn a_unique_four_letter_wubi_code_still_reports_its_word() {
    let directory = tempfile::tempdir().unwrap();
    let options = word_fixture(
        directory.path(),
        "INSERT INTO wubi86 VALUES('aaad','合成期',500);",
    );
    let before = tree(&directory.path().join("user"));
    let candidates = lookup_candidates(&options, Some(LookupScheme::Wubi), "aaad", 10).unwrap();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| (
                candidate.text.as_str(),
                candidate.code.as_str(),
                candidate.origin,
                candidate.weight
            ))
            .collect::<Vec<_>>(),
        [("合成期", "aaad", CandidateOrigin::Dictionary, Some(500))]
    );
    // 自动上屏走的是选词，但查询关了学习，用户数据里除了词库访问锁（空目录第一次查询时才建）什么都不多。
    let mut after = tree(&directory.path().join("user"));
    after.retain(|(path, _)| !path.ends_with(".msime-dictionary-access.lock"));
    assert_eq!(after, before);
}

/// 五笔版的 Engine 只跑五笔：查五笔（显式或按用户方案）照常，查全拼、双拼直接说明本版本没有这个方案，不报成词库打不开。
#[test]
#[cfg(not(target_os = "android"))]
fn a_wubi_edition_lookup_refuses_schemes_it_does_not_offer() {
    let directory = tempfile::tempdir().unwrap();
    word_fixture(
        directory.path(),
        "INSERT INTO wubi86 VALUES('aaa','合成工',500);
             INSERT INTO tbl_2_c VALUES('ce''shi','cs','测试',100);",
    );
    let root = directory.path().to_str().unwrap();
    let wubi = msime_client_core::edition::Edition::by_id("wubi").unwrap();
    let options = DictionaryOptions::from_host_document(json!({
        "api_version": 1,
        "resources": format!("{root}/resources"),
        "user_data": format!("{root}/user"),
        "cache": format!("{root}/cache"),
        "dictionaries": format!("{root}/dictionaries"),
        "preferences": msime_client_core::preferences::Preferences::for_edition(wubi),
        "preferences_directory": root,
        "edition": "wubi",
    }))
    .unwrap();
    for scheme in [None, Some(LookupScheme::Wubi)] {
        // 三码不会因唯一四码直接上屏，候选留在列表里。
        let candidates = lookup_candidates(&options, scheme, "aaa", 5).unwrap();
        assert_eq!(
            candidates.first().map(|candidate| candidate.text.as_str()),
            Some("合成工"),
            "{scheme:?}"
        );
    }
    for scheme in [LookupScheme::Quanpin, LookupScheme::Shuangpin] {
        assert_eq!(
            lookup_candidates(&options, Some(scheme), "ceshi", 5).unwrap_err(),
            "this edition does not offer that scheme"
        );
    }
}

#[test]
fn hans_entries_reject_what_the_import_format_rejects_without_touching_state() {
    let directory = tempfile::tempdir().unwrap();
    let resources = directory.path().to_str().unwrap();
    assert_eq!(
        hans_entries_json("你好", "relative/resources").unwrap_err(),
        "resources path must be absolute"
    );
    for text in ["", "# comment only\n", "hello", "你好\u{0}", "你好\t世界"] {
        assert_eq!(
            hans_entries_json(text, resources).unwrap_err(),
            "invalid dictionary import",
            "{text:?}"
        );
    }
    // No packaged dictionary: nothing to read a reading from, and nothing created in its place.
    assert_eq!(
        hans_entries_json("你好", resources).unwrap_err(),
        "dictionary pinyin unavailable"
    );
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);

    let read = |text: &[u8], resources: &[u8]| -> serde_json::Value {
        let pointer = unsafe {
            msime_client_dictionary_hans_entries(
                text.as_ptr(),
                text.len(),
                resources.as_ptr(),
                resources.len(),
            )
        };
        let value = unsafe { std::ffi::CStr::from_ptr(pointer) }
            .to_str()
            .unwrap()
            .to_owned();
        unsafe { crate::msime_client_string_free(pointer) };
        serde_json::from_str(&value).unwrap()
    };
    assert_eq!(read(b"\xff", resources.as_bytes())["ok"], false);
    assert_eq!(read("你好".as_bytes(), &[0xff])["ok"], false);
    let null =
        unsafe { msime_client_dictionary_hans_entries(std::ptr::null(), 0, std::ptr::null(), 0) };
    let value = unsafe { std::ffi::CStr::from_ptr(null) }
        .to_str()
        .unwrap()
        .to_owned();
    unsafe { crate::msime_client_string_free(null) };
    assert!(value.contains("\"ok\":false"));
}

fn pinyin(key: &str, value: &str) -> Entry {
    Entry {
        kind: Kind::Pinyin,
        key: key.into(),
        value: value.into(),
        weight: 10_000,
        source: None,
    }
}

#[test]
fn a_pinyin_search_finds_the_separated_key_without_separators() {
    let stored = pinyin("ni'hao", "你好");
    for query in ["nihao", "nih", "ni hao", "NIHAO", "ni'hao"] {
        assert!(stored.matches(Some(Kind::Pinyin), query), "{query:?}");
        assert!(stored.matches(None, query), "{query:?}");
    }
    assert!(!stored.matches(Some(Kind::Pinyin), "nhao"));
    assert!(!stored.matches(Some(Kind::Wubi), "nihao"));
}

#[test]
fn a_manual_pinyin_entry_is_cut_into_syllables_like_an_imported_one() {
    for typed in ["nihao", "NiHao", "ni hao", "ni'hao"] {
        let entry = replacement_for_engine(pinyin(typed, "你好")).unwrap();
        assert_eq!(entry.key, "ni'hao", "{typed:?}");
        // What this layer hands on is what the Engine accepts.
        msime_engine::host::dictionary_validate(&entry.into()).unwrap();
    }
    // The word's length picks the cut, as it does for an imported row.
    assert_eq!(
        replacement_for_engine(pinyin("xian", "西安")).unwrap().key,
        "xi'an"
    );
    assert_eq!(
        replacement_for_engine(pinyin("xian", "先")).unwrap().key,
        "xian"
    );
    // A key that cannot be cut reaches the Engine unchanged, and the Engine names the rule.
    assert_eq!(
        replacement_for_engine(pinyin("nhao", "你好")).unwrap().key,
        "nhao"
    );
}

#[test]
fn a_refused_edit_says_which_rule_it_broke() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().to_str().unwrap();
    let edit = |replacement: Entry| {
        let request = json!({
            "options": {
                "api_version": 1,
                "resources": format!("{root}/resources"),
                "user_data": format!("{root}/user"),
                "cache": format!("{root}/cache"),
                "dictionaries": format!("{root}/dictionaries"),
                "preferences": msime_client_core::preferences::Preferences::default(),
            },
            "action": {
                "operation": "edit",
                "previous": null,
                "replacement": replacement,
                "request_id": "synthetic-edit",
            },
        });
        dictionary_request_json(&serde_json::to_vec(&request).unwrap()).unwrap_err()
    };
    // Jianpin is not a full reading: the Engine's own reason comes back under the stable prefix the settings page maps to one code, before anything is locked or created.
    let refused = edit(pinyin("nhao", "你好"));
    assert!(
        refused.starts_with("invalid dictionary entry: "),
        "{refused}"
    );
    assert!(!refused.contains("nhao") && !refused.contains("你好"));
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    // Too many syllables for the word is the same kind of refusal.
    assert!(edit(pinyin("ni'hao'ma", "你好")).starts_with("invalid dictionary entry: "));
    // The host's own bounds name their rule too.
    let wubi = Entry {
        kind: Kind::Wubi,
        key: "abcde".into(),
        value: "测试".into(),
        weight: 1,
        source: None,
    };
    assert_eq!(
        edit(wubi),
        "invalid dictionary entry: code is empty or too long"
    );
    // `nihao` passes every entry rule; with no dictionary behind these paths it fails later, on storage, not as an invalid entry.
    let accepted = edit(pinyin("nihao", "你好"));
    assert!(
        !accepted.starts_with("invalid dictionary entry"),
        "{accepted}"
    );
}

/// English frequency learning lifts a user's own word past 100000000 with no ceiling, and the list hands that row back as it is stored, so an edit or removal of it must not be refused as out of range. The replacement keeps the ceiling.
#[test]
fn a_listed_english_word_above_the_ceiling_can_be_edited() {
    let english = |weight: i64| Entry {
        kind: Kind::English,
        key: "foo".into(),
        value: "foo".into(),
        weight,
        source: None,
    };
    assert!(validate_previous_entry(&english(20_000_001_000)).is_ok());
    // A stored row may carry 0 (a derived wubi word does); no writer stores a negative weight.
    assert!(validate_previous_entry(&english(0)).is_ok());
    assert_eq!(
        validate_previous_entry(&english(-1)).unwrap_err(),
        "invalid dictionary entry: weight is outside 1 to 100000000"
    );
    assert_eq!(
        replacement_for_engine(english(20_000_001_000))
            .err()
            .unwrap(),
        "invalid dictionary entry: weight is outside 1 to 100000000"
    );
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().to_str().unwrap();
    let request = json!({
        "options": {
            "api_version": 1,
            "resources": format!("{root}/resources"),
            "user_data": format!("{root}/user"),
            "cache": format!("{root}/cache"),
            "dictionaries": format!("{root}/dictionaries"),
            "preferences": msime_client_core::preferences::Preferences::default(),
        },
        "action": {
            "operation": "edit",
            "previous": english(20_000_001_000),
            "replacement": english(500),
            "request_id": "synthetic-edit",
        },
    });
    // With no dictionary behind these paths it fails later, on storage, not as an invalid entry.
    let refused = dictionary_request_json(&serde_json::to_vec(&request).unwrap()).unwrap_err();
    assert!(
        !refused.starts_with("invalid dictionary entry"),
        "{refused}"
    );
}

fn quick(key: &str, value: &str) -> Entry {
    Entry {
        kind: Kind::QuickPhrase,
        key: key.into(),
        value: value.into(),
        weight: 10_000,
        source: None,
    }
}

#[test]
fn a_quick_phrase_code_is_letters_only_for_new_input_but_a_stored_digit_row_stays_editable() {
    // New input follows the reference's letters-only rule, and says which rule it broke.
    assert_eq!(
        replacement_for_engine(quick("nh1", "你好")).err().unwrap(),
        "invalid dictionary entry: code contains characters this dictionary does not accept"
    );
    assert_eq!(
        replacement_for_engine(quick("NH", "你好")).unwrap().key,
        "nh"
    );
    // The previous row of an edit or delete may be a stored code with a digit; the check on it does not refuse that row.
    assert!(validate_entry(&quick("nh1", "你好")).is_ok());

    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().to_str().unwrap();
    let request = json!({
        "options": {
            "api_version": 1,
            "resources": format!("{root}/resources"),
            "user_data": format!("{root}/user"),
            "cache": format!("{root}/cache"),
            "dictionaries": format!("{root}/dictionaries"),
            "preferences": msime_client_core::preferences::Preferences::default(),
        },
        "action": {
            "operation": "edit",
            "previous": null,
            "replacement": quick("nh1", "你好"),
            "request_id": "synthetic-edit",
        },
    });
    assert_eq!(
        dictionary_request_json(&serde_json::to_vec(&request).unwrap()).unwrap_err(),
        "invalid dictionary entry: code contains characters this dictionary does not accept"
    );

    // A personal dictionary file carrying a digit code is refused, as the text import skips such a row.
    let file = r#"{"format":"msime-personal-dictionary","version":1,"entries":[{"kind":"quickPhrase","key":"nh1","value":"你好","weight":1}]}"#;
    assert_eq!(
        parse_personal_dictionary_import(file).unwrap_err(),
        "invalid personal dictionary entry"
    );

    // The validation iOS runs on every new entry follows the same rule.
    let validate = |entry: Entry| -> serde_json::Value {
        let bytes = serde_json::to_vec(&entry).unwrap();
        let pointer = unsafe { msime_client_dictionary_validate(bytes.as_ptr(), bytes.len()) };
        let value = unsafe { std::ffi::CStr::from_ptr(pointer) }
            .to_str()
            .unwrap()
            .to_owned();
        unsafe { crate::msime_client_string_free(pointer) };
        serde_json::from_str(&value).unwrap()
    };
    let refused = validate(quick("nh1", "你好"));
    assert_eq!(refused["ok"], false);
    assert_eq!(
        refused["error"],
        "invalid dictionary entry: code contains characters this dictionary does not accept"
    );
    let accepted = validate(quick("NH", "你好"));
    assert_eq!(accepted["ok"], true, "{accepted}");
    assert_eq!(accepted["value"]["key"], "nh");
}

#[test]
fn a_mobile_store_holding_a_digit_quick_phrase_still_exports_it() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().to_str().unwrap();
    let legacy = PersonalWord {
        kind: PersonalWordKind::QuickPhrase,
        key: "nh1".into(),
        value: "你好".into(),
        weight: 100_000,
    };
    PersonalDictionaryStore::new(directory.path().join("PersonalDictionary"))
        .synchronize(
            |_| Ok(()),
            |_| {
                Ok(
                    msime_client_services::dictionary::personal::PersonalWordPage {
                        entries: vec![legacy.clone()],
                        has_more: false,
                    },
                )
            },
        )
        .unwrap();
    let request = json!({
        "options": {
            "api_version": 1,
            "resources": format!("{root}/resources"),
            "user_data": format!("{root}/user"),
            "cache": format!("{root}/cache"),
            "dictionaries": format!("{root}/dictionaries"),
            "preferences": msime_client_core::preferences::Preferences::default(),
            "preferences_directory": root,
        },
        "action": {
            "operation": "export",
            "kind": "quick_phrase",
            "format": "standard",
            "offset": 0,
            "limit": 100,
        },
    });
    let exported =
        personal_dictionary_request_json(&serde_json::to_vec(&request).unwrap()).unwrap();
    assert_eq!(exported["text"], "你好\tnh1\t100000\n");
}

#[test]
fn malformed_and_oversized_requests_are_redacted() {
    for bytes in [b"invalid-fixture".as_slice(), b"{}", b"{\"options\":null}"] {
        assert_eq!(
            dictionary_request_json(bytes).unwrap_err(),
            "invalid dictionary request"
        );
    }
    assert_eq!(
        dictionary_request_json(&vec![0u8; DICTIONARY_REQUEST_LIMIT + 1]).unwrap_err(),
        "invalid dictionary buffer"
    );
}

#[test]
fn import_maps_shared_entries_onto_the_requested_engine_kind() {
    // 行的语义由 client-services 的 `dictionary::import` 完整覆盖，那里不需要 Engine 就能单测；这里只断言本模块负责的那层映射。
    let (standard, report) = parse_import(
        &Kind::Pinyin,
        "standard",
        "你好\tni'hao\t7\n# comment\n西安\txi'an\n",
        None,
    )
    .unwrap();
    assert_eq!(standard.len(), 2);
    assert_eq!(standard[0].value, "你好");
    assert_eq!(standard[0].key, "ni'hao");
    assert_eq!(standard[0].weight, 7);
    assert_eq!(standard[1].weight, 10000);
    assert!(standard
        .iter()
        .all(|entry| entry.kind == Kind::Pinyin.into()));
    assert_eq!(report.failed, 0);
    assert!(!report.truncated);

    let (windows, _) = parse_import(&Kind::Wubi, "windows", "wq\t你好\t9\n", None).unwrap();
    assert_eq!(windows[0].key, "wq");
    assert_eq!(windows[0].value, "你好");
    assert_eq!(windows[0].kind, Kind::Wubi.into());
}

#[test]
fn gboard_import_normalizes_continuous_pinyin_before_sending_it_to_engine() {
    let options = import_engine_options();
    let (entries, report) = parse_import(
        &Kind::Pinyin,
        "standard",
        "hecheng\t合成\tzh-CN\nzzzz\t测试\tzh-CN\n",
        Some(&options),
    )
    .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "he'cheng");
    assert_eq!(entries[0].value, "合成");
    assert_eq!(entries[0].weight, 10000);
    assert_eq!(report.failed, 1);
    assert_eq!(report.first_failures[0].line, 2);
    assert_eq!(
        report.first_failures[0].issue,
        msime_client_services::dictionary::import::ImportIssue::Pinyin
    );
}

#[test]
fn engine_backed_pinyin_import_resolves_lengths_and_reports_invalid_rows() {
    let options = import_engine_options();
    let (entries, report) = parse_import(
        &Kind::Pinyin,
        "standard",
        "西安\txian\n坏词\tzzzz\n你好\tnihao\n",
        Some(&options),
    )
    .unwrap();

    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.key.as_str())
            .collect::<Vec<_>>(),
        ["xi'an", "ni'hao"]
    );
    assert_eq!(report.failed, 1);
    assert_eq!(report.first_failures[0].line, 2);
    assert_eq!(
        report.first_failures[0].issue,
        msime_client_services::dictionary::import::ImportIssue::Pinyin
    );

    let (wubi, _) = parse_import(&Kind::Wubi, "windows", "wq\t你好\t9\n", Some(&options)).unwrap();
    assert_eq!(wubi[0].key, "wq");
}

#[test]
fn personal_import_accepts_the_apple_envelope_and_rejects_duplicates() {
    let text = r#"{
          "format": "msime-personal-dictionary",
          "version": 1,
          "entries": [
            {"kind":"pinyin","key":"ni hao","value":"你好","weight":100000},
            {"kind":"quickPhrase","key":"hello","value":"你好！","weight":2}
          ]
        }"#;
    let entries = parse_personal_dictionary_import(text).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "ni'hao");
    assert_eq!(entries[1].kind, PersonalWordKind::QuickPhrase);

    let duplicate = text.replace(
        r#"{"kind":"quickPhrase","key":"hello","value":"你好！","weight":2}"#,
        r#"{"kind":"pinyin","key":"ni hao","value":"你好","weight":3}"#,
    );
    assert_eq!(
        parse_personal_dictionary_import(&duplicate).unwrap_err(),
        "duplicate personal dictionary entry"
    );
}

#[test]
fn personal_import_enforces_file_and_entry_bounds() {
    let empty = r#"{"format":"msime-personal-dictionary","version":1,"entries":[]}"#;
    assert_eq!(
        parse_personal_dictionary_import(empty).unwrap_err(),
        "invalid personal dictionary entry count"
    );
    let malformed = r#"{"format":"msime-personal-dictionary","version":1,"entries":[{"kind":"pinyin","key":"nihao","value":"坏词","weight":1}]}"#;
    assert_eq!(
        parse_personal_dictionary_import(malformed).unwrap_err(),
        "invalid personal dictionary entry"
    );
    assert_eq!(
        parse_personal_dictionary_import(&"x".repeat(1_048_577)).unwrap_err(),
        "personal dictionary file is too large"
    );
}

#[test]
fn personal_import_normalizes_before_deduplicating_and_allows_multiline_quick_phrases() {
    let text = r#"{
            "format":"msime-personal-dictionary",
            "version":1,
            "entries":[
                {"kind":"pinyin","key":"NI HAO","value":"拟好","weight":100000},
                {"kind":"quickPhrase","key":"HELLO","value":"第一行\n第二行\t末列","weight":100000}
            ]
        }"#;
    let entries = parse_personal_dictionary_import(text).unwrap();
    assert_eq!(entries[0].key, "ni'hao");
    assert_eq!(entries[1].key, "hello");
    assert_eq!(entries[1].value, "第一行\n第二行\t末列");

    let large = json!({
        "format": "msime-personal-dictionary",
        "version": 1,
        "entries": [{
            "kind": "quickPhrase",
            "key": "large",
            "value": "你好\n".repeat(300),
            "weight": 100_000,
        }],
    })
    .to_string();
    assert_eq!(parse_personal_dictionary_import(&large).unwrap().len(), 1);

    let duplicate = r#"{
            "format":"msime-personal-dictionary",
            "version":1,
            "entries":[
                {"kind":"pinyin","key":"ni hao","value":"你好","weight":1},
                {"kind":"pinyin","key":"NI'HAO","value":"你好","weight":2}
            ]
        }"#;
    assert_eq!(
        parse_personal_dictionary_import(duplicate).unwrap_err(),
        "duplicate personal dictionary entry"
    );
}

#[test]
fn a_single_unusable_row_is_reported_rather_than_failing_the_import() {
    let (entries, report) = parse_import(
        &Kind::Pinyin,
        "standard",
        "你好\tni'hao\n没有制表符\n世界\tshi'jie\n",
        None,
    )
    .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(report.failed, 1);
    assert_eq!(report.first_failures[0].line, 2);
}

#[test]
fn an_unusable_envelope_is_still_rejected_outright() {
    assert!(parse_import(&Kind::Pinyin, "hans", "你好\tni'hao", None).is_err());
    assert!(parse_import(&Kind::Pinyin, "standard", "# only comments\n", None).is_err());
    // Every row unusable means nothing to import.
    assert!(parse_import(&Kind::Wubi, "windows", "abcde\t你好", None).is_err());
}

/// 五笔查码的请求文档：数据根里的工作词库带 86 和 98 两张五笔表。
fn wubi_codes_request(directory: &Path, text: &str) -> Vec<u8> {
    let root = directory.to_str().unwrap();
    for name in ["user", "resources", "dictionaries"] {
        std::fs::create_dir_all(directory.join(name)).unwrap();
    }
    for name in ["resources", "dictionaries"] {
        let path = directory.join(name).join("msime-pinyin.db");
        if path.exists() {
            continue;
        }
        rusqlite::Connection::open(path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                 INSERT INTO wubi86 VALUES('wq','你',200),('wqiy','你',100),('vbg','好',100),('wqvb','你好',300);
                 CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER);
                 INSERT INTO wubi98 VALUES('wqiy','你',100),('vbg','好',100);",
            )
            .unwrap();
    }
    serde_json::to_vec(&json!({
        "options": {
            "api_version": 1,
            "resources": format!("{root}/resources"),
            "user_data": format!("{root}/user"),
            "cache": format!("{root}/cache"),
            "dictionaries": format!("{root}/dictionaries"),
            "preferences": msime_client_core::preferences::Preferences::default(),
            "preferences_directory": root,
        },
        "action": { "operation": "wubi_codes", "text": text },
    }))
    .unwrap()
}

#[test]
fn wubi_codes_list_each_character_in_both_profiles_and_the_whole_word() {
    let directory = tempfile::tempdir().unwrap();
    let request = wubi_codes_request(directory.path(), "你好");
    let before = tree(&directory.path().join("user"));
    let expected = json!({
        "phrase": { "text": "你好", "wubi86": ["wqvb"], "wubi98": [] },
        "characters": [
            { "text": "你", "wubi86": ["wq", "wqiy"], "wubi98": ["wqiy"] },
            { "text": "好", "wubi86": ["vbg"], "wubi98": ["vbg"] },
        ],
        "truncated": false,
    });
    assert_eq!(dictionary_request_json(&request).unwrap(), expected);
    // Android 的设置页把所有词库操作都送进个人词库队列的入口；查码在那里得到同一个回答，而且不排队。
    assert_eq!(
        personal_dictionary_request_json(&request).unwrap(),
        expected
    );
    assert!(!directory.path().join("PersonalDictionary").exists());
    assert_eq!(tree(&directory.path().join("user")), before);
}

#[test]
fn wubi_codes_refuse_oversized_text_and_skip_what_is_not_han() {
    let directory = tempfile::tempdir().unwrap();
    let reply = dictionary_request_json(&wubi_codes_request(directory.path(), "a，你 b")).unwrap();
    assert_eq!(reply["phrase"], serde_json::Value::Null);
    assert_eq!(reply["characters"].as_array().unwrap().len(), 1);
    assert_eq!(reply["characters"][0]["text"], "你");

    let oversized = "你".repeat(MAX_WUBI_LOOKUP_TEXT / 3 + 1);
    let request = wubi_codes_request(directory.path(), &oversized);
    assert_eq!(
        dictionary_request_json(&request).unwrap_err(),
        "invalid wubi lookup"
    );
    assert_eq!(
        personal_dictionary_request_json(&request).unwrap_err(),
        "invalid wubi lookup"
    );
}

#[test]
fn journal_export_pages_only_the_requested_kind() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    for name in ["user", "resources", "dictionaries", "cache"] {
        std::fs::create_dir(root.join(name)).unwrap();
    }
    let mut connection = rusqlite::Connection::open(root.join("user/msime_user.db")).unwrap();
    connection.execute_batch(
        "CREATE TABLE user_dictionary_operations(dictionary TEXT NOT NULL,key TEXT NOT NULL,value TEXT NOT NULL,operation TEXT NOT NULL,weight INTEGER NOT NULL,user_inserted INTEGER NOT NULL,PRIMARY KEY(dictionary,key,value));
         INSERT INTO user_dictionary_operations VALUES
         ('english','alpha','first','upsert',11,1),
         ('english','alpha','second','upsert',12,1),
         ('english','beta','third','upsert',13,1);",
    ).unwrap();
    let transaction = connection.transaction().unwrap();
    for index in 0..1100 {
        transaction
            .execute(
                "INSERT INTO user_dictionary_operations VALUES('quick',?1,?2,'upsert',1,1)",
                rusqlite::params![format!("q{index:04}"), format!("合成短语{index:04}")],
            )
            .unwrap();
    }
    transaction.commit().unwrap();
    drop(connection);
    let options = json!({
        "api_version": 1,
        "resources": root.join("resources"),
        "user_data": root.join("user"),
        "cache": root.join("cache"),
        "dictionaries": root.join("dictionaries"),
        "preferences": msime_client_core::preferences::Preferences::default(),
    });
    let export = |kind: &str, format: &str, offset: usize, limit: usize| {
        dictionary_request_json(&serde_json::to_vec(&json!({
            "options": options,
            "action": {"operation": "export", "kind": kind, "format": format, "offset": offset, "limit": limit},
        })).unwrap())
    };
    assert_eq!(
        export("english", "standard", 0, 1).unwrap(),
        json!({"text": "first\talpha\t11\n", "has_more": true})
    );
    assert_eq!(
        export("english", "windows", 1, 1).unwrap(),
        json!({"text": "alpha\tsecond\t12\n", "has_more": true})
    );
    assert_eq!(
        export("english", "standard", 2, 1).unwrap(),
        json!({"text": "third\tbeta\t13\n", "has_more": false})
    );
    assert_eq!(
        export("english", "standard", 3, 1).unwrap(),
        json!({"text": "", "has_more": false})
    );
    assert_eq!(
        export("quick_phrase", "windows", 1099, 1).unwrap(),
        json!({"text": "q1099\t合成短语1099\t1\n", "has_more": false})
    );
    assert_eq!(
        export("wubi98", "standard", 0, 1000).unwrap(),
        json!({"text": "", "has_more": false})
    );
    for (format, offset, limit) in [
        ("bad", 0, 1),
        ("standard", 1_000_001, 1),
        ("standard", 0, 0),
        ("standard", 0, 1001),
    ] {
        assert_eq!(
            export("english", format, offset, limit).unwrap_err(),
            "invalid dictionary export"
        );
    }
}

#[test]
fn journal_list_kind_does_not_truncate_after_other_dictionaries() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    for name in ["user", "resources", "dictionaries", "cache"] {
        std::fs::create_dir(root.join(name)).unwrap();
    }
    let connection = rusqlite::Connection::open(root.join("user/msime_user.db")).unwrap();
    connection.execute_batch(
        "CREATE TABLE user_dictionary_operations(dictionary TEXT NOT NULL,key TEXT NOT NULL,value TEXT NOT NULL,operation TEXT NOT NULL,weight INTEGER NOT NULL,user_inserted INTEGER NOT NULL,PRIMARY KEY(dictionary,key,value));",
    ).unwrap();
    let transaction = connection.unchecked_transaction().unwrap();
    for index in 0..20_001 {
        transaction
            .execute(
                "INSERT INTO user_dictionary_operations VALUES('english',?1,?2,'upsert',1,1)",
                rusqlite::params![format!("english{index:05}"), format!("合成英文{index:05}")],
            )
            .unwrap();
    }
    transaction
        .execute(
            "INSERT INTO user_dictionary_operations VALUES('wubi','aaaa','合成五笔','upsert',7,1)",
            [],
        )
        .unwrap();
    transaction.commit().unwrap();
    drop(connection);
    let options = json!({
        "api_version": 1,
        "resources": root.join("resources"),
        "user_data": root.join("user"),
        "cache": root.join("cache"),
        "dictionaries": root.join("dictionaries"),
        "preferences": msime_client_core::preferences::Preferences::default(),
    });
    let response = dictionary_request_json(&serde_json::to_vec(&json!({
        "options": options,
        "action": {"operation": "list", "offset": 0, "limit": 1000, "kind": "wubi", "user_only": false},
    })).unwrap()).unwrap();
    assert_eq!(
        response,
        json!({
            "entries": [{"kind": "wubi", "key": "aaaa", "value": "合成五笔", "weight": 7, "source": "user"}],
            "has_more": false,
        })
    );
}

/// 个人粤语词按种类经 `list`、`count`、`export` 读出，编码前缀搜索不必打出音节间的空格；`browse` 的 `cantonese` 仍是发布的粤语词库（这里没装，报 `available: false`），不会读到个人词。
#[test]
fn cantonese_personal_words_list_count_and_export_while_browse_keeps_the_resource() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    for name in ["user", "resources", "dictionaries", "cache"] {
        std::fs::create_dir(root.join(name)).unwrap();
    }
    let connection = rusqlite::Connection::open(root.join("user/msime_user.db")).unwrap();
    connection.execute_batch(
        "CREATE TABLE user_dictionary_operations(dictionary TEXT NOT NULL,key TEXT NOT NULL,value TEXT NOT NULL,operation TEXT NOT NULL,weight INTEGER NOT NULL,user_inserted INTEGER NOT NULL,PRIMARY KEY(dictionary,key,value));
         INSERT INTO user_dictionary_operations VALUES
         ('cantonese','nei hou','禮好','upsert',100000,1),
         ('cantonese','gwong dung waa','廣東話','upsert',5,1),
         ('pinyin','ni''hao','你好','upsert',3,1);",
    ).unwrap();
    drop(connection);
    let options = json!({
        "api_version": 1,
        "resources": root.join("resources"),
        "user_data": root.join("user"),
        "cache": root.join("cache"),
        "dictionaries": root.join("dictionaries"),
        "preferences": msime_client_core::preferences::Preferences::default(),
    });
    let request = |action: serde_json::Value| {
        dictionary_request_json(
            &serde_json::to_vec(&json!({ "options": options, "action": action })).unwrap(),
        )
        .unwrap()
    };
    let nei_hou = json!({"kind": "cantonese", "key": "nei hou", "value": "禮好", "weight": 100000, "source": "user"});
    assert_eq!(
        request(
            json!({"operation": "list", "offset": 0, "limit": 100, "kind": "cantonese", "query": "neih"})
        ),
        json!({ "entries": [nei_hou], "has_more": false })
    );
    assert_eq!(
        request(json!({"operation": "list", "offset": 0, "limit": 100, "kind": "cantonese"})),
        json!({
            "entries": [
                {"kind": "cantonese", "key": "gwong dung waa", "value": "廣東話", "weight": 5, "source": "user"},
                nei_hou,
            ],
            "has_more": false,
        })
    );
    assert_eq!(
        request(json!({"operation": "count", "kind": "cantonese"}))["count"],
        json!(2)
    );
    assert_eq!(
        request(
            json!({"operation": "export", "kind": "cantonese", "format": "standard", "offset": 0, "limit": 1000})
        ),
        json!({"text": "廣東話\tgwong dung waa\t5\n禮好\tnei hou\t100000\n", "has_more": false})
    );
    assert_eq!(
        request(
            json!({"operation": "browse", "kind": "cantonese", "query": "", "offset": 0, "limit": 100})
        ),
        json!({ "entries": [], "has_more": false, "available": false })
    );
}

fn scel_text_reply(request: &[u8]) -> serde_json::Value {
    let pointer = unsafe { msime_client_dictionary_scel_text(request.as_ptr(), request.len()) };
    let value = unsafe { std::ffi::CStr::from_ptr(pointer) }
        .to_str()
        .unwrap()
        .to_owned();
    unsafe { crate::msime_client_string_free(pointer) };
    serde_json::from_str(&value).unwrap()
}

fn scel_text_request(bytes: &[u8]) -> Vec<u8> {
    serde_json::to_vec(&json!({ "bytes_base64": super::scel_fixture::base64(bytes) })).unwrap()
}

#[test]
fn scel_text_converts_a_cell_dictionary_to_standard_rows() {
    let bytes = super::scel_fixture::scel(&[
        (&["ce", "shi"], &["测试"]),
        (&["shi", "li"], &["实例", "事例"]),
        (&["?", "jie"], &["越界"]),
    ]);
    let reply = scel_text_reply(&scel_text_request(&bytes));
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(
        reply["value"],
        json!({
            "name": "合成词库",
            "category": "",
            "entries": 4,
            "text": "测试\tce'shi\n实例\tshi'li\n事例\tshi'li\n越界\t\n",
        })
    );
}

#[test]
fn scel_text_refusals_have_stable_codes() {
    let error = |request: &[u8]| scel_text_reply(request)["error"].clone();
    let sample = super::scel_fixture::scel(&[(&["ce", "shi"], &["测试"])]);
    let null = unsafe { msime_client_dictionary_scel_text(std::ptr::null(), 0) };
    let value = unsafe { std::ffi::CStr::from_ptr(null) }
        .to_str()
        .unwrap()
        .to_owned();
    unsafe { crate::msime_client_string_free(null) };
    assert!(value.contains("dictionary_scel_invalid"), "{value}");
    assert_eq!(
        error(&vec![b' '; SCEL_TEXT_REQUEST_LIMIT + 1]),
        "dictionary_too_large"
    );
    assert_eq!(error(b"not json"), "dictionary_scel_invalid");
    assert_eq!(
        error(br#"{"bytes_base64":"","text":""}"#),
        "dictionary_scel_invalid"
    );
    assert_eq!(
        error(br#"{"bytes_base64":"not base64!"}"#),
        "dictionary_scel_invalid"
    );
    assert_eq!(
        error(&scel_text_request("词\tci\n".as_bytes())),
        "dictionary_scel_unsupported"
    );
    assert_eq!(
        error(&scel_text_request(&sample[..sample.len() - 1])),
        "dictionary_scel_malformed"
    );
    let mut empty = sample.clone();
    empty[0x120..0x124].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(error(&scel_text_request(&empty)), "dictionary_scel_empty");
    assert_eq!(
        scel_text_json(&"A".repeat(scel_base64_limit() + 4)).unwrap_err(),
        "dictionary_too_large"
    );
}

/// iOS 的路径：转出的文本以 `standard` 交给 `msime_client_dictionary_import_entries`，队列只收 128 个词，坏词条按词序报出行号。
#[test]
#[cfg(not(target_os = "android"))]
fn scel_text_feeds_the_queued_import_with_word_order_lines() {
    let words: Vec<String> = (0..140u32)
        .map(|index| {
            [0x4e00 + index, 0x5000 + index]
                .into_iter()
                .filter_map(char::from_u32)
                .collect()
        })
        .collect();
    let mut groups: Vec<(&[&str], Vec<&str>)> = Vec::new();
    groups.push((&["?", "jie"], vec!["越界"]));
    for word in &words {
        groups.push((&["ce", "shi"], vec![word.as_str()]));
    }
    let borrowed: Vec<(&[&str], &[&str])> = groups
        .iter()
        .map(|(reading, words)| (*reading, words.as_slice()))
        .collect();
    let converted = scel_text_json(&super::scel_fixture::base64(&super::scel_fixture::scel(
        &borrowed,
    )))
    .unwrap();
    let request = serde_json::to_vec(&json!({
        "kind": "pinyin",
        "format": "standard",
        "text": converted["text"],
    }))
    .unwrap();
    let resources = tempfile::tempdir().unwrap();
    let report = import_entries_json(&request, resources.path().to_str().unwrap()).unwrap();
    assert_eq!(report["applied"], 128, "{report}");
    assert_eq!(report["entries"].as_array().unwrap().len(), 128);
    assert_eq!(report["truncated"], true);
    assert_eq!(report["swapped"], false);
    assert_eq!(report["failed"], 1);
    assert_eq!(report["first_failures"][0]["line"], 1);
    assert_eq!(report["entries"][0]["key"], "ce'shi");
    assert_eq!(report["entries"][0]["weight"], 10_000);
}

fn queued_collection_sync_fixture() -> (tempfile::TempDir, HostOptions, PersonalDictionaryStore) {
    use msime_client_services::dictionary::collections::DictionaryCollectionsStore;
    let directory = tempfile::tempdir().unwrap();
    let options = HostOptions::from_document(crate::tests::host_options_with_main_dictionary(
        directory.path(),
    ))
    .unwrap();
    let personal = PersonalDictionaryStore::new(directory.path().join("PersonalDictionary"));
    let words = (0..129)
        .map(|index: u32| PersonalWord {
            kind: msime_client_services::dictionary::personal::PersonalWordKind::Pinyin,
            key: "ni'hao".into(),
            value: format!("{}词", char::from_u32(0x4e00 + index).unwrap()),
            weight: 100,
        })
        .collect();
    let collections = DictionaryCollectionsStore::new(
        directory.path(),
        PersonalDictionaryStore::new(directory.path().join("PersonalDictionary")),
    );
    assert_eq!(collections.queue_words(words).unwrap().outbox_pending, 1);
    (directory, options, personal)
}

/// 在下一批开始时由真实输入会话抢占租约，已完成的进展仍返回给通知方。
#[test]
fn collection_sync_keeps_progress_when_next_batch_is_busy() {
    let (_directory, options, personal) = queued_collection_sync_fixture();
    let engine_options = options.clone().into_engine_options();
    let mut attempts = 0;
    let mut session = None;
    let result =
        synchronize_collections_with_access(&options, std::time::Duration::from_secs(60), || {
            attempts += 1;
            if attempts == 2 {
                session = DictionaryAccess::try_session(
                    Path::new(&engine_options.user_data),
                    Path::new(&engine_options.dictionaries),
                )
                .unwrap();
                assert!(session.is_some(), "前一批已释放维护租约");
            }
            DictionaryAccess::try_maintenance(
                Path::new(&engine_options.user_data),
                Path::new(&engine_options.dictionaries),
            )
            .map_err(|_| "dictionary access unavailable".to_owned())
        });
    let progress = result.expect("后批忙不能丢掉已应用数量");
    assert_eq!(progress["applied_count"], 128, "{progress}");
    assert_eq!(progress["pending_count"], 1, "{progress}");
    assert_eq!(progress["sent"], 1, "{progress}");
    assert_eq!(personal.read().unwrap().pending_count(), 1);
    drop(session);
    let resumed =
        synchronize_collections_with_budget(&options, std::time::Duration::from_secs(60)).unwrap();
    assert_eq!(resumed["pending_count"], 0, "{resumed}");
    assert_eq!(resumed["applied_count"], 1, "{resumed}");
}

/// 忙碌可重试，但维护租约的存储失败仍要报告，不能拿前批成功遮住它。
#[test]
fn collection_sync_does_not_hide_later_lease_storage_errors() {
    let (_directory, options, _personal) = queued_collection_sync_fixture();
    let engine_options = options.clone().into_engine_options();
    let mut attempts = 0;
    let result =
        synchronize_collections_with_access(&options, std::time::Duration::from_secs(60), || {
            attempts += 1;
            if attempts == 2 {
                return Err("dictionary access unavailable".to_owned());
            }
            DictionaryAccess::try_maintenance(
                Path::new(&engine_options.user_data),
                Path::new(&engine_options.dictionaries),
            )
            .map_err(|_| "dictionary access unavailable".to_owned())
        });
    assert_eq!(result.unwrap_err(), "dictionary access unavailable");
}
