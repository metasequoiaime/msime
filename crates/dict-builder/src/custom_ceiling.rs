//! `custom-words` 阶段给 `custom/words.txt` 新插入的词封顶（清洗方案 H11）：一个词在全拼表里原本没有这一行、由 custom 插入时，它的权重不超过同一全拼键、以及同一九键码上最高的常用词减 1。投稿权重是按音节数统一给的（2 音节 3855、3333），高于发布库 2 音节词的中位数，原样插入会让 挽昼 压过 万州、白冰 压过 百病。
//!
//! - 常用词是全拼表里权重不低于 [`COMMON_WEIGHT`] 的行，与清洗方案 §2.1 统计「custom 越位」时用的线一致。低于这条线的词不封顶 custom：投稿的新词本来就应该排在冷僻词前面。
//! - 九键码相同、音节数也相同的行互相竞争（`fang'an` 方案 与 `fan'gan` 反感、`wan'zhou` 与 `yan'zhou`）。同一全拼键的行音节数和九键码必然相同，所以九键这一条已经包含全拼这一条。音节数不同的行（`xian` 与 `xi'an`）不算：单字的权重与词不在一个量级，拿来封顶词会把词压得过低。
//! - 竞争者按构建做完 `reading-corrections`、`word-vetoes`、`weight-overrides` 之后的样子算（调用方给的 `effective_weight`）：被纠正的错误读音和被否决的行不算，被覆盖的行按覆盖后的权重算。否则一个之后会被降权的碎片词（门的）会先把 custom 词压下去。
//! - 只管新插入的词。custom 从已有的行提权、或已有的行不低于 custom 权重时，仍按 [`msime::apply_custom_words`] 只升不降；那些行的越位由维护者在 `curation/pinyin-weight-overrides.txt` 里处理（H2）。覆盖表在本阶段之后施加，也能把封过顶的词设成别的权重。
//! - 只改并入全拼表的权重，不改 `custom/words.txt`（只追加），也不影响五笔派生读到的投稿权重。

use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::{Context, Result};
use msime_engine::format::{quanpin_table, SHIPPED_INITIALS};
use msime_engine::KeyGrid;
use rusqlite::{params, Connection, OptionalExtension};

use crate::curation::PinyinCuration;
use crate::msime::{self, CustomWord, CustomWordCounts, ReadingCorrection};

/// 常用词的权重下限，见模块说明。
pub const COMMON_WEIGHT: i64 = 1000;

/// 构建在本阶段之后对已有行做的事：`reading-corrections` 删掉错误读音，`word-vetoes` 删掉否决的词和读音，`weight-overrides` 改权重。返回的函数给出一行在这些阶段之后的权重，被删的行为 `None`。
pub fn curated_weight<'a>(
    curation: &'a PinyinCuration,
    corrections: &[ReadingCorrection],
) -> impl Fn(&str, &str, i64) -> Option<i64> + 'a {
    let wrong: HashSet<(String, String)> = corrections
        .iter()
        .map(|correction| (correction.value.clone(), correction.wrong.clone()))
        .collect();
    move |value, key, weight| {
        if curation.vetoes.vetoes(value, key) || wrong.contains(&(value.to_owned(), key.to_owned()))
        {
            return None;
        }
        Some(curation.overrides.get(value, key).unwrap_or(weight))
    }
}

/// 一个被封顶的新词。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CappedWord {
    pub word: String,
    pub key: String,
    /// `custom/words.txt` 写的权重。
    pub requested: i64,
    /// 并入全拼表的权重：`competitor_weight - 1`。
    pub weight: i64,
    pub competitor: String,
    pub competitor_key: String,
    pub competitor_weight: i64,
}

impl CappedWord {
    /// 构建日志里的一行：哪个词从多少降到多少，压在哪个词下面，是同一全拼键还是只同九键码。
    pub fn describe(&self) -> String {
        let scope = if self.competitor_key == self.key {
            "同全拼"
        } else {
            "同九键码"
        };
        format!(
            "{} {} {} -> {}（{scope} {} {} {}）",
            self.word,
            self.key,
            self.requested,
            self.weight,
            self.competitor,
            self.competitor_key,
            self.competitor_weight
        )
    }
}

/// 竞争的范围：音节数和九键码。
type Slot = (usize, String);

fn slot(key: &str) -> Slot {
    (key.split('\'').count(), KeyGrid::NineKey.encode(key))
}

/// 一个九键码上权重最高的两个不同的词（词、全拼、权重），高的在前；两个是为了在最高的就是这个 custom 词本身（另一个读音）时还有下一个可比。
type Leaders = HashMap<Slot, Vec<(String, String, i64)>>;

/// 按 `slot` 的音节数只扫可能放着这些行的全拼表；超过 7 个音节的行在同一张溢出表里，按全拼重新数音节。`effective_weight` 返回 `None` 的行不算。
fn leaders(
    connection: &Connection,
    wanted: &BTreeSet<Slot>,
    effective_weight: &dyn Fn(&str, &str, i64) -> Option<i64>,
) -> Result<Leaders> {
    let mut tables = BTreeSet::new();
    for (syllables, _) in wanted {
        for initial in SHIPPED_INITIALS.bytes() {
            tables.insert(
                quanpin_table(*syllables, initial).context("a custom word maps to no table")?,
            );
        }
    }
    let mut leaders: Leaders = HashMap::new();
    for table in tables {
        let mut statement =
            connection.prepare(&format!("select key, value, weight from {table}"))?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let (key, value, weight): (String, String, i64) =
                (row.get(0)?, row.get(1)?, row.get(2)?);
            let Some(weight) = effective_weight(&value, &key, weight) else {
                continue;
            };
            if weight < COMMON_WEIGHT {
                continue;
            }
            let slot = slot(&key);
            if !wanted.contains(&slot) {
                continue;
            }
            let entry = leaders.entry(slot).or_default();
            match entry.iter_mut().find(|(other, _, _)| *other == value) {
                Some(existing) if existing.2 >= weight => continue,
                Some(existing) => *existing = (value, key, weight),
                None => entry.push((value, key, weight)),
            }
            // 权重相同时按词、全拼排，报告里点名的竞争者不随表的顺序变。
            entry.sort_by(|a, b| {
                b.2.cmp(&a.2)
                    .then_with(|| a.0.cmp(&b.0))
                    .then_with(|| a.1.cmp(&b.1))
            });
            entry.truncate(2);
        }
    }
    Ok(leaders)
}

/// 按模块说明给新插入的词封顶后，交给 [`msime::apply_custom_words`] 并入全拼表。返回它的计数和被封顶的词（按 `entries` 的顺序）。
pub fn apply_custom_words_with_ceiling(
    connection: &mut Connection,
    entries: &[CustomWord],
    effective_weight: &dyn Fn(&str, &str, i64) -> Option<i64>,
) -> Result<(CustomWordCounts, Vec<CappedWord>)> {
    let mut new = vec![false; entries.len()];
    for (index, entry) in entries.iter().enumerate() {
        let table = msime::pinyin_table(&entry.key).context("custom word without a table")?;
        new[index] = connection
            .query_row(
                &format!("select 1 from {table} where key = ? and value = ? limit 1"),
                params![entry.key, entry.value],
                |_| Ok(()),
            )
            .optional()?
            .is_none();
    }
    let wanted: BTreeSet<Slot> = entries
        .iter()
        .zip(&new)
        .filter(|(_, new)| **new)
        .map(|(entry, _)| slot(&entry.key))
        .collect();
    let leaders = if wanted.is_empty() {
        Leaders::new()
    } else {
        leaders(connection, &wanted, effective_weight)?
    };
    let mut capped = Vec::new();
    let mut adjusted = entries.to_vec();
    for (entry, new) in adjusted.iter_mut().zip(&new) {
        if !*new {
            continue;
        }
        let Some((competitor, competitor_key, competitor_weight)) = leaders
            .get(&slot(&entry.key))
            .and_then(|words| words.iter().find(|(word, _, _)| *word != entry.value))
        else {
            continue;
        };
        let ceiling = competitor_weight - 1;
        if entry.weight <= ceiling {
            continue;
        }
        capped.push(CappedWord {
            word: entry.value.clone(),
            key: entry.key.clone(),
            requested: entry.weight,
            weight: ceiling,
            competitor: competitor.clone(),
            competitor_key: competitor_key.clone(),
            competitor_weight: *competitor_weight,
        });
        entry.weight = ceiling;
    }
    let counts = msime::apply_custom_words(connection, &adjusted)?;
    Ok((counts, capped))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msime::parse_custom_words;

    fn database(rows: &[(&str, &str, i64)]) -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        for table in msime::quanpin_tables() {
            connection
                .execute_batch(&format!(
                    "create table {table} (key text, jp text, value text, weight integer default 0);"
                ))
                .unwrap();
        }
        for (value, key, weight) in rows {
            let table = msime::pinyin_table(key).unwrap();
            connection
                .execute(
                    &format!("insert into {table} (key, jp, value, weight) values (?, '', ?, ?)"),
                    params![key, value, weight],
                )
                .unwrap();
        }
        connection
    }

    fn weight(connection: &Connection, value: &str, key: &str) -> Option<i64> {
        let table = msime::pinyin_table(key).unwrap();
        connection
            .query_row(
                &format!("select weight from {table} where key = ? and value = ?"),
                params![key, value],
                |row| row.get(0),
            )
            .optional()
            .unwrap()
    }

    fn unchanged(_: &str, _: &str, weight: i64) -> Option<i64> {
        Some(weight)
    }

    #[test]
    fn new_words_stay_below_common_words_of_the_same_key_or_nine_key_code() {
        let mut connection = database(&[
            ("万州", "wan'zhou", 3000),
            ("兖州", "yan'zhou", 5000),
            ("百病", "bai'bing", 1500),
            ("掌纹", "zhang'wen", 900),
            ("先", "xian", 500000),
        ]);
        let words = parse_custom_words(
            "挽昼\twan'zhou\t3333\n白冰\tbai'bing\t1200\n张雯\tzhang'wen\t3855\n嘻安\txi'an\t3855\n鲜鲜\txian'xian\t3855\n",
        )
        .unwrap();
        let (counts, capped) =
            apply_custom_words_with_ceiling(&mut connection, &words, &unchanged).unwrap();
        assert_eq!(counts.inserted, 5);
        // 挽昼 同全拼只有 万州 3000，但同九键码 9269468 上还有 兖州 5000：按最高的常用词封顶，3333 不超过 4999，不动。
        assert_eq!(weight(&connection, "挽昼", "wan'zhou"), Some(3333));
        // 白冰 1200 本来就在 百病 1500 之下。掌纹 900 不是常用词，张雯 不封顶。
        assert_eq!(weight(&connection, "白冰", "bai'bing"), Some(1200));
        assert_eq!(weight(&connection, "张雯", "zhang'wen"), Some(3855));
        // 单字 先 xian 与 嘻安 xi'an 九键码相同但音节数不同，不算竞争者。
        assert_eq!(weight(&connection, "嘻安", "xi'an"), Some(3855));
        assert_eq!(weight(&connection, "鲜鲜", "xian'xian"), Some(3855));
        assert!(capped.is_empty(), "{capped:?}");
    }

    #[test]
    fn a_new_word_above_the_common_word_is_capped_one_below_it() {
        let mut connection = database(&[
            ("万州", "wan'zhou", 3000),
            ("兖州", "yan'zhou", 2000),
            ("百病", "bai'bing", 1500),
            ("方案", "fang'an", 2500),
            ("反感", "fan'gan", 1800),
            ("门的", "men'de", 435135),
            ("闷的", "men'de", 3000),
        ]);
        let words = parse_custom_words(
            "挽昼\twan'zhou\t3333\n白冰\tbai'bing\t3855\n凡甘\tfan'gan\t3855\n们得\tmen'de\t3855\n",
        )
        .unwrap();
        // 覆盖表把 门的 降到 100：它不再是常用词，们得 只压在 闷的 下面。
        let curated = |value: &str, key: &str, weight: i64| {
            Some(if (value, key) == ("门的", "men'de") {
                100
            } else {
                weight
            })
        };
        let (counts, capped) =
            apply_custom_words_with_ceiling(&mut connection, &words, &curated).unwrap();
        assert_eq!(counts.inserted, 4);
        assert_eq!(weight(&connection, "挽昼", "wan'zhou"), Some(2999));
        assert_eq!(weight(&connection, "白冰", "bai'bing"), Some(1499));
        // 凡甘 fan'gan 同全拼的 反感 只有 1800，同九键码 326426 的 方案 fang'an 2500 更高。
        assert_eq!(weight(&connection, "凡甘", "fan'gan"), Some(2499));
        assert_eq!(weight(&connection, "们得", "men'de"), Some(2999));
        assert_eq!(
            capped.iter().map(CappedWord::describe).collect::<Vec<_>>(),
            [
                "挽昼 wan'zhou 3333 -> 2999（同全拼 万州 wan'zhou 3000）",
                "白冰 bai'bing 3855 -> 1499（同全拼 百病 bai'bing 1500）",
                "凡甘 fan'gan 3855 -> 2499（同九键码 方案 fang'an 2500）",
                "们得 men'de 3855 -> 2999（同全拼 闷的 men'de 3000）",
            ]
        );
    }

    #[test]
    fn existing_rows_keep_the_raise_only_rule_and_removed_rows_do_not_cap() {
        let mut connection = database(&[
            ("万州", "wan'zhou", 3000),
            ("挽昼", "wan'zhou", 10),
            ("百病", "bai'bing", 1500),
            ("白冰", "bai'bing", 2000),
            ("车检", "che'jian", 5000),
            ("车间", "che'jian", 9000),
        ]);
        let words = parse_custom_words(
            "挽昼\twan'zhou\t3333\n白冰\tbai'bing\t1800\n车坚\tche'jian\t6000\n",
        )
        .unwrap();
        // 车间 被否决（或是之后要纠正掉的错误读音）：不算竞争者，车坚 只压在 车检 下面。
        let curated = |value: &str, _: &str, weight: i64| (value != "车间").then_some(weight);
        let (counts, capped) =
            apply_custom_words_with_ceiling(&mut connection, &words, &curated).unwrap();
        assert_eq!(
            counts,
            CustomWordCounts {
                inserted: 1,
                promoted: 1,
                unchanged: 1
            }
        );
        // 已有的行照旧只升不降：挽昼 从 10 提到 3333，白冰 2000 高于 1800 不变。
        assert_eq!(weight(&connection, "挽昼", "wan'zhou"), Some(3333));
        assert_eq!(weight(&connection, "白冰", "bai'bing"), Some(2000));
        assert_eq!(weight(&connection, "车坚", "che'jian"), Some(4999));
        assert_eq!(capped.len(), 1);
    }

    #[test]
    fn the_word_itself_under_another_reading_is_not_its_own_competitor() {
        // 方案 已有 fang'an，custom 再加 fan'gan 这个读音：两者九键码相同，但 方案 不和自己比，比的是同码的下一个词 反感。
        let mut connection = database(&[("方案", "fang'an", 2500), ("反感", "fan'gan", 1800)]);
        let words = parse_custom_words("方案\tfan'gan\t3000\n").unwrap();
        let (_, capped) =
            apply_custom_words_with_ceiling(&mut connection, &words, &unchanged).unwrap();
        assert_eq!(weight(&connection, "方案", "fan'gan"), Some(1799));
        assert_eq!(
            capped.iter().map(CappedWord::describe).collect::<Vec<_>>(),
            ["方案 fan'gan 3000 -> 1799（同全拼 反感 fan'gan 1800）"]
        );
    }

    #[test]
    fn curated_weights_drop_corrected_and_vetoed_rows_and_apply_overrides() {
        let curation =
            PinyinCuration::parse("车间\n闷的\tmen'de\n", "门的\tmen'de\t100\n").unwrap();
        let corrections = msime::parse_reading_corrections("血型\txie'xing\txue'xing\n").unwrap();
        let weight = curated_weight(&curation, &corrections);
        assert_eq!(weight("车间", "che'jian", 9000), None);
        assert_eq!(weight("闷的", "men'de", 3000), None);
        assert_eq!(weight("闷的", "men'di", 3000), Some(3000));
        assert_eq!(weight("血型", "xie'xing", 5000), None);
        assert_eq!(weight("血型", "xue'xing", 5000), Some(5000));
        assert_eq!(weight("门的", "men'de", 435135), Some(100));
    }
}
