//! Cantonese input in toneless Jyutping, read against `msime-cantonese.db` (`language_dictionary`) and the Cantonese rows of the user journal (`UserWords`): the words the user added to the personal dictionary, and what typing learned. Output is Traditional as stored. 选词按拼音的学习开关调频、造词，规则见 [`learning`]；发布的 `msime-cantonese.db` 只读，学到的都写在日志里。

pub mod learning;
pub mod scheme;
pub mod shuangpin;
pub mod syllable;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::{Connection, OpenFlags};

pub use scheme::{CantoneseCandidate, CantoneseScheme};
pub use syllable::Inventory;

use crate::error::Result;
use crate::language_dictionary::{self, LanguageDictionary, LanguageEntry};
use crate::types::PersonalDictionaryKind;
use crate::user_dictionary::journal::open_database;

/// The non-letter keys the scheme spells with while composing: `'` is an explicit syllable boundary.
pub const SPELLING_SYMBOLS_COMPOSING: &str = "'";

/// `msime-cantonese.db` opened for one activation of the scheme, with its syllable inventory read once.
pub struct CantoneseDictionary {
    dictionary: LanguageDictionary,
    inventory: Arc<Inventory>,
}

impl CantoneseDictionary {
    /// Opens `path` read-only; fails as `language_dictionary::open_read_only` does, which the caller reports as the scheme being unavailable.
    pub fn open(path: &Path) -> Result<Self> {
        let dictionary = language_dictionary::open_read_only(path)?;
        let inventory = Arc::new(Inventory::load(&dictionary)?);
        Ok(Self {
            dictionary,
            inventory,
        })
    }

    pub fn dictionary(&self) -> &LanguageDictionary {
        &self.dictionary
    }

    /// The inventory a `CantoneseScheme` segments against.
    pub fn inventory(&self) -> Arc<Inventory> {
        Arc::clone(&self.inventory)
    }
}

/// 用户日志里的粤语行（`PersonalDictionaryKind::Cantonese`）：用户在个人词库里加的词和打字造的词（`user_inserted=1`），以及调频写下的权重（`user_inserted=0`，同一个词在发布词库里也有，合并时取较大的权重）。它们只记在用户日志 `user_dictionary_operations` 里，没有工作表，所以粤语方案自己读：整份读进内存，之后每次查词前只问一次 `PRAGMA data_version`，别的连接（设置页、键盘的个人词库队列、会话学习时经 `open_journal` 开的那条）提交过才重读。逐键的查询因此不碰日志，也不分配。
///
/// 日志还不存在时什么也不打开，下次查词再看；读失败（另一个进程正在提交、表还没建）时保留上一次读到的词，不让候选因此消失。连接只读，忙时不等（`busy_timeout` 为 0），输入线程不会被别的进程的写入卡住。
pub struct UserWords {
    journal: PathBuf,
    connection: Option<Connection>,
    /// 上次读词时的 `PRAGMA data_version`；`None` 表示还没成功读过。
    version: Option<i64>,
    /// 键是不带声调、以单个空格连接的粤拼（与 `msime-cantonese.db` 的 `entries.key` 同一写法），值按权重从重到轻、同权重按文字排好。
    words: BTreeMap<String, Vec<LanguageEntry>>,
    /// `words` 的内容每变一次加一（重读到的词与原来相同时不变）。按读音缓存查词结果的地方（组句的 `CantoneseQueryBuffer::best`）凭它判断缓存是否还对得上。
    generation: u64,
}

impl UserWords {
    /// `journal` 是用户日志的位置（`RuntimePaths::user(assets::USER_JOURNAL)`）；这里不打开它。
    pub fn new(journal: PathBuf) -> Self {
        Self {
            journal,
            connection: None,
            version: None,
            words: BTreeMap::new(),
            generation: 0,
        }
    }

    /// 日志被别的连接改过（或还没读过）时重读用户的粤语词。
    pub fn refresh(&mut self) {
        if self.connection.is_none() {
            if !self.journal.is_file() {
                return;
            }
            let Ok(connection) = open_database(&self.journal, OpenFlags::SQLITE_OPEN_READ_ONLY)
            else {
                return;
            };
            if connection.busy_timeout(std::time::Duration::ZERO).is_err() {
                return;
            }
            self.connection = Some(connection);
        }
        let Some(connection) = &self.connection else {
            return;
        };
        let Ok(version) = connection
            .prepare_cached("PRAGMA data_version")
            .and_then(|mut statement| statement.query_row([], |row| row.get::<_, i64>(0)))
        else {
            return;
        };
        if self.version == Some(version) {
            return;
        }
        if let Ok(words) = read_user_words(connection) {
            if words != self.words {
                self.words = words;
                self.generation = self.generation.wrapping_add(1);
            }
            self.version = Some(version);
        }
    }

    fn exact(&self, key: &str) -> &[LanguageEntry] {
        self.words.get(key).map_or(&[], Vec::as_slice)
    }

    /// 键以 `prefix` 开头、`prefix` 之后不再有音节边界的词，即补全 `prefix` 最后一个音节的那些（同 `LanguageDictionary::lookup_completions`）。
    fn completions<'a>(
        &'a self,
        prefix: &'a str,
    ) -> impl Iterator<Item = (&'a String, &'a LanguageEntry)> + 'a {
        self.words
            .range::<str, _>((
                std::ops::Bound::Included(prefix),
                std::ops::Bound::Unbounded,
            ))
            .take_while(move |(key, _)| key.starts_with(prefix))
            .filter(move |(key, _)| !key[prefix.len()..].contains(' '))
            .flat_map(|(key, entries)| entries.iter().map(move |entry| (key, entry)))
    }
}

fn read_user_words(
    connection: &Connection,
) -> rusqlite::Result<BTreeMap<String, Vec<LanguageEntry>>> {
    let mut statement = connection.prepare(
        "SELECT key,value,weight FROM user_dictionary_operations WHERE dictionary=?1 AND operation='upsert'",
    )?;
    let mut rows = statement.query([PersonalDictionaryKind::Cantonese.journal_name()])?;
    let mut words = BTreeMap::<String, Vec<LanguageEntry>>::new();
    while let Some(row) = rows.next()? {
        words.entry(row.get(0)?).or_default().push(LanguageEntry {
            text: row.get(1)?,
            weight: row.get(2)?,
        });
    }
    for entries in words.values_mut() {
        entries.sort_by(|left, right| {
            right
                .weight
                .cmp(&left.weight)
                .then_with(|| left.text.cmp(&right.text))
        });
    }
    Ok(words)
}

/// 粤语方案查词的来源：发布的 `msime-cantonese.db`，加上用户的粤语词（可能没有）。两边同一个键下的同一个词合成一行，取较大的权重；合并后仍按权重从重到轻、同权重按文字排，至多 `limit` 行，与只查发布词库时的顺序规则相同。
#[derive(Clone, Copy)]
pub struct CantoneseLexicon<'a> {
    dictionary: &'a LanguageDictionary,
    user: Option<&'a UserWords>,
    /// `UserWords::generation`，在空词表被滤成 `None` 之前取：用户把词删光也算一次变化。
    user_generation: u64,
}

impl<'a> CantoneseLexicon<'a> {
    pub fn new(dictionary: &'a LanguageDictionary, user: Option<&'a UserWords>) -> Self {
        Self {
            dictionary,
            user_generation: user.map_or(0, |user| user.generation),
            user: user.filter(|user| !user.words.is_empty()),
        }
    }

    /// 用户粤语词的内容版本（`UserWords::generation`）；不带用户词时是 0。发布词库在会话内只读，查词结果只会随它变。
    pub(crate) fn user_generation(&self) -> u64 {
        self.user_generation
    }

    /// `key` 下有没有 `text` 这一行（发布词库或用户的粤语行），不分配。
    pub(crate) fn contains(&self, key: &str, text: &str) -> Result<bool> {
        if self
            .user
            .is_some_and(|user| user.exact(key).iter().any(|entry| entry.text == text))
        {
            return Ok(true);
        }
        self.dictionary.contains(key, text)
    }

    /// `LanguageDictionary::lookup_readings(positions, 1)`，并入用户的词：键按位置逐段取自 `positions[i]` 的词条里最重的一条，两边按同一规则比较（权重从重到轻，再按文字、键）。同一个键下的同一个词两边都有时取较大的权重，与 `lookup_into` 的合并规则一致。
    pub(crate) fn lookup_best_reading(
        &self,
        positions: &[&[String]],
    ) -> Result<Option<(String, LanguageEntry)>> {
        let mut best = self
            .dictionary
            .lookup_readings(positions, 1)?
            .into_iter()
            .next();
        let Some(user) = self.user else {
            return Ok(best);
        };
        let Some((first, rest)) = positions.split_first() else {
            return Ok(best);
        };
        let mut consider = |key: &String, entries: &Vec<LanguageEntry>| {
            // 每个键下的词已按权重从重到轻、同权重按文字排好，第一条就是这个键最重的。
            let Some(entry) = entries.first() else {
                return;
            };
            let heavier = best.as_ref().is_none_or(|(best_key, best_entry)| {
                entry
                    .weight
                    .cmp(&best_entry.weight)
                    .then_with(|| best_entry.text.cmp(&entry.text))
                    .then_with(|| best_key.cmp(key))
                    .is_gt()
            });
            if heavier {
                best = Some((key.clone(), entry.clone()));
            }
        };
        for reading in first.iter() {
            if rest.is_empty() {
                if let Some((key, entries)) = user.words.get_key_value(reading.as_str()) {
                    consider(key, entries);
                }
                continue;
            }
            // 以 `reading` 加空格开头的键都落在 [`reading `, `reading!`) 里（`!` 紧跟空格），同发布库的范围查询。
            let lower = format!("{reading} ");
            let upper = format!("{reading}!");
            for (key, entries) in user.words.range::<str, _>((
                std::ops::Bound::Included(lower.as_str()),
                std::ops::Bound::Excluded(upper.as_str()),
            )) {
                if language_dictionary::key_matches(key, positions) {
                    consider(key, entries);
                }
            }
        }
        Ok(best)
    }

    /// `LanguageDictionary::lookup_into`，并入用户在 `key` 下的词。
    pub(crate) fn lookup_into(
        &self,
        key: &str,
        limit: usize,
        result: &mut Vec<LanguageEntry>,
    ) -> Result<()> {
        self.dictionary.lookup_into(key, limit, result)?;
        let Some(user) = self.user else {
            return Ok(());
        };
        let added = user.exact(key);
        if added.is_empty() {
            return Ok(());
        }
        for entry in added {
            match result
                .iter_mut()
                .find(|existing| existing.text == entry.text)
            {
                Some(existing) => existing.weight = existing.weight.max(entry.weight),
                None => result.push(entry.clone()),
            }
        }
        result.sort_by(|left, right| {
            right
                .weight
                .cmp(&left.weight)
                .then_with(|| left.text.cmp(&right.text))
        });
        result.truncate(limit);
        Ok(())
    }

    /// `LanguageDictionary::lookup_completions_into`，并入用户补全 `prefix` 的词。
    pub(crate) fn lookup_completions_into(
        &self,
        prefix: &str,
        limit: usize,
        result: &mut Vec<(String, LanguageEntry)>,
    ) -> Result<()> {
        self.dictionary
            .lookup_completions_into(prefix, limit, result)?;
        let Some(user) = self.user else {
            return Ok(());
        };
        let mut merged = false;
        for (key, entry) in user.completions(prefix) {
            merged = true;
            match result
                .iter_mut()
                .find(|(existing_key, existing)| existing_key == key && existing.text == entry.text)
            {
                Some((_, existing)) => existing.weight = existing.weight.max(entry.weight),
                None => result.push((key.clone(), entry.clone())),
            }
        }
        if merged {
            result.sort_by(|(left_key, left), (right_key, right)| {
                right
                    .weight
                    .cmp(&left.weight)
                    .then_with(|| left.text.cmp(&right.text))
                    .then_with(|| left_key.cmp(right_key))
            });
            result.truncate(limit);
        }
        Ok(())
    }
}

impl<'a> From<&'a LanguageDictionary> for CantoneseLexicon<'a> {
    fn from(dictionary: &'a LanguageDictionary) -> Self {
        Self::new(dictionary, None)
    }
}
