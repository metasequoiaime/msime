//! 五笔码表的 provider（`R/providers/wubi_candidate_provider.cpp`，含 wubi_prefix_learning overlay）：按 `WubiProfileKind` 读 `wubi86` 或 `wubi98`。Exact code first, then weight, no value dedup, at most 50 rows. The provider only reads: learning and removal of a wubi row go through `session`, which journals them with the wubi kind and then resets this cache.

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use lru::LruCache;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::dictionary::pinyin::BUSY_TIMEOUT;
use crate::types::{CandidateSource, QueryRequest, SchemeType, WordItem, WubiProfileKind};

const QUERY_LIMIT: i64 = 50;
const REVERSE_CACHE_CAPACITY: usize = 1024;
const REVERSE_QUERY_SQL_86: &str = "SELECT \"key\" FROM wubi86 WHERE \"value\" = ?1 ORDER BY length(\"key\") DESC, \"weight\" DESC, \"key\" ASC, rowid ASC LIMIT 1";
const REVERSE_QUERY_SQL_98: &str = "SELECT \"key\" FROM wubi98 WHERE \"value\" = ?1 ORDER BY length(\"key\") DESC, \"weight\" DESC, \"key\" ASC, rowid ASC LIMIT 1";

/// A prefix query: the typed code's own rows lead, then every longer code it prefixes by weight. The same word reached through several codes (工 at a, aaa and aaaa) is kept once per code, because ranking and removal act on the code the row arrived with.
const QUERY_SQL_86: &str = "SELECT \"key\", \"value\", \"weight\" FROM wubi86 WHERE \"key\" >= ?1 AND \"key\" < ?2 ORDER BY (\"key\" = ?1) DESC, \"weight\" DESC, \"key\" ASC, rowid ASC LIMIT ?3";
/// 与 `QUERY_SQL_86` 相同，只是读 `wubi98`。
const QUERY_SQL_98: &str = "SELECT \"key\", \"value\", \"weight\" FROM wubi98 WHERE \"key\" >= ?1 AND \"key\" < ?2 ORDER BY (\"key\" = ?1) DESC, \"weight\" DESC, \"key\" ASC, rowid ASC LIMIT ?3";

/// 反查按 `"value"` 找行，而发布词库只有 `("key","value")` 的唯一约束和 `("key","weight")` 索引，没有索引时每次反查都要扫整张表。每张五笔表配一个只按词条建的索引，由 [`ensure_reverse_indexes`] 在可写的代次副本里补上。
const REVERSE_INDEXES: [(&str, &str, &str); 2] = [
    (
        "wubi86",
        "idx_wubi86_value",
        "CREATE INDEX IF NOT EXISTS idx_wubi86_value ON wubi86(\"value\")",
    ),
    (
        "wubi98",
        "idx_wubi98_value",
        "CREATE INDEX IF NOT EXISTS idx_wubi98_value ON wubi98(\"value\")",
    ),
];

/// 给库里已有的五笔表补上反查索引；按表名判断而不是按文件名，所以词库无论拆成几个文件都能调用。表不存在就跳过，索引已在时只读一次 schema、不开写事务，可以每次准备代次都调用。
pub(crate) fn ensure_reverse_indexes(connection: &Connection) -> rusqlite::Result<()> {
    for (table, index, create) in REVERSE_INDEXES {
        let (has_table, has_index): (bool, bool) = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1), EXISTS(SELECT 1 FROM sqlite_master WHERE type='index' AND name=?2)",
            (table, index),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if has_table && !has_index {
            connection.execute_batch(create)?;
        }
    }
    Ok(())
}

fn query_sql(profile: WubiProfileKind) -> &'static str {
    match profile {
        WubiProfileKind::Wubi86 => QUERY_SQL_86,
        WubiProfileKind::Wubi98 => QUERY_SQL_98,
    }
}

pub struct WubiProvider {
    main_db: PathBuf,
    profile: WubiProfileKind,
    connection: Option<Connection>,
    reverse_cache: LruCache<String, Option<String>>,
}

impl WubiProvider {
    /// Opens lazily, read-only, on the first query.
    pub fn new(main_db: &Path) -> Self {
        Self {
            main_db: main_db.to_path_buf(),
            profile: WubiProfileKind::Wubi86,
            connection: None,
            reverse_cache: LruCache::new(NonZeroUsize::new(REVERSE_CACHE_CAPACITY).unwrap()),
        }
    }

    /// 切换码表版本；下一次查询起读新表。
    pub fn set_profile(&mut self, profile: WubiProfileKind) {
        self.profile = profile;
        self.reverse_cache.clear();
    }

    /// `SELECT "key","value","weight" FROM wubi86 WHERE "key" >= ?1 AND "key" < ?2 ORDER BY ("key" = ?1) DESC, "weight" DESC, "key" ASC, rowid ASC LIMIT ?3`, `?2` = the code with its last letter incremented and `?3` = 50. Rows carry `scheme = Wubi`. Any SQLite failure is an empty answer, as in the reference.
    pub fn query(&mut self, request: &QueryRequest) -> Vec<WordItem> {
        if !request.valid
            || request.scheme != SchemeType::Wubi
            || request.normalized_input.is_empty()
        {
            return Vec::new();
        }
        let profile = self.profile;
        let Some(connection) = self.connection() else {
            return Vec::new();
        };
        match query_rows(connection, profile, &request.normalized_input) {
            Ok(rows) => rows,
            Err(_) => {
                // The reference dropped a statement that failed and prepared it again on the next key; closing gives the same retry.
                self.connection = None;
                Vec::new()
            }
        }
    }

    /// Closes the connection, so the next query sees what learning or removal wrote since.
    pub fn reset_cache(&mut self) {
        self.connection = None;
        self.reverse_cache.clear();
    }

    /// 返回词条的完整五笔 86 编码。反查只服务于候选展示，不改变候选排序或选择身份；同一词条的多个编码优先取完整编码、再取词库权重最高的一条。
    pub fn reverse_code(&mut self, word: &str) -> Option<String> {
        if word.is_empty() {
            return None;
        }
        if let Some(code) = self.reverse_cache.get(word) {
            return code.clone();
        }
        let profile = self.profile;
        let code = self
            .connection()
            .and_then(|connection| reverse_code(connection, profile, word).ok().flatten());
        self.reverse_cache.put(word.to_owned(), code.clone());
        code
    }

    fn connection(&mut self) -> Option<&Connection> {
        if self.connection.is_none() {
            self.connection = open_read_only(&self.main_db);
        }
        self.connection.as_ref()
    }
}

fn open_read_only(path: &Path) -> Option<Connection> {
    // An empty path is how `RuntimePaths` spells a missing file; SQLite would open a private temporary database for it instead.
    if path.as_os_str().is_empty() {
        return None;
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    // Another session's learning write briefly holds the commit lock; waiting keeps a keystroke that lands in that window from showing an empty list.
    connection.busy_timeout(BUSY_TIMEOUT).ok()?;
    Some(connection)
}

/// The exclusive upper bound of every code `code` prefixes: its last letter incremented, so `ab` gives `ac` and `az` gives `a{`. Codes are ASCII letters, where that is the reference's last-byte increment.
fn prefix_upper_bound(code: &str) -> String {
    let mut upper = code.to_owned();
    if let Some(last) = upper.pop() {
        upper.push(char::from_u32(u32::from(last) + 1).unwrap_or(char::MAX));
    }
    upper
}

fn query_rows(
    connection: &Connection,
    profile: WubiProfileKind,
    code: &str,
) -> rusqlite::Result<Vec<WordItem>> {
    let mut statement = connection.prepare_cached(query_sql(profile))?;
    let upper = prefix_upper_bound(code);
    let mut rows = statement.query((code, upper.as_str(), QUERY_LIMIT))?;
    let mut candidates = Vec::with_capacity(QUERY_LIMIT as usize);
    while let Some(row) = rows.next()? {
        // The reference skipped rows whose key or value read back as NULL.
        let (ValueRef::Text(key), ValueRef::Text(value)) = (row.get_ref(0)?, row.get_ref(1)?)
        else {
            continue;
        };
        let key = String::from_utf8_lossy(key).into_owned();
        let value = String::from_utf8_lossy(value).into_owned();
        let weight = match row.get_ref(2)? {
            ValueRef::Integer(weight) => weight,
            ValueRef::Real(weight) => weight as i64,
            ValueRef::Null | ValueRef::Text(_) | ValueRef::Blob(_) => 0,
        };
        let mut item = WordItem::new(key, value, weight, CandidateSource::Database, "");
        item.scheme = SchemeType::Wubi;
        candidates.push(item);
    }
    Ok(candidates)
}

fn reverse_code(
    connection: &Connection,
    profile: WubiProfileKind,
    word: &str,
) -> rusqlite::Result<Option<String>> {
    // 每次刷新会对几十到几百个候选逐个反查；有了索引后单次查询只要几微秒，重复解析 SQL 反而成了大头，所以复用已准备的语句。
    connection
        .prepare_cached(match profile {
            WubiProfileKind::Wubi86 => REVERSE_QUERY_SQL_86,
            WubiProfileKind::Wubi98 => REVERSE_QUERY_SQL_98,
        })?
        .query_row([word], |row| row.get(0))
        .optional()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        _root: tempfile::TempDir,
        provider: WubiProvider,
    }

    fn fixture(sql: &str) -> Fixture {
        let root = tempfile::tempdir().expect("temporary directory");
        let main_db = root.path().join("msime-pinyin.db");
        Connection::open(&main_db)
            .expect("fixture database")
            .execute_batch(sql)
            .expect("fixture SQL");
        let provider = WubiProvider::new(&main_db);
        Fixture {
            _root: root,
            provider,
        }
    }

    fn request(code: &str) -> QueryRequest {
        QueryRequest {
            scheme: SchemeType::Wubi,
            raw_input: code.to_owned(),
            raw_input_with_cases: code.to_owned(),
            normalized_input: code.to_owned(),
            valid: !code.is_empty(),
            ..QueryRequest::default()
        }
    }

    fn rows(provider: &mut WubiProvider, code: &str) -> Vec<(String, String, i64)> {
        provider
            .query(&request(code))
            .into_iter()
            .map(|item| (item.pinyin, item.word, item.weight))
            .collect()
    }

    fn row(code: &str, word: &str, weight: i64) -> (String, String, i64) {
        (code.to_owned(), word.to_owned(), weight)
    }

    // test_wubi_input_session.cpp:68-73. The pinned assertions there are stale (tests-inventory §2.1); these follow the overlay order the golden `wubi_prefix_codes` records.
    const SESSION_FIXTURE: &str = "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
        INSERT INTO wubi86 VALUES('w','人',20);\
        INSERT INTO wubi86 VALUES('wq','你',10);\
        INSERT INTO wubi86 VALUES('wqb','爷',20);\
        INSERT INTO wubi86 VALUES('wqi','你',10);\
        INSERT INTO wubi86 VALUES('wqbb','父子',30);";

    #[test]
    fn the_profile_picks_the_table() {
        let mut fixture = fixture(
            "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
             INSERT INTO wubi86 VALUES('kl','号',10);\
             CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER);\
             INSERT INTO wubi98 VALUES('kg','号',10);",
        );
        let provider = &mut fixture.provider;
        assert_eq!(rows(provider, "kl"), vec![row("kl", "号", 10)]);
        assert!(rows(provider, "kg").is_empty());
        provider.set_profile(WubiProfileKind::Wubi98);
        assert_eq!(rows(provider, "kg"), vec![row("kg", "号", 10)]);
        assert!(rows(provider, "kl").is_empty());
    }

    #[test]
    fn exact_code_leads_then_weight_without_word_dedup() {
        let mut fixture = fixture(SESSION_FIXTURE);
        let provider = &mut fixture.provider;
        assert_eq!(
            rows(provider, "wq"),
            vec![
                row("wq", "你", 10),
                row("wqbb", "父子", 30),
                row("wqb", "爷", 20),
                row("wqi", "你", 10),
            ]
        );
        assert_eq!(
            rows(provider, "w"),
            vec![
                row("w", "人", 20),
                row("wqbb", "父子", 30),
                row("wqb", "爷", 20),
                row("wq", "你", 10),
                row("wqi", "你", 10),
            ]
        );
        assert_eq!(rows(provider, "wqbb"), vec![row("wqbb", "父子", 30)]);
        assert!(rows(provider, "wx").is_empty());
    }

    #[test]
    fn rows_are_tagged_wubi_with_an_empty_canonical_key() {
        let mut fixture = fixture(SESSION_FIXTURE);
        let items = fixture.provider.query(&request("w"));
        assert!(items.iter().all(|item| item.scheme == SchemeType::Wubi
            && item.source == CandidateSource::Database
            && item.canonical_pinyin.is_empty()));
    }

    // engine-bridge tests.rs:191: 55 `b..` rows reach the host as exactly 50.
    #[test]
    fn prefix_query_is_bounded() {
        let mut sql = String::from("CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);");
        for index in 0..55 {
            sql.push_str(&format!(
                "INSERT INTO wubi86 VALUES('b{}','合成{index:02}',{});",
                char::from(b'a' + (index % 25) as u8),
                1_000 - index
            ));
        }
        let mut fixture = fixture(&sql);
        let words: Vec<String> = fixture
            .provider
            .query(&request("b"))
            .into_iter()
            .map(|item| item.word)
            .collect();
        let expected: Vec<String> = (0..QUERY_LIMIT)
            .map(|index| format!("合成{index:02}"))
            .collect();
        assert_eq!(words, expected);
    }

    #[test]
    fn last_letter_bound_covers_z_codes() {
        let mut fixture = fixture(
            "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
             INSERT INTO wubi86 VALUES('yz','甲',1);\
             INSERT INTO wubi86 VALUES('yzz','乙',2);\
             INSERT INTO wubi86 VALUES('z','丙',3);",
        );
        assert_eq!(prefix_upper_bound("az"), "a{");
        assert_eq!(
            rows(&mut fixture.provider, "yz"),
            vec![row("yz", "甲", 1), row("yzz", "乙", 2)]
        );
    }

    #[test]
    fn null_rows_are_skipped_and_null_weight_reads_zero() {
        let mut fixture = fixture(
            "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
             INSERT INTO wubi86 VALUES('a',NULL,5);\
             INSERT INTO wubi86 VALUES('a','工',NULL);",
        );
        assert_eq!(rows(&mut fixture.provider, "a"), vec![row("a", "工", 0)]);
    }

    #[test]
    fn missing_table_or_invalid_request_is_empty() {
        let mut other = fixture("CREATE TABLE other(x);");
        assert!(rows(&mut other.provider, "a").is_empty());

        let mut wubi = fixture(SESSION_FIXTURE);
        assert!(rows(&mut wubi.provider, "").is_empty());
        let mut quanpin = request("w");
        quanpin.scheme = SchemeType::Quanpin;
        assert!(wubi.provider.query(&quanpin).is_empty());
    }

    #[test]
    fn missing_database_is_empty() {
        let root = tempfile::tempdir().expect("temporary directory");
        let mut provider = WubiProvider::new(&root.path().join("absent.db"));
        assert!(provider.query(&request("a")).is_empty());
        assert!(!root.path().join("absent.db").exists());
    }

    #[test]
    fn reset_cache_sees_rows_written_since() {
        let mut fixture = fixture(SESSION_FIXTURE);
        assert_eq!(rows(&mut fixture.provider, "wqbb").len(), 1);
        Connection::open(&fixture.provider.main_db)
            .expect("writer")
            .execute("UPDATE wubi86 SET weight=99 WHERE key='wqbb'", ())
            .expect("update");
        fixture.provider.reset_cache();
        assert_eq!(
            rows(&mut fixture.provider, "wqbb"),
            vec![row("wqbb", "父子", 99)]
        );
    }

    fn reverse_plan(connection: &Connection, profile: WubiProfileKind) -> String {
        let sql = match profile {
            WubiProfileKind::Wubi86 => REVERSE_QUERY_SQL_86,
            WubiProfileKind::Wubi98 => REVERSE_QUERY_SQL_98,
        };
        let mut statement = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap();
        let details: Vec<String> = statement
            .query_map(["你好"], |row| row.get(3))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        details.join("; ")
    }

    #[test]
    fn reverse_indexes_are_created_once_for_the_tables_present() {
        let both = fixture(
            "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));\
             CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));",
        );
        let connection = Connection::open(&both.provider.main_db).unwrap();
        assert!(reverse_plan(&connection, WubiProfileKind::Wubi86).contains("SCAN wubi86"));
        ensure_reverse_indexes(&connection).unwrap();
        ensure_reverse_indexes(&connection).unwrap();
        let indexes: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='index' AND name IN ('idx_wubi86_value','idx_wubi98_value')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(indexes, 2);
        assert!(reverse_plan(&connection, WubiProfileKind::Wubi86)
            .contains("USING INDEX idx_wubi86_value"));
        assert!(reverse_plan(&connection, WubiProfileKind::Wubi98)
            .contains("USING INDEX idx_wubi98_value"));

        // 没有五笔表的库（例如 english.db）原样不动。
        let other = fixture("CREATE TABLE english_words(word TEXT);");
        let connection = Connection::open(&other.provider.main_db).unwrap();
        ensure_reverse_indexes(&connection).unwrap();
        let tables: i64 = connection
            .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))
            .unwrap();
        assert_eq!(tables, 1);
    }

    #[test]
    fn the_reverse_index_does_not_change_which_code_is_shown() {
        // 同一词条的多个编码：完整编码优先，同长度按权重，再按编码；索引只改变查法，不改变答案。
        let sql = "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));\
             INSERT INTO wubi86 VALUES('a','工',900);\
             INSERT INTO wubi86 VALUES('aaaa','工',10);\
             INSERT INTO wubi86 VALUES('aaab','工',20);\
             INSERT INTO wubi86 VALUES('wqvb','你好',300);\
             INSERT INTO wubi86 VALUES('wqvc','你好',300);\
             INSERT INTO wubi86 VALUES('wq','你',10);";
        let words = ["工", "你好", "你", "缺"];
        let mut plain = fixture(sql);
        let before: Vec<Option<String>> = words
            .iter()
            .map(|word| plain.provider.reverse_code(word))
            .collect();
        assert_eq!(
            before,
            [
                Some("aaab".to_owned()),
                Some("wqvb".to_owned()),
                Some("wq".to_owned()),
                None
            ]
        );
        let mut indexed = fixture(sql);
        ensure_reverse_indexes(&Connection::open(&indexed.provider.main_db).unwrap()).unwrap();
        let after: Vec<Option<String>> = words
            .iter()
            .map(|word| indexed.provider.reverse_code(word))
            .collect();
        assert_eq!(after, before);
    }

    #[test]
    fn reverse_cache_does_not_grow_without_bound() {
        let mut fixture = fixture("CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);");
        for index in 0..=1024 {
            let word = format!("合成{index:04}");
            assert_eq!(fixture.provider.reverse_code(&word), None);
        }
        assert!(fixture.provider.reverse_cache.len() <= 1024);
    }
}
