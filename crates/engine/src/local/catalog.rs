//! The emoji, symbol and kaomoji catalog the host's picker pages through (api-contract §1c, bridge.cpp:1018-1137, 1237-1260). Errors deliberately carry no SQLite detail.

use std::path::Path;

use rusqlite::{Connection, Statement};

use super::database::open_read_only;
use crate::assets;
use crate::diagnostics;
use crate::error::{EngineError, Result};

pub const MAXIMUM_PAGE: usize = 4_096;

const KAOMOJI_CATEGORY: &str = "kaomoji";
const SYMBOLS_CATEGORY: &str = "symbols";

const KAOMOJI_SQL: &str = "SELECT kaomoji,'All',keywords FROM kaomoji_catalog WHERE (?1 = '' OR kaomoji LIKE ?2 OR keywords LIKE ?2) AND (?5 = '' OR ?5 = 'All') ORDER BY sort_order LIMIT ?3 OFFSET ?4";
const SYMBOLS_SQL: &str = "SELECT symbol,category,keywords FROM symbol_catalog WHERE (?1 = '' OR symbol LIKE ?2 OR category LIKE ?2 OR parent_category LIKE ?2 OR keywords LIKE ?2) AND (?5 = '' OR category = ?5) AND (?6 = '' OR COALESCE(NULLIF(parent_category,''),category) = ?6) ORDER BY sort_order LIMIT ?3 OFFSET ?4";
const EMOJI_SQL: &str = "SELECT emoji,category,keywords FROM emoji WHERE (?1 = '' OR category = ?1) AND (?2 = '' OR pinyin LIKE ?3 OR keywords LIKE ?3 OR emoji LIKE ?3) ORDER BY sort_order LIMIT ?4 OFFSET ?5";

const KAOMOJI_GROUPS_SQL: &str = "SELECT 'All' FROM kaomoji_catalog LIMIT 1";
const SYMBOLS_GROUPS_SQL: &str = "SELECT category FROM symbol_catalog WHERE category IS NOT NULL AND category != '' GROUP BY category ORDER BY MIN(sort_order), category";
const EMOJI_GROUPS_SQL: &str = "SELECT category FROM emoji WHERE category IS NOT NULL AND category != '' GROUP BY category ORDER BY MIN(sort_order), category";
const SYMBOL_PARENTS_SQL: &str = "SELECT COALESCE(NULLIF(parent_category,''),category) AS parent, category FROM symbol_catalog WHERE category IS NOT NULL AND category != '' GROUP BY parent, category ORDER BY MIN(sort_order), parent, category";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiCatalogItem {
    pub text: String,
    pub annotation: String,
    pub group: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiCatalogSlice {
    pub items: Vec<EmojiCatalogItem>,
    /// One past the last row scanned, skipped rows included.
    pub next_offset: usize,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiSymbolGroup {
    pub parent: String,
    pub title: String,
}

/// One page of `category` (`kaomoji`, `symbols`, or an emoji category) from `resources/msime-others.db`. `limit` is 1..=4096. With `deduplicate` the first occurrence of each text wins; without it rows with empty text (or empty group, except kaomoji) are skipped.
#[allow(clippy::too_many_arguments)]
pub fn read_emoji_catalog_slice(
    resources: &Path,
    search: &str,
    category: &str,
    group: &str,
    offset: usize,
    limit: usize,
    parent: &str,
    deduplicate: bool,
) -> Result<EmojiCatalogSlice> {
    let Some(sql_offset) = i64::try_from(offset)
        .ok()
        .filter(|_| (1..=MAXIMUM_PAGE).contains(&limit))
    else {
        return Err(EngineError::invalid(
            diagnostics::INVALID_EMOJI_CATALOG_PAGE,
        ));
    };
    let connection = open_catalog(resources)?;
    let kaomoji = category == KAOMOJI_CATEGORY;
    let symbols = category == SYMBOLS_CATEGORY;
    let sql = if kaomoji {
        KAOMOJI_SQL
    } else if symbols {
        SYMBOLS_SQL
    } else {
        EMOJI_SQL
    };
    let mut statement = prepare(&connection, sql)?;
    let pattern = search_pattern(search);
    let sql_limit = limit as i64;
    let bound = if kaomoji || symbols {
        bind(&mut statement, 1, search)
            .and_then(|()| bind(&mut statement, 2, &pattern))
            .and_then(|()| bind(&mut statement, 3, sql_limit))
            .and_then(|()| bind(&mut statement, 4, sql_offset))
            .and_then(|()| bind(&mut statement, 5, group))
            .and_then(|()| {
                if symbols {
                    bind(&mut statement, 6, parent)
                } else {
                    Ok(())
                }
            })
    } else {
        let selected_group = if group.is_empty() { category } else { group };
        bind(&mut statement, 1, selected_group)
            .and_then(|()| bind(&mut statement, 2, search))
            .and_then(|()| bind(&mut statement, 3, &pattern))
            .and_then(|()| bind(&mut statement, 4, sql_limit))
            .and_then(|()| bind(&mut statement, 5, sql_offset))
    };
    bound.map_err(|_| EngineError::failed(diagnostics::EMOJI_CATALOG_QUERY_REJECTED))?;

    let mut result = EmojiCatalogSlice {
        items: Vec::with_capacity(limit),
        next_offset: offset,
        complete: false,
    };
    let mut rows = statement.raw_query();
    loop {
        let row = match rows.next() {
            Ok(Some(row)) => row,
            Ok(None) => break,
            Err(_) => return Err(read_failed()),
        };
        // The returned cursor is fed back as the next page's SQL OFFSET, so it must stay representable as an i64 (bridge.cpp:1087-1088).
        if i64::try_from(result.next_offset + 1).is_err() {
            return Err(EngineError::invalid(
                diagnostics::INVALID_EMOJI_CATALOG_CURSOR,
            ));
        }
        result.next_offset += 1;
        let column = |index| {
            row.get::<_, Option<String>>(index)
                .map_err(|_| read_failed())
        };
        let (text, group, annotation) = (column(0)?, column(1)?, column(2)?);
        let blank = |value: &Option<String>| value.as_deref().is_none_or(str::is_empty);
        if !deduplicate && (blank(&text) || (!kaomoji && blank(&group))) {
            continue;
        }
        if let Some(text) = text {
            if !deduplicate || !contains_catalog_text(&result.items, &text) {
                result.items.push(EmojiCatalogItem {
                    text,
                    annotation: annotation.unwrap_or_default(),
                    group: group.unwrap_or_default(),
                });
            }
        }
    }
    result.complete = result.next_offset - offset < limit;
    Ok(result)
}

fn contains_catalog_text(items: &[EmojiCatalogItem], text: &str) -> bool {
    items.iter().any(|item| item.text == text)
}

fn search_pattern(search: &str) -> String {
    let mut pattern = String::with_capacity(search.len() + 2);
    pattern.push('%');
    pattern.push_str(search);
    pattern.push('%');
    pattern
}

/// The groups of a category in first-appearance order.
pub fn emoji_catalog_groups(resources: &Path, category: &str) -> Result<Vec<String>> {
    let sql = match category {
        KAOMOJI_CATEGORY => KAOMOJI_GROUPS_SQL,
        SYMBOLS_CATEGORY => SYMBOLS_GROUPS_SQL,
        _ => EMOJI_GROUPS_SQL,
    };
    let connection = open_catalog(resources)?;
    let mut statement = prepare(&connection, sql)?;
    let rows = statement
        .query_map([], |row| row.get::<_, Option<String>>(0))
        .map_err(|_| read_failed())?;
    let mut groups = Vec::new();
    for row in rows {
        if let Some(group) = row.map_err(|_| read_failed())? {
            groups.push(group);
        }
    }
    Ok(groups)
}

/// Symbol groups under their parent categories.
pub fn emoji_symbol_groups(resources: &Path) -> Result<Vec<EmojiSymbolGroup>> {
    let connection = open_catalog(resources)?;
    let mut statement = prepare(&connection, SYMBOL_PARENTS_SQL)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        })
        .map_err(|_| read_failed())?;
    let mut groups = Vec::new();
    for row in rows {
        if let (Some(parent), Some(title)) = row.map_err(|_| read_failed())? {
            groups.push(EmojiSymbolGroup { parent, title });
        }
    }
    Ok(groups)
}

/// A fresh read-only connection per call, as bridge.cpp:1024-1030 opened one. The picker's calls are not per keystroke, and a cached handle would keep reading a replaced or once-unreadable `msime-others.db` until the process restarts.
fn open_catalog(resources: &Path) -> Result<Connection> {
    open_read_only(&resources.join(assets::OTHER_DICTIONARY))
        .map_err(|_| EngineError::failed(diagnostics::EMOJI_CATALOG_UNAVAILABLE))
}

// SQLite's messages can expose resource paths or data, so every failure maps to a fixed message (bridge.cpp:1062).

fn prepare<'a>(connection: &'a Connection, sql: &str) -> Result<rusqlite::CachedStatement<'a>> {
    connection
        .prepare_cached(sql)
        .map_err(|_| EngineError::failed(diagnostics::EMOJI_CATALOG_QUERY_UNAVAILABLE))
}

fn bind(
    statement: &mut Statement<'_>,
    index: usize,
    value: impl rusqlite::ToSql,
) -> rusqlite::Result<()> {
    statement.raw_bind_parameter(index, value)
}

fn read_failed() -> EngineError {
    EngineError::failed(diagnostics::EMOJI_CATALOG_READ_FAILED)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let resources = dir.path().to_path_buf();
        Connection::open(resources.join(assets::OTHER_DICTIONARY))
            .unwrap()
            .execute_batch(
                "CREATE TABLE emoji (emoji TEXT NOT NULL, category TEXT NOT NULL, sort_order INTEGER NOT NULL, keywords TEXT NOT NULL, pinyin TEXT NOT NULL, PRIMARY KEY (emoji)) WITHOUT ROWID;
                 INSERT INTO emoji VALUES('😀','Smileys and emotion',1,'笑脸 grinning','xiaolian');
                 INSERT INTO emoji VALUES('😄','Smileys and emotion',2,'大笑 smile','daxiao');
                 INSERT INTO emoji VALUES('🐶','Animals',3,'狗 dog','gou');
                 INSERT INTO emoji VALUES('🙂','',4,'微笑','weixiao');
                 INSERT INTO emoji VALUES('😺','Animals',0,'猫 cat','mao');
                 CREATE TABLE kaomoji_catalog (kaomoji TEXT NOT NULL, sort_order INTEGER NOT NULL, keywords TEXT NOT NULL, PRIMARY KEY (kaomoji)) WITHOUT ROWID;
                 INSERT INTO kaomoji_catalog VALUES('(^_^)',2,'开心');
                 INSERT INTO kaomoji_catalog VALUES('(*/ω＼*)',1,'害羞');
                 CREATE TABLE symbol_catalog (symbol TEXT NOT NULL, category TEXT NOT NULL, parent_category TEXT NOT NULL, sort_order INTEGER NOT NULL, keywords TEXT NOT NULL, PRIMARY KEY (symbol, category)) WITHOUT ROWID;
                 INSERT INTO symbol_catalog VALUES('→','Arrows','Math',2,'right arrow');
                 INSERT INTO symbol_catalog VALUES('←','Arrows','Math',3,'left arrow');
                 INSERT INTO symbol_catalog VALUES('+','Operators','Math',1,'plus');
                 INSERT INTO symbol_catalog VALUES('→','Favorites','',4,'right');
                 INSERT INTO symbol_catalog VALUES('','Arrows','Math',5,'blank');",
            )
            .unwrap();
        (dir, resources)
    }

    fn texts(slice: &EmojiCatalogSlice) -> Vec<&str> {
        slice.items.iter().map(|item| item.text.as_str()).collect()
    }

    #[test]
    fn catalog_text_lookup_uses_owned_items() {
        let items = vec![EmojiCatalogItem {
            text: "😀".into(),
            annotation: String::new(),
            group: String::new(),
        }];
        assert!(contains_catalog_text(&items, "😀"));
        assert!(!contains_catalog_text(&items, "😄"));
    }

    #[test]
    fn search_pattern_allocates_only_result_bytes() {
        let pattern = search_pattern("arrow");
        assert_eq!(pattern, "%arrow%");
        assert_eq!(pattern.capacity(), pattern.len());
    }

    #[test]
    fn emoji_pages() {
        let (_dir, resources) = fixture();
        let all = read_emoji_catalog_slice(&resources, "", "", "", 0, 10, "", false).unwrap();
        assert_eq!(texts(&all), ["😺", "😀", "😄", "🐶"]);
        assert_eq!(all.next_offset, 5);
        assert!(all.complete);
        assert_eq!(
            all.items[1],
            EmojiCatalogItem {
                text: "😀".into(),
                annotation: "笑脸 grinning".into(),
                group: "Smileys and emotion".into(),
            }
        );

        let page = read_emoji_catalog_slice(&resources, "", "", "", 1, 2, "", false).unwrap();
        assert_eq!(texts(&page), ["😀", "😄"]);
        assert_eq!(page.next_offset, 3);
        assert!(!page.complete);

        let animals =
            read_emoji_catalog_slice(&resources, "", "Animals", "", 0, 10, "", false).unwrap();
        assert_eq!(texts(&animals), ["😺", "🐶"]);
        let by_group =
            read_emoji_catalog_slice(&resources, "", "ignored", "Animals", 0, 10, "", true)
                .unwrap();
        assert_eq!(texts(&by_group), ["😺", "🐶"]);

        let search =
            read_emoji_catalog_slice(&resources, "xiao", "", "", 0, 10, "", false).unwrap();
        assert_eq!(texts(&search), ["😀", "😄"]);
        // Deduplicated pages keep rows with an empty group.
        let dedup =
            read_emoji_catalog_slice(&resources, "weixiao", "", "", 0, 10, "", true).unwrap();
        assert_eq!(texts(&dedup), ["🙂"]);
        assert_eq!(dedup.items[0].group, "");
    }

    #[test]
    fn symbol_and_kaomoji_pages() {
        let (_dir, resources) = fixture();
        let symbols =
            read_emoji_catalog_slice(&resources, "", "symbols", "", 0, 10, "", false).unwrap();
        assert_eq!(texts(&symbols), ["+", "→", "←", "→"]);
        assert_eq!(symbols.next_offset, 5);
        let dedup =
            read_emoji_catalog_slice(&resources, "", "symbols", "", 0, 10, "", true).unwrap();
        assert_eq!(texts(&dedup), ["+", "→", "←", ""]);
        let arrows =
            read_emoji_catalog_slice(&resources, "", "symbols", "Arrows", 0, 10, "", false)
                .unwrap();
        assert_eq!(texts(&arrows), ["→", "←"]);
        let favorites =
            read_emoji_catalog_slice(&resources, "", "symbols", "", 0, 10, "Favorites", false)
                .unwrap();
        assert_eq!(texts(&favorites), ["→"]);
        let search =
            read_emoji_catalog_slice(&resources, "arrow", "symbols", "", 0, 10, "", false).unwrap();
        assert_eq!(texts(&search), ["→", "←"]);

        let kaomoji =
            read_emoji_catalog_slice(&resources, "", "kaomoji", "", 0, 10, "", false).unwrap();
        assert_eq!(texts(&kaomoji), ["(*/ω＼*)", "(^_^)"]);
        assert_eq!(kaomoji.items[0].group, "All");
        assert_eq!(kaomoji.items[0].annotation, "害羞");
        let all =
            read_emoji_catalog_slice(&resources, "", "kaomoji", "All", 0, 10, "", false).unwrap();
        assert_eq!(all.items.len(), 2);
        let other =
            read_emoji_catalog_slice(&resources, "", "kaomoji", "Other", 0, 10, "", false).unwrap();
        assert!(other.items.is_empty());
        assert!(other.complete);
        let search =
            read_emoji_catalog_slice(&resources, "开心", "kaomoji", "", 0, 10, "", false).unwrap();
        assert_eq!(texts(&search), ["(^_^)"]);
    }

    #[test]
    fn groups() {
        let (_dir, resources) = fixture();
        assert_eq!(
            emoji_catalog_groups(&resources, "kaomoji").unwrap(),
            ["All"]
        );
        assert_eq!(
            emoji_catalog_groups(&resources, "symbols").unwrap(),
            ["Operators", "Arrows", "Favorites"]
        );
        assert_eq!(
            emoji_catalog_groups(&resources, "emoji").unwrap(),
            ["Animals", "Smileys and emotion"]
        );
        assert_eq!(
            emoji_symbol_groups(&resources).unwrap(),
            [
                EmojiSymbolGroup {
                    parent: "Math".into(),
                    title: "Operators".into()
                },
                EmojiSymbolGroup {
                    parent: "Math".into(),
                    title: "Arrows".into()
                },
                EmojiSymbolGroup {
                    parent: "Favorites".into(),
                    title: "Favorites".into()
                },
            ]
        );
    }

    #[test]
    fn errors_carry_fixed_messages() {
        let (_dir, resources) = fixture();
        for limit in [0, MAXIMUM_PAGE + 1] {
            let error =
                read_emoji_catalog_slice(&resources, "", "", "", 0, limit, "", false).unwrap_err();
            assert!(matches!(error, EngineError::InvalidArgument(_)));
            assert_eq!(error.to_string(), diagnostics::INVALID_EMOJI_CATALOG_PAGE);
        }
        let error =
            read_emoji_catalog_slice(&resources, "", "", "", usize::MAX, 1, "", false).unwrap_err();
        assert_eq!(error.to_string(), diagnostics::INVALID_EMOJI_CATALOG_PAGE);
        assert!(
            read_emoji_catalog_slice(&resources, "", "", "", 0, MAXIMUM_PAGE, "", false).is_ok()
        );

        let missing = tempfile::tempdir().unwrap();
        let error = emoji_catalog_groups(missing.path(), "emoji").unwrap_err();
        assert_eq!(error.to_string(), diagnostics::EMOJI_CATALOG_UNAVAILABLE);
        assert!(!missing.path().join(assets::OTHER_DICTIONARY).exists());

        let empty = tempfile::tempdir().unwrap();
        Connection::open(empty.path().join(assets::OTHER_DICTIONARY))
            .unwrap()
            .execute_batch("CREATE TABLE unrelated(x)")
            .unwrap();
        let error =
            read_emoji_catalog_slice(empty.path(), "", "", "", 0, 1, "", false).unwrap_err();
        assert_eq!(
            error.to_string(),
            diagnostics::EMOJI_CATALOG_QUERY_UNAVAILABLE
        );
        let error = emoji_symbol_groups(empty.path()).unwrap_err();
        assert_eq!(
            error.to_string(),
            diagnostics::EMOJI_CATALOG_QUERY_UNAVAILABLE
        );
    }
}
