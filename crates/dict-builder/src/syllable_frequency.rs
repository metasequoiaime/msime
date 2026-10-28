//! `syllable-frequency`：从词库 release 的 msime-pinyin.db 写出九宫格切分用的音节频度表 `crates/engine/src/nine_key/syllable_frequency.tsv`，引擎用 `include_str!` 内嵌。
//!
//! 一个音节的频度是它在词库里出现的总权重：读音就是这个音节的单字行，权重照计；多字词的读音里每出现一次这个音节，计一次这个词的权重。按词频统计的词库里，这就是音节在语料里出现的次数。只看单字行不行：词库把「饭、午、物」这类多半出现在词里的字记成单用的次数（dict-v2.0.39 的 fan 前三个字合计 374521，dict-v2.0.24 是 3234183），音节的先后随词库怎么分单字和词的权重而翻转；把词里的出现也算上，两版词库的音节排名几乎一致（421 个音节的秩相关 0.977，只看前三个单字时 0.780）。
//!
//! 第一行 `# msime-pinyin.db sha256=<hex>` 记下取自哪份词库，和词库发布锁里同名文件的 sha256 对得上；之后每行 `syllable<TAB>weight`，按音节字典序。

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::sqlite;

/// 统计 `pinyin` 里全部 `tbl_*` 表，返回写好的表文本。
pub fn build(pinyin: &Path) -> Result<String> {
    let connection =
        sqlite::open_read_only(pinyin).with_context(|| format!("open {}", pinyin.display()))?;
    let tables: Vec<String> = {
        let mut statement = connection.prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'tbl\\_%' ESCAPE '\\' ORDER BY name",
        )?;
        let names = statement.query_map([], |row| row.get(0))?;
        names.collect::<rusqlite::Result<_>>()?
    };
    let mut weights: BTreeMap<String, i64> = BTreeMap::new();
    for table in &tables {
        let mut statement =
            connection.prepare(&format!("SELECT key, value, weight FROM \"{table}\""))?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let key: String = row.get(0)?;
            let value: String = row.get(1)?;
            let weight: i64 = row.get::<_, Option<i64>>(2)?.unwrap_or(0).max(0);
            add(&mut weights, &key, &value, weight);
        }
    }
    Ok(render(&sha256(pinyin)?, &weights))
}

fn sha256(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// 一行词库条目计入音节频度：单音节读音只算单字行，多音节读音每个音节各计一次。
fn add(weights: &mut BTreeMap<String, i64>, key: &str, value: &str, weight: i64) {
    if !key
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte == b'\'')
    {
        return;
    }
    if !key.contains('\'') {
        if value.chars().count() == 1 {
            *weights.entry(key.to_owned()).or_default() += weight;
        }
        return;
    }
    for syllable in key.split('\'').filter(|syllable| !syllable.is_empty()) {
        *weights.entry(syllable.to_owned()).or_default() += weight;
    }
}

fn render(source_sha256: &str, weights: &BTreeMap<String, i64>) -> String {
    let mut out = format!("# msime-pinyin.db sha256={source_sha256}\n");
    for (syllable, weight) in weights {
        let _ = writeln!(out, "{syllable}\t{weight}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_character_rows_and_every_syllable_of_a_phrase_count() {
        let mut weights = BTreeMap::new();
        add(&mut weights, "fan", "饭", 40);
        // 单音节读音的多字行（儿化、合音）不是这个音节的单字，不计。
        add(&mut weights, "fan", "饭儿", 7);
        add(&mut weights, "chi'fan", "吃饭", 100);
        add(&mut weights, "fan'fan", "饭饭", 3);
        add(&mut weights, "Fan", "F", 9);
        assert_eq!(weights.get("fan"), Some(&(40 + 100 + 3 + 3)));
        assert_eq!(weights.get("chi"), Some(&100));
        assert_eq!(
            render("ab12", &weights),
            "# msime-pinyin.db sha256=ab12\nchi\t100\nfan\t146\n"
        );
    }
}
