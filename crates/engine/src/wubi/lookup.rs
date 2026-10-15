//! 五笔查码：给一段汉字列出它在 86 和 98 码表里的编码，供设置页的「五笔查码」使用。只读码表，不建会话、不碰用户数据；字根拆分没有授权数据，不在这里。

use std::path::Path;

use rusqlite::Connection;

use super::provider::open_read_only;
use crate::text::is_han;

/// 一次最多列出的不同汉字数。粘贴一整段文章时只看前面这些，结果里标出截断。
pub const MAX_LOOKUP_CHARACTERS: usize = 64;
/// 整段当作一个词去查的最大字数；更长的文字不会是码表里的词。
pub const MAX_LOOKUP_PHRASE_CHARACTERS: usize = 16;
/// 每个码表每项最多列出的编码数。
const MAX_CODES_PER_TABLE: i64 = 8;

/// 同一个词在码表里的全部编码：短码在前，同样长的按权重。`GROUP BY` 把同一编码的重复行并成一行。
const CODES_SQL_86: &str = "SELECT \"key\" FROM wubi86 WHERE \"value\" = ?1 GROUP BY \"key\" ORDER BY length(\"key\") ASC, max(\"weight\") DESC, \"key\" ASC LIMIT ?2";
/// 与 `CODES_SQL_86` 相同，只是读 `wubi98`。
const CODES_SQL_98: &str = "SELECT \"key\" FROM wubi98 WHERE \"value\" = ?1 GROUP BY \"key\" ORDER BY length(\"key\") ASC, max(\"weight\") DESC, \"key\" ASC LIMIT ?2";

/// 一段文字（一个字或整段词）在两个码表里的编码，各自短码在前。码表里没有这段文字时对应列表为空。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WubiTextCodes {
    pub text: String,
    pub wubi86: Vec<String>,
    pub wubi98: Vec<String>,
}

/// 查码结果。`characters` 按文字里第一次出现的顺序列出每个不同的汉字；`phrase` 是整段文字作为一个词的编码，只在整段都是汉字、不止一个字、且至少一个码表有它时给出。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WubiCodeLookup {
    pub phrase: Option<WubiTextCodes>,
    pub characters: Vec<WubiTextCodes>,
    /// 不同的汉字超过 [`MAX_LOOKUP_CHARACTERS`]，后面的没有列出。
    pub truncated: bool,
}

/// 在 `database`（含 `wubi86`/`wubi98` 两张表的五笔码表）里查 `text` 的编码。数据库打不开时每一项都没有编码；只缺一张表时只有那一列为空，旧的发布只带 `wubi86` 也照常可用。
pub fn lookup_codes(database: &Path, text: &str) -> WubiCodeLookup {
    let connection = open_read_only(database);
    let codes = |word: &str| WubiTextCodes {
        text: word.to_owned(),
        wubi86: table_codes(connection.as_ref(), CODES_SQL_86, word),
        wubi98: table_codes(connection.as_ref(), CODES_SQL_98, word),
    };

    let mut seen = ['\0'; MAX_LOOKUP_CHARACTERS];
    let (seen_length, truncated) = distinct_han_characters(text, &mut seen);
    let characters = seen[..seen_length]
        .iter()
        .map(|character| codes(character.encode_utf8(&mut [0; 4])))
        .collect();

    let trimmed = text.trim();
    let length = trimmed.chars().count();
    let phrase =
        (length > 1 && length <= MAX_LOOKUP_PHRASE_CHARACTERS && trimmed.chars().all(is_han))
            .then(|| codes(trimmed))
            .filter(|phrase| !phrase.wubi86.is_empty() || !phrase.wubi98.is_empty());

    WubiCodeLookup {
        phrase,
        characters,
        truncated,
    }
}

fn distinct_han_characters(text: &str, seen: &mut [char; MAX_LOOKUP_CHARACTERS]) -> (usize, bool) {
    let mut seen_length = 0;
    let mut truncated = false;
    for character in text.chars().filter(|character| is_han(*character)) {
        if seen[..seen_length].contains(&character) {
            continue;
        }
        if seen_length == MAX_LOOKUP_CHARACTERS {
            truncated = true;
            break;
        }
        seen[seen_length] = character;
        seen_length += 1;
    }
    (seen_length, truncated)
}

/// 一张表里 `word` 的编码；表不存在或查询失败时为空，与输入时反查的规则一样不把失败报给用户。
fn table_codes(connection: Option<&Connection>, sql: &str, word: &str) -> Vec<String> {
    let Some(connection) = connection else {
        return Vec::new();
    };
    let Ok(mut statement) = connection.prepare_cached(sql) else {
        return Vec::new();
    };
    statement
        .query_map((word, MAX_CODES_PER_TABLE), |row| row.get::<_, String>(0))
        .map(|rows| rows.filter_map(std::result::Result::ok).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database(sql: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("msime-wubi.db");
        Connection::open(&path).unwrap().execute_batch(sql).unwrap();
        (root, path)
    }

    const FIXTURE: &str = "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
        INSERT INTO wubi86 VALUES('wqiy','你',100),('wq','你',200),('wqi','你',50),('wq','你',10),('vbg','好',100),('vb','好',300),('wqvb','你好',300);
        CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER);
        INSERT INTO wubi98 VALUES('wqiy','你',100),('wq','你',200),('vbg','好',100);";

    fn codes(text: &str, wubi86: &[&str], wubi98: &[&str]) -> WubiTextCodes {
        WubiTextCodes {
            text: text.into(),
            wubi86: wubi86.iter().map(|code| (*code).into()).collect(),
            wubi98: wubi98.iter().map(|code| (*code).into()).collect(),
        }
    }

    #[test]
    fn distinct_character_scan_does_not_need_a_heap_buffer() {
        let mut seen = ['\0'; MAX_LOOKUP_CHARACTERS];
        let ((length, truncated), allocations) =
            crate::ime::personal_rerank::allocations::count(|| {
                distinct_han_characters("你你好", &mut seen)
            });
        assert_eq!((length, truncated), (2, false));
        assert_eq!(&seen[..length], &['你', '好']);
        assert_eq!(
            allocations, 0,
            "去重汉字扫描不应为工作区申请堆存储: {allocations}"
        );
    }

    #[test]
    fn distinct_character_scan_marks_the_limit_without_losing_order() {
        let mut seen = ['\0'; MAX_LOOKUP_CHARACTERS];
        let text: String = (0..=MAX_LOOKUP_CHARACTERS)
            .map(|index| char::from_u32(0x4e00 + index as u32).unwrap())
            .collect();
        let (length, truncated) = distinct_han_characters(&text, &mut seen);
        assert_eq!(length, MAX_LOOKUP_CHARACTERS);
        assert!(truncated);
        assert_eq!(seen[0], '一');
        assert_eq!(
            seen[MAX_LOOKUP_CHARACTERS - 1],
            char::from_u32(0x4e00 + (MAX_LOOKUP_CHARACTERS - 1) as u32).unwrap()
        );
    }

    #[test]
    fn distinct_character_scan_matches_the_original_vector_scan() {
        fn original_scan(text: &str) -> (Vec<char>, bool) {
            let mut seen = Vec::new();
            for character in text.chars().filter(|character| is_han(*character)) {
                if seen.contains(&character) {
                    continue;
                }
                if seen.len() == MAX_LOOKUP_CHARACTERS {
                    return (seen, true);
                }
                seen.push(character);
            }
            (seen, false)
        }

        let mut inputs = vec![String::new(), "abc !😀".into(), "你你好𠀀好𠀀".into()];
        for count in [1, 4, 5, 33, 63, 64, 65, 80] {
            let text: String = (0..count)
                .map(|offset| char::from_u32(0x4e00 + offset).unwrap())
                .collect();
            inputs.push(text.clone());
            inputs.push(format!("abc{text}{text}😀"));
        }
        for input in inputs {
            let ((expected, expected_truncated), old_allocations) =
                crate::ime::personal_rerank::allocations::count(|| original_scan(&input));
            let mut seen = ['\0'; MAX_LOOKUP_CHARACTERS];
            let ((length, truncated), allocations) =
                crate::ime::personal_rerank::allocations::count(|| {
                    distinct_han_characters(&input, &mut seen)
                });
            assert_eq!(&seen[..length], expected);
            assert_eq!(truncated, expected_truncated);
            assert_eq!(allocations, 0);
            assert_eq!(old_allocations > allocations, !expected.is_empty());
        }
    }

    #[test]
    fn every_code_of_each_character_is_listed_shortest_first() {
        let (_root, path) = database(FIXTURE);
        let lookup = lookup_codes(&path, "你好");
        assert_eq!(
            lookup.characters,
            vec![
                codes("你", &["wq", "wqi", "wqiy"], &["wq", "wqiy"]),
                codes("好", &["vb", "vbg"], &["vbg"]),
            ]
        );
        // 整段是一个词：86 有它的码，98 没有。
        assert_eq!(lookup.phrase, Some(codes("你好", &["wqvb"], &[])));
        assert!(!lookup.truncated);
    }

    #[test]
    fn non_han_text_is_skipped_and_repeats_are_listed_once() {
        let (_root, path) = database(FIXTURE);
        let lookup = lookup_codes(&path, " 你，hello 你好！\n");
        let listed: Vec<&str> = lookup
            .characters
            .iter()
            .map(|item| item.text.as_str())
            .collect();
        assert_eq!(listed, ["你", "好"]);
        // 带标点的一段话不是一个词。
        assert_eq!(lookup.phrase, None);
        // 一个字时只有逐字的结果。
        assert_eq!(lookup_codes(&path, "你").phrase, None);
        assert_eq!(lookup_codes(&path, "").characters, Vec::new());
    }

    #[test]
    fn a_character_the_tables_lack_is_listed_without_codes() {
        let (_root, path) = database(FIXTURE);
        let lookup = lookup_codes(&path, "你龘");
        assert_eq!(lookup.characters[1], codes("龘", &[], &[]));
        assert_eq!(lookup.phrase, None);
    }

    #[test]
    fn a_missing_table_or_database_leaves_only_that_column_empty() {
        let (_root, path) = database(
            "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
             INSERT INTO wubi86 VALUES('wq','你',1);",
        );
        assert_eq!(
            lookup_codes(&path, "你").characters,
            vec![codes("你", &["wq"], &[])]
        );
        let missing = path.with_file_name("missing.db");
        assert_eq!(
            lookup_codes(&missing, "你").characters,
            vec![codes("你", &[], &[])]
        );
    }

    #[test]
    fn long_text_is_cut_at_the_character_limit() {
        let (_root, path) = database(FIXTURE);
        let text: String = (0..(MAX_LOOKUP_CHARACTERS as u32 + 10))
            .filter_map(|offset| char::from_u32(0x4E00 + offset))
            .collect();
        let lookup = lookup_codes(&path, &text);
        assert_eq!(lookup.characters.len(), MAX_LOOKUP_CHARACTERS);
        assert_eq!(lookup.characters.capacity(), lookup.characters.len());
        assert!(lookup.truncated);
        assert_eq!(lookup.phrase, None);
    }

    #[test]
    fn character_result_keeps_only_the_required_row_capacity() {
        let (_root, path) = database(FIXTURE);
        let text: String = (0..33)
            .map(|offset| char::from_u32(0x4e00 + offset).unwrap())
            .collect();
        let lookup = lookup_codes(&path, &text);
        assert_eq!(lookup.characters.len(), 33);
        assert_eq!(lookup.characters.capacity(), 33);
        assert!(!lookup.truncated);
    }
}
