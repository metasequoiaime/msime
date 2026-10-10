use super::*;
use crate::dictionary::fixtures::english_db;

#[test]
fn missing_prefix_keeps_no_candidate_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = english_db(directory.path(), &[("alpha", "Alpha", 10)], &[], &[]);
    let dictionary = EnglishDictionary::open(&path, None, None);
    let result = dictionary.query_prefix("missing", 1000);
    assert!(result.is_empty());
    assert_eq!(result.capacity(), 0);
}

// 冻结 4a3cbde0f52d6fc5b63c3e6450468bb8d7e9ea9e 的完整查询正文，仅翻译注释。
fn original_query(dictionary: &EnglishDictionary, prefix: &str, limit: usize) -> Vec<WordItem> {
    let Some(connection) = &dictionary.connection else {
        return Vec::new();
    };
    if !is_lower_ascii_word(prefix) || limit == 0 {
        return Vec::new();
    }
    let upper_bound = prefix_upper_bound(prefix);
    let Ok(mut statement) = connection.prepare_cached(&prefix_sql(sql_limit(limit))) else {
        return Vec::new();
    };
    let Ok(mut rows) = statement.query([prefix, upper_bound.as_str()]) else {
        return Vec::new();
    };
    let mut candidates = Vec::with_capacity(limit);
    loop {
        match rows.next() {
            Ok(Some(row)) => {
                // 跳过 NULL 词或展示，保持原查询正文。
                let is_null = |index| matches!(row.get_ref(index), Ok(ValueRef::Null));
                if is_null(0) || is_null(1) {
                    continue;
                }
                let (Ok(word), Ok(display), Ok(weight)) =
                    (column_text(row, 0), column_text(row, 1), column_i64(row, 2))
                else {
                    return Vec::new();
                };
                candidates.push(WordItem::new(
                    word,
                    display,
                    weight,
                    CandidateSource::EnglishDictionary,
                    "",
                ));
            }
            Ok(None) => return candidates,
            // 步进失败丢弃部分结果。
            Err(_) => return Vec::new(),
        }
    }
}

use crate::ime::personal_rerank::allocations::{count, measure};

fn compare_hot(dictionary: &EnglishDictionary, prefix: &str, limit: usize, saved: usize) {
    // 两侧预热同一 SQL；热区间只比较次数，不把缓存键替换的净差当堆峰值。
    drop(original_query(dictionary, prefix, limit));
    drop(dictionary.query_prefix(prefix, limit));
    let (old, old_count) = count(|| original_query(dictionary, prefix, limit));
    let (new, new_count) = count(|| dictionary.query_prefix(prefix, limit));
    assert_eq!(new, old, "prefix={prefix:?}, limit={limit}");
    assert_eq!(
        new_count + saved,
        old_count,
        "prefix={prefix:?}, limit={limit}"
    );
    if !new.is_empty() {
        assert_eq!(new.capacity(), old.capacity());
    } else {
        assert_eq!(new.capacity(), 0);
    }
}

#[test]
fn hits_preserve_every_field_order_capacity_and_allocation_count() {
    let directory = tempfile::tempdir().unwrap();
    let path = english_db(
        directory.path(),
        &[
            ("alpha", "Alpha", -2),
            ("alpha", "Alpha alternate", -3),
            ("alphabet", "Alphabet", i64::from(i32::MAX) + 100),
            ("alpine", "Alpine", 20),
            ("alps", "Alps B", 20),
            ("alps", "Alps A", 20),
            ("aloft", "Aloft", 20),
        ],
        &[],
        &[],
    );
    let dictionary = EnglishDictionary::open(&path, None, None);
    for prefix in ["alpha", "al", "alps"] {
        for limit in [1, 2, 5, 1000] {
            compare_hot(&dictionary, prefix, limit, 0);
        }
    }
    let result = dictionary.query_prefix("al", 1000);
    assert_eq!(
        result
            .iter()
            .map(|item| item.word.as_str())
            .collect::<Vec<_>>(),
        [
            "Alphabet",
            "Alps A",
            "Alps B",
            "Aloft",
            "Alpine",
            "Alpha",
            "Alpha alternate"
        ]
    );
    assert_eq!(dictionary.query_prefix("alpha", 1)[0].word, "Alpha");
    for limit in [1, 5, 1000] {
        compare_hot(&dictionary, "missing", limit, 1);
    }
}

#[test]
fn null_rows_keep_sql_limit_and_allocate_only_for_valid_rows() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nullable.db");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);
         INSERT INTO english_words VALUES('alpha',NULL,100),('alphabet','Alphabet',20),
         ('alpine','Alpine',NULL),('nullword',NULL,1),(NULL,'Ignored',1);",
        )
        .unwrap();
    drop(connection);
    let dictionary = EnglishDictionary::open(&path, None, None);
    compare_hot(&dictionary, "alpha", 1, 1);
    assert!(dictionary.query_prefix("alpha", 1).is_empty());
    compare_hot(&dictionary, "alpha", 2, 0);
    assert_eq!(dictionary.query_prefix("alpha", 2)[0].word, "Alphabet");
    compare_hot(&dictionary, "al", 1000, 0);
    assert_eq!(dictionary.query_prefix("alpine", 5)[0].weight, 0);
    compare_hot(&dictionary, "nullword", 1000, 1);
}

#[test]
fn invalid_missing_and_prepare_failure_do_not_reach_the_reservation() {
    let dictionary = EnglishDictionary::open(Path::new(""), None, None);
    compare_hot(&dictionary, "alpha", 1000, 0);
    let directory = tempfile::tempdir().unwrap();
    let path = english_db(directory.path(), &[("alpha", "Alpha", 10)], &[], &[]);
    let dictionary = EnglishDictionary::open(&path, None, None);
    for prefix in ["", "Alpha", "al1", "合成"] {
        compare_hot(&dictionary, prefix, 1000, 0);
    }
    compare_hot(&dictionary, "alpha", 0, 0);
    Connection::open(&path)
        .unwrap()
        .execute_batch("DROP TABLE english_words")
        .unwrap();
    // 新连接已知表不存在，确保是 prepare 失败；旧连接的陈旧 schema 可延迟到步进失败。
    let dictionary = EnglishDictionary {
        connection: Some(Connection::open(&path).unwrap()),
        ..dictionary
    };
    assert!(dictionary
        .connection
        .as_ref()
        .unwrap()
        .prepare_cached(&prefix_sql(1000))
        .is_err());
    compare_hot(&dictionary, "alpha", 1000, 0);
}

#[test]
fn full_pages_keep_a_single_candidate_reservation() {
    let directory = tempfile::tempdir().unwrap();
    let path = english_db(directory.path(), &[], &[], &[]);
    let mut connection = Connection::open(&path).unwrap();
    let transaction = connection.transaction().unwrap();
    for index in 0..1024 {
        let suffix = [
            b'a' + (index / 676) as u8,
            b'a' + ((index / 26) % 26) as u8,
            b'a' + (index % 26) as u8,
        ];
        let word = format!("al{}", std::str::from_utf8(&suffix).unwrap());
        transaction
            .execute(
                "INSERT INTO english_words VALUES(?1,?1,?2)",
                (&word, index as i64),
            )
            .unwrap();
    }
    transaction.commit().unwrap();
    drop(connection);
    let dictionary = EnglishDictionary::open(&path, None, None);
    for limit in [1, 5, 1000] {
        compare_hot(&dictionary, "al", limit, 0);
        assert_eq!(dictionary.query_prefix("al", limit).len(), limit);
    }
}

#[test]
fn first_step_failure_saves_the_unused_reservation() {
    let directory = tempfile::tempdir().unwrap();
    let path = english_db(directory.path(), &[("alpha", "Alpha", 10)], &[], &[]);
    let dictionary = EnglishDictionary::open(&path, None, None);
    compare_hot(&dictionary, "alpha", 1000, 0);
    let writer = Connection::open(&path).unwrap();
    writer.execute_batch("BEGIN EXCLUSIVE").unwrap();
    // 确认失败发生在 rows.next，原路径已到达候选预留点。
    {
        let mut statement = dictionary
            .connection
            .as_ref()
            .unwrap()
            .prepare_cached(&prefix_sql(1000))
            .unwrap();
        let mut rows = statement.query(["alpha", "alpha{"]).unwrap();
        assert!(rows.next().is_err());
    }
    compare_hot(&dictionary, "alpha", 1000, 1);
    writer.execute_batch("ROLLBACK").unwrap();
    compare_hot(&dictionary, "alpha", 1000, 0);
}

#[test]
fn cold_empty_query_removes_limit_sized_peak_and_returned_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = english_db(directory.path(), &[("alpha", "Alpha", 10)], &[], &[]);
    // fixture、路径借用留在区间外；字典与缓存全部在区间内创建和释放。
    let cold = |limit, original| {
        measure(|| {
            let dictionary = EnglishDictionary::open(&path, None, None);
            let result = if original {
                original_query(&dictionary, "missing", limit)
            } else {
                dictionary.query_prefix("missing", limit)
            };
            drop(dictionary);
            result
        })
    };
    let (old, old_heap) = cold(1000, true);
    let (new, new_heap) = cold(1000, false);
    assert_eq!(new, old);
    assert_eq!(new.capacity(), 0);
    assert_eq!(old_heap.minimum_bytes, 0);
    assert_eq!(new_heap.minimum_bytes, 0);
    assert_eq!(new_heap.remaining_bytes, 0);
    assert_eq!(
        old_heap.remaining_bytes,
        (old.capacity() * size_of::<WordItem>()) as i128
    );
    assert_eq!(new_heap.allocations + 1, old_heap.allocations);
    assert!(old_heap.peak_bytes > new_heap.peak_bytes);
    let (_, larger_heap) = cold(9999, false);
    assert_eq!(larger_heap.minimum_bytes, 0);
    assert_eq!(larger_heap.remaining_bytes, 0);
    assert_eq!(larger_heap.peak_bytes, new_heap.peak_bytes);
    eprintln!(
        "WordItem={} old={old_heap:?} new={new_heap:?}",
        size_of::<WordItem>()
    );
}
