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
#[cfg(test)]
use crate::pinyin::segment::split_segments;
use crate::pinyin::segment::{join_segments, segments_to_jianpin};
use crate::pinyin::syllables::{
    canonical_lattice_syllable, has_only_complete_pinyin_segments, intact_pinyin_set,
    prefix_pinyin_set,
};

pub const BUSY_TIMEOUT: Duration = Duration::from_millis(250);
/// Weight of a word the user or learning inserts (QD:1243).
pub const INSERTED_WEIGHT: i64 = 10_000;

/// The reference kept every prepared statement for the life of the connection (QQ:572-590). A session touches a few dozen tables with five statement shapes each plus one batch shape per key count, so the rusqlite default of 16 would re-prepare on nearly every keystroke.
const STATEMENT_CACHE_CAPACITY: usize = 512;

// 错误纠正一次最多提交 96 个键；短批次用线性扫描可以省掉临时哈希表分配。
const SMALL_QUERY_KEY_BATCH: usize = 64;

// 普通九宫格简拼最多取 64 行，小页先放栈上，较大扫描继续使用原堆页预算。
const INLINE_JIANPIN_ROWS: usize = 64;

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
        let mut rows = Vec::new();
        for extra in 1..=extra_syllables {
            let Some(table) = quanpin_table(segments.len() + extra, initial) else {
                continue;
            };
            let page = self.rows(
                &range_sql(&table, sql_limit(limit)),
                [prefix.as_str(), upper_bound.as_str()],
                query_capacity(limit),
            );
            if !page.is_empty() {
                // 短页只按实际行数预留，满页才按剩余页的上限一次性预留。
                let remaining_pages = extra_syllables - extra + 1;
                let planned = if page.len() == limit {
                    remaining_pages.saturating_mul(limit)
                } else {
                    page.len()
                };
                let required = rows
                    .len()
                    .saturating_add(planned)
                    .saturating_sub(rows.capacity());
                if required != 0 {
                    rows.reserve_exact(required);
                }
            }
            rows.extend(page);
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
        let keys_by_table = exact_segmentations_by_table(segmentations);
        let mut rows = Vec::new();
        for (index, (table, keys)) in keys_by_table.iter().enumerate() {
            let page = self.batch_rows(table, keys, limit);
            if page.is_empty() {
                continue;
            }
            if rows.is_empty() {
                // 首个非空页直接接管，不再分配并搬移一份相同的行存储。
                rows = page;
                continue;
            }
            // 满页按剩余有效表数规划，短页仅补实际行数；不可表示的提示退回本页。
            let planned = if page.len() == limit {
                (keys_by_table.len() - index)
                    .checked_mul(limit)
                    .filter(|additional| {
                        rows.len().checked_add(*additional).is_some_and(|total| {
                            total <= isize::MAX as usize / size_of::<DictRow>()
                        })
                    })
                    .unwrap_or(page.len())
            } else {
                page.len()
            };
            if planned > rows.capacity() - rows.len() {
                rows.reserve_exact(planned);
            }
            rows.extend(page);
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
        let mut result: HashMap<String, Vec<DictRow>> = HashMap::new();
        if self.connection.is_none() || keys.is_empty() || per_key_limit == 0 {
            return result;
        }
        let mut keys_by_table: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        let mut seen =
            (keys.len() > SMALL_QUERY_KEY_BATCH).then(|| HashSet::with_capacity(keys.len()));
        for (index, key) in keys.iter().enumerate() {
            if !query_key_is_new(keys, index, &mut seen) {
                continue;
            }
            let Some((table, syllables)) = complete_key_table(key) else {
                continue;
            };
            if syllables == 1 {
                let rows = self.rows(
                    &exact_sql(&table, sql_limit(per_key_limit)),
                    [key.as_str()],
                    query_capacity(per_key_limit),
                );
                if !rows.is_empty() {
                    if result.capacity() == 0 {
                        result.reserve(keys.len());
                    }
                    result.insert(key.clone(), rows);
                }
                continue;
            }
            keys_by_table.entry(table).or_default().push(key.as_str());
        }
        for (table, table_keys) in &keys_by_table {
            let rows = self.batch_rows(table, table_keys, usize::MAX);
            if !rows.is_empty() && result.capacity() == 0 {
                result.reserve(keys.len());
            }
            // 按权重读取，每个键最先出现的是它的最高权重行；已有槽只借用键查找。
            for row in rows {
                if let Some(slot) = result.get_mut(row.key.as_str()) {
                    if slot.len() < per_key_limit {
                        slot.push(row);
                    }
                } else {
                    let key = row.key.clone();
                    let mut slot =
                        query_capacity(per_key_limit).map_or_else(Vec::new, Vec::with_capacity);
                    slot.push(row);
                    result.insert(key, slot);
                }
            }
        }
        result
    }

    /// 按简拼查词：`codes` 是同样长度的简拼（每个音节的首字母，`mt`、`cflm`），一个字母一个音节，词条的 `jp` 等于其中任何一个就算。按首字母分表，每张表一条 `jp IN (...)` 语句（`jp` 有索引），合起来按权重从高到低取前 `limit` 行；同一个词按不同的码出现时只留第一行。九宫格用它把每个数字当成一个音节的声母来查（#5640）。
    pub fn query_jianpin_codes(&self, codes: &[String], limit: usize) -> Vec<DictRow> {
        let mut rows = self.query_jianpin_codes_per_table(codes, limit);
        rows.truncate(limit);
        rows
    }

    /// 同 `query_jianpin_codes`，但只截每张首字母表（各取权重最高的 `table_limit` 行），合起来按权重排、去重之后不再截断，最多是表数乘 `table_limit` 行。出货词库里大量词的权重相同（默认的 100），合起来再截时同权重的行按表的先后取舍，排在后面的表里的词会被整批截掉；九宫格要从中找出用户用过的词，所以先拿到每张表各自的前 `table_limit` 行（#6185）。
    pub fn query_jianpin_codes_per_table(
        &self,
        codes: &[String],
        table_limit: usize,
    ) -> Vec<DictRow> {
        if self.connection.is_none() || codes.is_empty() || table_limit == 0 {
            return Vec::new();
        }
        let mut codes_by_table: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        for code in codes {
            let Some(&first) = code.as_bytes().first() else {
                continue;
            };
            let Some(table) = quanpin_table(code.len(), first) else {
                continue;
            };
            let table_codes = codes_by_table.entry(table).or_default();
            if !table_codes.contains(&code.as_str()) {
                table_codes.push(code.as_str());
            }
        }
        let mut rows = Vec::new();
        for (table, table_codes) in &codes_by_table {
            let sql = jianpin_batch_sql(table, table_codes.len(), sql_limit(table_limit));
            let mut inline = [const { None }; INLINE_JIANPIN_ROWS];
            let mut inline_len = 0;
            let mut overflow = Vec::new();
            self.visit_rows(&sql, params_from_iter(table_codes), |item| {
                if inline_len < INLINE_JIANPIN_ROWS {
                    inline[inline_len] = Some(item);
                    inline_len += 1;
                } else {
                    if overflow.is_empty() {
                        overflow.reserve_exact(query_capacity(table_limit).unwrap_or(128));
                        overflow.extend(inline.iter_mut().map(|row| row.take().unwrap()));
                    }
                    overflow.push(item);
                }
            });
            if inline_len == 0 {
                continue;
            }
            if rows.capacity() == 0 {
                rows.reserve_exact(table_limit.min(128));
            }
            // 按实际页长一次追加，保持不均匀多表结果的原扩容行为。
            if overflow.is_empty() {
                rows.extend(inline.into_iter().take(inline_len).map(Option::unwrap));
            } else {
                rows.extend(overflow);
            }
        }
        rows.sort_by_key(|row| std::cmp::Reverse(row.weight));
        deduplicate_by_value(&mut rows);
        rows
    }

    /// 词网格跨度逐音节规范化后只查精确键，`gun'qi` 不会借用 `gun'qiu` 的行（QQ:1398-1418）。
    pub fn query_lattice_span(&self, span: &[String], span_limit: usize) -> Vec<DictRow> {
        if self.connection.is_none() || span.is_empty() || span_limit == 0 {
            return Vec::new();
        }
        let normalized = span
            .iter()
            .map(|syllable| canonical_lattice_syllable(syllable));
        let intact = intact_pinyin_set();
        if !normalized.clone().all(|syllable| intact.contains(syllable)) {
            return Vec::new();
        }
        let initial = normalized
            .clone()
            .next()
            .and_then(|first| first.as_bytes().first());
        let Some(table) = initial.and_then(|first| quanpin_table(span.len(), *first)) else {
            return Vec::new();
        };
        let capacity = normalized.clone().map(str::len).sum::<usize>() + span.len() - 1;
        let mut key = String::with_capacity(capacity);
        for (index, syllable) in normalized.enumerate() {
            if index > 0 {
                key.push('\'');
            }
            key.push_str(syllable);
        }
        // 唯一跨度没有分组与去重需求，直接接管 SQL 页，避免复制同一份行存储。
        let mut rows = self.batch_rows(&table, &[key.as_str()], span_limit);
        rows.sort_by_key(|row| std::cmp::Reverse(row.weight));
        rows
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

    /// 按原键的段数选表，查询 `key` 与 `value` 的首行权重（QD:484-499）。
    pub fn find_weight(&self, key: &str, value: &str) -> Option<i64> {
        let connection = self.connection.as_ref()?;
        let (table, _) = word_key_table(key)?;
        // 缺表或首步失败沿用参考实现的未找到结果（QD:490-497）。
        let mut statement = connection.prepare_cached(&find_weight_sql(&table)).ok()?;
        let mut rows = statement.query((key, value)).ok()?;
        let row = rows.next().ok()??;
        column_i64(row, 0).ok()
    }

    /// 插入原键、各非空段的首个字符、词值及 `INSERTED_WEIGHT`；调用方负责查重和键校验。
    pub fn insert_word(&self, key: &str, value: &str) -> Result<()> {
        let connection = self.writable()?;
        let (table, syllables) =
            word_key_table(key).ok_or_else(|| EngineError::invalid(INVALID_DICTIONARY_KEY))?;
        let mut jp = String::with_capacity(syllables);
        for segment in key.split('\'') {
            if let Some(initial) = segment.chars().next() {
                jp.push(initial);
            }
        }
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

    /// QQ:686-740：按键数生成 `IN (...)` 语句，`prepare_cached` 按语句文本分别缓存。
    fn batch_rows<K: AsRef<str>>(&self, table: &str, keys: &[K], limit: usize) -> Vec<DictRow> {
        if table.is_empty() || keys.is_empty() || limit == 0 {
            return Vec::new();
        }
        let sql = batch_sql(table, keys.len(), sql_limit(limit));
        self.rows(
            &sql,
            params_from_iter(keys.iter().map(AsRef::as_ref)),
            query_capacity(limit),
        )
    }

    /// 执行 `key`、`value`、`weight` 查询；缺表返回空页，步进或转换失败时保留已读取的行。
    fn rows(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
        capacity: Option<usize>,
    ) -> Vec<DictRow> {
        let mut result = Vec::new();
        self.visit_rows(sql, params, |item| {
            if let (Some(capacity), 0) = (capacity, result.capacity()) {
                result.reserve_exact(capacity);
            }
            result.push(item);
        });
        result
    }

    /// 按 SQLite 顺序交付成功转换的行，读取失败时结束本页。
    fn visit_rows(&self, sql: &str, params: impl rusqlite::Params, mut visit: impl FnMut(DictRow)) {
        let Some(connection) = &self.connection else {
            return;
        };
        let Ok(mut statement) = connection.prepare_cached(sql) else {
            return;
        };
        let Ok(mut rows) = statement.query(params) else {
            return;
        };
        while let Ok(Some(row)) = rows.next() {
            match dict_row(row) {
                Ok(item) => visit(item),
                Err(_) => break,
            }
        }
    }
}

fn open_connection(path: &Path) -> Option<Connection> {
    // An empty path is how `RuntimePaths` spells a missing file; SQLite would open a private temporary database for it instead.
    if path.as_os_str().is_empty() {
        return None;
    }
    // No CREATE: a missing dictionary must stay missing instead of becoming an empty file (QD:237-246).
    let path = crate::paths::sqlite_path_no_follow(path).ok()?;
    let connection = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NOFOLLOW
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
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

/// `batch_sql` 的简拼版：`jp IN (...)`。
fn jianpin_batch_sql(table: &str, code_count: usize, limit: i64) -> String {
    let mut sql = String::with_capacity(
        table
            .len()
            .saturating_add(code_count.saturating_mul(2))
            .saturating_add(96)
            .saturating_add(limit_len(limit)),
    );
    sql.push_str(SELECT_ROWS);
    sql.push_str(table);
    sql.push_str("\" WHERE \"jp\" IN (");
    for index in 0..code_count {
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

#[cfg(test)]
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

const SMALL_TABLE_KEY_BATCH: usize = 16;

// 小批次直接扫描已保留键；大批次保留哈希去重，避免无界查询的平方级退化。
fn deduplicate_table_keys(table_keys: &mut Vec<(String, String)>) {
    if table_keys.len() <= SMALL_TABLE_KEY_BATCH {
        let mut write = 0;
        for read in 0..table_keys.len() {
            if table_keys[..write]
                .iter()
                .any(|(_, key)| key == &table_keys[read].1)
            {
                continue;
            }
            if write != read {
                table_keys.swap(write, read);
            }
            write += 1;
        }
        table_keys.truncate(write);
        return;
    }
    let mut seen_keys = HashSet::with_capacity(table_keys.len());
    let duplicates = table_keys
        .iter()
        .enumerate()
        .filter_map(|(index, (_, key))| (!seen_keys.insert(key.as_str())).then_some(index))
        .collect::<Vec<_>>();
    drop(seen_keys);
    let mut duplicates = duplicates.into_iter().peekable();
    let mut write = 0;
    for read in 0..table_keys.len() {
        if duplicates.peek() == Some(&read) {
            duplicates.next();
            continue;
        }
        if write != read {
            table_keys.swap(write, read);
        }
        write += 1;
    }
    table_keys.truncate(write);
}

fn exact_segmentations_by_table(segmentations: &[Vec<String>]) -> BTreeMap<String, Vec<String>> {
    let mut keys_by_table: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if segmentations.len() <= SMALL_TABLE_KEY_BATCH {
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
            let keys = keys_by_table.entry(table).or_default();
            if !keys.iter().any(|existing| existing == &key) {
                keys.push(key);
            }
        }
        return keys_by_table;
    }
    let mut table_keys = Vec::with_capacity(segmentations.len());
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
        table_keys.push((table, key));
    }
    deduplicate_table_keys(&mut table_keys);
    for (table, key) in table_keys {
        keys_by_table.entry(table).or_default().push(key);
    }
    keys_by_table
}

fn query_capacity(limit: usize) -> Option<usize> {
    (limit < i32::MAX as usize).then_some(limit)
}

/// 词条入口只按段数和首字节规划，保留空段计数且不增加完整音节校验。
fn word_key_table(key: &str) -> Option<(String, usize)> {
    let initial = *key.as_bytes().first()?;
    let syllables = key.bytes().filter(|&byte| byte == b'\'').count() + 1;
    quanpin_table(syllables, initial).map(|table| (table, syllables))
}

/// 借用完整键的音节切片校验并计数，表名仍遵循共享格式规则。
fn complete_key_table(key: &str) -> Option<(String, usize)> {
    let initial = *key.as_bytes().first()?;
    let intact = intact_pinyin_set();
    let mut syllables = 0;
    for segment in key.split('\'') {
        if !intact.contains(segment) {
            return None;
        }
        syllables += 1;
    }
    quanpin_table(syllables, initial).map(|table| (table, syllables))
}

fn query_key_is_new<'a>(
    keys: &'a [String],
    index: usize,
    seen: &mut Option<HashSet<&'a str>>,
) -> bool {
    match seen {
        Some(seen) => seen.insert(keys[index].as_str()),
        None => !keys[..index]
            .iter()
            .any(|existing| existing == &keys[index]),
    }
}

/// First occurrence wins (QQ:774-780).
fn deduplicate_by_value(rows: &mut Vec<DictRow>) {
    if rows.len() < 2 {
        return;
    }
    // 长词续接每次最多收集 3 * 12 行；这个规模用已保留行线性扫描比建立哈希表更省分配。
    if rows.len() <= 36 {
        let mut write = 0;
        for read in 0..rows.len() {
            if rows[..write]
                .iter()
                .any(|row| row.value.as_str() == rows[read].value.as_str())
            {
                continue;
            }
            if write != read {
                rows.swap(write, read);
            }
            write += 1;
        }
        rows.truncate(write);
        return;
    }
    // Check duplicate values through borrowed slices, then retain in place after releasing the set.
    let mut seen = HashSet::with_capacity(rows.len());
    let duplicates = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (!seen.insert(row.value.as_str())).then_some(index))
        .collect::<Vec<_>>();
    drop(seen);
    let mut duplicates = duplicates.into_iter().peekable();
    let mut write = 0;
    for read in 0..rows.len() {
        if duplicates.peek() == Some(&read) {
            duplicates.next();
            continue;
        }
        if write != read {
            rows.swap(write, read);
        }
        write += 1;
    }
    rows.truncate(write);
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
    fn small_exact_segmentation_batches_avoid_transition_vector_allocation() {
        let segmentations = [strings(&["ni", "hao"]), strings(&["ni", "men"])];
        let (keys, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            exact_segmentations_by_table(&segmentations)
        });
        assert_eq!(keys["tbl_2_n"], ["ni'hao", "ni'men"]);
        assert!(allocations <= 7, "unexpected allocations: {allocations}");
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
    fn jianpin_batch_sql_lists_one_placeholder_per_code() {
        assert_eq!(
            jianpin_batch_sql("tbl_2_m", 3, 64),
            "SELECT \"key\", \"value\", \"weight\" FROM \"tbl_2_m\" WHERE \"jp\" IN (?,?,?) ORDER BY \"weight\" DESC LIMIT 64"
        );
    }

    /// 九宫格简拼（#5640）：同样长度的一组简拼按首字母分表查，合起来按权重排、按词去重；没有的表（i、u、v 开头）当作没有行。
    #[test]
    fn jianpin_codes_merge_their_tables_by_weight() {
        let directory = tempfile::tempdir().unwrap();
        let database = cascade_fixture(directory.path());
        let codes = strings(&["nh", "sj", "ih", "mh", "nh"]);
        assert_eq!(
            values(&database.query_jianpin_codes(&codes, 10)),
            ["你好", "时间", "拟好", "世界", "男孩"]
        );
        assert_eq!(
            values(&database.query_jianpin_codes(&codes, 2)),
            ["你好", "时间"]
        );
        // 简拼只比整码：nh 不会带出 n 开头的单字或三音节的词。
        assert_eq!(
            values(&database.query_jianpin_codes(&strings(&["n"]), 10)),
            ["你", "您", "那"]
        );
        assert!(database.query_jianpin_codes(&codes, 0).is_empty());
        assert!(database.query_jianpin_codes(&[], 10).is_empty());

        // 只截每张表：n 表的前两行和 s 表的前两行都在，合起来不再截到两行（#6185）。
        assert_eq!(
            values(&database.query_jianpin_codes_per_table(&codes, 2)),
            ["你好", "时间", "拟好", "世界"]
        );
        assert!(database.query_jianpin_codes_per_table(&codes, 0).is_empty());
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
    fn value_dedup_skips_trivial_row_lists() {
        let mut rows = vec![DictRow {
            key: "ni".to_owned(),
            value: "你".to_owned(),
            weight: 1,
        }];

        deduplicate_by_value(&mut rows);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].value, "你");
    }

    #[test]
    fn value_dedup_for_longer_phrases_does_not_allocate() {
        let mut rows = (0..36)
            .map(|index| DictRow {
                key: format!("key-{index}"),
                value: format!("value-{}", index % 18),
                weight: index,
            })
            .collect::<Vec<_>>();

        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            deduplicate_by_value(&mut rows);
        });

        assert_eq!(allocations, 0);
        assert_eq!(rows.len(), 18);
    }

    #[test]
    fn exact_segmentation_key_dedup_uses_no_temporary_heap_state_for_small_batches() {
        let mut table_keys = (0..16)
            .map(|index| ("tbl_2_n".to_owned(), format!("ni'{}", index % 8)))
            .collect::<Vec<_>>();

        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            deduplicate_table_keys(&mut table_keys);
        });

        assert_eq!(allocations, 0);
        assert_eq!(
            table_keys,
            (0..8)
                .map(|index| ("tbl_2_n".to_owned(), format!("ni'{index}")))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn exact_segmentation_key_dedup_keeps_first_rows_on_both_sides_of_the_boundary() {
        for length in [0, 1, 15, 16, 17, 64, 128] {
            let mut table_keys = (0..length)
                .map(|index| (format!("table-{index}"), format!("key-{}", index % 17)))
                .collect::<Vec<_>>();
            let expected = table_keys[..length.min(17)].to_vec();
            let pointer = table_keys.as_ptr();
            let capacity = table_keys.capacity();

            deduplicate_table_keys(&mut table_keys);

            assert_eq!(table_keys, expected, "length={length}");
            assert_eq!(table_keys.as_ptr(), pointer);
            assert_eq!(table_keys.capacity(), capacity);
        }
    }

    #[test]
    fn exact_key_query_dedup_avoids_temporary_heap_state_for_small_batches() {
        let keys = strings(&["ni", "ni", "hao", "ni"]);
        let mut seen = None;
        let mut accepted = 0;
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            for index in 0..keys.len() {
                if query_key_is_new(&keys, index, &mut seen) {
                    accepted += 1;
                }
            }
        });

        assert_eq!(allocations, 0);
        assert_eq!(accepted, 2);
    }

    #[test]
    fn exact_key_query_dedup_switches_to_hashing_after_small_batch_boundary() {
        for length in [SMALL_QUERY_KEY_BATCH, SMALL_QUERY_KEY_BATCH + 1] {
            let keys = (0..length)
                .map(|index| format!("key-{}", index % (length / 2).max(1)))
                .collect::<Vec<_>>();
            let mut seen = (length > SMALL_QUERY_KEY_BATCH).then(|| HashSet::with_capacity(length));
            let mut unique = 0;
            for index in 0..keys.len() {
                if query_key_is_new(&keys, index, &mut seen) {
                    unique += 1;
                }
            }
            assert_eq!(unique, length / 2, "length={length}");
        }
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

#[cfg(test)]
#[path = "pinyin/empty_page_tests.rs"]
mod empty_page_tests;

#[cfg(test)]
#[path = "pinyin/empty_aggregate_tests.rs"]
mod empty_aggregate_tests;

#[cfg(test)]
#[path = "pinyin/empty_key_map_tests.rs"]
mod empty_key_map_tests;

#[cfg(test)]
#[path = "pinyin/dedup_before_split_tests.rs"]
mod dedup_before_split_tests;

#[cfg(test)]
#[path = "pinyin/borrowed_key_plan_tests.rs"]
mod borrowed_key_plan_tests;

#[cfg(test)]
#[path = "pinyin/borrowed_key_groups_tests.rs"]
mod borrowed_key_groups_tests;

#[cfg(test)]
#[path = "pinyin/result_key_reuse_tests.rs"]
mod result_key_reuse_tests;

#[cfg(test)]
#[path = "pinyin/lattice_span_key_plan_tests.rs"]
mod lattice_span_key_plan_tests;

#[cfg(test)]
#[path = "pinyin/aggregate_page_reuse_tests.rs"]
mod aggregate_page_reuse_tests;

#[cfg(test)]
#[path = "pinyin/per_key_unbounded_slot_tests.rs"]
mod per_key_unbounded_slot_tests;

#[cfg(test)]
#[path = "pinyin/word_key_plan_tests.rs"]
mod word_key_plan_tests;

#[cfg(test)]
#[path = "pinyin/jianpin_inline_page_tests.rs"]
mod jianpin_inline_page_tests;
