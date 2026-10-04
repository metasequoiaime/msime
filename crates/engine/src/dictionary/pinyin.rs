//! The pinyin tables `tbl_<n>_<initial>` (quanpin.md §9). A missing file or a missing table is an empty answer, never an error: `i`, `u` and `v` name tables that do not exist, and the reference treated a failed prepare as no rows.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use rusqlite::{params_from_iter, Connection, OpenFlags, Row};

use super::{column_i64, column_text, sql_limit, DictRow};
use crate::error::{EngineError, Result};
use crate::format::{build_table_name, quanpin_table, SHIPPED_INITIALS};
use crate::pinyin::jianpin::{
    build_key_like_pattern, build_mixed_jianpin_scan_limit, can_match_exact_key, is_pure_jianpin,
    key_prefix_upper_bound, matches_mixed_segments, needs_mixed_jianpin_query, QuerySource,
};
use crate::pinyin::segment::{join_segments, segments_to_jianpin, split_segments};
use crate::pinyin::syllables::{
    canonical_lattice_syllable, has_only_complete_pinyin_segments, intact_pinyin_set,
    prefix_pinyin_set,
};

pub const BUSY_TIMEOUT: Duration = Duration::from_millis(250);
/// Weight of a word the user or learning inserts (QD:1243).
pub const INSERTED_WEIGHT: i64 = 10_000;

/// The reference kept every prepared statement for the life of the connection (QQ:572-590). A session touches a few dozen tables with five statement shapes each plus one batch shape per key count, so the rusqlite default of 16 would re-prepare on nearly every keystroke.
const STATEMENT_CACHE_CAPACITY: usize = 512;

/// The C++ returned an error code without text for these writes (QD:1502-1536); callers map the failure to their own diagnostic.
const DICTIONARY_CLOSED: &str = "Pinyin dictionary is not open";
const INVALID_DICTIONARY_KEY: &str = "Invalid pinyin dictionary key";

pub struct PinyinDatabase {
    connection: Option<Connection>,
    data_version: Option<i64>,
}

impl PinyinDatabase {
    /// READWRITE without CREATE, busy timeout 250 ms (QD:239-246). A missing file leaves the database closed and every query empty.
    pub fn open(path: &Path) -> Self {
        let mut database = Self {
            connection: open_connection(path),
            data_version: None,
        };
        // The reference constructor records the version, so the first poll after opening already compares (QD:268).
        database.database_changed();
        database
    }

    pub fn is_open(&self) -> bool {
        self.connection.is_some()
    }

    /// Whether `PRAGMA data_version` moved since the last call, meaning another connection wrote the file and cached answers are stale (QD:1409-1432). The first call records the version and answers false.
    pub fn database_changed(&mut self) -> bool {
        let Some(connection) = &self.connection else {
            return false;
        };
        // A failed pragma leaves the recorded version alone and reports no change (QD:1415-1425).
        let Ok(mut statement) = connection.prepare_cached("PRAGMA data_version") else {
            return false;
        };
        let Ok(current) = statement.query_row((), |row| row.get::<_, i64>(0)) else {
            return false;
        };
        let changed = self
            .data_version
            .is_some_and(|recorded| recorded != current);
        self.data_version = Some(current);
        changed
    }

    /// 开一个读事务，直到 `end_read` 为止的查询共用一把共享锁和同一份快照。自动提交模式下每条语句都要自己加锁、检查热日志、解锁（`stat`/`fcntl`/`pread`），九键一次刷新几百条语句，这部分开销在手机上和查询本身相当。上次的事务没收尾（中途出错）就先结束它。
    pub fn begin_read(&self) {
        let Some(connection) = &self.connection else {
            return;
        };
        if !connection.is_autocommit() {
            // 失败时下面的 `BEGIN` 也会失败，查询照常按自动提交执行。
            let _ = connection.execute_batch("COMMIT");
        }
        let _ = connection.execute_batch("BEGIN");
    }

    /// 结束 `begin_read` 开的读事务；只读事务提交不写任何东西。
    pub fn end_read(&self) {
        let Some(connection) = &self.connection else {
            return;
        };
        if !connection.is_autocommit() {
            let _ = connection.execute_batch("COMMIT");
        }
    }

    /// 当前的 `PRAGMA data_version`，不改 `database_changed` 记下的值；词典没打开或读取失败时为 `None`。
    pub fn data_version(&self) -> Option<i64> {
        let connection = self.connection.as_ref()?;
        let mut statement = connection.prepare_cached("PRAGMA data_version").ok()?;
        statement.query_row((), |row| row.get::<_, i64>(0)).ok()
    }

    /// Runs the cascade for `n` and `ni` once so the first keystroke does not pay for statement preparation (QQ:1052-1065).
    pub fn warm_up(&self) {
        intact_pinyin_set();
        prefix_pinyin_set();
        if self.connection.is_none() {
            return;
        }
        self.query_single_cut_keyed(&["n".to_owned()], 1, QuerySource::Quanpin);
        self.query_single_cut_keyed(&["ni".to_owned()], 1, QuerySource::Quanpin);
    }

    /// The exact / prefix / mixed-jianpin / pure-jianpin cascade over one segmentation, deduplicated by value, stable-sorted by weight, truncated to `limit` (QQ:840-898, QQ:1248-1266).
    pub fn query_segments_keyed_flat(
        &self,
        segments: &[String],
        limit: usize,
        source: QuerySource,
    ) -> Vec<DictRow> {
        if self.connection.is_none() || segments.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut rows = self.query_single_cut_keyed(segments, limit, source);
        deduplicate_by_value(&mut rows);
        // The C++ used std::sort, which leaves ties unspecified; quanpin.md §5.2 fixes them to SQLite's row order with a stable sort.
        rows.sort_by_key(|row| std::cmp::Reverse(row.weight));
        rows.truncate(limit);
        rows
    }

    /// Rows of `tbl_1_<c>` whose key starts with a one-letter `prefix` (QQ:1067-1098).
    pub fn query_initial(&self, prefix: &str, limit: usize) -> Vec<DictRow> {
        let Some(&first) = prefix.as_bytes().first() else {
            return Vec::new();
        };
        if !first.is_ascii_lowercase() || limit == 0 {
            return Vec::new();
        }
        let sql = initial_sql(first, sql_limit(limit));
        // The upper bound is written out here in the reference too (QQ:1083).
        let upper_bound = key_prefix_upper_bound(prefix);
        self.rows(&sql, [prefix, upper_bound.as_str()], query_capacity(limit))
    }

    /// Whole-syllable continuations of complete `segments` over 1..=`extra_syllables` more syllables, deduplicated by value, weight-sorted, at most `limit` (QQ:1305-1343).
    pub fn query_longer_phrases(
        &self,
        segments: &[String],
        extra_syllables: usize,
        limit: usize,
    ) -> Vec<DictRow> {
        if self.connection.is_none()
            || segments.len() < 2
            || extra_syllables == 0
            || limit == 0
            || !has_only_complete_pinyin_segments(segments)
        {
            return Vec::new();
        }
        // Keys separate syllables with `'`, so the trailing `'` admits only whole-syllable continuations: ping'guo' reaches ping'guo'ji and nothing spelled differently.
        let prefix = longer_phrase_prefix(segments);
        let upper_bound = key_prefix_upper_bound(&prefix);
        let initial = segments[0].as_bytes()[0];
        let mut rows = Vec::with_capacity(extra_syllables.saturating_mul(limit));
        for extra in 1..=extra_syllables {
            let Some(table) = quanpin_table(segments.len() + extra, initial) else {
                continue;
            };
            rows.extend(self.rows(
                &range_sql(&table, sql_limit(limit)),
                [prefix.as_str(), upper_bound.as_str()],
                query_capacity(limit),
            ));
        }
        deduplicate_by_value(&mut rows);
        rows.sort_by_key(|row| std::cmp::Reverse(row.weight));
        rows.truncate(limit);
        rows
    }

    /// Exact-key batch lookup of complete segmentations, one `IN (...)` statement per table in table-name order, stable-sorted by weight; no value dedup (QQ:1268-1303).
    pub fn query_exact_segmentations_keyed_flat(
        &self,
        segmentations: &[Vec<String>],
        limit: usize,
    ) -> Vec<DictRow> {
        if self.connection.is_none() || segmentations.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut keys_by_table: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for segments in segmentations {
            if !has_only_complete_pinyin_segments(segments) {
                continue;
            }
            let Some(table) = build_table_name(segments) else {
                continue;
            };
            let key = join_segments(segments);
            if key.is_empty() {
                continue;
            }
            let table_keys = keys_by_table.entry(table).or_default();
            if !contains_table_key(table_keys, &key) {
                table_keys.push(key);
            }
        }
        let mut rows = Vec::with_capacity(segmentations.len().saturating_mul(limit));
        for (table, keys) in &keys_by_table {
            rows.extend(self.batch_rows(table, keys, limit));
        }
        rows.sort_by_key(|row| std::cmp::Reverse(row.weight));
        rows
    }

    /// Up to `per_key_limit` rows per complete key; keys without rows are absent (QQ:1356-1396).
    pub fn query_exact_keys_per_key(
        &self,
        keys: &[String],
        per_key_limit: usize,
    ) -> HashMap<String, Vec<DictRow>> {
        let mut result: HashMap<String, Vec<DictRow>> = HashMap::with_capacity(keys.len());
        if self.connection.is_none() || keys.is_empty() || per_key_limit == 0 {
            return result;
        }
        let mut keys_by_table: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut seen = HashSet::with_capacity(keys.len());
        for key in keys {
            let segments = split_segments(key);
            if !seen.insert(key.as_str()) || !has_only_complete_pinyin_segments(&segments) {
                continue;
            }
            let Some(table) = build_table_name(&segments) else {
                continue;
            };
            if segments.len() == 1 {
                let rows = self.rows(
                    &exact_sql(&table, sql_limit(per_key_limit)),
                    [key.as_str()],
                    query_capacity(per_key_limit),
                );
                if !rows.is_empty() {
                    result.insert(key.clone(), rows);
                }
                continue;
            }
            keys_by_table.entry(table).or_default().push(key.clone());
        }
        for (table, table_keys) in &keys_by_table {
            // Ordered by weight, so the first rows of each key are its best.
            for row in self.batch_rows(table, table_keys, usize::MAX) {
                let slot = result
                    .entry(row.key.clone())
                    .or_insert_with(|| Vec::with_capacity(per_key_limit));
                if slot.len() < per_key_limit {
                    slot.push(row);
                }
            }
        }
        result
    }

    /// The lattice's span lookup: each syllable canonicalised with `canonical_lattice_syllable`, then an exact-key lookup only, so `gun'qi` never borrows `gun'qiu`'s rows (QQ:1398-1418).
    pub fn query_lattice_span(&self, span: &[String], span_limit: usize) -> Vec<DictRow> {
        let normalized: Vec<String> = span
            .iter()
            .map(|syllable| canonical_lattice_syllable(syllable).to_owned())
            .collect();
        self.query_exact_segmentations_keyed_flat(&[normalized], span_limit)
    }

    /// Whether any shipped single-character table holds `han` (QQ:1017-1050).
    pub fn han_char_exists(&self, han: &str) -> bool {
        let Some(connection) = &self.connection else {
            return false;
        };
        if han.is_empty() {
            return false;
        }
        SHIPPED_INITIALS.bytes().any(|initial| {
            let Some(table) = quanpin_table(1, initial) else {
                return false;
            };
            // A table that fails to prepare is skipped (QQ:1034-1038).
            let Ok(mut statement) = connection.prepare_cached(&han_char_exists_sql(table.as_str()))
            else {
                return false;
            };
            statement.exists((han,)).unwrap_or(false)
        })
    }

    /// `SELECT weight FROM <t> WHERE key=?1 AND value=?2 LIMIT 1`, table from the key's segments (QD:484-499).
    pub fn find_weight(&self, key: &str, value: &str) -> Option<i64> {
        let connection = self.connection.as_ref()?;
        let table = build_table_name(&split_segments(key))?;
        // A missing table or a failed step is "not found" in the reference (QD:490-497).
        let mut statement = connection.prepare_cached(&find_weight_sql(&table)).ok()?;
        let mut rows = statement.query((key, value)).ok()?;
        let row = rows.next().ok()??;
        column_i64(row, 0).ok()
    }

    /// Insert `(key, jp, value, INSERTED_WEIGHT)` into the key's table; `jp` is the first letter of each syllable. The caller has checked the row is absent and validated the key.
    pub fn insert_word(&self, key: &str, value: &str) -> Result<()> {
        let connection = self.writable()?;
        let segments = split_segments(key);
        let table = build_table_name(&segments)
            .ok_or_else(|| EngineError::invalid(INVALID_DICTIONARY_KEY))?;
        let jp = segments_to_jianpin(&segments);
        connection
            .prepare_cached(&insert_word_sql(&table))?
            .execute((key, jp.as_str(), value, INSERTED_WEIGHT))?;
        Ok(())
    }

    fn writable(&self) -> Result<&Connection> {
        self.connection
            .as_ref()
            .ok_or_else(|| EngineError::failed(DICTIONARY_CLOSED))
    }

    /// QQ:840-898, the statement-cached cascade.
    fn query_single_cut_keyed(
        &self,
        segments: &[String],
        limit: usize,
        source: QuerySource,
    ) -> Vec<DictRow> {
        let Some(table) = build_table_name(segments) else {
            return Vec::new();
        };
        let key = join_segments(segments);
        let jp = segments_to_jianpin(segments);
        let bound = sql_limit(limit);

        if can_match_exact_key(segments) {
            let rows = self.rows(
                &exact_sql(&table, bound),
                [key.as_str()],
                query_capacity(limit),
            );
            if !rows.is_empty() {
                return rows;
            }
        }

        // The pattern always ends in the last segment's `%`; an inner single letter keeps its literal `%`, so that range matches nothing and the input moves on to the mixed step (QQ:854-856).
        let pattern = build_key_like_pattern(segments);
        let prefix = &pattern[..pattern.len() - 1];
        let upper_bound = key_prefix_upper_bound(prefix);
        let rows = self.rows(
            &range_sql(&table, bound),
            [prefix, upper_bound.as_str()],
            query_capacity(limit),
        );
        if !rows.is_empty() {
            return rows;
        }

        if needs_mixed_jianpin_query(segments, source) {
            let scan_limit_value = build_mixed_jianpin_scan_limit(limit);
            let scan_limit = sql_limit(scan_limit_value);
            // The scan has a minimum page of 128 rows; use it as a bounded
            // initial buffer without turning an unbounded caller limit into
            // an enormous allocation.
            let mut rows = Vec::with_capacity(limit.min(128));
            rows.extend(
                self.rows(
                    &jianpin_sql(&table, scan_limit),
                    [jp.as_str()],
                    query_capacity(scan_limit_value),
                )
                .into_iter()
                .filter(|row| matches_mixed_segments(&row.key, segments, source))
                .take(limit),
            );
            if !rows.is_empty() {
                return rows;
            }
        }

        if !is_pure_jianpin(segments) {
            return Vec::new();
        }
        self.rows(
            &jianpin_sql(&table, bound),
            [jp.as_str()],
            query_capacity(limit),
        )
    }

    /// QQ:686-740: one `IN (...)` statement per key count, which `prepare_cached` keeps apart by its text.
    fn batch_rows(&self, table: &str, keys: &[String], limit: usize) -> Vec<DictRow> {
        if table.is_empty() || keys.is_empty() || limit == 0 {
            return Vec::new();
        }
        let sql = batch_sql(table, keys.len(), sql_limit(limit));
        self.rows(&sql, params_from_iter(keys), query_capacity(limit))
    }

    /// Runs a `"key", "value", "weight"` statement. A statement that fails to prepare (the table does not exist) yields no rows, and a failed step ends the rows read so far, exactly as the reference's `while (sqlite3_step(...) == SQLITE_ROW)` loops did (QQ:584-605).
    fn rows(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
        capacity: Option<usize>,
    ) -> Vec<DictRow> {
        let Some(connection) = &self.connection else {
            return Vec::new();
        };
        let Ok(mut statement) = connection.prepare_cached(sql) else {
            return Vec::new();
        };
        let Ok(mut rows) = statement.query(params) else {
            return Vec::new();
        };
        let mut result = capacity.map_or_else(Vec::new, Vec::with_capacity);
        while let Ok(Some(row)) = rows.next() {
            match dict_row(row) {
                Ok(item) => result.push(item),
                Err(_) => break,
            }
        }
        result
    }
}

fn open_connection(path: &Path) -> Option<Connection> {
    // An empty path is how `RuntimePaths` spells a missing file; SQLite would open a private temporary database for it instead.
    if path.as_os_str().is_empty() {
        return None;
    }
    // No CREATE: a missing dictionary must stay missing instead of becoming an empty file (QD:237-246).
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    // Learning writes briefly hold the commit lock; waiting keeps a query that lands in that window from becoming an empty page.
    connection.busy_timeout(BUSY_TIMEOUT).ok()?;
    connection.set_prepared_statement_cache_capacity(STATEMENT_CACHE_CAPACITY);
    Some(connection)
}

const SELECT_ROWS: &str = "SELECT \"key\", \"value\", \"weight\" FROM \"";

/// 行数上限写成字面量，不用 `LIMIT ?`。SQLite 规划查询时会读 `LIMIT` 绑定的值，于是这样的语句每执行一次都过期、整条重新解析和规划一遍（`sqlite3Reprepare`），九键每按一键要查几百次，这部分开销和查询本身相当。重新规划时看到的正是这个值，写成字面量得到同样的计划和同样的行序；不同上限按语句文本分别缓存，上限只有少数几种。
fn push_limit(sql: &mut String, limit: i64) {
    use std::fmt::Write;
    // 写进 `String` 不会失败。
    let _ = write!(sql, "{limit}");
}

/// `push_limit` 写出的字节数，好让语句一次分配到位。
fn limit_len(limit: i64) -> usize {
    let digits = limit
        .unsigned_abs()
        .checked_ilog10()
        .map_or(1, |log| log as usize + 1);
    digits + usize::from(limit < 0)
}

fn select_rows_sql(table: &str, condition: &str, limit: i64) -> String {
    let mut sql =
        String::with_capacity(SELECT_ROWS.len() + table.len() + condition.len() + limit_len(limit));
    sql.push_str(SELECT_ROWS);
    sql.push_str(table);
    sql.push_str(condition);
    push_limit(&mut sql, limit);
    sql
}

fn exact_sql(table: &str, limit: i64) -> String {
    select_rows_sql(
        table,
        "\" WHERE \"key\" = ? ORDER BY \"weight\" DESC LIMIT ",
        limit,
    )
}

fn range_sql(table: &str, limit: i64) -> String {
    select_rows_sql(
        table,
        "\" WHERE \"key\" >= ? AND \"key\" < ? ORDER BY \"weight\" DESC LIMIT ",
        limit,
    )
}

fn longer_phrase_prefix(segments: &[String]) -> String {
    let capacity = segments.iter().map(String::len).sum::<usize>() + segments.len();
    let mut prefix = String::with_capacity(capacity);
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            prefix.push('\'');
        }
        prefix.push_str(segment);
    }
    prefix.push('\'');
    prefix
}

fn batch_sql(table: &str, key_count: usize, limit: i64) -> String {
    let mut sql = String::with_capacity(
        table
            .len()
            .saturating_add(key_count.saturating_mul(2))
            .saturating_add(96)
            .saturating_add(limit_len(limit)),
    );
    sql.push_str(SELECT_ROWS);
    sql.push_str(table);
    sql.push_str("\" WHERE \"key\" IN (");
    for index in 0..key_count {
        if index > 0 {
            sql.push(',');
        }
        sql.push('?');
    }
    sql.push_str(") ORDER BY \"weight\" DESC LIMIT ");
    push_limit(&mut sql, limit);
    sql
}

fn jianpin_sql(table: &str, limit: i64) -> String {
    select_rows_sql(
        table,
        "\" WHERE \"jp\" = ? ORDER BY \"weight\" DESC LIMIT ",
        limit,
    )
}

fn han_char_exists_sql(table: &str) -> String {
    const PREFIX: &str = "SELECT 1 FROM \"";
    const SUFFIX: &str = "\" WHERE value=?1 LIMIT 1";
    let mut sql = String::with_capacity(PREFIX.len() + table.len() + SUFFIX.len());
    sql.push_str(PREFIX);
    sql.push_str(table);
    sql.push_str(SUFFIX);
    sql
}

fn find_weight_sql(table: &str) -> String {
    const PREFIX: &str = "SELECT weight FROM \"";
    const SUFFIX: &str = "\" WHERE key=?1 AND value=?2 LIMIT 1";
    let mut sql = String::with_capacity(PREFIX.len() + table.len() + SUFFIX.len());
    sql.push_str(PREFIX);
    sql.push_str(table);
    sql.push_str(SUFFIX);
    sql
}

fn insert_word_sql(table: &str) -> String {
    const PREFIX: &str = "INSERT INTO \"";
    const SUFFIX: &str = "\" (\"key\", \"jp\", \"value\", \"weight\") VALUES (?1, ?2, ?3, ?4)";
    let mut sql = String::with_capacity(PREFIX.len() + table.len() + SUFFIX.len());
    sql.push_str(PREFIX);
    sql.push_str(table);
    sql.push_str(SUFFIX);
    sql
}

fn initial_sql(first: u8, limit: i64) -> String {
    const SUFFIX: &str = "\" WHERE \"key\" >= ?1 AND \"key\" < ?2 ORDER BY \"weight\" DESC LIMIT ";
    let mut sql = String::with_capacity(
        SELECT_ROWS.len() + "tbl_1_".len() + 1 + SUFFIX.len() + limit_len(limit),
    );
    sql.push_str(SELECT_ROWS);
    sql.push_str("tbl_1_");
    sql.push(first as char);
    sql.push_str(SUFFIX);
    push_limit(&mut sql, limit);
    sql
}

fn contains_table_key(keys: &[String], key: &str) -> bool {
    keys.iter().any(|existing| existing == key)
}

fn dict_row(row: &Row<'_>) -> rusqlite::Result<DictRow> {
    Ok(DictRow {
        key: column_text(row, 0)?,
        value: column_text(row, 1)?,
        weight: column_i64(row, 2)?,
    })
}

fn query_capacity(limit: usize) -> Option<usize> {
    (limit < i32::MAX as usize).then_some(limit)
}

/// First occurrence wins (QQ:774-780).
fn deduplicate_by_value(rows: &mut Vec<DictRow>) {
    // Check duplicate values through borrowed slices, then retain in place after releasing the set.
    let mut seen = HashSet::with_capacity(rows.len());
    let unique = rows
        .iter()
        .map(|row| seen.insert(row.value.as_str()))
        .collect::<Vec<_>>();
    drop(seen);
    let mut index = 0;
    rows.retain(|_| {
        let keep = unique[index];
        index += 1;
        keep
    });
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::dictionary::fixtures::{eval_resources, pinyin_db};

    fn strings(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[test]
    fn longer_phrase_prefix_allocates_one_result_string() {
        let prefix = longer_phrase_prefix(&strings(&["ping", "guo"]));
        assert_eq!(prefix, "ping'guo'");
        assert_eq!(prefix.capacity(), prefix.len());
    }

    fn values(rows: &[DictRow]) -> Vec<&str> {
        rows.iter().map(|row| row.value.as_str()).collect()
    }

    #[test]
    fn table_key_lookup_scans_existing_keys() {
        let keys = vec!["ni'hao".to_owned()];
        assert!(contains_table_key(&keys, "ni'hao"));
        assert!(!contains_table_key(&keys, "ni'he"));
    }

    #[test]
    fn batch_sql_writes_placeholders_without_intermediate_vectors() {
        assert_eq!(
            batch_sql("tbl_2_n", 3, 32),
            "SELECT \"key\", \"value\", \"weight\" FROM \"tbl_2_n\" WHERE \"key\" IN (?,?,?) ORDER BY \"weight\" DESC LIMIT 32"
        );
    }

    #[test]
    fn limit_len_counts_the_written_digits() {
        for limit in [0, 7, 10, 32, 128, i64::from(i32::MAX), -5] {
            let mut written = String::new();
            push_limit(&mut written, limit);
            assert_eq!(limit_len(limit), written.len(), "{limit}");
        }
    }

    #[test]
    fn exact_sql_writes_the_lookup_statement_directly() {
        let sql = exact_sql("tbl_2_n", 2147483647);
        assert_eq!(
            sql,
            "SELECT \"key\", \"value\", \"weight\" FROM \"tbl_2_n\" WHERE \"key\" = ? ORDER BY \"weight\" DESC LIMIT 2147483647"
        );
        assert_eq!(sql.capacity(), sql.len());
    }

    #[test]
    fn range_sql_writes_the_lookup_statement_directly() {
        let sql = range_sql("tbl_2_n", 32);
        assert_eq!(
            sql,
            "SELECT \"key\", \"value\", \"weight\" FROM \"tbl_2_n\" WHERE \"key\" >= ? AND \"key\" < ? ORDER BY \"weight\" DESC LIMIT 32"
        );
        assert_eq!(sql.capacity(), sql.len());
    }

    #[test]
    fn jianpin_sql_writes_the_lookup_statement_directly() {
        let sql = jianpin_sql("tbl_2_n", 128);
        assert_eq!(
            sql,
            "SELECT \"key\", \"value\", \"weight\" FROM \"tbl_2_n\" WHERE \"jp\" = ? ORDER BY \"weight\" DESC LIMIT 128"
        );
        assert_eq!(sql.capacity(), sql.len());
    }

    #[test]
    fn han_char_exists_sql_writes_the_lookup_statement_directly() {
        let sql = han_char_exists_sql("tbl_1_n");
        assert_eq!(sql, "SELECT 1 FROM \"tbl_1_n\" WHERE value=?1 LIMIT 1");
        assert_eq!(sql.capacity(), sql.len());
    }

    #[test]
    fn find_weight_sql_writes_the_lookup_statement_directly() {
        let sql = find_weight_sql("tbl_2_n");
        assert_eq!(
            sql,
            "SELECT weight FROM \"tbl_2_n\" WHERE key=?1 AND value=?2 LIMIT 1"
        );
        assert_eq!(sql.capacity(), sql.len());
    }

    #[test]
    fn insert_word_sql_writes_the_lookup_statement_directly() {
        let sql = insert_word_sql("tbl_2_n");
        assert_eq!(
            sql,
            "INSERT INTO \"tbl_2_n\" (\"key\", \"jp\", \"value\", \"weight\") VALUES (?1, ?2, ?3, ?4)"
        );
        assert_eq!(sql.capacity(), sql.len());
    }

    #[test]
    fn initial_sql_writes_the_lookup_statement_directly() {
        let sql = initial_sql(b'n', 24);
        assert_eq!(
            sql,
            "SELECT \"key\", \"value\", \"weight\" FROM \"tbl_1_n\" WHERE \"key\" >= ?1 AND \"key\" < ?2 ORDER BY \"weight\" DESC LIMIT 24"
        );
        assert_eq!(sql.capacity(), sql.len());
    }

    fn cascade_fixture(directory: &Path) -> PinyinDatabase {
        let path = pinyin_db(
            directory,
            &[
                ("tbl_1_n", "ni", "你", 100),
                ("tbl_1_n", "na", "那", 90),
                ("tbl_1_n", "nin", "您", 95),
                ("tbl_2_n", "ni'hao", "你好", 1000),
                ("tbl_2_n", "ni'hao", "拟好", 800),
                ("tbl_2_n", "ni'hao", "你好", 10),
                ("tbl_2_n", "ni'men", "你们", 900),
                ("tbl_2_n", "nan'hai", "男孩", 500),
                ("tbl_2_s", "shi'jian", "时间", 900),
                ("tbl_2_s", "shi'jie", "世界", 700),
                ("tbl_2_z", "zha'ba", "扎吧", 1),
                ("tbl_2_z", "zha'ba", "扎吧", 1),
            ],
        );
        PinyinDatabase::open(&path)
    }

    // ---- Connection, change detection and the pinyin-free lookups ----

    #[test]
    fn a_missing_file_stays_missing_and_answers_empty() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("msime-pinyin.db");
        let mut database = PinyinDatabase::open(&path);
        assert!(!database.is_open());
        assert!(!path.exists(), "opening must not create the dictionary");
        assert!(!database.database_changed());
        assert!(database.query_initial("n", 10).is_empty());
        assert!(!database.han_char_exists("你"));
        assert!(database.insert_word("ni", "你").is_err());

        let empty = PinyinDatabase::open(Path::new(""));
        assert!(!empty.is_open());
    }

    #[test]
    fn initial_lookup_ranges_over_the_single_letter_table() {
        let directory = tempfile::tempdir().unwrap();
        let database = cascade_fixture(directory.path());
        assert!(database.is_open());
        let rows = database.query_initial("n", 10);
        assert_eq!(rows.capacity(), 10);
        assert_eq!(values(&rows), ["你", "您", "那"]);
        assert_eq!(
            rows[0],
            DictRow {
                key: "ni".into(),
                value: "你".into(),
                weight: 100
            }
        );
        assert_eq!(values(&database.query_initial("n", 2)), ["你", "您"]);
        assert_eq!(values(&database.query_initial("ni", 10)), ["你", "您"]);
        // `i`, `u` and `v` name tables that do not exist; that is no rows, not an error.
        assert!(database.query_initial("i", 10).is_empty());
        assert!(database.query_initial("N", 10).is_empty());
        assert!(database.query_initial("", 10).is_empty());
        assert!(database.query_initial("n", 0).is_empty());
    }

    #[test]
    fn han_char_existence_checks_only_single_character_tables() {
        let directory = tempfile::tempdir().unwrap();
        let database = cascade_fixture(directory.path());
        assert!(database.han_char_exists("你"));
        assert!(database.han_char_exists("那"));
        assert!(!database.han_char_exists("你好"));
        assert!(!database.han_char_exists("好"));
        assert!(!database.han_char_exists(""));
    }

    #[test]
    fn data_version_moves_only_for_writes_from_other_connections() {
        let directory = tempfile::tempdir().unwrap();
        let mut database = cascade_fixture(directory.path());
        assert!(!database.database_changed());
        let other = Connection::open(directory.path().join("msime-pinyin.db")).unwrap();
        other
            .execute(
                "INSERT INTO tbl_1_n (key, jp, value, weight) VALUES ('ni', 'n', '伱', 1)",
                (),
            )
            .unwrap();
        assert!(database.database_changed());
        assert!(!database.database_changed());
    }

    #[test]
    fn weights_beyond_i32_survive() {
        let directory = tempfile::tempdir().unwrap();
        let path = pinyin_db(directory.path(), &[("tbl_1_n", "ni", "你", 23_135_851_162)]);
        let database = PinyinDatabase::open(&path);
        assert_eq!(database.query_initial("n", 1)[0].weight, 23_135_851_162);
    }

    // ---- The cascade and the lookups built on the pinyin helpers ----

    #[test]
    fn cascade_prefers_exact_then_prefix_then_mixed_then_pure_jianpin() {
        let directory = tempfile::tempdir().unwrap();
        let database = cascade_fixture(directory.path());
        let query = |segments: &[&str], limit| {
            database.query_segments_keyed_flat(&strings(segments), limit, QuerySource::Quanpin)
        };

        // Exact key, deduplicated by value with the heavier row kept.
        let exact = query(&["ni", "hao"], usize::MAX);
        assert_eq!(values(&exact), ["你好", "拟好"]);
        assert_eq!(exact[0].weight, 1000);

        // Prefix range over the last, incomplete syllable.
        assert_eq!(values(&query(&["ni", "ha"], usize::MAX)), ["你好", "拟好"]);
        assert_eq!(values(&query(&["ni", "m"], usize::MAX)), ["你们"]);

        // An inner single letter keeps its literal `%`, so the range misses and the mixed jianpin filter answers.
        assert_eq!(values(&query(&["n", "hao"], usize::MAX)), ["你好", "拟好"]);
        assert_eq!(values(&query(&["s", "jie"], usize::MAX)), ["世界"]);

        // Pure jianpin: the mixed filter already admits every row with those initials.
        assert_eq!(
            values(&query(&["n", "h"], usize::MAX)),
            ["你好", "拟好", "男孩"]
        );
        assert_eq!(values(&query(&["s", "j"], usize::MAX)), ["时间", "世界"]);

        // A single letter is a prefix range over its single-character table.
        assert_eq!(values(&query(&["n"], usize::MAX)), ["你", "您", "那"]);

        assert_eq!(values(&query(&["ni", "hao"], 1)), ["你好"]);
        assert!(query(&["ni", "hao"], 0).is_empty());
        assert!(query(&["i"], usize::MAX).is_empty());
        assert!(query(&["ni", "xyz"], usize::MAX).is_empty());
        assert!(query(&[], usize::MAX).is_empty());
        // Duplicate shipped rows are tolerated.
        assert_eq!(values(&query(&["zha", "ba"], usize::MAX)), ["扎吧"]);
    }

    #[test]
    fn mixed_jianpin_filter_reserves_the_requested_limit() {
        let directory = tempfile::tempdir().unwrap();
        let database = cascade_fixture(directory.path());
        let rows =
            database.query_single_cut_keyed(&strings(&["n", "hao"]), 1, QuerySource::Quanpin);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows.capacity(), 1);
    }

    #[test]
    fn shuangpin_treats_zh_ch_sh_as_initial_tokens() {
        let directory = tempfile::tempdir().unwrap();
        let path = pinyin_db(
            directory.path(),
            &[
                ("tbl_2_s", "shi'jian", "时间", 900),
                ("tbl_2_s", "si'jian", "四件", 800),
            ],
        );
        let database = PinyinDatabase::open(&path);
        let shuangpin = database.query_segments_keyed_flat(
            &strings(&["sh", "jian"]),
            usize::MAX,
            QuerySource::Shuangpin,
        );
        assert_eq!(values(&shuangpin), ["时间"]);
    }

    #[test]
    fn longer_phrases_continue_whole_syllables_only() {
        let directory = tempfile::tempdir().unwrap();
        let path = pinyin_db(
            directory.path(),
            &[
                ("tbl_2_p", "ping'guo", "苹果", 1000),
                ("tbl_3_p", "ping'guo'shu", "苹果树", 300),
                ("tbl_3_p", "ping'guo'yuan", "苹果园", 200),
                ("tbl_3_p", "ping'guoa'x", "错键", 900),
                ("tbl_4_p", "ping'guo'shou'ji", "苹果手机", 500),
                ("tbl_4_p", "ping'guo'shu'ye", "苹果树", 100),
                ("tbl_5_p", "ping'guo'shou'ji'ke", "苹果手机壳", 50),
            ],
        );
        let database = PinyinDatabase::open(&path);
        let segments = strings(&["ping", "guo"]);
        assert_eq!(
            values(&database.query_longer_phrases(&segments, 3, 12)),
            ["苹果手机", "苹果树", "苹果园", "苹果手机壳"]
        );
        assert_eq!(
            values(&database.query_longer_phrases(&segments, 1, 12)),
            ["苹果树", "苹果园"]
        );
        assert_eq!(
            values(&database.query_longer_phrases(&segments, 3, 2)),
            ["苹果手机", "苹果树"]
        );
        assert!(database
            .query_longer_phrases(&strings(&["ping"]), 3, 12)
            .is_empty());
        assert!(database
            .query_longer_phrases(&strings(&["ping", "g"]), 3, 12)
            .is_empty());
        assert!(database.query_longer_phrases(&segments, 0, 12).is_empty());
        assert!(database.query_longer_phrases(&segments, 3, 0).is_empty());
    }

    #[test]
    fn exact_segmentations_batch_per_table_without_value_dedup() {
        let directory = tempfile::tempdir().unwrap();
        let path = pinyin_db(
            directory.path(),
            &[
                ("tbl_1_x", "xian", "先", 500),
                ("tbl_1_x", "xian", "西安", 50),
                ("tbl_2_x", "xi'an", "西安", 800),
                ("tbl_2_x", "xi'an", "西岸", 500),
                ("tbl_2_x", "xi'ang", "嘻昂", 1),
            ],
        );
        let database = PinyinDatabase::open(&path);
        let rows = database.query_exact_segmentations_keyed_flat(
            &[
                strings(&["xian"]),
                strings(&["xi", "an"]),
                strings(&["xi", "an"]),
                strings(&["x", "an"]),
            ],
            128,
        );
        // tbl_1_x runs before tbl_2_x; the stable weight sort keeps that order among equal weights.
        let keyed: Vec<(&str, &str)> = rows
            .iter()
            .map(|row| (row.key.as_str(), row.value.as_str()))
            .collect();
        assert_eq!(
            keyed,
            [
                ("xi'an", "西安"),
                ("xian", "先"),
                ("xi'an", "西岸"),
                ("xian", "西安")
            ]
        );
        assert_eq!(
            database
                .query_exact_segmentations_keyed_flat(&[strings(&["xi", "an"])], 1)
                .len(),
            1
        );
        assert!(database
            .query_exact_segmentations_keyed_flat(&[], 128)
            .is_empty());
        assert!(database
            .query_exact_segmentations_keyed_flat(&[strings(&["xi", "an"])], 0)
            .is_empty());
    }

    #[test]
    fn per_key_lookup_caps_each_key_and_omits_misses() {
        let directory = tempfile::tempdir().unwrap();
        let database = cascade_fixture(directory.path());
        let result = database.query_exact_keys_per_key(
            &strings(&[
                "ni", "ni'hao", "ni'hao", "ni'men", "n'h", "nan'hao", "shi'jie",
            ]),
            2,
        );
        let mut keys: Vec<&str> = result.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["ni", "ni'hao", "ni'men", "shi'jie"]);
        assert_eq!(values(&result["ni"]), ["你"]);
        assert_eq!(values(&result["ni'hao"]), ["你好", "拟好"]);
        assert_eq!(values(&result["ni'men"]), ["你们"]);
        assert_eq!(values(&result["shi'jie"]), ["世界"]);
        assert!(database
            .query_exact_keys_per_key(&strings(&["ni"]), 0)
            .is_empty());
    }

    // test_pinyin.cpp:640-665: the real lookup never hands the lattice a prefix row and canonicalises the accepted ü spellings first.
    #[test]
    fn lattice_span_is_exact_and_canonicalises_umlaut_spellings() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("msime-pinyin.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE tbl_2_g (key TEXT, value TEXT, weight INTEGER);
                 CREATE TABLE tbl_1_j (key TEXT, value TEXT, weight INTEGER);
                 CREATE TABLE tbl_1_l (key TEXT, value TEXT, weight INTEGER);
                 INSERT INTO tbl_2_g VALUES ('gun''qiu', '滚球', 30000), ('gun''qi', '滚起', 12000);
                 INSERT INTO tbl_1_j VALUES ('ju', '居', 10000);
                 INSERT INTO tbl_1_l VALUES ('lve', '略', 10000);",
            )
            .unwrap();
        let database = PinyinDatabase::open(&path);
        let gun_qi = database.query_lattice_span(&strings(&["gun", "qi"]), 32);
        assert_eq!(
            gun_qi,
            [DictRow {
                key: "gun'qi".into(),
                value: "滚起".into(),
                weight: 12000
            }]
        );
        assert_eq!(
            values(&database.query_lattice_span(&strings(&["jv"]), 32)),
            ["居"]
        );
        assert_eq!(
            values(&database.query_lattice_span(&strings(&["lue"]), 32)),
            ["略"]
        );
        assert!(database
            .query_lattice_span(&strings(&["gun", "q"]), 32)
            .is_empty());
    }

    #[test]
    fn inserted_words_get_the_user_weight_and_their_jianpin() {
        let directory = tempfile::tempdir().unwrap();
        let path = pinyin_db(
            directory.path(),
            &[
                ("tbl_2_c", "ce'li", "策立", 20_000),
                ("tbl_1_n", "ni", "你", 100),
            ],
        );
        let database = PinyinDatabase::open(&path);
        assert_eq!(database.find_weight("ce'li", "测棂"), None);
        database.insert_word("ce'li", "测棂").unwrap();
        assert_eq!(database.find_weight("ce'li", "测棂"), Some(INSERTED_WEIGHT));
        let jp: String = Connection::open(&path)
            .unwrap()
            .query_row("SELECT jp FROM tbl_2_c WHERE value='测棂'", (), |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(jp, "cl");

        assert_eq!(database.find_weight("ni", "你"), Some(100));
        assert_eq!(database.find_weight("", "你"), None);
        assert_eq!(database.find_weight("ia", "你"), None);
        // No table for the key: the write fails rather than landing anywhere else.
        assert!(database.insert_word("ia", "呀").is_err());
        assert!(database.insert_word("", "呀").is_err());
    }

    #[test]
    fn warm_up_runs_on_open_and_closed_databases() {
        let directory = tempfile::tempdir().unwrap();
        cascade_fixture(directory.path()).warm_up();
        PinyinDatabase::open(&directory.path().join("absent.db")).warm_up();
    }

    #[test]
    fn real_dictionary_answers_the_common_lookups() {
        let Some(resources) = eval_resources("real_dictionary_answers_the_common_lookups") else {
            return;
        };
        let database = PinyinDatabase::open(&resources.join(crate::assets::MAIN_DICTIONARY));
        assert!(database.is_open());
        database.warm_up();
        let nihao = database.query_segments_keyed_flat(
            &strings(&["ni", "hao"]),
            usize::MAX,
            QuerySource::Quanpin,
        );
        assert_eq!(nihao[0].value, "你好");
        assert!(nihao.iter().any(|row| row.value == "拟好"));
        let lattice = database.query_lattice_span(&strings(&["ni", "hao"]), 32);
        assert!(lattice.iter().all(|row| row.key == "ni'hao"));
        assert!(database.han_char_exists("你"));
        assert!(!database.query_initial("n", 5).is_empty());
        assert!(database.find_weight("ni'hao", "你好").is_some());
    }
}
