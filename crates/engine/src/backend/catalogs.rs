//! Paged browsing: the emoji, kaomoji and symbol catalogs under `/v1/catalog/{kind}`, and a dictionary's entries for one code, which the account service merges with a user's own words. The queries and the response shapes are the ones the C++ bridge used (`native/catalog.h`, `native/dictionary_catalog.h` in msime-cloud), so paging and search behave as before.

use rusqlite::types::ToSql;
use serde_json::{json, Value};

use super::common::{shuangpin_profile, Roots};
use super::dictionary_checks::{open_read_only, table_exists};
use super::input::build_query;
use super::request::Request;
use super::{BackendError, Outcome};
use crate::format;
use crate::ime::registry::wubi_database;
use crate::pinyin::segment::split_segments;
use crate::types::SchemeType;
use crate::{assets, RuntimePaths};

const DEFAULT_PAGE: i64 = 50;
const MAXIMUM_OFFSET: i64 = 1_000_000;

/// `offset` and `limit` of a page. Both catalogs default to 50 rows rather than the protocol's 20.
fn page(request: &Request) -> Result<(i64, i64), BackendError> {
    let number = |name: &str, default: i64| match request.field(name) {
        None => Ok(default),
        Some(value) => value.as_i64().ok_or(BackendError::InvalidRequest),
    };
    let offset = number("offset", 0)?;
    let limit = number("limit", DEFAULT_PAGE)?;
    if !(0..=MAXIMUM_OFFSET).contains(&offset) || !(1..=200).contains(&limit) {
        return Err(BackendError::InvalidRequest);
    }
    Ok((offset, limit))
}

fn text_column(row: &rusqlite::Row, index: usize) -> rusqlite::Result<String> {
    Ok(row.get::<_, Option<String>>(index)?.unwrap_or_default())
}

/// `kind`: `emoji`, `kaomoji` or `symbols`. `text` searches keywords (case-insensitively) and the item itself; `category` keeps one category. Every category is listed with its size whatever the filter.
pub(super) fn catalog(request: &Request, roots: Roots) -> Outcome {
    let (table, value, category, parent) = match request.string("kind")? {
        "emoji" => ("emoji", "emoji", "category", "''"),
        "kaomoji" => ("kaomoji_catalog", "kaomoji", "'All'", "''"),
        "symbols" => ("symbol_catalog", "symbol", "category", "parent_category"),
        _ => return Err(BackendError::InvalidRequest),
    };
    roots.require_resources()?;
    let connection = open_read_only(&roots.resource(assets::OTHER_DICTIONARY))?;
    connection
        .busy_timeout(std::time::Duration::from_secs(1))
        .map_err(|_| BackendError::ResourcesUnavailable)?;
    let (offset, limit) = page(request)?;
    let search = request.text();
    let filter = request.string_or("category", "")?;
    // Every identifier above is a literal of this function; only bound parameters come from the request.
    let sql = format!(
        "SELECT {value},{category},{parent},keywords FROM {table} WHERE (?1='' OR {category}=?1) AND (?2='' OR instr(lower(keywords),lower(?2))>0 OR instr({value},?2)>0) ORDER BY sort_order,{value} LIMIT ?3 OFFSET ?4"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|_| BackendError::ResourcesUnavailable)?;
    let mut items = statement
        .query_map(
            rusqlite::params![filter, search, limit + 1, offset],
            |row| {
                Ok(json!({
                    "text": text_column(row, 0)?,
                    "category": text_column(row, 1)?,
                    "parent_category": text_column(row, 2)?,
                    "keywords": text_column(row, 3)?,
                }))
            },
        )
        .and_then(Iterator::collect::<rusqlite::Result<Vec<Value>>>)
        .map_err(|_| BackendError::ResourcesUnavailable)?;
    let more = items.len() > limit as usize;
    items.truncate(limit as usize);
    let sql = format!(
        "SELECT {category},{parent},count(*) FROM {table} GROUP BY {category},{parent} ORDER BY min(sort_order)"
    );
    let categories = connection
        .prepare(&sql)
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok(json!({
                        "name": text_column(row, 0)?,
                        "parent": text_column(row, 1)?,
                        "count": row.get::<_, i64>(2)?,
                    }))
                })?
                .collect::<rusqlite::Result<Vec<Value>>>()
        })
        .map_err(|_| BackendError::ResourcesUnavailable)?;
    Ok(json!({
        "items": items,
        "categories": categories,
        "offset": offset,
        "has_more": more,
    }))
}

/// The exclusive upper bound of a prefix range: the last character moved one code point up.
fn prefix_upper_bound(prefix: &str) -> Option<String> {
    let mut upper = prefix.to_owned();
    let last = upper.pop()?;
    upper.push(char::from_u32(u32::from(last).checked_add(1)?)?);
    Some(upper)
}

/// One page of a dictionary's entries for `text`, best first. `kind`: `pinyin` (the entries whose key is the input's normalised segmentation; `scheme: shuangpin` reads shuangpin input), `wubi` and `quick` (entries whose code starts with `text`), or `english` (words starting with `text`, an exact match first). With `exact`, only the entry whose code is `text` and whose word is `word`. `normalized` echoes the code that was looked up.
pub(super) fn dictionary(request: &Request, roots: Roots) -> Outcome {
    let kind = request.string("kind")?;
    let mut text = request.string("text")?.to_owned();
    let (offset, limit) = page(request)?;
    let (table, key, word, mut condition, order, path);
    let paths = RuntimePaths {
        resources: roots.resources.to_path_buf(),
        dictionaries: roots.dictionaries.to_path_buf(),
        ..RuntimePaths::default()
    };
    match kind {
        "pinyin" => {
            let scheme = if request.string_or("scheme", "pinyin")? == "shuangpin" {
                SchemeType::Shuangpin
            } else {
                SchemeType::Quanpin
            };
            let query = build_query(scheme, shuangpin_profile(request)?, &text)?;
            if query.normalized_segmentation.is_empty() {
                return Err(BackendError::InvalidRequest);
            }
            text = query.normalized_segmentation;
            table = format::build_table_name(&split_segments(&text))
                .ok_or(BackendError::InvalidRequest)?;
            (key, word) = ("key", "value");
            condition = "key=?1";
            order = "weight DESC,value";
            path = paths.dictionary(assets::MAIN_DICTIONARY);
        }
        "wubi" | "quick" => {
            table = if kind == "wubi" {
                "wubi86"
            } else {
                "quick_parases"
            }
            .to_owned();
            (key, word) = ("key", "value");
            condition = "key LIKE ?1 || '%'";
            order = "weight DESC,key,value";
            // The separately shipped wubi tables are merged into a replayed generation's main dictionary; outside one they are read where they ship.
            path = if kind == "wubi" {
                wubi_database(&paths)
            } else {
                paths.dictionary(assets::MAIN_DICTIONARY)
            };
        }
        "english" => {
            table = "english_words".to_owned();
            (key, word) = ("word", "display");
            condition = "word>=?1 AND word<?4";
            order = "CASE WHEN word=?1 THEN 0 ELSE 1 END,weight DESC,length(word),word,display";
            path = paths.dictionary(assets::ENGLISH_DICTIONARY);
        }
        _ => return Err(BackendError::InvalidRequest),
    }
    let connection = open_read_only(&path)?;
    if !table_exists(&connection, &table)? {
        return Ok(json!({
            "entries": [],
            "offset": offset,
            "has_more": false,
            "normalized": text,
        }));
    }
    let exact = match request.field("exact") {
        None => false,
        Some(value) => value.as_bool().ok_or(BackendError::InvalidRequest)?,
    };
    let exact_word = if exact {
        condition = if kind == "english" {
            "word=?1 AND display=?5"
        } else {
            "key=?1 AND value=?5"
        };
        request.string("word")?.to_owned()
    } else {
        String::new()
    };
    if kind == "english" && text.is_empty() {
        return Err(BackendError::InvalidRequest);
    }
    let upper = prefix_upper_bound(&text);
    if kind == "english" && !exact && upper.is_none() {
        // U+10FFFF has no successor. Match the prefix directly instead of
        // inventing an upper bound equal to the prefix itself or admitting
        // unrelated words that merely sort after it.
        condition = "(word=?1 OR (length(word)>length(?1) AND substr(word,1,length(?1))=?1))";
    }
    // The table comes from the format contract or a literal above, never from the request.
    let sql = format!(
        "SELECT {key},{word},weight FROM \"{table}\" WHERE {condition} ORDER BY {order} LIMIT ?2 OFFSET ?3"
    );
    let mut statement = connection.prepare(&sql)?;
    let page_size = limit + 1;
    let mut parameters: Vec<(usize, &dyn ToSql)> = vec![(1, &text), (2, &page_size), (3, &offset)];
    if kind == "english" && !exact {
        if let Some(upper) = upper.as_ref() {
            parameters.push((4, upper));
        }
    }
    if exact {
        parameters.push((5, &exact_word));
    }
    for (index, value) in parameters {
        statement.raw_bind_parameter(index, value)?;
    }
    let mut rows = statement.raw_query();
    let mut entries = Vec::with_capacity(page_size as usize);
    while let Some(row) = rows.next()? {
        entries.push(json!({
            "kind": kind,
            "code": text_column(row, 0)?,
            "word": text_column(row, 1)?,
            "weight": row.get::<_, Option<i64>>(2)?.unwrap_or_default(),
        }));
    }
    let more = entries.len() > limit as usize;
    entries.truncate(limit as usize);
    Ok(json!({
        "entries": entries,
        "offset": offset,
        "has_more": more,
        "normalized": text,
    }))
}
