//! `kind = "wordbook"`：单词本，作为一本词书出现在背单词里。
//!
//! ```toml
//! [wordbook]
//! file = "words.tsv"
//! ```
//!
//! `[wordbook]` 只有 `file` 一个键，点名包里的一个 `.tsv` 数据文件：1 字节到 4 MiB，UTF-8，可以带 BOM。按 `\n` 分行，每行去掉一个行尾的 `\r`，其余位置的 `\r` 算控制字符。空行和 `#` 开头的行跳过；其余每行是 `单词\t释义` 或 `单词\t音标\t释义`，各列不去空白，引号是普通字符。每条都必须通过 `WordbookEntry::is_valid`，一本 1 到 20000 个单词，同一个单词不能出现两次，任何一行不合规整包拒绝。这里不用背单词导入的宽松解析器（`vocabulary::import::parse`），因为那个会跳过坏行。
//!
//! 包的 id 还要能当词书 id：书的 id 是 `pack-<插件 id>`，所以插件 id 必须满足 `wordbook::id_is_well_formed` 且不超过 59 个字符，`name` 不超过 `wordbook::MAX_NAME_CHARS` 个字符。选中哪本书保存在背单词的进度文档里，这里不加偏好。

use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use toml::Value;

use super::{data_lines, only_keys, read_file, valid_file_name, PluginContent, PluginKind};
use crate::vocabulary::wordbook::{self, Wordbook, WordbookEntry};

pub(crate) const MANIFEST_KEYS: [&str; 1] = ["wordbook"];

/// 词表文件的字节上限。
pub const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
/// 词表文件的扩展名。
pub const FILE_EXTENSION: &str = "tsv";
/// 插件词书 id 的前缀：书的 id 是 `pack-<插件 id>`。
pub const BOOK_ID_PREFIX: &str = "pack-";
/// 插件 id 的字符上限，让 `pack-<插件 id>` 不超过 `wordbook::MAX_ID_CHARS`。
pub const MAX_PACK_ID_CHARS: usize = wordbook::MAX_ID_CHARS - BOOK_ID_PREFIX.len();
/// 详情里预览的单词数。
pub const PREVIEW_WORDS: usize = 5;

/// 设置页看到的单词本：文件名、单词数和前几个单词，不带整本书。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WordbookPack {
    pub file: String,
    pub word_count: usize,
    pub first_words: Vec<String>,
}

/// 清单里点名的词表文件名；内容由 [`read`] 在文件检查之后读取。
pub(crate) fn parse(table: &toml::map::Map<String, Value>) -> Result<String, String> {
    let section = table
        .get("wordbook")
        .and_then(Value::as_table)
        .ok_or("单词本缺少 wordbook 表")?;
    only_keys(section, &["file"], "wordbook")?;
    let name = section
        .get("file")
        .and_then(Value::as_str)
        .ok_or("wordbook.file 必须是字符串")?;
    if !valid_file_name(name) {
        return Err(format!("wordbook.file 的文件名 {name} 无效"));
    }
    Ok(name.to_owned())
}

/// 插件 id 和名字能否当作一本词书的 id 和名字。
pub(crate) fn check_identity(id: &str, name: &str) -> Result<(), String> {
    if !wordbook::id_is_well_formed(id) || id.chars().count() > MAX_PACK_ID_CHARS {
        return Err(format!(
            "单词本的 id 只能由小写字母、数字和 - 组成，首尾不能是 -，且不超过 {MAX_PACK_ID_CHARS} 个字符"
        ));
    }
    if name.chars().count() > wordbook::MAX_NAME_CHARS {
        return Err(format!(
            "单词本的 name 不能超过 {} 个字符",
            wordbook::MAX_NAME_CHARS
        ));
    }
    Ok(())
}

/// 读取并严格解析包目录里的词表。
pub(crate) fn read(directory: &Path, file: &str) -> Result<WordbookPack, String> {
    let entries = read_entries(directory, file)?;
    Ok(WordbookPack {
        file: file.to_owned(),
        word_count: entries.len(),
        first_words: entries
            .into_iter()
            .take(PREVIEW_WORDS)
            .map(|entry| entry.word)
            .collect(),
    })
}

fn read_entries(directory: &Path, file: &str) -> Result<Vec<WordbookEntry>, String> {
    let bytes =
        read_file(directory, file, MAX_FILE_BYTES).map_err(|_| format!("{file} 无法读取或太大"))?;
    parse_words(&bytes, file)
}

/// 词表文本的严格解析，规则见模块文档。
pub(crate) fn parse_words(bytes: &[u8], name: &str) -> Result<Vec<WordbookEntry>, String> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for (number, line) in data_lines(bytes, name)? {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        let (word, phonetic, meaning) = match columns.as_slice() {
            [word, meaning] => (*word, "", *meaning),
            [word, phonetic, meaning] => (*word, *phonetic, *meaning),
            _ => {
                return Err(format!(
                    "{name} 第 {number} 行必须是「单词\\t释义」或「单词\\t音标\\t释义」"
                ))
            }
        };
        let entry = WordbookEntry {
            word: word.to_owned(),
            phonetic: phonetic.to_owned(),
            meaning: meaning.to_owned(),
        };
        if !entry.is_valid() {
            return Err(format!(
                "{name} 第 {number} 行的单词或释义为空，或者某一列太长、含有控制字符"
            ));
        }
        if !seen.insert(entry.word.clone()) {
            return Err(format!("{name} 里「{}」出现了不止一次", entry.word));
        }
        if entries.len() == wordbook::MAX_ENTRIES {
            return Err(format!("{name} 的单词数超过 {}", wordbook::MAX_ENTRIES));
        }
        entries.push(entry);
    }
    if entries.is_empty() {
        return Err(format!("{name} 里没有任何单词"));
    }
    Ok(entries)
}

/// 插件 `id` 对应的词书 id：`pack-<id>`。
pub fn book_id(id: &str) -> String {
    format!("{BOOK_ID_PREFIX}{id}")
}

/// 词书 id 是不是插件词书。
pub fn is_pack_book(book: &str) -> bool {
    book.starts_with(BOOK_ID_PREFIX)
}

/// 已安装的单词本插件里 `book`（`pack-<插件 id>`）那一本，按 `scan` 列出包的规则校验过。插件不在或载入失败时为 `None`：背单词把它当作已经不在的书，显示书目选择。读一个最大 4 MiB 的文件，只解析一遍：清单和文件先按 `load_package` 的规则检查，词表的严格解析既是校验也是结果。
pub fn load_book(root: &Path, book: &str) -> Option<Wordbook> {
    let id = book.strip_prefix(BOOK_ID_PREFIX)?;
    let package = super::load_package_unread(root, PluginKind::Wordbook, id).ok()?;
    let PluginContent::Wordbook(pack) = &package.content else {
        return None;
    };
    let entries = read_entries(&package.directory, &pack.file).ok()?;
    Some(Wordbook {
        id: book_id(&package.id),
        name: package.name,
        entries,
    })
}

/// `root` 下每个能载入的单词本插件，作为背单词书目里的一行，按名字和 id 排序。载入失败的包不列出，插件页会报告它。
pub fn summaries(root: &Path) -> Vec<crate::vocabulary::library::WordbookSummary> {
    let mut books: Vec<_> = super::scan_kind_packages(root, PluginKind::Wordbook)
        .into_iter()
        .filter_map(|package| match package.content {
            PluginContent::Wordbook(pack) => Some(crate::vocabulary::library::WordbookSummary {
                id: book_id(&package.id),
                name: package.name,
                total: pack.word_count,
                builtin: false,
                pack: true,
            }),
            _ => None,
        })
        .collect();
    books.sort_by(|a, b| (&a.name, &a.id).cmp(&(&b.name, &b.id)));
    books
}
