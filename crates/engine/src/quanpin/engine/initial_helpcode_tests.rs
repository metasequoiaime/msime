use std::collections::HashMap;
use std::sync::Arc;

use super::QuanpinEngine;
use crate::helpcode::HelpcodeKeymap;
use crate::quanpin::fixture::Fixture;
use crate::quanpin::QuanpinScheme;
use crate::types::{CandidateSource, Command, QueryRequest, SchemeKey, SchemeSet, SchemeType};
use crate::{Session, SessionOptions};

fn request(text: &str) -> QueryRequest {
    let mut scheme = QuanpinScheme::new();
    for byte in text.bytes() {
        scheme.handle_key(SchemeKey::Letter(byte));
    }
    let mut request = QueryRequest {
        enable_quanpin_helpcode: true,
        ..QueryRequest::default()
    };
    scheme.build_request_into(&mut request);
    request
}

fn fixture(code: &str) -> (Fixture, HelpcodeKeymap) {
    let fixture = Fixture::new();
    let mut codes = HashMap::from([("龘".to_string(), "zz".to_string())]);
    for index in 0..40 {
        let word = char::from_u32(0x4e10 + index).unwrap().to_string();
        fixture.insert(
            if index % 2 == 0 {
                code
            } else if code == "a" {
                "an"
            } else {
                "en"
            },
            &word,
            1000 - i64::from(index),
        );
        codes.insert(
            word,
            match index % 3 {
                0 => "za",
                1 => "az",
                _ => "aa",
            }
            .to_string(),
        );
    }
    fixture.insert(code, "龘", 1);
    (fixture, HelpcodeKeymap::from_codes(codes))
}

#[test]
fn initial_helpcode_cold_rows_match_expanded_rows_for_all_modes() {
    for code in ["a", "e"] {
        let (fixture, keymap) = fixture(code);
        for (suffix, strict) in [("Z", false), ("Z", true), ("ZZ", false)] {
            let mut request = request(&format!("{code}{suffix}"));
            request.strict_quanpin_helpcode = strict;
            let mut expanded = QuanpinEngine::new(&fixture.paths);
            let base = self::request(code);
            let mut rows = expanded.query(&base, Some(&keymap));
            assert_eq!(rows.len(), 24);
            assert!(expanded.expand_initial_candidates(&base, &mut rows));
            let expected = expanded.query(&request, Some(&keymap));
            assert!(expected.iter().any(|row| row.word == "龘"));

            let mut cold = QuanpinEngine::new(&fixture.paths);
            assert_eq!(cold.query(&request, Some(&keymap)), expected);
            assert_eq!(cold.query(&request, Some(&keymap)), expected);
        }
    }
}

#[test]
fn initial_helpcode_keeps_unfiltered_base_cache_and_current_keymap() {
    let (fixture, keymap) = fixture("a");
    let mut engine = QuanpinEngine::new(&fixture.paths);
    let base = request("a");
    assert_eq!(engine.query(&base, Some(&keymap)).len(), 24);
    let filtered = engine.query(&request("aZZ"), Some(&keymap));
    assert_eq!(
        filtered
            .iter()
            .map(|row| row.word.as_str())
            .collect::<Vec<_>>(),
        ["龘"]
    );
    let ordinary = engine.query(&base, Some(&keymap));
    assert_eq!(ordinary.len(), 41);
    assert!(ordinary
        .iter()
        .any(|row| keymap.code(&row.word) == Some("aa")));
    assert!(ordinary.iter().all(|row| row.pinyin == "a"));
    assert!(ordinary.iter().any(|row| row.canonical_pinyin == "an"));
    assert!(engine.query(&request("aQQ"), Some(&keymap)).is_empty());
    assert!(engine
        .query(&request("aZZ"), Some(&HelpcodeKeymap::default()))
        .is_empty());
    assert_eq!(engine.query(&request("aZZ"), Some(&keymap)), filtered);
}

#[test]
fn initial_helpcode_handles_short_and_duplicate_reading_pages() {
    for count in [0, 1, 23, 24, 25] {
        let fixture = Fixture::new();
        for index in 0..count {
            fixture.insert(if index % 2 == 0 { "e" } else { "en" }, "甲", 1000 - index);
        }
        fixture.insert("e", "龘", 1);
        let keymap = HelpcodeKeymap::from_codes(HashMap::from([
            ("甲".to_string(), "aa".to_string()),
            ("龘".to_string(), "zz".to_string()),
        ]));
        let mut engine = QuanpinEngine::new(&fixture.paths);
        let rows = engine.query(&request("eZZ"), Some(&keymap));
        assert_eq!(
            rows.iter().map(|row| row.word.as_str()).collect::<Vec<_>>(),
            ["龘"],
            "前置行数 {count}"
        );
    }
}

#[test]
fn initial_helpcode_invalidates_full_rows_on_database_change() {
    let (fixture, keymap) = fixture("a");
    let mut engine = QuanpinEngine::new(&fixture.paths);
    let query = request("aZZ");
    assert!(engine
        .query(&query, Some(&keymap))
        .iter()
        .any(|row| row.word == "龘"));
    fixture
        .connection()
        .execute("DELETE FROM tbl_1_a WHERE value='龘'", [])
        .unwrap();
    assert!(engine.query(&query, Some(&keymap)).is_empty());
    fixture.insert("a", "龘", 2000);
    assert_eq!(
        engine.query(&query, Some(&keymap)),
        QuanpinEngine::new(&fixture.paths).query(&query, Some(&keymap))
    );
}

#[test]
fn initial_helpcode_preserves_online_rows_and_their_expanded_positions() {
    let (fixture, keymap) = fixture("a");
    let base = request("a");
    let query = request("aZ");
    let mut expected_engine = QuanpinEngine::new(&fixture.paths);
    let mut actual_engine = QuanpinEngine::new(&fixture.paths);
    for engine in [&mut expected_engine, &mut actual_engine] {
        engine.query(&base, Some(&keymap));
        assert!(engine.insert_online_words(
            &base,
            &["甲乙".to_string()],
            CandidateSource::AiSuggestion,
        ));
    }
    let mut expanded = expected_engine.query(&base, Some(&keymap));
    assert!(expected_engine.expand_initial_candidates(&base, &mut expanded));
    let expected = expected_engine.query(&query, Some(&keymap));
    assert!(expected
        .iter()
        .any(|row| row.source == CandidateSource::AiSuggestion));
    assert_eq!(actual_engine.query(&query, Some(&keymap)), expected);
    assert_eq!(actual_engine.query(&base, Some(&keymap)), expanded);
}

#[test]
fn initial_helpcode_recovers_after_base_pages_are_evicted() {
    let (fixture, keymap) = fixture("a");
    let mut engine = QuanpinEngine::new(&fixture.paths);
    let query = request("aZZ");
    let expected = engine.query(&query, Some(&keymap));
    assert!(expected.iter().any(|row| row.word == "龘"));
    for syllable in crate::pinyin::syllables::intact_pinyin_list()
        .iter()
        .filter(|syllable| !syllable.starts_with('a'))
        .take(crate::quanpin::dictionary::CACHE_CAPACITY + 16)
    {
        engine.query(&request(syllable), Some(&keymap));
    }
    assert_eq!(engine.query(&request("a"), Some(&keymap)).len(), 24);
    assert_eq!(engine.query(&query, Some(&keymap)), expected);
}

#[test]
fn initial_helpcode_session_repeats_cancels_resets_and_commits() {
    let (fixture, keymap) = fixture("a");
    let mut options = SessionOptions::new(fixture.paths.clone());
    options.enabled_schemes = SchemeSet::of(&[SchemeType::Quanpin]);
    options.helpcode = true;
    options.helpcode_table = Some(Arc::new(keymap));
    options.learning = false;
    options.personal_context = false;
    options.autocorrect_types = 0;
    options.mixed_candidates = false;
    let mut session = Session::new(options).unwrap();
    for reset in [false, true] {
        if reset {
            session.reset_cache();
        }
        for byte in b"aZZ" {
            session.character(*byte, false);
        }
        let state = session.snapshot();
        assert_eq!(state.candidates.len(), 1);
        assert_eq!(state.candidates[0].word, "龘");
        assert_eq!(state.candidates[0].source, CandidateSource::Database);
        session.command(Command::Backspace);
        assert!(session
            .snapshot()
            .candidates
            .iter()
            .any(|row| row.word == "龘"));
        session.command(Command::Cancel);
    }
    for byte in b"aZZ" {
        session.character(*byte, false);
    }
    assert_eq!(session.select(0).commit.as_deref(), Some("龘"));
    assert!(session.snapshot().preedit.is_empty());
}
