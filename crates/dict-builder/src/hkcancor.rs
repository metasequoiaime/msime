//! `hkcancor-counts`: generates msime-dictionary's `sources/cantonese/hkcancor-word-counts.txt`, how often each word of two or more Han characters occurs in the Hong Kong Cantonese Corpus (HKCanCor, K. K. Luke and May L. Y. Wong, CC BY 4.0), from the corpus transcriptions pinned under `hkcancor/` in the sources lock (`data/utf8/*` of https://github.com/fcbond/hkcancor).
//!
//! The corpus is conversational Hong Kong Cantonese from 1997–98, word-segmented and tagged, about 160 thousand tokens. rime-cantonese's essay is mostly written Chinese and lacks the commonest colloquial words (我哋, 佢哋, 嗰啲, 有冇), so `cantonese.rs` weighs a word the essay does not list by its count here.
//!
//! Each transcription holds `<sent_tag>` blocks with one `word/part-of-speech/jyutping/` token per line. A token's count is the number of such lines with that word, summed over every file and part of speech. A line that does not split into exactly those three fields and an empty fourth is not a token the corpus vouches for (the transcribers' `○/#/#/@` placeholder for an unclear syllable, `？幾/m/gei2/@`), and is counted as skipped rather than read. Only words of two or more characters, all of them Han, are written: single characters are weighed by the essay alone, and the corpus' Latin letters, digits and punctuation are not dictionary words.

use std::collections::HashMap;
use std::fmt::Write as _;

use anyhow::{bail, Result};

pub const OUTPUT: &str = "sources/cantonese/hkcancor-word-counts.txt";
/// The prefix under which the sources lock pins the corpus transcriptions, one entry per file of `data/utf8/`.
pub const PREFIX: &str = "hkcancor/";
/// The sources lock reference that records the corpus commit.
pub const REFERENCE: &str = "hkcancor";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub words: HashMap<String, i64>,
    pub files: usize,
    pub tokens: usize,
    pub skipped: usize,
}

fn is_han(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{323af}')
}

/// Counts the tokens of every transcription, each given as `(name, text)`.
pub fn count(files: &[(&str, &str)]) -> Result<Counts> {
    let mut counts = Counts::default();
    for (name, source) in files {
        let mut inside = false;
        let mut blocks = 0;
        for line in source.lines() {
            let line = line.trim();
            match line {
                "<sent_tag>" if !inside => {
                    inside = true;
                    blocks += 1;
                }
                "<sent_tag>" => bail!("{name}: <sent_tag> inside <sent_tag>"),
                "</sent_tag>" => inside = false,
                "" => {}
                _ if inside => {
                    let fields: Vec<&str> = line.split('/').collect();
                    let [word, part, reading, ""] = fields[..] else {
                        counts.skipped += 1;
                        continue;
                    };
                    if word.is_empty() || part.is_empty() || reading.is_empty() {
                        counts.skipped += 1;
                        continue;
                    }
                    counts.tokens += 1;
                    if word.chars().count() >= 2 && word.chars().all(is_han) {
                        *counts.words.entry(word.to_owned()).or_insert(0) += 1;
                    }
                }
                _ => {}
            }
        }
        if inside {
            bail!("{name}: a <sent_tag> block is not closed");
        }
        if blocks == 0 {
            bail!("{name}: no <sent_tag> block; not an HKCanCor transcription");
        }
        counts.files += 1;
    }
    Ok(counts)
}

pub struct Provenance<'a> {
    pub upstream_commit: &'a str,
}

/// The file: `#` header lines, then `word<TAB>count` by descending count, ties in code point order.
pub fn render(counts: &Counts, provenance: &Provenance) -> String {
    let mut ordered: Vec<(i64, &str)> = counts
        .words
        .iter()
        .map(|(word, count)| (-count, word.as_str()))
        .collect();
    ordered.sort_unstable();
    let mut out = String::new();
    let _ = writeln!(out, "# 香港粤语语料库（HKCanCor）词频表，由 msime 仓库 crates/dict-builder/src/hkcancor.rs 的 `msime-dict-build hkcancor-counts --cache <dir> --out {OUTPUT}` 生成，生成器所在的提交就是在 resources/dictionary-sources.lock.json 里固定本文件的 msime 提交；不要手工编辑。");
    let _ = writeln!(out, "# 上游：https://github.com/fcbond/hkcancor 提交 {} 的 data/utf8/ 下 {} 个转写文件（K. K. Luke and May L. Y. Wong, The Hong Kong Cantonese Corpus: Design and Uses, Journal of Chinese Linguistics Monograph Series 25, 2015；CC BY 4.0，见上游 data/LICENSE）。", provenance.upstream_commit, counts.files);
    let _ = writeln!(out, "# 计数：<sent_tag> 块里每行一个 词/词性/粤拼/ 标注，一行计一次，不分词性、跨文件相加；拆不成这三段加空尾段的行（转写者的 ○/#/#/@ 占位等）不计。共 {} 个标注，跳过 {} 行。只写出两个及以上汉字组成的词，单字、字母、数字与标点不写；不与其他文件对照去重，每个词一行。", counts.tokens, counts.skipped);
    for (count, word) in ordered {
        let _ = writeln!(out, "{word}\t{}", -count);
    }
    out
}

/// `word<TAB>count` lines of the generated file, skipping `#` lines.
pub fn parse(source: &str) -> Result<HashMap<&str, i64>> {
    let mut counts = HashMap::new();
    for (index, line) in source.lines().enumerate() {
        if line.starts_with('#') {
            continue;
        }
        let parsed = line.split_once('\t').and_then(|(word, count)| {
            Some((word, count.parse::<i64>().ok().filter(|count| *count > 0)?))
        });
        let Some((word, count)) = parsed.filter(|(word, _)| !word.is_empty()) else {
            bail!("{OUTPUT}: line {} is not word<TAB>count", index + 1);
        };
        if counts.insert(word, count).is_some() {
            bail!("{OUTPUT}: line {}: {word} is listed twice", index + 1);
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "<info>\r\n\t1-TN-001\r\n\tINFO-END\r\n</info>\n\t<sent>\n\t\t<sent_head>\n\t\t\tA:\n\t\t</sent_head>\n\t\t<sent_tag>\n\t\t\t我哋/r/ngo5dei6/\n\t\t\t去/v/heoi3/\n\t\t\t○/#/#/@\n\t\t\t嗰啲/r/go2di1/\n\t\t\tCD/n/si1di1/\n\t\t</sent_tag>\n\t\t<sent_tran>\n\n\t\t</sent_tran>\n\t</sent>\n\t<sent>\n\t\t<sent_tag>\n\t\t\t我哋/r/ngo5dei6/\n\t\t\t𠻺吓/v/ngam3haa5/\n\t\t</sent_tag>\n\t</sent>\n";

    #[test]
    fn tokens_of_han_words_are_counted_across_files() {
        let counts = count(&[("FC-001_v2", FILE), ("FC-005a_v2", FILE)]).unwrap();
        assert_eq!((counts.files, counts.tokens, counts.skipped), (2, 12, 2));
        let mut words: Vec<(&str, i64)> = counts
            .words
            .iter()
            .map(|(word, count)| (word.as_str(), *count))
            .collect();
        words.sort_unstable();
        // 去 is one character and CD is not Han, so neither is written; 𠻺 is outside the BMP.
        assert_eq!(words, [("嗰啲", 2), ("我哋", 4), ("𠻺吓", 2)]);
    }

    #[test]
    fn the_rendered_file_reads_back_in_count_order() {
        let counts = count(&[("FC-001_v2", FILE)]).unwrap();
        let rendered = render(
            &counts,
            &Provenance {
                upstream_commit: "39aeadf920e0b5ca93d0ad7792c59e740e7bdd65",
            },
        );
        let body: Vec<&str> = rendered
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect();
        assert_eq!(body, ["我哋\t2", "嗰啲\t1", "𠻺吓\t1"]);
        assert!(rendered.contains(
            "提交 39aeadf920e0b5ca93d0ad7792c59e740e7bdd65 的 data/utf8/ 下 1 个转写文件"
        ));
        let parsed = parse(&rendered).unwrap();
        assert_eq!(parsed["我哋"], 2);
        assert_eq!(parsed.len(), 3);
    }

    #[test]
    fn malformed_input_is_rejected() {
        let error = count(&[("x", "<sent_tag>\n我哋/r/ngo5dei6/\n")]).unwrap_err();
        assert!(error.to_string().contains("not closed"), "{error}");
        let error = count(&[("x", "我哋/r/ngo5dei6/\n")]).unwrap_err();
        assert!(error.to_string().contains("no <sent_tag> block"), "{error}");
        for (source, expected) in [
            ("我哋\n", "line 1 is not"),
            ("我哋\t0\n", "line 1 is not"),
            ("我哋\t2\n我哋\t1\n", "line 2: 我哋 is listed twice"),
        ] {
            let error = parse(source).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn the_lock_pins_every_transcription_at_one_commit() {
        let lock = crate::sources::Lock::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/dictionary-sources.lock.json"),
        )
        .unwrap();
        let commit = &lock.references[REFERENCE].commit;
        let files: Vec<_> = lock
            .files
            .iter()
            .filter(|file| file.path.starts_with(PREFIX))
            .collect();
        assert_eq!(files.len(), 58);
        for file in files {
            let name = &file.path[PREFIX.len()..];
            assert_eq!(
                file.url,
                format!(
                    "https://raw.githubusercontent.com/fcbond/hkcancor/{commit}/data/utf8/{name}"
                )
            );
        }
    }
}
