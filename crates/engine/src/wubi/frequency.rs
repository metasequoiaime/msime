//! 五笔 14 键排序用的词频：一组键展开成多个编码，不同编码的行要按词本身有多常用来比。五笔码表的 `weight` 只排同一个编码里的先后（多是 20、10 这种档位），跨编码比不出高低；拼音主词库的权重是语料词频，用它来比。
//!
//! 单字一次扫完 `tbl_1_*`（约五万行）建成表，按词库路径在进程里缓存，和导入时的读音表一样；词按单字读音的首字母拼出简拼，到 `tbl_<字数>_<首字母>` 里按 `jp` 索引查，结果放进 LRU。拼音主词库不在或读不出时一律答 0，排序退回码表权重。

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use lru::LruCache;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};

/// 词库里有表的最长词（`tbl_7_*`）；更长的词没有词频。
const MAX_PHRASE_CHARACTERS: usize = 7;
/// 多音字让简拼有多种拼法，最多试这么多种。
const MAX_INITIAL_COMBINATIONS: usize = 16;
const PHRASE_CACHE_CAPACITY: usize = 4096;
const CHARACTER_TABLE_CACHE_CAPACITY: usize = 4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CharacterEntry {
    /// 这个字各个读音里最大的权重。
    weight: i64,
    /// 各个读音的首字母，`a`..=`z` 各占一位。
    initials: u32,
}

type CharacterTable = HashMap<char, CharacterEntry>;

static CHARACTER_TABLES: OnceLock<Mutex<LruCache<PathBuf, Arc<CharacterTable>>>> = OnceLock::new();

pub struct WubiFrequency {
    pinyin_db: PathBuf,
    connection: Option<Connection>,
    characters: Option<Arc<CharacterTable>>,
    phrases: LruCache<String, i64>,
    jianpin: String,
    sql: String,
}

impl WubiFrequency {
    /// `pinyin_db` 是拼音主词库；打开推迟到第一次要词频时。
    pub fn new(pinyin_db: &Path) -> Self {
        Self {
            pinyin_db: pinyin_db.to_path_buf(),
            connection: None,
            characters: None,
            phrases: LruCache::new(NonZeroUsize::new(PHRASE_CACHE_CAPACITY).unwrap()),
            jianpin: String::new(),
            sql: String::new(),
        }
    }

    /// 词库换了代次或学到了新词：关掉连接、清掉词的缓存，单字表按路径保留（单字表只读主词库自带的行）。
    pub fn reset_cache(&mut self) {
        self.connection = None;
        self.phrases.clear();
    }

    /// `word` 在拼音主词库里的权重；查不到是 0。
    pub fn weight(&mut self, word: &str) -> i64 {
        let mut characters = word.chars();
        let (Some(first), second) = (characters.next(), characters.next()) else {
            return 0;
        };
        if self.characters.is_none() {
            self.characters = Some(self.load_characters());
        }
        let table = Arc::clone(self.characters.as_ref().expect("单字表刚建好"));
        if second.is_none() {
            return table.get(&first).map_or(0, |entry| entry.weight);
        }
        if let Some(weight) = self.phrases.get(word) {
            return *weight;
        }
        let weight = self.phrase_weight(&table, word);
        self.phrases.put(word.to_owned(), weight);
        weight
    }

    fn connection(&mut self) -> Option<&Connection> {
        if self.connection.is_none() {
            self.connection = open(&self.pinyin_db);
        }
        self.connection.as_ref()
    }

    fn load_characters(&mut self) -> Arc<CharacterTable> {
        let cache = CHARACTER_TABLES.get_or_init(|| {
            Mutex::new(LruCache::new(
                NonZeroUsize::new(CHARACTER_TABLE_CACHE_CAPACITY).unwrap(),
            ))
        });
        if let Some(found) = cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&self.pinyin_db)
        {
            return Arc::clone(found);
        }
        let path = self.pinyin_db.clone();
        let Some(connection) = self.connection() else {
            return Arc::new(CharacterTable::new());
        };
        let (table, complete) = character_table(connection);
        let table = Arc::new(table);
        // 碰上别的进程正在写、只读到一部分的表这次照用，但不缓存，下次重建。
        if complete {
            cache
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .put(path, Arc::clone(&table));
        }
        table
    }

    fn phrase_weight(&mut self, table: &CharacterTable, word: &str) -> i64 {
        let length = word.chars().count();
        if length > MAX_PHRASE_CHARACTERS {
            return 0;
        }
        let mut masks = [0u32; MAX_PHRASE_CHARACTERS];
        let mut combinations = 1usize;
        for (slot, character) in masks.iter_mut().zip(word.chars()) {
            let Some(entry) = table.get(&character).filter(|entry| entry.initials != 0) else {
                return 0;
            };
            *slot = entry.initials;
            combinations = combinations.saturating_mul(entry.initials.count_ones() as usize);
        }
        let combinations = combinations.min(MAX_INITIAL_COMBINATIONS);
        let mut jianpin = std::mem::take(&mut self.jianpin);
        let mut sql = std::mem::take(&mut self.sql);
        let mut best = 0;
        for combination in 0..combinations {
            jianpin.clear();
            let mut rest = combination;
            for mask in &masks[..length] {
                let count = mask.count_ones() as usize;
                let initial = nth_initial(*mask, rest % count);
                rest /= count;
                jianpin.push(char::from(initial));
            }
            sql.clear();
            sql.push_str("SELECT max(\"weight\") FROM \"tbl_");
            sql.push_str(&length.to_string());
            sql.push('_');
            sql.push_str(&jianpin[..1]);
            sql.push_str("\" WHERE \"jp\" = ?1 AND \"value\" = ?2");
            let weight = self.connection().and_then(|connection| {
                connection
                    .prepare_cached(&sql)
                    .ok()?
                    .query_row((jianpin.as_str(), word), |row| {
                        Ok(match row.get_ref(0)? {
                            ValueRef::Integer(weight) => weight,
                            ValueRef::Real(weight) => weight as i64,
                            _ => 0,
                        })
                    })
                    .ok()
            });
            best = best.max(weight.unwrap_or(0));
        }
        self.jianpin = jianpin;
        self.sql = sql;
        best
    }
}

/// `mask` 里第 `index` 个置位对应的字母。
fn nth_initial(mask: u32, index: usize) -> u8 {
    let mut seen = 0;
    for offset in 0..26u8 {
        if mask & (1 << offset) != 0 {
            if seen == index {
                return b'a' + offset;
            }
            seen += 1;
        }
    }
    b'a'
}

fn open(path: &Path) -> Option<Connection> {
    if path.as_os_str().is_empty() {
        return None;
    }
    let path = crate::paths::sqlite_read_only_path_no_follow(path).ok()?;
    let connection = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NOFOLLOW
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    connection
        .busy_timeout(crate::dictionary::pinyin::BUSY_TIMEOUT)
        .ok()?;
    Some(connection)
}

/// 扫一遍各个 `tbl_1_<字母>`：每个字取最大权重，记下每个读音的首字母。表不存在就跳过；碰到忙或锁时第二项为假。
fn character_table(connection: &Connection) -> (CharacterTable, bool) {
    let busy = |error: &rusqlite::Error| {
        matches!(
            error.sqlite_error_code(),
            Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
        )
    };
    let mut complete = true;
    let mut table = CharacterTable::new();
    let mut sql = String::new();
    for initial in b'a'..=b'z' {
        sql.clear();
        sql.push_str("SELECT \"key\", \"value\", \"weight\" FROM \"tbl_1_");
        sql.push(char::from(initial));
        sql.push('"');
        let mut statement = match connection.prepare(&sql) {
            Ok(statement) => statement,
            Err(error) => {
                complete &= !busy(&error);
                continue;
            }
        };
        let mut rows = match statement.query(()) {
            Ok(rows) => rows,
            Err(error) => {
                complete &= !busy(&error);
                continue;
            }
        };
        loop {
            let row = match rows.next() {
                Ok(Some(row)) => row,
                Ok(None) => break,
                Err(error) => {
                    complete &= !busy(&error);
                    break;
                }
            };
            let (Ok(ValueRef::Text(key)), Ok(ValueRef::Text(value))) =
                (row.get_ref(0), row.get_ref(1))
            else {
                continue;
            };
            let Ok(value) = std::str::from_utf8(value) else {
                continue;
            };
            let mut characters = value.chars();
            let (Some(character), None) = (characters.next(), characters.next()) else {
                continue;
            };
            let weight = match row.get_ref(2) {
                Ok(ValueRef::Integer(weight)) => weight,
                Ok(ValueRef::Real(weight)) => weight as i64,
                _ => 0,
            };
            let entry = table.entry(character).or_default();
            entry.weight = entry.weight.max(weight);
            if let Some(first) = key.first().filter(|first| first.is_ascii_lowercase()) {
                entry.initials |= 1 << (first - b'a');
            }
        }
    }
    (table, complete)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// 测试用的拼音主词库：单字表和二字词表，权重是虚构的。
    pub(crate) const PINYIN_FIXTURE: &str = "
        CREATE TABLE tbl_1_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);
        CREATE TABLE tbl_1_h(key TEXT, jp TEXT, value TEXT, weight INTEGER);
        CREATE TABLE tbl_1_x(key TEXT, jp TEXT, value TEXT, weight INTEGER);
        CREATE TABLE tbl_1_d(key TEXT, jp TEXT, value TEXT, weight INTEGER);
        CREATE TABLE tbl_1_c(key TEXT, jp TEXT, value TEXT, weight INTEGER);
        CREATE TABLE tbl_1_w(key TEXT, jp TEXT, value TEXT, weight INTEGER);
        CREATE TABLE tbl_1_q(key TEXT, jp TEXT, value TEXT, weight INTEGER);
        CREATE TABLE tbl_1_y(key TEXT, jp TEXT, value TEXT, weight INTEGER);
        CREATE TABLE tbl_2_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);
        CREATE TABLE tbl_2_y(key TEXT, jp TEXT, value TEXT, weight INTEGER);
        INSERT INTO tbl_1_n VALUES('ni','n','你',3000000);
        INSERT INTO tbl_1_h VALUES('hao','h','好',2000000),('hang','h','行',2700000);
        INSERT INTO tbl_1_x VALUES('xing','x','行',2750000);
        INSERT INTO tbl_1_d VALUES('duo','d','多',1500000);
        INSERT INTO tbl_1_c VALUES('cong','c','从',1400000);
        INSERT INTO tbl_1_q VALUES('qian','q','欠',90000);
        INSERT INTO tbl_1_w VALUES('wo','w','我',9000000),('wo','w','卧',1000);
        INSERT INTO tbl_1_y VALUES('yin','y','银',300000);
        INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',332885);
        INSERT INTO tbl_2_y VALUES('yin''hang','yh','银行',250000);";

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("msime-pinyin.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch(PINYIN_FIXTURE)
            .unwrap();
        (root, path)
    }

    #[test]
    fn characters_take_their_heaviest_reading() {
        let (_root, path) = fixture();
        let mut frequency = WubiFrequency::new(&path);
        assert_eq!(frequency.weight("你"), 3000000);
        assert_eq!(frequency.weight("行"), 2750000);
        assert_eq!(frequency.weight("龘"), 0);
        assert_eq!(frequency.weight(""), 0);
    }

    #[test]
    fn phrases_are_found_through_every_reading_initial() {
        let (_root, path) = fixture();
        let mut frequency = WubiFrequency::new(&path);
        assert_eq!(frequency.weight("你好"), 332885);
        // 行 读 hang 和 xing，简拼 yh 和 yx 都要试。
        assert_eq!(frequency.weight("银行"), 250000);
        // 词库里没有的词、有字不认识的词都是 0。
        assert_eq!(frequency.weight("你多"), 0);
        assert_eq!(frequency.weight("你龘"), 0);
        assert_eq!(frequency.weight("你你你你你你你你"), 0);
    }

    #[test]
    fn a_missing_dictionary_answers_zero() {
        let root = tempfile::tempdir().unwrap();
        let mut frequency = WubiFrequency::new(&root.path().join("missing.db"));
        assert_eq!(frequency.weight("你"), 0);
        assert_eq!(frequency.weight("你好"), 0);
        let mut empty = WubiFrequency::new(Path::new(""));
        assert_eq!(empty.weight("你"), 0);
    }
}
