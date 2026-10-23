use super::*;

// 固定 3a706bd021661aadcfcdf0cc477e28282c7928d8 的完整分组查询与行解码，仅改私有名称并移除注释。
impl EnglishDictionary {
    pub(crate) fn legacy_query_keypad_prefix(
        &self,
        prefix: &str,
        pattern: &str,
        typed: usize,
        limit: usize,
    ) -> Vec<WordItem> {
        let mut candidates = Vec::new();
        let Some(connection) = &self.connection else {
            return candidates;
        };
        if !is_lower_ascii_word(prefix) || pattern.is_empty() || limit == 0 {
            return candidates;
        }
        let upper_bound = prefix_upper_bound(prefix);
        let sql = format!("{KEYPAD_PREFIX_SQL}{}", sql_limit(limit));
        let Ok(mut statement) = connection.prepare_cached(&sql) else {
            return candidates;
        };
        let Ok(mut rows) = statement.query(rusqlite::params![
            prefix,
            upper_bound,
            pattern,
            sql_limit(typed)
        ]) else {
            return candidates;
        };
        frozen_read_rows(&mut rows, limit, &mut candidates);
        candidates
    }
}

fn frozen_read_rows(rows: &mut rusqlite::Rows<'_>, limit: usize, candidates: &mut Vec<WordItem>) {
    let reusable = candidates.len();
    let mut count = 0;
    loop {
        match rows.next() {
            Ok(Some(row)) => {
                let is_null = |index| matches!(row.get_ref(index), Ok(ValueRef::Null));
                if is_null(0) || is_null(1) {
                    continue;
                }
                let (Ok(word), Ok(display), Ok(weight)) = (
                    column_text_ref(row, 0),
                    column_text_ref(row, 1),
                    column_i64(row, 2),
                ) else {
                    candidates.clear();
                    return;
                };
                if count < limit {
                    if count < reusable {
                        let candidate = &mut candidates[count];
                        candidate.pinyin.clear();
                        candidate.pinyin.push_str(&word);
                        candidate.canonical_pinyin.clear();
                        candidate.word.clear();
                        candidate.word.push_str(&display);
                        candidate.weight = weight;
                        candidate.source = CandidateSource::EnglishDictionary;
                        candidate.scheme = Default::default();
                        candidate.fixed_position = 0;
                        candidate.fuzzy = false;
                        candidate.corrected_from.clear();
                        candidate.sentence_association = false;
                        candidate.sentence_words.clear();
                    } else {
                        if count == candidates.capacity() {
                            candidates.reserve_exact(limit - count);
                        }
                        candidates.push(WordItem::new(
                            word.as_ref(),
                            display.as_ref(),
                            weight,
                            CandidateSource::EnglishDictionary,
                            "",
                        ));
                    }
                    count += 1;
                }
            }
            Ok(None) => {
                candidates.truncate(count);
                return;
            }
            Err(_) => {
                candidates.clear();
                return;
            }
        }
    }
}

#[test]
fn keypad_append_matches_frozen_pages_without_changing_prior_rows() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("english.db");
    let writer = Connection::open(&path).unwrap();
    writer
        .execute_batch(
            "CREATE TABLE english_words(word,display,weight);
        INSERT INTO english_words VALUES
        ('alpha','Alpha',10),('alpha','AlpHa',10),('alphabet','Alphabet',999),
        ('alpine','共享展示',0),('alps','共享展示',-1),('alto','',5),
        ('also',NULL,9000),('alphanumeric','合成😀',2147484000),
        ('aLPha','Upper',200),('al1','Invalid',9999),('al' || char(0),'Nul',9999),
        ('als',X'41ff42','12'),('alg',7,1.9),('alf',1.5,NULL);",
        )
        .unwrap();
    let dictionary = EnglishDictionary::open(&path, None, None);
    for prefix in ["", "A", "al1", "al", "alpha", "a", "missing"] {
        for pattern in ["", "*", "[aA][lL]*", "[aA][bB]*"] {
            for limit in [0, 1, 5, 32] {
                for prior in [false, true] {
                    let page = dictionary.legacy_query_keypad_prefix(prefix, pattern, 5, limit);
                    let mut expected = Vec::new();
                    let mut actual = Vec::new();
                    if prior {
                        expected.reserve_exact(320);
                        actual.reserve_exact(320);
                        let mut sentinel =
                            WordItem::new("prior", "前页", 7, CandidateSource::Generated, "prior");
                        sentinel.sentence_words = vec!["前页".into()];
                        expected.push(sentinel.clone());
                        actual.push(sentinel);
                    }
                    for row in page {
                        if expected.capacity() == 0 {
                            expected.reserve_exact(320);
                        }
                        expected.push(row);
                    }
                    dictionary.append_keypad_prefix(prefix, pattern, 5, limit, &mut actual, 320);
                    assert_eq!(
                        actual, expected,
                        "prefix={prefix:?}, pattern={pattern:?}, limit={limit}, prior={prior}"
                    );
                    assert_eq!(actual.capacity(), expected.capacity());
                }
            }
        }
    }
}

#[test]
fn keypad_append_prepare_and_first_step_failures_keep_previous_pages() {
    let directory = tempfile::tempdir().unwrap();
    let path =
        super::super::fixtures::english_db(directory.path(), &[("alpha", "Alpha", 10)], &[], &[]);
    let dictionary = EnglishDictionary::open(&path, None, None);
    let writer = Connection::open(&path).unwrap();
    writer.execute_batch("BEGIN EXCLUSIVE").unwrap();
    for prior in [false, true] {
        let mut actual = Vec::new();
        if prior {
            actual.push(WordItem::new(
                "prior",
                "前页",
                7,
                CandidateSource::Generated,
                "prior",
            ));
        }
        let expected = actual.clone();
        let capacity = actual.capacity();
        assert!(dictionary
            .legacy_query_keypad_prefix("al", "*", 2, 5)
            .is_empty());
        dictionary.append_keypad_prefix("al", "*", 2, 5, &mut actual, 320);
        assert_eq!(actual, expected);
        assert_eq!(actual.capacity(), capacity);
    }
    writer
        .execute_batch("ROLLBACK; DROP TABLE english_words")
        .unwrap();
    let dictionary = EnglishDictionary {
        connection: Some(Connection::open(&path).unwrap()),
        ..dictionary
    };
    let mut actual = vec![WordItem::default()];
    let expected = actual.clone();
    dictionary.append_keypad_prefix("al", "*", 2, 5, &mut actual, 320);
    assert_eq!(actual, expected);
    let missing = EnglishDictionary::open(Path::new(""), None, None);
    missing.append_keypad_prefix("al", "*", 2, 5, &mut actual, 320);
    assert_eq!(actual, expected);
}

#[test]
fn append_decoder_rolls_back_partial_pages_and_preserves_zero_storage() {
    let database = Connection::open_in_memory().unwrap();
    for sql in [
        "SELECT 'alpha','Alpha',1 UNION ALL SELECT 'also','Broken',abs(-9223372036854775808)",
        "SELECT 'alpha','Alpha'",
    ] {
        for limit in [0, 1, 5] {
            for prior in [false, true] {
                let mut statement = database.prepare(sql).unwrap();
                let mut page = Vec::new();
                frozen_read_rows(&mut statement.query([]).unwrap(), limit, &mut page);
                assert!(page.is_empty());
                let mut actual = Vec::new();
                if prior {
                    actual.reserve_exact(320);
                    actual.push(WordItem::new(
                        "prior",
                        "前页",
                        7,
                        CandidateSource::Generated,
                        "prior",
                    ));
                }
                let expected = actual.clone();
                let capacity = actual.capacity();
                append_prefix_rows(&mut statement.query([]).unwrap(), limit, &mut actual, 320);
                assert_eq!(actual, expected);
                assert_eq!(actual.capacity(), capacity);
            }
        }
    }
}

#[test]
fn ordinary_decoder_reuses_and_resets_rows_like_frozen_entry() {
    let database = Connection::open_in_memory().unwrap();
    let sql = "SELECT 'alpha','Alpha',1 UNION ALL SELECT 'alpine',X'41ff42',-4 UNION ALL SELECT NULL,'Skipped',5";
    for limit in [0, 1, 5] {
        let mut original = WordItem::new(
            "long_prior_pinyin",
            "先前合成候选😀",
            88,
            CandidateSource::Generated,
            "canonical",
        );
        original.fixed_position = 2;
        original.fuzzy = true;
        original.corrected_from = "from".into();
        original.sentence_association = true;
        original.sentence_words = vec!["sentence".into()];
        let mut expected = vec![original.clone(), original.clone(), original.clone()];
        let mut actual = expected.clone();
        let old_ptr = actual.as_ptr();
        let mut statement = database.prepare(sql).unwrap();
        frozen_read_rows(&mut statement.query([]).unwrap(), limit, &mut expected);
        read_prefix_rows(&mut statement.query([]).unwrap(), limit, &mut actual);
        assert_eq!(actual, expected);
        assert_eq!(actual.capacity(), expected.capacity());
        assert_eq!(actual.as_ptr(), old_ptr);
    }
}
