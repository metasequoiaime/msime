//! `E` and `M` modes and the mixed emoji / kaomoji rows (emoji_query.cpp:59-140, kaomoji_query.cpp). Shuangpin also queries the code normalised to quanpin.
//!
//! 混排（26 键和九宫格）用 `query_mixed_*` / `query_*_readings`：行带上目录里的关键词，混排按关键词把它接在描绘的那个候选词后面。九宫格的数字串没有唯一的拼音，按几种可能的读法依次查。

use std::borrow::Cow;
use std::path::Path;

use rusqlite::Connection;

use super::database::{lock, open_local_database};
use super::LocalQueryResult;
use crate::diagnostics;
use crate::shuangpin::query::normalize_input;
use crate::shuangpin::ShuangpinProfile;
use crate::types::{CandidateSource, SchemeType, WordItem};

pub const MODE_RESULT_LIMIT: usize = 10;
/// 混排时每组（emoji、颜文字）最多留几行接不上任何候选词的行，它们排在列表末尾。
pub const MIXED_RESULT_LIMIT: usize = 3;
/// 混排时每种读法最多取几行：编码与读法完全相同的排在前面，再按目录顺序。行要靠关键词接到候选词后面，取得太少会漏掉候选里那个词的 emoji（`ji` 有两百多行，🐔 按目录顺序排在九十几位），取得太多又拖慢每次按键。
pub const MIXED_FETCH_PER_READING: usize = 16;
/// 混排时每组最多取几行，各种读法合计。不超过 64，混排去重用一个 `u64` 记每组的去留。
pub const MIXED_FETCH_LIMIT: usize = 48;

/// 混排的一行 emoji 或颜文字，带着它在目录里的关键词（`emoji.keywords` / `kaomoji_catalog.keywords`，空格分隔，中文词在前）。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExpressiveRow {
    pub item: WordItem,
    pub keywords: String,
}

/// The one difference between the two files of the reference: the table, the source and the diagnostics.
struct Catalog {
    sql: &'static str,
    /// 混排用的语句：取回文字、命中的两列编码（emoji 只有 `key`，第二列为空）和目录里的关键词；编码与读法完全相同的在前，再按目录顺序。不带 `LIMIT`，由调用方核对编码后取够为止。
    mixed_sql: &'static str,
    /// 资源里没有目录表（`emoji` / `kaomoji_catalog`）时退回的语句：同样的行，关键词为空，混排时都接不上候选词，排在末尾。
    mixed_sql_without_keywords: &'static str,
    source: CandidateSource,
    unavailable: &'static str,
    query_failed: &'static str,
}

const EMOJI: Catalog = Catalog {
    sql: "SELECT emoji,sort_order FROM emoji_pinyin WHERE key>=?1 AND key<?2 ORDER BY sort_order LIMIT ?3",
    mixed_sql: "SELECT p.emoji,p.key,'',COALESCE(e.keywords,'') FROM emoji_pinyin p LEFT JOIN emoji e ON e.emoji=p.emoji WHERE p.key>=?1 AND p.key<?2 ORDER BY p.key<>?1,p.sort_order",
    mixed_sql_without_keywords: "SELECT emoji,key,'','' FROM emoji_pinyin WHERE key>=?1 AND key<?2 ORDER BY key<>?1,sort_order",
    source: CandidateSource::Emoji,
    unavailable: diagnostics::EMOJI_UNAVAILABLE,
    query_failed: diagnostics::EMOJI_QUERY_FAILED,
};

const KAOMOJI: Catalog = Catalog {
    sql: "SELECT kaomoji,sort_order FROM kaomoji WHERE (pinyin>=?1 AND pinyin<?2) OR (jianpin>=?1 AND jianpin<?2) ORDER BY sort_order LIMIT ?3",
    mixed_sql: "SELECT k.kaomoji,k.pinyin,k.jianpin,COALESCE(c.keywords,'') FROM kaomoji k LEFT JOIN kaomoji_catalog c ON c.kaomoji=k.kaomoji WHERE (k.pinyin>=?1 AND k.pinyin<?2) OR (k.jianpin>=?1 AND k.jianpin<?2) ORDER BY k.pinyin<>?1 AND k.jianpin<>?1,k.sort_order",
    mixed_sql_without_keywords: "SELECT kaomoji,pinyin,jianpin,'' FROM kaomoji WHERE (pinyin>=?1 AND pinyin<?2) OR (jianpin>=?1 AND jianpin<?2) ORDER BY pinyin<>?1 AND jianpin<>?1,sort_order",
    source: CandidateSource::Kaomoji,
    unavailable: diagnostics::KAOMOJI_UNAVAILABLE,
    query_failed: diagnostics::KAOMOJI_QUERY_FAILED,
};

struct QueryPrefixes<'a> {
    lower: &'a str,
    normalized: Option<String>,
}

impl<'a> QueryPrefixes<'a> {
    fn new(lower: &'a str, scheme: SchemeType, profile: &ShuangpinProfile) -> Self {
        let normalized = if scheme == SchemeType::Shuangpin {
            let quanpin = normalize_input(lower, profile);
            (!quanpin.is_empty() && quanpin != lower).then_some(quanpin)
        } else {
            None
        };
        Self { lower, normalized }
    }

    fn len(&self) -> usize {
        1 + usize::from(self.normalized.is_some())
    }

    fn iter(&self) -> impl Iterator<Item = &str> + Clone {
        std::iter::once(self.lower).chain(self.normalized.iter().map(String::as_str))
    }
}

pub fn query_emoji(
    code: &str,
    scheme: SchemeType,
    others_db: &Path,
    limit: usize,
    profile: &ShuangpinProfile,
) -> LocalQueryResult {
    query(&EMOJI, code, scheme, others_db, limit, profile)
}

pub fn query_kaomoji(
    code: &str,
    scheme: SchemeType,
    others_db: &Path,
    limit: usize,
    profile: &ShuangpinProfile,
) -> LocalQueryResult {
    query(&KAOMOJI, code, scheme, others_db, limit, profile)
}

/// 26 键混排：按输入码查 emoji（双拼另查归一成全拼的码），见 `query_readings`。输入码不全是字母和 `'` 时没有行。
pub fn query_mixed_emoji(
    code: &str,
    scheme: SchemeType,
    others_db: &Path,
    profile: &ShuangpinProfile,
) -> Vec<ExpressiveRow> {
    query_mixed(&EMOJI, code, scheme, others_db, profile)
}

/// 26 键混排：按输入码查颜文字，见 `query_mixed_emoji`。
pub fn query_mixed_kaomoji(
    code: &str,
    scheme: SchemeType,
    others_db: &Path,
    profile: &ShuangpinProfile,
) -> Vec<ExpressiveRow> {
    query_mixed(&KAOMOJI, code, scheme, others_db, profile)
}

fn query_mixed(
    catalog: &Catalog,
    code: &str,
    scheme: SchemeType,
    others_db: &Path,
    profile: &ShuangpinProfile,
) -> Vec<ExpressiveRow> {
    if code.is_empty()
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || byte == b'\'')
    {
        return Vec::new();
    }
    let lower = if code
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte == b'\'')
    {
        Cow::Borrowed(code)
    } else {
        Cow::Owned(code.to_ascii_lowercase())
    };
    let prefixes = QueryPrefixes::new(lower.as_ref(), scheme, profile);
    query_readings(catalog, prefixes.iter(), others_db, &|_| true)
}

/// 九宫格混排：依次按 `readings`（小写全拼，可能性高的在前）查 emoji，见 `query_readings`。
pub fn query_emoji_readings(
    readings: &[String],
    others_db: &Path,
    accept: &dyn Fn(&str) -> bool,
) -> Vec<ExpressiveRow> {
    query_readings(
        &EMOJI,
        readings.iter().map(String::as_str),
        others_db,
        accept,
    )
}

/// 九宫格混排：依次按 `readings` 查颜文字，见 `query_readings`。
pub fn query_kaomoji_readings(
    readings: &[String],
    others_db: &Path,
    accept: &dyn Fn(&str) -> bool,
) -> Vec<ExpressiveRow> {
    query_readings(
        &KAOMOJI,
        readings.iter().map(String::as_str),
        others_db,
        accept,
    )
}

/// 读法逐个查，每种读法内编码与读法完全相同的在前、再按目录顺序；命中的编码（emoji 的 `key`，颜文字的 `pinyin` 或 `jianpin`）以这个读法开头、并且通过 `accept`，这一行才算。每种读法最多 `MIXED_FETCH_PER_READING` 行，跨读法按文字去重，合计取够 `MIXED_FETCH_LIMIT` 行就不再查。行的 `pinyin` 是命中的读法，权重从行数往下数。混排不报诊断：资源打不开或查询失败时只是没有这些行。
fn query_readings<'a, I>(
    catalog: &Catalog,
    readings: I,
    others_db: &Path,
    accept: &dyn Fn(&str) -> bool,
) -> Vec<ExpressiveRow>
where
    I: IntoIterator<Item = &'a str> + Clone,
{
    if readings.clone().into_iter().next().is_none() {
        return Vec::new();
    }
    let Some(database) = open_local_database(others_db) else {
        return Vec::new();
    };
    let connection = lock(&database);
    let entries = match read_readings(&connection, catalog.mixed_sql, readings.clone(), accept) {
        Ok(entries) => entries,
        // 目录表缺失只是接不上候选词，行照样给；其他失败由退回的语句再报一次，同样没有行。
        Err(_) => read_readings(
            &connection,
            catalog.mixed_sql_without_keywords,
            readings,
            accept,
        )
        .unwrap_or_default(),
    };
    let count = entries.len();
    entries
        .into_iter()
        .enumerate()
        .map(|(index, (reading, text, keywords))| ExpressiveRow {
            item: WordItem::new(reading, text, (count - index) as i64, catalog.source, ""),
            keywords,
        })
        .collect()
}

fn read_readings<'a, I>(
    connection: &Connection,
    sql: &str,
    readings: I,
    accept: &dyn Fn(&str) -> bool,
) -> rusqlite::Result<Vec<(&'a str, String, String)>>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut statement = connection.prepare_cached(sql)?;
    let mut entries: Vec<(&str, String, String)> = Vec::with_capacity(MIXED_FETCH_PER_READING);
    for reading in readings {
        let upper_bound = prefix_upper_bound(reading);
        let mut rows = statement.query(rusqlite::params![reading, upper_bound])?;
        let mut taken = 0;
        while let Some(row) = rows.next()? {
            let Some(text) = row.get::<_, Option<String>>(0)? else {
                continue;
            };
            // `OR` 的另一列可能才是命中的那一列，两列各自核对。
            let mut matched = false;
            for column in 1..=2 {
                let key = row.get::<_, Option<String>>(column)?.unwrap_or_default();
                if key.starts_with(reading) && accept(&key) {
                    matched = true;
                    break;
                }
            }
            if !matched || entries.iter().any(|(_, entry, _)| *entry == text) {
                continue;
            }
            let keywords = row.get::<_, Option<String>>(3)?.unwrap_or_default();
            entries.push((reading, text, keywords));
            if entries.len() == MIXED_FETCH_LIMIT {
                return Ok(entries);
            }
            taken += 1;
            if taken == MIXED_FETCH_PER_READING {
                break;
            }
        }
    }
    Ok(entries)
}

fn query(
    catalog: &Catalog,
    code: &str,
    scheme: SchemeType,
    others_db: &Path,
    limit: usize,
    profile: &ShuangpinProfile,
) -> LocalQueryResult {
    if code.is_empty()
        || limit == 0
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || byte == b'\'')
    {
        return LocalQueryResult::default();
    }
    let lower = code.to_ascii_lowercase();
    let prefixes = QueryPrefixes::new(&lower, scheme, profile);
    let Some(database) = open_local_database(others_db) else {
        return LocalQueryResult::failure(catalog.unavailable);
    };
    let entries = match read(
        &lock(&database),
        catalog.sql,
        prefixes.iter(),
        prefixes.len(),
        limit,
    ) {
        Ok(entries) => entries,
        // Reported without SQLite's text, which could name the path or the typed code (emoji_query.cpp:98-127).
        Err(_) => return LocalQueryResult::failure(catalog.query_failed),
    };
    let count = entries.len().min(limit);
    let candidates = entries
        .into_iter()
        .take(count)
        .enumerate()
        .map(|(index, (text, _))| {
            WordItem::new(
                lower.clone(),
                text,
                (count - index) as i64,
                catalog.source,
                "",
            )
        })
        .collect();
    LocalQueryResult {
        candidates,
        diagnostic: None,
    }
}

/// Every prefix's rows, the first occurrence of a text kept, then stably ordered by `sort_order` so the raw and normalised shuangpin matches interleave in catalog order.
fn read<'a, I>(
    connection: &Connection,
    sql: &str,
    prefixes: I,
    prefix_count: usize,
    limit: usize,
) -> rusqlite::Result<Vec<(String, i64)>>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut statement = connection.prepare_cached(sql)?;
    let capacity = limit.saturating_mul(prefix_count);
    let mut entries = Vec::with_capacity(capacity);
    for prefix in prefixes {
        let upper_bound = prefix_upper_bound(prefix);
        let rows = statement.query_map(
            rusqlite::params![prefix, upper_bound, super::sql_limit(limit)],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                ))
            },
        )?;
        for row in rows {
            if let (Some(text), sort_order) = row? {
                // The result page is deliberately small (normally ten rows, at most one
                // page per spelling). Scanning the collected rows avoids allocating a
                // second owned copy of every emoji/kaomoji just to deduplicate it.
                if !entries.iter().any(|(entry, _)| entry == &text) {
                    entries.push((text, sort_order.unwrap_or(0)));
                }
            }
        }
    }
    entries.sort_by_key(|(_, sort_order)| *sort_order);
    Ok(entries)
}

fn prefix_upper_bound(prefix: &str) -> String {
    let mut upper_bound = String::with_capacity(prefix.len() + 1);
    upper_bound.push_str(prefix);
    upper_bound.push('\x7f');
    upper_bound
}

#[cfg(test)]
fn contains_text(entries: &[(String, i64)], text: &str) -> bool {
    entries.iter().any(|(entry, _)| entry == text)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::types::ShuangpinProfileKind;

    const QUANPIN_ONLY: ShuangpinProfile = ShuangpinProfile {
        kind: ShuangpinProfileKind::Xiaohe,
        initials: &[],
        zero_initials: &[],
        finals: &[],
    };

    fn fixture(dir: &Path) -> PathBuf {
        let path = dir.join("msime-others.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE emoji_pinyin(key TEXT,emoji TEXT,sort_order INTEGER);
                 INSERT INTO emoji_pinyin VALUES('xiaolian','😀',10);
                 INSERT INTO emoji_pinyin VALUES('xiaolian','😄',20);
                 INSERT INTO emoji_pinyin VALUES('xiaolian','😀',30);
                 INSERT INTO emoji_pinyin VALUES('xl','😀',10);
                 INSERT INTO emoji_pinyin VALUES('xiao''lian','😄',20);
                 INSERT INTO emoji_pinyin VALUES('laugh','😀',10);
                 INSERT INTO emoji_pinyin VALUES('xnlm','raw shuangpin match',40);
                 CREATE TABLE kaomoji(pinyin TEXT,jianpin TEXT,kaomoji TEXT,sort_order INTEGER);
                 INSERT INTO kaomoji VALUES('haixiu','hx','(*/ω＼*)',10);
                 INSERT INTO kaomoji VALUES('haixiu','hx','(^_^)',20);
                 INSERT INTO kaomoji VALUES('haixiu','hx','(*/ω＼*)',30);
                 INSERT INTO kaomoji VALUES('kiss','','( ˘ ³˘)♥',40);
                 INSERT INTO kaomoji VALUES('kind','','single prefix',50);",
            )
            .unwrap();
        path
    }

    fn words(result: &LocalQueryResult) -> Vec<&str> {
        result
            .candidates
            .iter()
            .map(|row| row.word.as_str())
            .collect()
    }

    #[test]
    fn duplicate_text_lookup_scans_existing_entries() {
        let entries = vec![("😀".to_owned(), 1)];
        assert!(contains_text(&entries, "😀"));
        assert!(!contains_text(&entries, "😄"));
    }

    #[test]
    fn quanpin_prefixes_borrow_the_lowercase_input_without_allocating() {
        let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            QueryPrefixes::new("ni", SchemeType::Quanpin, &QUANPIN_ONLY)
        });
        assert_eq!(allocations, 0);
    }

    #[test]
    fn prefix_upper_bound_allocates_only_result_bytes() {
        let prefix = "xiao'lian";
        let upper_bound = prefix_upper_bound(prefix);
        assert_eq!(upper_bound, "xiao'lian\x7f");
        assert_eq!(upper_bound.capacity(), upper_bound.len());
    }

    /// test_local_modes.cpp:269-296, the quanpin half.
    #[test]
    fn emoji_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path());
        let emoji = query_emoji("XIAOLIAN", SchemeType::Quanpin, &path, 10, &QUANPIN_ONLY);
        assert_eq!(emoji.diagnostic, None);
        assert_eq!(words(&emoji), ["😀", "😄"]);
        assert_eq!(emoji.candidates[0].pinyin, "xiaolian");
        assert_eq!(emoji.candidates[0].source, CandidateSource::Emoji);
        assert_eq!(emoji.candidates[0].weight, 2);
        assert_eq!(emoji.candidates[1].weight, 1);

        let quanpin = SchemeType::Quanpin;
        // The local_emoji_kaomoji golden types the apostrophe, which is part of the key.
        let separated = query_emoji("xiao'lian", quanpin, &path, 10, &QUANPIN_ONLY);
        assert_eq!(words(&separated), ["😄"]);
        assert_eq!(separated.candidates[0].pinyin, "xiao'lian");
        assert_eq!(
            words(&query_emoji("xl", quanpin, &path, 10, &QUANPIN_ONLY))[0],
            "😀"
        );
        assert_eq!(
            words(&query_emoji("laugh", quanpin, &path, 10, &QUANPIN_ONLY))[0],
            "😀"
        );
        assert_eq!(
            words(&query_emoji("xiao", quanpin, &path, 10, &QUANPIN_ONLY)),
            ["😀", "😄"]
        );
        assert_eq!(
            query_emoji("xiaolian", quanpin, &path, 1, &QUANPIN_ONLY)
                .candidates
                .len(),
            1
        );
        for code in ["", "bad1", "xiao lian", "表情"] {
            assert_eq!(
                query_emoji(code, quanpin, &path, 10, &QUANPIN_ONLY),
                LocalQueryResult::default()
            );
        }
        assert_eq!(
            query_emoji("x", quanpin, &path, 0, &QUANPIN_ONLY),
            LocalQueryResult::default()
        );
    }

    /// test_local_modes.cpp:298-317, the quanpin half.
    #[test]
    fn kaomoji_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path());
        let quanpin = SchemeType::Quanpin;
        let kaomoji = query_kaomoji("HAIXIU", quanpin, &path, 10, &QUANPIN_ONLY);
        assert_eq!(kaomoji.diagnostic, None);
        assert_eq!(words(&kaomoji), ["(*/ω＼*)", "(^_^)"]);
        assert_eq!(kaomoji.candidates[0].pinyin, "haixiu");
        assert_eq!(kaomoji.candidates[0].source, CandidateSource::Kaomoji);
        assert_eq!(
            words(&query_kaomoji("hx", quanpin, &path, 10, &QUANPIN_ONLY))[0],
            "(*/ω＼*)"
        );
        assert_eq!(
            words(&query_kaomoji("kiss", quanpin, &path, 10, &QUANPIN_ONLY))[0],
            "( ˘ ³˘)♥"
        );
        assert_eq!(
            words(&query_kaomoji("k", quanpin, &path, 10, &QUANPIN_ONLY)),
            ["( ˘ ³˘)♥", "single prefix"]
        );
    }

    /// test_local_modes.cpp:285-291, 311-314: the raw code and its Xiaohe normalisation are merged in catalog order.
    #[test]
    fn shuangpin_merges_both_prefixes() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path());
        let xiaohe = crate::shuangpin::profile::profile(ShuangpinProfileKind::Xiaohe);
        let emoji = query_emoji("xnlm", SchemeType::Shuangpin, &path, 10, xiaohe);
        assert_eq!(words(&emoji), ["😀", "😄", "raw shuangpin match"]);
        assert!(emoji.candidates.iter().all(|row| row.pinyin == "xnlm"));
        let kaomoji = query_kaomoji("hx", SchemeType::Shuangpin, &path, 10, xiaohe);
        assert_eq!(words(&kaomoji), ["(*/ω＼*)", "(^_^)"]);
    }

    fn readings(list: &[&str]) -> Vec<String> {
        list.iter().map(|reading| (*reading).to_owned()).collect()
    }

    fn mixed_words(rows: &[ExpressiveRow]) -> Vec<&str> {
        rows.iter().map(|row| row.item.word.as_str()).collect()
    }

    /// 带目录表的混排夹具：`ji` 前缀下 🐔 的编码正好是 `ji`，但目录顺序排在二十行前缀命中的后面；🇺🇲、🇺🇸 的编码和次序与随包数据相同。
    fn mixed_fixture(dir: &Path) -> PathBuf {
        let path = dir.join("msime-others.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE emoji_pinyin(key TEXT,emoji TEXT,sort_order INTEGER);
                 CREATE TABLE emoji(emoji TEXT PRIMARY KEY,keywords TEXT);
                 INSERT INTO emoji_pinyin VALUES('meiguobentuwaixiaodaoyu','🇺🇲',1891),('meiguo','🇺🇸',1893),('ji','🐔',626),('jiba','🐔',626),('jitou','🐔',626);
                 INSERT INTO emoji VALUES('🇺🇲','美国本土外小岛屿 flag: u.s. outlying islands'),('🇺🇸','美国 美利坚 星条旗'),('🐔','鸡 鸡头 chicken');
                 CREATE TABLE kaomoji(pinyin TEXT,jianpin TEXT,kaomoji TEXT,sort_order INTEGER);
                 CREATE TABLE kaomoji_catalog(kaomoji TEXT PRIMARY KEY,keywords TEXT);
                 INSERT INTO kaomoji VALUES('haixiu','hx','(*/ω＼*)',10),('kaixin','kx','(^_^)',20);
                 INSERT INTO kaomoji_catalog VALUES('(*/ω＼*)','hai xiu 害羞');",
            )
            .unwrap();
        for index in 0..20 {
            connection
                .execute(
                    "INSERT INTO emoji_pinyin VALUES(?1,?2,?3)",
                    rusqlite::params![format!("jiqi{index:02}"), format!("机{index:02}"), index],
                )
                .unwrap();
        }
        path
    }

    /// 混排的查询：读法按给定的先后查，读法内编码完全相同的在前、再按目录顺序；行带着目录里的关键词，行的 `pinyin` 是命中的读法。
    #[test]
    fn mixed_rows_put_exact_keys_first_and_carry_their_keywords() {
        let dir = tempfile::tempdir().unwrap();
        let path = mixed_fixture(dir.path());
        let all = |_: &str| true;
        // 🇺🇲 目录顺序在前，但只是前缀命中；🇺🇸 的编码就是 meiguo（#5907）。
        let flags = query_emoji_readings(&readings(&["meiguo"]), &path, &all);
        assert_eq!(mixed_words(&flags), ["🇺🇸", "🇺🇲"]);
        assert_eq!(flags[0].keywords, "美国 美利坚 星条旗");
        assert_eq!(
            flags[1].keywords,
            "美国本土外小岛屿 flag: u.s. outlying islands"
        );
        assert_eq!(flags[0].item.pinyin, "meiguo");
        assert_eq!(flags[0].item.source, CandidateSource::Emoji);
        assert_eq!((flags[0].item.weight, flags[1].item.weight), (2, 1));

        // ji 有二十行目录顺序更靠前的前缀命中，🐔 的编码完全相同，排第一；每种读法最多取 `MIXED_FETCH_PER_READING` 行。
        let ji = query_emoji_readings(&readings(&["ji"]), &path, &all);
        assert_eq!(ji[0].item.word, "🐔");
        assert_eq!(ji.len(), MIXED_FETCH_PER_READING);
        // 读法按给定的先后，跨读法按文字去重。
        let both = query_emoji_readings(&readings(&["meiguo", "ji", "jiba"]), &path, &all);
        assert_eq!(mixed_words(&both)[..3], ["🇺🇸", "🇺🇲", "🐔"]);
        assert_eq!(both[2].item.pinyin, "ji");
        assert_eq!(both.len(), 2 + MIXED_FETCH_PER_READING);
        assert!(query_emoji_readings(&[], &path, &all).is_empty());

        // 26 键按输入码查，输入码里有字母和 `'` 以外的字符时没有行。
        let quanpin = SchemeType::Quanpin;
        assert_eq!(
            query_mixed_emoji("MeiGuo", quanpin, &path, &QUANPIN_ONLY),
            flags
        );
        assert!(query_mixed_emoji("mei guo", quanpin, &path, &QUANPIN_ONLY).is_empty());

        let kaomoji = query_mixed_kaomoji("hx", quanpin, &path, &QUANPIN_ONLY);
        assert_eq!(mixed_words(&kaomoji), ["(*/ω＼*)"]);
        assert_eq!(kaomoji[0].keywords, "hai xiu 害羞");
        assert_eq!(kaomoji[0].item.source, CandidateSource::Kaomoji);
        // 目录里没有的颜文字照样给，只是没有关键词。
        let kaixin = query_mixed_kaomoji("kaixin", quanpin, &path, &QUANPIN_ONLY);
        assert_eq!(mixed_words(&kaixin), ["(^_^)"]);
        assert_eq!(kaixin[0].keywords, "");
    }

    #[test]
    fn mixed_quanpin_queries_do_not_allocate_temporary_readings() {
        let dir = tempfile::tempdir().unwrap();
        let path = mixed_fixture(dir.path());
        let readings = readings(&["meiguo"]);
        let expected = query_emoji_readings(&readings, &path, &|_| true);
        let (direct, direct_allocations) = crate::ime::personal_rerank::allocations::count(|| {
            query_emoji_readings(&readings, &path, &|_| true)
        });
        assert_eq!(direct, expected);

        for (code, lowercase_allocations) in [("meiguo", 0), ("MeiGuo", 1)] {
            let (rows, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                query_mixed_emoji(code, SchemeType::Quanpin, &path, &QUANPIN_ONLY)
            });
            assert_eq!(rows, expected);
            assert_eq!(
                allocations,
                direct_allocations + lowercase_allocations,
                "混排查询不应复制读法列表：{code} 分配了 {allocations} 次，直接查询为 {direct_allocations} 次"
            );
        }
    }

    /// 合计最多取 `MIXED_FETCH_LIMIT` 行。
    #[test]
    fn mixed_rows_stop_at_the_fetch_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("msime-others.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("CREATE TABLE emoji_pinyin(key TEXT,emoji TEXT,sort_order INTEGER);")
            .unwrap();
        let mut keys = Vec::new();
        for reading in 0..8 {
            let key = format!("k{}", char::from(b'a' + reading));
            for index in 0..MIXED_FETCH_PER_READING {
                connection
                    .execute(
                        "INSERT INTO emoji_pinyin VALUES(?1,?2,?3)",
                        rusqlite::params![key, format!("{key}{index}"), index as i64],
                    )
                    .unwrap();
            }
            keys.push(key);
        }
        let rows = query_emoji_readings(&keys, &path, &|_| true);
        assert_eq!(rows.len(), MIXED_FETCH_LIMIT);
    }

    /// 资源里只有编码表、没有目录表时行照样给，关键词为空；`accept` 核对的是命中的那一列编码，不通过的行跳过，不占名额。
    #[test]
    fn mixed_rows_without_catalog_tables_are_checked_against_the_matched_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path());
        let all = |_: &str| true;
        let emoji = query_emoji_readings(&readings(&["laugh", "xiao"]), &path, &all);
        assert_eq!(mixed_words(&emoji), ["😀", "😄"]);
        assert_eq!(emoji[0].item.pinyin, "laugh");
        assert_eq!(emoji[1].item.pinyin, "xiao");
        assert!(emoji.iter().all(|row| row.keywords.is_empty()));

        let not_xiaolian = |key: &str| key != "xiaolian";
        let emoji = query_emoji_readings(&readings(&["xiao"]), &path, &not_xiaolian);
        // `xiaolian` 的两行被拒，`xiao'lian` 的 😄 还在。
        assert_eq!(mixed_words(&emoji), ["😄"]);

        // 颜文字的 `jianpin` 列命中时核对 `jianpin`。
        let only_jianpin = |key: &str| key == "hx";
        let kaomoji = query_kaomoji_readings(&readings(&["hx"]), &path, &only_jianpin);
        assert_eq!(mixed_words(&kaomoji), ["(*/ω＼*)", "(^_^)"]);
        let none = |_: &str| false;
        assert!(query_kaomoji_readings(&readings(&["hx"]), &path, &none).is_empty());

        // 资源打不开时没有行，混排不报诊断。
        let missing = dir.path().join("private-others-missing.db");
        assert!(query_emoji_readings(&readings(&["xiao"]), &missing, &all).is_empty());
    }

    /// test_local_modes.cpp:319-331.
    #[test]
    fn missing_and_corrupt_databases_have_private_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("private-others-missing.db");
        let result = query_emoji(
            "privatecode",
            SchemeType::Quanpin,
            &missing,
            10,
            &QUANPIN_ONLY,
        );
        assert!(result.candidates.is_empty());
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::EMOJI_UNAVAILABLE)
        );
        let result = query_kaomoji(
            "privatecode",
            SchemeType::Quanpin,
            &missing,
            10,
            &QUANPIN_ONLY,
        );
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::KAOMOJI_UNAVAILABLE)
        );

        let corrupt = dir.path().join("private-corrupt.db");
        std::fs::write(&corrupt, "not a sqlite database").unwrap();
        let result = query_kaomoji(
            "privatecode",
            SchemeType::Quanpin,
            &corrupt,
            10,
            &QUANPIN_ONLY,
        );
        assert!(result.candidates.is_empty());
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::KAOMOJI_QUERY_FAILED)
        );
        let result = query_emoji(
            "privatecode",
            SchemeType::Quanpin,
            &corrupt,
            10,
            &QUANPIN_ONLY,
        );
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::EMOJI_QUERY_FAILED)
        );
    }
}
