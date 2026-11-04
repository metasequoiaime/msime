//! 不拿来拼整句的粗俗词。词表、豁免写法、收词标准和为什么只管拼句见 `offensive_words.txt` 的文件头。
//!
//! 词网格给多字词每个音节 `PHRASE_LENGTH_BONUS` 的奖励，一个只有几千权重的两字词也能压过同码的两个常用单字，所以一个罕见的粗俗词会被拼进用户本来想打的句子当首选（九键 我们走路去 解成「我们有巨乳」）。含粗俗词的长词条也一样（个鸡巴、吃屎了、是傻逼），所以按子串查：词里任何一段在表里就算命中，除非这一段被一条豁免写法整个盖住（水乳交融 里的 乳交、口交换机 里的 口交）。整句是猜出来的，用户没打这个词；命中的词因此不进词网格（整段输入恰好是这个词时除外，那时它就是这串输入对应的词库行），也不进纠错整句（纠错边的词来自另一串读音，盖住整段也不用）和上屏后的联想。词库行不受影响。
//!
//! 表有两个来源。词库仓库 msime-dictionary 维护 `curation/offensive-words.txt`，dict-builder 把它写进 `msime-pinyin.db` 的 `sentence_excluded` 表（`format::SENTENCE_EXCLUDED_TABLE`，见 `docs/data-formats.md` §4.5）；词库带着这张表时，引擎打开词库就用它（[`OffensiveWords::from_database`]），换表随词库发版，不用发引擎。旧词库没有这张表，或者表读不出来、一个词都没有时，退回编进引擎的 `offensive_words.txt`。两个来源的格式和查法完全相同，内嵌表在编译期解析成静态数组，读进来的表在打开词库时排好序放进 `Arc`，按键路径上两者都只做二分查找，不分配内存。

use std::cmp::Ordering;
use std::sync::Arc;

use rusqlite::Connection;

use crate::format::SENTENCE_EXCLUDED_TABLE;

const WORDS: &str = include_str!("offensive_words.txt");

/// 编进引擎的词表文件原文：`msime-pinyin.db` 没有 `sentence_excluded` 表时用它。dict-builder 生成报告时也拿它当默认表，和引擎读同一份。
pub const EMBEDDED_OFFENSIVE_WORDS: &str = WORDS;

/// 豁免写法在词表文件里以这个字符开头。
const EXEMPT_MARK: u8 = b'!';

const LISTED_COUNT: usize = count_entries(false);
const EXEMPT_COUNT: usize = count_entries(true);

/// 表里的词，按字节序排好，查表用二分查找。和豁免写法一样在编译期从 `offensive_words.txt` 解析出来：运行时不建表、不分配内存，第一次按键不付初始化的开销，数分配次数的测试也不受先后顺序影响。
static LISTED: [&str; LISTED_COUNT] = parse_entries(false);
/// 豁免写法，去掉了开头的 `!`。
static EXEMPT: [&str; EXEMPT_COUNT] = parse_entries(true);
/// 表里最长的词有几个字，按子串查时每个起点最多看这么长。
const LONGEST: usize = longest(&parse_entries::<LISTED_COUNT>(false));

/// 从 `rest` 开头取下一个条目：跳过空行和 `#` 开头的行，去掉首尾空白。返回这个条目和它之后剩下的部分。
const fn next_entry(mut rest: &'static [u8]) -> Option<(&'static [u8], &'static [u8])> {
    while !rest.is_empty() {
        let mut end = 0;
        while end < rest.len() && rest[end] != b'\n' {
            end += 1;
        }
        let (line, tail) = rest.split_at(end);
        rest = if tail.is_empty() {
            tail
        } else {
            tail.split_at(1).1
        };
        let line = line.trim_ascii();
        if !line.is_empty() && line[0] != b'#' {
            return Some((line, rest));
        }
    }
    None
}

/// 词表文件里有几个词（`exempt` 为假）或几条豁免写法（`exempt` 为真）。
const fn count_entries(exempt: bool) -> usize {
    let mut count = 0;
    let mut rest = WORDS.as_bytes();
    while let Some((entry, tail)) = next_entry(rest) {
        rest = tail;
        if (entry[0] == EXEMPT_MARK) == exempt {
            count += 1;
        }
    }
    count
}

/// 按字节比较两个串，与运行时 `str` 的 `Ord` 一致，二分查找因此能用编译期排出的顺序。
const fn compare(a: &[u8], b: &[u8]) -> Ordering {
    let mut i = 0;
    while i < a.len() && i < b.len() {
        if a[i] != b[i] {
            return if a[i] < b[i] {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        i += 1;
    }
    if a.len() < b.len() {
        Ordering::Less
    } else if a.len() > b.len() {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

/// 取出全部词或全部豁免写法，按字节序插入排序。重复的条目和不是 UTF-8 的条目让编译失败。
const fn parse_entries<const N: usize>(exempt: bool) -> [&'static str; N] {
    let mut out = [""; N];
    let mut len = 0;
    let mut rest = WORDS.as_bytes();
    while let Some((entry, tail)) = next_entry(rest) {
        rest = tail;
        if (entry[0] == EXEMPT_MARK) != exempt {
            continue;
        }
        let text = if exempt { entry.split_at(1).1 } else { entry };
        let Ok(text) = std::str::from_utf8(text) else {
            panic!("offensive_words.txt 里有不是 UTF-8 的条目");
        };
        let mut at = len;
        while at > 0 {
            match compare(out[at - 1].as_bytes(), text.as_bytes()) {
                Ordering::Less => break,
                Ordering::Equal => panic!("offensive_words.txt 里有重复的条目"),
                Ordering::Greater => {
                    out[at] = out[at - 1];
                    at -= 1;
                }
            }
        }
        out[at] = text;
        len += 1;
    }
    out
}

/// 最长的条目有几个字（数 UTF-8 里不是续字节的字节）。
const fn longest(entries: &[&str]) -> usize {
    let mut longest = 0;
    let mut i = 0;
    while i < entries.len() {
        let bytes = entries[i].as_bytes();
        let mut chars = 0;
        let mut j = 0;
        while j < bytes.len() {
            if bytes[j] & 0xC0 != 0x80 {
                chars += 1;
            }
            j += 1;
        }
        if chars > longest {
            longest = chars;
        }
        i += 1;
    }
    longest
}

/// `word` 是否含内嵌表里的粗俗词，见 [`matches`]。
pub(crate) fn is_offensive(word: &str) -> bool {
    matches(&LISTED, &EXEMPT, LONGEST, word)
}

/// `word` 是否含 `listed` 里的词：它本身或其中任何一段在表里，而且这一段没有被 `exempt` 里的豁免写法盖住。`listed` 按字节序排好，`longest` 是其中最长的词有几个字。表里只有两字以上的词，单字（UTF-8 不超过 4 字节）不查表，词网格里大多数边是单字。
fn matches<S: AsRef<str>>(listed: &[S], exempt: &[S], longest: usize, word: &str) -> bool {
    if word.len() <= 4 {
        return false;
    }
    for (start, _) in word.char_indices() {
        let rest = &word[start..];
        for (count, (offset, ch)) in rest.char_indices().enumerate() {
            if count >= longest {
                break;
            }
            let end = offset + ch.len_utf8();
            if count >= 1
                && listed
                    .binary_search_by(|probe| probe.as_ref().cmp(&rest[..end]))
                    .is_ok()
                && !exempted(exempt, word, start, start + end)
            {
                return true;
            }
        }
    }
    false
}

/// `word[start..end]` 这一段命中了词表时，有没有一条豁免写法在 `word` 里出现并整个盖住这一段。
fn exempted<S: AsRef<str>>(exempt: &[S], word: &str, start: usize, end: usize) -> bool {
    exempt.iter().any(|pattern| {
        let pattern = pattern.as_ref();
        word.match_indices(pattern)
            .any(|(at, _)| at <= start && end <= at + pattern.len())
    })
}

/// 词网格里 `start..end` 这条词库行的边可不可以用 `word`：含粗俗词的词只在独占整段输入（`0..len`）时成边，这时整句就是这个词本身，与它的词库行相同。纠错边不用它，见 `decode_typo_on_graph_with_policy`。
#[cfg(test)]
pub(crate) fn admits_edge(word: &str, start: usize, end: usize, len: usize) -> bool {
    OffensiveWords::default().admits_edge(word, start, end, len)
}

/// 词表文件里的一条：词（`exempt` 为假）或豁免写法（`exempt` 为真，已去掉开头的 `!`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OffensiveEntry {
    pub text: String,
    pub exempt: bool,
}

/// 按词表文件的格式（`offensive_words.txt` 的文件头）解析 `text`，并检查内嵌表的单元测试也检查的那几条规则：每条至少两个字、全是汉字、不重复；豁免写法不在表里、而且含一个表里的词；至少有一个词。dict-builder 用它读 msime-dictionary 的 `curation/offensive-words.txt`，所以两边对同一个文件的理解不会分岔。出错时返回带行号的说明。
pub fn parse_offensive_words(text: &str) -> std::result::Result<Vec<OffensiveEntry>, String> {
    let mut entries: Vec<OffensiveEntry> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (text, exempt) = match line.strip_prefix(char::from(EXEMPT_MARK)) {
            Some(pattern) => (pattern, true),
            None => (line, false),
        };
        if text.chars().count() < 2 || !crate::text::is_all_han(text) {
            return Err(format!(
                "line {}: {line} is not two or more Han characters",
                index + 1
            ));
        }
        if !seen.insert(text.to_owned()) {
            return Err(format!("line {}: {text} is listed twice", index + 1));
        }
        entries.push(OffensiveEntry {
            text: text.to_owned(),
            exempt,
        });
    }
    let listed: Vec<&str> = entries
        .iter()
        .filter(|entry| !entry.exempt)
        .map(|entry| entry.text.as_str())
        .collect();
    if listed.is_empty() {
        return Err("no word is listed".to_owned());
    }
    for entry in entries.iter().filter(|entry| entry.exempt) {
        if !listed.iter().any(|word| entry.text.contains(word)) {
            return Err(format!(
                "the exempt pattern !{} contains no listed word, so it exempts nothing",
                entry.text
            ));
        }
    }
    Ok(entries)
}

/// 从词库读进来的表。
#[derive(Debug)]
struct LoadedWords {
    listed: Vec<String>,
    exempt: Vec<String>,
    longest: usize,
}

/// 这次用哪张粗俗词表：默认是内嵌表，词库带着 `sentence_excluded` 时是那张表。复制只是复制一个 `Arc`，词网格的每份选项各持一份也不分配内存。
#[derive(Debug, Clone, Default)]
pub struct OffensiveWords {
    loaded: Option<Arc<LoadedWords>>,
}

impl OffensiveWords {
    /// 用这些条目建表；一个词都没有时为 `None`，调用方应退回内嵌表，免得一张空表把过滤整个关掉。重复的条目只留一条。
    pub fn from_entries(entries: impl IntoIterator<Item = OffensiveEntry>) -> Option<Self> {
        let (mut listed, mut exempt) = (Vec::new(), Vec::new());
        for entry in entries {
            if entry.exempt {
                exempt.push(entry.text);
            } else {
                listed.push(entry.text);
            }
        }
        if listed.is_empty() {
            return None;
        }
        listed.sort_unstable();
        listed.dedup();
        exempt.sort_unstable();
        exempt.dedup();
        let longest = listed
            .iter()
            .map(|word| word.chars().count())
            .max()
            .unwrap_or(0);
        Some(Self {
            loaded: Some(Arc::new(LoadedWords {
                listed,
                exempt,
                longest,
            })),
        })
    }

    /// 读 `connection` 的 `sentence_excluded` 表（`value` 是词或豁免写法，`exempt` 非零表示豁免写法）。没有这张表（`dict-v2.0.x` 早期的词库）、表的列不对、读的时候出错，或者表里一个词都没有，都用内嵌表：这是词库数据的边界，读不懂的表不能让过滤失效。
    pub(crate) fn from_database(connection: &Connection) -> Self {
        let read = || -> rusqlite::Result<Vec<OffensiveEntry>> {
            let mut statement = connection.prepare(&format!(
                "SELECT value, exempt FROM \"{SENTENCE_EXCLUDED_TABLE}\""
            ))?;
            let rows = statement.query_map([], |row| {
                Ok(OffensiveEntry {
                    text: row.get::<_, String>(0)?,
                    exempt: row.get::<_, i64>(1)? != 0,
                })
            })?;
            rows.collect()
        };
        read().ok().and_then(Self::from_entries).unwrap_or_default()
    }

    /// 是不是内嵌表。
    pub fn is_embedded(&self) -> bool {
        self.loaded.is_none()
    }

    /// `word` 是否含这张表里的粗俗词，规则见 [`matches`]。
    pub fn is_offensive(&self, word: &str) -> bool {
        match &self.loaded {
            None => is_offensive(word),
            Some(table) => matches(&table.listed, &table.exempt, table.longest, word),
        }
    }

    /// 词网格里 `start..end` 这条词库行的边可不可以用 `word`：含粗俗词的词只在独占整段输入（`0..len`）时成边，这时整句就是这个词本身，与它的词库行相同。纠错边不用它，见 `decode_typo_on_graph_with_policy`。
    pub(crate) fn admits_edge(&self, word: &str, start: usize, end: usize, len: usize) -> bool {
        (start == 0 && end == len) || !self.is_offensive(word)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// 用标准库逐行解析词表文件，与编译期的 `parse_entries` 互相核对。
    fn entries() -> impl Iterator<Item = &'static str> {
        WORDS
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
    }

    #[test]
    fn list_holds_only_multi_character_han_words_once_each() {
        let mut seen = HashSet::new();
        for entry in entries() {
            let text = entry.strip_prefix(char::from(EXEMPT_MARK)).unwrap_or(entry);
            assert!(text.chars().count() >= 2, "{entry} 不到两个字");
            assert!(crate::text::is_all_han(text), "{entry} 不全是汉字");
            assert!(seen.insert(entry), "{entry} 重复");
        }
        assert_eq!(seen.len(), LISTED.len() + EXEMPT.len());
    }

    #[test]
    fn compile_time_tables_match_the_file_and_are_sorted() {
        let mut listed: Vec<&str> = entries()
            .filter(|entry| !entry.starts_with(char::from(EXEMPT_MARK)))
            .collect();
        listed.sort_unstable();
        assert_eq!(LISTED.as_slice(), listed.as_slice());
        let mut exempt: Vec<&str> = entries()
            .filter_map(|entry| entry.strip_prefix(char::from(EXEMPT_MARK)))
            .collect();
        exempt.sort_unstable();
        assert_eq!(EXEMPT.as_slice(), exempt.as_slice());
        let longest = listed.iter().map(|word| word.chars().count()).max();
        assert_eq!(Some(LONGEST), longest);
    }

    #[test]
    fn every_exempt_pattern_covers_a_listed_word_and_is_not_one() {
        for pattern in &EXEMPT {
            assert!(
                LISTED.binary_search(pattern).is_err(),
                "{pattern} 既在表里又是豁免写法"
            );
            assert!(
                LISTED.iter().any(|word| pattern.contains(word)),
                "{pattern} 不含表里的词，豁免它没有意义"
            );
        }
    }

    #[test]
    fn offensive_words_are_recognised_and_ordinary_words_are_not() {
        assert!(is_offensive("巨乳"));
        assert!(is_offensive("他妈的"));
        for word in [
            "走路", "我们", "乳", "巨", "邮局", "坚韧", "阴道", "强奸", "",
        ] {
            assert!(!is_offensive(word), "{word}");
        }
    }

    #[test]
    fn a_longer_word_containing_an_offensive_word_is_offensive() {
        for word in [
            "个鸡巴",
            "净鸡巴",
            "吃屎了",
            "去吃屎",
            "是傻逼",
            "都是傻逼",
            "我们有巨乳",
            "去他妈的",
            "约炮吗",
            "童颜巨乳",
            "巨乳模特",
            "性奴调教",
        ] {
            assert!(is_offensive(word), "{word}");
        }
    }

    #[test]
    fn an_exempt_pattern_releases_only_the_part_it_covers() {
        for word in [
            "水乳交融",
            "口交换机",
            "多口交换机",
            "港口交货",
            "人口爆炸",
            "众口交赞",
            "内射函数",
            "马奶子酒",
            "奶子酒",
            "酸奶子",
            "奸夫淫妇",
            "狗吃屎",
            "摔了个狗吃屎",
            "狗改不了吃屎",
            "巨乳症",
            "巨乳缩小术",
            "性奴隶",
            "荡妇羞辱",
        ] {
            assert!(!is_offensive(word), "{word}");
        }
        // 豁免写法只放行它盖住的那一段，同一个词里别处的命中照样算。
        assert!(is_offensive("水乳交融傻逼"));
        assert!(is_offensive("口交换机口交"));
    }

    /// 运行时的解析和编译期的解析对内嵌文件给出同样的词和豁免写法，dict-builder 用前者读词库仓库里的同格式文件。
    #[test]
    fn the_runtime_parser_reads_the_embedded_file_as_the_compile_time_tables() {
        let entries = parse_offensive_words(EMBEDDED_OFFENSIVE_WORDS).unwrap();
        let mut listed: Vec<&str> = entries
            .iter()
            .filter(|entry| !entry.exempt)
            .map(|entry| entry.text.as_str())
            .collect();
        listed.sort_unstable();
        assert_eq!(LISTED.as_slice(), listed.as_slice());
        let mut exempt: Vec<&str> = entries
            .iter()
            .filter(|entry| entry.exempt)
            .map(|entry| entry.text.as_str())
            .collect();
        exempt.sort_unstable();
        assert_eq!(EXEMPT.as_slice(), exempt.as_slice());
    }

    #[test]
    fn the_runtime_parser_rejects_what_the_embedded_file_may_not_hold() {
        for (text, problem) in [
            ("巨\n", "two or more"),
            ("巨乳\nabc\n", "two or more"),
            ("巨乳\n巨乳\n", "twice"),
            ("巨乳\n!巨乳\n", "twice"),
            ("巨乳\n!走路\n", "no listed word"),
            ("# 只有注释\n!水乳交\n", "no word"),
        ] {
            let error = parse_offensive_words(text).unwrap_err();
            assert!(error.contains(problem), "{text:?}: {error}");
        }
    }

    fn entry(text: &str, exempt: bool) -> OffensiveEntry {
        OffensiveEntry {
            text: text.to_owned(),
            exempt,
        }
    }

    /// 读进来的表整个取代内嵌表：表里没有的 巨乳 放行，表里的 走路 拦下，豁免写法照样只放行它盖住的一段。
    #[test]
    fn a_loaded_table_replaces_the_embedded_one() {
        let table =
            OffensiveWords::from_entries([entry("走路", false), entry("我们走路", true)]).unwrap();
        assert!(!table.is_embedded());
        assert!(!table.is_offensive("巨乳"));
        assert!(table.is_offensive("去走路"));
        assert!(!table.is_offensive("我们走路"));
        assert!(table.is_offensive("我们走路走路"));
        assert!(!table.admits_edge("走路", 1, 3, 4));
        assert!(table.admits_edge("走路", 0, 2, 2));
        let embedded = OffensiveWords::default();
        assert!(embedded.is_embedded());
        assert!(embedded.is_offensive("巨乳"));
        assert!(!embedded.is_offensive("走路"));
    }

    /// 一个词都没有的表不能把过滤关掉：调用方拿到 `None`，退回内嵌表。
    #[test]
    fn a_table_without_a_listed_word_is_refused() {
        assert!(OffensiveWords::from_entries([entry("水乳交", true)]).is_none());
        assert!(OffensiveWords::from_entries([]).is_none());
    }

    /// 新引擎配旧词库（没有 `sentence_excluded`）、表的列不对、表是空的：都用内嵌表。新词库带表时用表。
    #[test]
    fn the_database_table_is_used_when_present_and_readable() {
        let connection = Connection::open_in_memory().unwrap();
        assert!(OffensiveWords::from_database(&connection).is_embedded());

        connection
            .execute_batch("CREATE TABLE sentence_excluded(value TEXT, flag INTEGER);")
            .unwrap();
        assert!(OffensiveWords::from_database(&connection).is_embedded());
        connection
            .execute_batch("DROP TABLE sentence_excluded;")
            .unwrap();

        connection
            .execute_batch(crate::format::SENTENCE_EXCLUDED_SCHEMA)
            .unwrap();
        assert!(OffensiveWords::from_database(&connection).is_embedded());
        connection
            .execute_batch(
                "INSERT INTO sentence_excluded VALUES('走路', 0); INSERT INTO sentence_excluded VALUES('我们走路', 1);",
            )
            .unwrap();
        let table = OffensiveWords::from_database(&connection);
        assert!(!table.is_embedded());
        assert!(table.is_offensive("去走路"));
        assert!(!table.is_offensive("我们走路"));
        assert!(!table.is_offensive("巨乳"));
    }

    #[test]
    fn an_offensive_word_is_an_edge_only_when_it_spans_the_whole_input() {
        assert!(admits_edge("巨乳", 0, 2, 2));
        assert!(!admits_edge("巨乳", 3, 5, 5));
        assert!(!admits_edge("巨乳", 0, 2, 3));
        assert!(!admits_edge("吃屎了", 2, 5, 5));
        assert!(admits_edge("水乳交融", 2, 6, 6));
        assert!(admits_edge("走路", 2, 4, 5));
    }
}
