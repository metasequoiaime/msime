//! Unit tests for the parent module, in their own file because the module
//! is large enough that mixing them with the implementation obscured both.
//! Same `mod tests` as before, so `use super::*` still names the parent.

use super::*;

fn import_engine_options() -> msime_engine_bridge::EngineOptions {
    msime_engine_bridge::EngineOptions {
        resources: String::new(),
        user_data: String::new(),
        cache: String::new(),
        dictionaries: String::new(),
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
        english_minimum_prefix: 2,
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
        rusqlite::Connection::open(path.join("msime.db"))
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
    let wubi = msime_engine_bridge::DictionaryEntry {
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
        let connection = rusqlite::Connection::open(path.join("msime.db")).unwrap();
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
                let mut statement = connection
                    .prepare(&format!("SELECT * FROM \"{table}\" ORDER BY rowid"))
                    .unwrap();
                let columns = statement.column_count();
                let mut query = statement.query([]).unwrap();
                while let Some(row) = query.next().unwrap() {
                    let values: Vec<rusqlite::types::Value> =
                        (0..columns).map(|index| row.get(index).unwrap()).collect();
                    rows.push(format!("{table} {values:?}"));
                }
            }
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
        msime_engine_bridge::dictionary_validate(&entry.into()).unwrap();
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
                Ok(msime_client_core::dictionary::personal::PersonalWordPage {
                    entries: vec![legacy.clone()],
                    has_more: false,
                })
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
    // Row semantics are covered exhaustively in
    // client-core::dictionary_import, which is unit-testable without the
    // Engine. This asserts only the mapping this module is responsible for.
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
        msime_client_core::dictionary::import::ImportIssue::Pinyin
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
