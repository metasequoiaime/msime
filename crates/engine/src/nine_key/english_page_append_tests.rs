use super::*;

use crate::ime::personal_rerank::allocations::count;

// 固定 3a706bd021661aadcfcdf0cc477e28282c7928d8 的完整英文候选入口，仅改名称和私有查询名，移除注释。
impl NineKeySession {
    fn legacy_english_candidates(&mut self, unanswered: bool) -> Vec<WordItem> {
        if !self.grid.spells_english() {
            return Vec::new();
        }
        if self.english_only {
            if self.digits.is_empty() {
                return Vec::new();
            }
        } else if unanswered {
            if self.digits.is_empty() || !self.locked.is_empty() {
                return Vec::new();
            }
        } else if !self.english_options.mixed_candidates
            || !self.locked.is_empty()
            || self.digits.len() < self.english_options.minimum_prefix
        {
            return Vec::new();
        }
        let grid = self.grid;
        if self.open_english().is_none() {
            return Vec::new();
        }
        let Some(english) = self.english.as_ref() else {
            return Vec::new();
        };
        let digits = self.digits.as_str();
        self.english_pattern.clear();
        for digit in digits.bytes() {
            let letters = grid.letters(digit);
            if letters.is_empty() {
                return Vec::new();
            }
            self.english_pattern.push('[');
            self.english_pattern.push_str(letters);
            for letter in letters.bytes() {
                self.english_pattern
                    .push(char::from(letter.to_ascii_uppercase()));
            }
            self.english_pattern.push(']');
        }
        self.english_pattern.push('*');
        let prefixes = letter_prefixes(grid, digits, ENGLISH_PREFIX_BUDGET);
        let capacity = prefixes.len().saturating_mul(ENGLISH_LIMIT);
        let mut words = Vec::new();
        for prefix in prefixes {
            for word in english.legacy_query_keypad_prefix(
                &prefix,
                &self.english_pattern,
                digits.len(),
                ENGLISH_LIMIT,
            ) {
                if words.capacity() == 0 {
                    words.reserve_exact(capacity);
                }
                words.push(word);
            }
        }
        deduplicate_english_words(&mut words);
        rank_english(&mut words, digits.len());
        words
    }
}

fn english_db(root: &std::path::Path, words: &[(&str, &str, i64)]) {
    let database = rusqlite::Connection::open(root.join(assets::ENGLISH_DICTIONARY)).unwrap();
    database
        .execute_batch("CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER)")
        .unwrap();
    for row in words {
        database
            .execute("INSERT INTO english_words VALUES(?1,?2,?3)", *row)
            .unwrap();
    }
}

fn session(root: &std::path::Path) -> NineKeySession {
    let paths = RuntimePaths {
        resources: root.into(),
        user_data: root.into(),
        cache: root.into(),
        dictionaries: root.into(),
    };
    let mut session = NineKeySession::new(
        &paths,
        false,
        FrequencyAdjustmentOptions::default(),
        FuzzyPinyinOptions::default(),
        EnglishInputOptions::default(),
        true,
        false,
    );
    session.english_only = true;
    session.digits = "65".into();
    session
}

#[test]
fn english_page_append_removes_each_successful_page_container() {
    let directory = tempfile::tempdir().unwrap();
    english_db(
        directory.path(),
        &[
            ("ok", "OK", 900),
            ("old", "Old", 1000),
            ("older", "Older", 800),
        ],
    );
    let mut before = session(directory.path());
    let mut after = session(directory.path());
    before.legacy_english_candidates(false);
    after.english_candidates(false);
    let (expected, old) = count(|| before.legacy_english_candidates(false));
    let (actual, new) = count(|| after.english_candidates(false));
    assert_eq!(actual, expected);
    assert_eq!(actual.capacity(), expected.capacity());
    eprintln!("ENGLISH_PAGE_ALLOC old={old} new={new} successful_pages=2");
    assert_eq!(new + 2, old, "两个非空成功页应各少申请一个临时容器");
}

#[test]
fn english_page_append_matches_complete_frozen_english_entry() {
    let directory = tempfile::tempdir().unwrap();
    english_db(
        directory.path(),
        &[
            ("ok", "OK", 900),
            ("old", "Old", 1000),
            ("older", "Older", 800),
            ("oh", "共享展示", 200),
            ("of", "共享展示", 900),
            ("ogham", "ExactZero", 0),
            ("dont", "don't", 800),
            ("donu", "共享展示", 800),
            ("fort", "同长合成😀", -1),
            ("hello", "Hello", 99),
            ("helloa", "HelloA", 9000),
            ("qq", "QQ", 500),
        ],
    );
    for grid in [KeyGrid::NineKey, KeyGrid::FourteenKey] {
        for word in ["", "ok", "old", "dont", "hello", "qq", "zzzzzz"] {
            for english_only in [false, true] {
                for unanswered in [false, true] {
                    for mixed in [false, true] {
                        for locked in [false, true] {
                            let mut before = session(directory.path());
                            let mut after = session(directory.path());
                            for session in [&mut before, &mut after] {
                                session.grid = grid;
                                session.digits = grid.encode(word);
                                session.english_only = english_only;
                                session.english_options.mixed_candidates = mixed;
                                session.english_options.minimum_prefix = 3;
                                if locked {
                                    session.locked.push("ni".into());
                                }
                            }
                            for _ in 0..2 {
                                let expected = before.legacy_english_candidates(unanswered);
                                let actual = after.english_candidates(unanswered);
                                assert_eq!(actual, expected, "grid={grid:?}, word={word}, only={english_only}, unanswered={unanswered}, mixed={mixed}, locked={locked}");
                                assert_eq!(actual.capacity(), expected.capacity());
                                assert_eq!(after.english_pattern, before.english_pattern);
                            }
                        }
                    }
                }
            }
        }
    }
    for digits in ["1", "65'", "😀", "000", "999999"] {
        let mut before = session(directory.path());
        let mut after = session(directory.path());
        before.digits = digits.into();
        after.digits = digits.into();
        assert_eq!(
            after.english_candidates(false),
            before.legacy_english_candidates(false)
        );
    }
}

#[test]
fn english_page_append_allocation_saving_tracks_nonempty_prefixes() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join(assets::ENGLISH_DICTIONARY);
    english_db(directory.path(), &[]);
    let writer = rusqlite::Connection::open(database_path).unwrap();
    for first in b'a'..=b'c' {
        for second in b'a'..=b'c' {
            for third in b'a'..=b'c' {
                let word = String::from_utf8(vec![first, second, third]).unwrap();
                writer
                    .execute("INSERT INTO english_words VALUES(?1,?1,1)", [&word])
                    .unwrap();
            }
        }
    }
    for digits in ["2", "22", "222", "2222", "9"] {
        let mut before = session(directory.path());
        let mut after = session(directory.path());
        before.digits = digits.into();
        after.digits = digits.into();
        before.legacy_english_candidates(false);
        after.english_candidates(false);
        let english = before.english.as_ref().unwrap();
        let pages = letter_prefixes(KeyGrid::NineKey, digits, ENGLISH_PREFIX_BUDGET)
            .iter()
            .filter(|prefix| {
                !english
                    .legacy_query_keypad_prefix(
                        prefix,
                        &before.english_pattern,
                        digits.len(),
                        ENGLISH_LIMIT,
                    )
                    .is_empty()
            })
            .count();
        let (expected, old) = count(|| before.legacy_english_candidates(false));
        let (actual, new) = count(|| after.english_candidates(false));
        assert_eq!(actual, expected);
        assert_eq!(actual.capacity(), expected.capacity());
        assert_eq!(new + pages, old, "digits={digits}, pages={pages}");
        eprintln!(
            "ENGLISH_PAGE_MATRIX digits={digits} old={old} new={new} successful_pages={pages}"
        );
    }
}

#[test]
#[ignore = "需要通过 MSIME_TEST_ENGLISH_DICTIONARY 指定已校验的发布英文词库"]
fn english_page_append_matches_frozen_entry_on_shipped_dictionary() {
    let source =
        std::env::var_os("MSIME_TEST_ENGLISH_DICTIONARY").expect("指定已校验的发布英文词库");
    let directory = tempfile::tempdir().unwrap();
    std::fs::copy(source, directory.path().join(assets::ENGLISH_DICTIONARY)).unwrap();
    let mut comparisons = 0;
    for grid in [KeyGrid::NineKey, KeyGrid::FourteenKey] {
        for word in [
            "github", "preview", "commit", "clash", "hello", "world", "ok", "zzzzzz",
        ] {
            for length in 1..=word.len() {
                let mut before = session(directory.path());
                let mut after = session(directory.path());
                for session in [&mut before, &mut after] {
                    session.grid = grid;
                    session.digits = grid.encode(&word[..length]);
                }
                for _ in 0..2 {
                    let expected = before.legacy_english_candidates(false);
                    let actual = after.english_candidates(false);
                    assert_eq!(actual, expected, "grid={grid:?}, length={length}");
                    assert_eq!(actual.capacity(), expected.capacity());
                    comparisons += 1;
                }
            }
        }
    }
    eprintln!("SHIPPED_ENGLISH_COMPARISONS={comparisons}");
}
