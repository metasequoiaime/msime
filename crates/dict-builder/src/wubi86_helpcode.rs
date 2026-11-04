//! `wubi86-helpcode`：从 msime-dictionary 的 86 五笔码表（`sources/wubi/wubi86-jidian.txt`，拆分之后连同 `sources/wubi/wubi86-rime-wubi.txt`，见 `wubi86_table.rs`）生成辅助码方案 `wubi86` 的码表 `resources/helpcodes/wubi86_helpcode.txt`，每行 `字=前两码`。
//!
//! 每个字的码取它的全码的前两码。全码与 `wubi86-supplement` 用的是同一个定义（[`full_codes`]）：码表里该字单独出现时最长的编码；一个字有几个最长编码时，只有它们的前两码一致才收，否则跳过，不替用户猜哪一种拆法。只收基本区汉字：扩展 A 区和基本多文种平面以外的字在 `msime-wubi.db` 的 wubi86 表里同样被去掉（[`msime::outside_basic_cjk`]），候选里出不来，收进来只会让码表变大。CJK 兼容汉字区（U+F900–U+FAFF）的字 wubi86 表也不写入（[`msime::unsupported_wubi_value`]），这里仍按原样收：其中有几个只来自 rime-wubi 的行，去掉它们要连同各平台许可说明里「只来自 rime-wubi 的字数」一起改，留给重新生成这张表的那次改动。标点和其他非汉字的行不收。
//!
//! 输出按码点排序，表头只记输入文件的 SHA-256 和规则，不记任何提交，所以同一份输入在任何提交上都生成逐字节相同的文件。引擎读码表时跳过 `#` 开头的行，每行只取 `=` 后的前两个小写字母（`crates/engine/src/helpcode.rs`）。

use std::collections::BTreeMap;
use std::fmt::Write as _;

use anyhow::{bail, Result};

use crate::msime;
use crate::places_supplement::is_han;
use crate::wubi86_supplement::{full_codes, prefix, JIDIAN, REPORT_EXAMPLES};

/// 码表在 msime 仓库里的位置。
pub const OUTPUT: &str = "resources/helpcodes/wubi86_helpcode.txt";

/// 辅助码取全码的前几码。引擎的码表格式每字最多两码。
const LETTERS: usize = 2;

pub struct Helpcode {
    /// 收录的字和它的两码，按码点排序。
    pub codes: BTreeMap<char, String>,
    /// 几个最长编码的前两码不一致、因而跳过的字，按码点排序。
    pub conflicting: Vec<char>,
    /// 最长编码不足两码、因而跳过的字，按码点排序。
    pub short: Vec<char>,
    /// 因为不在基本区而跳过的字数。
    pub outside: usize,
}

/// `rime_wubi` 是新布局下 rime-wubi 补进来的行，旧布局为 `None`；两种布局读同样的字节得到同一张表，因为这里只看编码、不看权重。
pub fn build(jidian: &str, rime_wubi: Option<&str>) -> Result<Helpcode> {
    let rows: Vec<(String, &str, i64)> = crate::wubi86_table::rows(jidian, rime_wubi)
        .flatten()
        .collect();
    if rows.is_empty() {
        bail!("{JIDIAN} has no rows");
    }
    let all = full_codes(&rows);
    let mut characters: Vec<char> = all.keys().copied().filter(|c| is_han(*c)).collect();
    characters.sort_unstable();
    let mut helpcode = Helpcode {
        codes: BTreeMap::new(),
        conflicting: Vec::new(),
        short: Vec::new(),
        outside: 0,
    };
    let mut buffer = [0u8; 4];
    for c in characters {
        if msime::outside_basic_cjk(c.encode_utf8(&mut buffer)) {
            helpcode.outside += 1;
            continue;
        }
        if let Some(code) = prefix(&all, c, LETTERS) {
            helpcode.codes.insert(c, code.to_owned());
        } else if all[&c].iter().all(|code| code.len() >= LETTERS) {
            helpcode.conflicting.push(c);
        } else {
            helpcode.short.push(c);
        }
    }
    if helpcode.codes.is_empty() {
        bail!("{JIDIAN} gives no character a two-letter code");
    }
    Ok(helpcode)
}

/// `inputs` 是读到的 86 原表文件及其 SHA-256（`(path, sha256)`），按读的先后。
pub fn render(helpcode: &Helpcode, inputs: &[(&str, &str)]) -> String {
    let inputs = inputs
        .iter()
        .map(|(path, sha256)| format!("{path}（SHA-256 {sha256}）"))
        .collect::<Vec<_>>()
        .join("、");
    let mut out = String::new();
    let _ = writeln!(out, "# 五笔 86 辅助码表（方案 wubi86），由 msime 的 crates/dict-builder/src/wubi86_helpcode.rs 以 `msime-dict-build wubi86-helpcode --dictionary <msime-dictionary checkout> --out {OUTPUT}` 从 msime-dictionary 的 {inputs} 生成；不要手工编辑。");
    let _ = writeln!(out, "# 规则：每个字取它在该表里单独出现时最长的编码（全码）的前两码；几个全码的前两码不一致、或全码不足两码的字不收；只收基本区汉字，不含扩展 A 区和基本多文种平面以外的字。");
    let _ = writeln!(out, "# 本文件是对上述码表的修改（只保留单字并截成两码），来源与许可见同目录的 NOTICE-wubi86.md。");
    for (c, code) in &helpcode.codes {
        let _ = writeln!(out, "{c}={code}");
    }
    out
}

/// 生成器对收录和舍弃内容的说明，每组一行，供评审查看。
pub fn report(helpcode: &Helpcode) -> Vec<String> {
    let examples =
        |characters: &[char]| -> String { characters.iter().take(REPORT_EXAMPLES).collect() };
    vec![
        format!("{} characters with a two-letter code", helpcode.codes.len()),
        format!(
            "{} characters left out because their full codes disagree on the first two letters: {}",
            helpcode.conflicting.len(),
            examples(&helpcode.conflicting)
        ),
        format!(
            "{} characters left out because their longest code is shorter than two letters: {}",
            helpcode.short.len(),
            examples(&helpcode.short)
        ),
        format!(
            "{} characters of CJK Extension A or beyond the Basic Multilingual Plane left out",
            helpcode.outside
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // 合成的码表片段：工有简码 a 和全码 aaaa；码的全码 dcg 不足四码；艹有两个前两码不同的全码；㐀（扩展 A）和 𠂇（扩展 B）不在基本区；人民是词；。是标点；了只有一码。
    const FIXTURE: &str = "\u{feff}工\ta\t20\r\n工\taaaa\t20\r\n码\tdc\t10\r\n码\tdcg\t10\r\n一\tg\t10\r\n一\tggll\t10\r\n艹\taghh\t0\r\n艹\tghgh\t0\r\n㐀\tgrnr\t0\r\n𠂇\tdhk\t0\r\n人民\twwna\t20\r\n。\tou\t10\r\n了\tb\t10\r\n";

    #[test]
    fn each_character_takes_the_first_two_letters_of_its_full_code() {
        let helpcode = build(FIXTURE, None).unwrap();
        let codes: Vec<(char, &str)> = helpcode
            .codes
            .iter()
            .map(|(c, code)| (*c, code.as_str()))
            .collect();
        assert_eq!(codes, [('一', "gg"), ('工', "aa"), ('码', "dc")]);
        assert_eq!(helpcode.conflicting, ['艹']);
        assert_eq!(helpcode.short, ['了']);
        assert_eq!(helpcode.outside, 2);
    }

    #[test]
    fn several_full_codes_that_agree_on_two_letters_are_kept() {
        let helpcode = build("码\tdcgh\t10\n码\tdcgg\t10\n", None).unwrap();
        assert_eq!(helpcode.codes.get(&'码').map(String::as_str), Some("dc"));
        assert!(helpcode.conflicting.is_empty());
    }

    #[test]
    fn a_table_without_rows_or_codes_is_refused() {
        assert!(build("", None).is_err());
        assert!(build("了\tb\t10\n", None).is_err());
    }

    #[test]
    fn rendered_rows_parse_the_way_the_engine_reads_them() {
        let helpcode = build(FIXTURE, None).unwrap();
        let rendered = render(&helpcode, &[(JIDIAN, "0")]);
        let header: Vec<&str> = rendered
            .lines()
            .take_while(|line| line.starts_with('#'))
            .collect();
        assert_eq!(header.len(), 3);
        assert!(
            header[0].contains(&format!("{JIDIAN}（SHA-256 0）")),
            "{}",
            header[0]
        );
        let rows: Vec<&str> = rendered
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect();
        assert_eq!(rows, ["一=gg", "工=aa", "码=dc"]);
        assert!(rendered.ends_with('\n'));
    }

    /// 拆成两个文件读和读合并的文件得到同一张表；表头按读的先后记下两个文件的 SHA-256。
    #[test]
    fn the_split_layout_gives_the_same_table() {
        let (jidian, rime_wubi) = FIXTURE.split_at(FIXTURE.find("艹").unwrap());
        let split = build(jidian, Some(rime_wubi)).unwrap();
        let merged = build(FIXTURE, None).unwrap();
        assert_eq!(split.codes, merged.codes);
        assert_eq!(split.conflicting, merged.conflicting);
        let rendered = render(
            &split,
            &[(JIDIAN, "0"), (crate::wubi86_table::RIME_WUBI, "1")],
        );
        let first = rendered.lines().next().unwrap();
        assert!(
            first.contains(&format!(
                "{JIDIAN}（SHA-256 0）、{}（SHA-256 1）",
                crate::wubi86_table::RIME_WUBI
            )),
            "{first}"
        );
    }
}
