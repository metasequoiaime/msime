//! `cleanse-report`：给 msime-dictionary 的清洗出候选清单，不改任何库和源文件。维护者逐条审过之后，才把选中的行抄进 `curation/pinyin-vetoes.txt`、`curation/pinyin-weight-overrides.txt`（见 `curation.rs`）。
//!
//! 被检查的行是 msime-dictionary 进发布的拼音输入：`sources/pinyin/rime-ice.txt`、`sources/pinyin/rime-ice-supplement.txt`、`sources/pinyin/places.txt` 和 `custom/words.txt`，按构建读它们的方式读（[`weighted_rows`]）。已经被 `curation/` 否决或覆盖过的行不再列出，所以审过一批、写进表里之后再跑一次，剩下的就是还没处理的。每一类单独写一个 TSV，文件头记下 msime 提交、msime-dictionary 提交和每个对照文件的 SHA-256。
//!
//! - `weight-disagreement.tsv`（给了 `--upstream`）：`rime-ice.txt` 和 `rime-ice-supplement.txt` 里与上游 rime-ice 检出的 `cn_dicts/base.dict.yaml`、`cn_dicts/ext.dict.yaml` 同词同音、权重相差 `--ratio` 倍以上的行，建议恢复上游的权重。先按四份发布来源取同词同音的最高权重，不列已被区划或人工提权覆盖的旧上游行。给了 `--reference-scale` 时，权重恰好等于参照表权重乘这个系数（四舍五入）的行算作有意按参照表改写过的，不列出。
//! - `placeholder-weights.tsv`：多字词的权重是占位值：不小于 1000 且每一位数字都相同（9999、1111、33333），或是 [`MEME_WEIGHTS`] 里的梗数字。给了参照表时，参照表收了这个词（任何读音）的不列出。
//! - `fragment-over-common.tsv`（给了 `--reference`）：参照表任何读音都不收的多字词，在同一个全拼键上排在一个参照权重不低于 `--reference-min` 的词前面，建议降到它压过的那些词最低的权重之下。`custom/words.txt` 的行不列：覆盖表不能改人工词条的权重。
//! - `vulgar-fragments.tsv`：含粗俗词、而本身不是表里的词、也没有被豁免写法盖住的词（引擎拼整句时拦的同一种），建议整词否决。粗俗词表用 `curation/offensive-words.txt`，没有这个文件时用引擎内嵌的表。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::curation::{self, PinyinCuration};
use crate::places_supplement::weighted_rows;
use crate::sources::sha256_file;
use crate::text;

/// 被检查的拼音输入，按构建并入的顺序。
pub const SOURCES: [&str; 4] = [
    "sources/pinyin/rime-ice.txt",
    "sources/pinyin/rime-ice-supplement.txt",
    "sources/pinyin/places.txt",
    "custom/words.txt",
];

/// 与上游比权重的两个文件：只有它们是从上游 rime-ice 整理来的。
const UPSTREAM_COMPARED: [&str; 2] = [SOURCES[0], SOURCES[1]];

/// 上游 rime-ice 检出里有权重的两张表。
pub const UPSTREAM_TABLES: [&str; 2] = ["cn_dicts/base.dict.yaml", "cn_dicts/ext.dict.yaml"];

/// 不是词频、是梗的权重值。
pub const MEME_WEIGHTS: [i64; 2] = [19_260_817, 12_345_678];

/// 占位权重的下限：再小的同位数权重（111、99）在真实词频里很常见。
const PLACEHOLDER_MINIMUM: i64 = 1000;

/// 读音转成构建用的写法：小写、去声调（ü 和 u 加分音符写成 v）、音节之间用 `'`。Rime 词典的读音用空格分隔、带声调（万象写 `nǐ hǎo`，rime-ice 写 `ni hao`），构建的输入已经是 `ni'hao`。去掉声调后还有 a–z 以外的字符（辅助码、标点），返回 `None`。
pub fn normalize_reading(reading: &str) -> Option<String> {
    let mut out = String::with_capacity(reading.len());
    for c in reading.chars() {
        let plain = match c {
            'ā' | 'á' | 'ǎ' | 'à' | 'Ā' | 'Á' | 'Ǎ' | 'À' => 'a',
            'ē' | 'é' | 'ě' | 'è' | 'ê' | 'Ē' | 'É' | 'Ě' | 'È' | 'Ê' => 'e',
            'ī' | 'í' | 'ǐ' | 'ì' | 'Ī' | 'Í' | 'Ǐ' | 'Ì' => 'i',
            'ō' | 'ó' | 'ǒ' | 'ò' | 'Ō' | 'Ó' | 'Ǒ' | 'Ò' => 'o',
            'ū' | 'ú' | 'ǔ' | 'ù' | 'Ū' | 'Ú' | 'Ǔ' | 'Ù' => 'u',
            'ǖ' | 'ǘ' | 'ǚ' | 'ǜ' | 'ü' | 'Ǖ' | 'Ǘ' | 'Ǚ' | 'Ǜ' | 'Ü' => 'v',
            'ń' | 'ň' | 'ǹ' | 'Ń' | 'Ň' | 'Ǹ' => 'n',
            'ḿ' | 'Ḿ' => 'm',
            // 分解形式的分音符：u 加 U+0308 是 ü。
            '\u{0308}' => {
                if out.ends_with('u') {
                    out.pop();
                    out.push('v');
                }
                continue;
            }
            // 其余分解形式的声调符号。
            '\u{0300}'..='\u{036f}' => continue,
            ' ' | '\'' => {
                if !out.is_empty() && !out.ends_with('\'') {
                    out.push('\'');
                }
                continue;
            }
            c if c.is_ascii_alphabetic() => c.to_ascii_lowercase(),
            _ => return None,
        };
        out.push(plain);
    }
    while out.ends_with('\'') {
        out.pop();
    }
    (!out.is_empty()).then_some(out)
}

/// 词 → 读音 → 最高权重。
type Weights = HashMap<String, HashMap<String, i64>>;

/// 读一份参照或上游词表：Rime 的 `.dict.yaml`（`...` 一行结束文件头，之后是 `词<TAB>读音<TAB>权重[<TAB>…]`），或者没有文件头的 `词<TAB>读音[<TAB>权重]`。读音按 [`normalize_reading`] 统一，读不懂的读音和没有词的行跳过；没有权重的行权重算 0。同词同音取最大权重。
pub fn parse_weights(text: &str, weights: &mut Weights) {
    let lines = text::universal_lines(text::without_bom(text));
    let body = match lines.iter().position(|line| *line == "...") {
        Some(end)
            if lines
                .first()
                .is_some_and(|line| line.starts_with('#') || *line == "---") =>
        {
            &lines[end + 1..]
        }
        _ => &lines[..],
    };
    for line in body {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let (Some(word), Some(reading)) = (fields.next(), fields.next()) else {
            continue;
        };
        let word = text::strip(word);
        let Some(key) = normalize_reading(text::strip(reading)) else {
            continue;
        };
        if word.is_empty() {
            continue;
        }
        let weight = fields
            .next()
            .and_then(|weight| text::strip(weight).parse::<i64>().ok())
            .unwrap_or(0);
        let slot = weights
            .entry(word.to_owned())
            .or_default()
            .entry(key)
            .or_insert(weight);
        *slot = (*slot).max(weight);
    }
}

/// 一个被检查的行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row<'a> {
    pub source: &'a str,
    pub word: &'a str,
    pub key: &'a str,
    pub weight: i64,
}

/// 被检查的行：每个来源按 [`weighted_rows`] 读，去掉 `curation` 已经否决或覆盖的行。
pub fn examined_rows<'a>(
    sources: &[(&'a str, &'a str)],
    curation: &PinyinCuration,
) -> Vec<Row<'a>> {
    let mut rows = Vec::new();
    for &(source, text) in sources {
        for (word, key, weight) in weighted_rows(text) {
            if curation.vetoes.vetoes(word, key) || curation.overrides.get(word, key).is_some() {
                continue;
            }
            rows.push(Row {
                source,
                word,
                key,
                weight,
            });
        }
    }
    rows
}

/// 构建之后的样子：每个全拼键上每个词的最高权重（`curation` 施加之后），以及这个权重来自哪个来源。
fn merged<'a>(
    sources: &[(&'a str, &'a str)],
    curation: &'a PinyinCuration,
) -> HashMap<&'a str, HashMap<&'a str, (i64, &'a str)>> {
    let mut keys: HashMap<&str, HashMap<&str, (i64, &str)>> = HashMap::new();
    for &(source, text) in sources {
        for (word, key, weight) in curation.rows(text) {
            let slot = keys
                .entry(key)
                .or_default()
                .entry(word)
                .or_insert((weight, source));
            if weight > slot.0 {
                *slot = (weight, source);
            }
        }
    }
    keys
}

fn is_multi_character(word: &str) -> bool {
    word.chars().nth(1).is_some()
}

/// 本地权重与上游相差 `ratio` 倍以上的一行。
#[derive(Debug, Clone, PartialEq)]
pub struct Disagreement<'a> {
    pub row: Row<'a>,
    pub upstream: i64,
    pub ratio: f64,
}

pub fn weight_disagreements<'a>(
    rows: &[Row<'a>],
    upstream: &Weights,
    ratio: f64,
    rewrite: Option<(&Weights, f64)>,
) -> Vec<Disagreement<'a>> {
    // 同词同音先按全部发布来源取最高权重（`build_quanpin` 和人工提权的规则），再比较仍来自 rime-ice 的行；被区划或人工提权覆盖的旧行不能生成可施加的覆盖建议。
    let mut kept: HashMap<(&str, &str), &Row> = HashMap::new();
    for row in rows {
        let slot = kept.entry((row.word, row.key)).or_insert(row);
        if row.weight > slot.weight {
            *slot = row;
        }
    }
    let mut found: Vec<Disagreement> = kept
        .into_values()
        .filter(|row| UPSTREAM_COMPARED.contains(&row.source))
        .filter_map(|row| {
            let upstream = *upstream.get(row.word)?.get(row.key)?;
            if upstream <= 0 || row.weight <= 0 {
                return None;
            }
            let (high, low) = (upstream.max(row.weight), upstream.min(row.weight));
            let found = high as f64 / low as f64;
            if found < ratio {
                return None;
            }
            let rewritten = rewrite.is_some_and(|(reference, scale)| {
                reference
                    .get(row.word)
                    .and_then(|keys| keys.get(row.key))
                    .is_some_and(|weight| (*weight as f64 * scale).round() as i64 == row.weight)
            });
            (!rewritten).then(|| Disagreement {
                row: row.clone(),
                upstream,
                ratio: found,
            })
        })
        .collect();
    found.sort_by(|a, b| {
        b.row
            .weight
            .cmp(&a.row.weight)
            .then(a.row.word.cmp(b.row.word))
            .then(a.row.key.cmp(b.row.key))
            .then(a.row.source.cmp(b.row.source))
    });
    found
}

/// 权重是不是占位值，见模块文档。
pub fn is_placeholder_weight(weight: i64) -> bool {
    if MEME_WEIGHTS.contains(&weight) {
        return true;
    }
    if weight < PLACEHOLDER_MINIMUM {
        return false;
    }
    let digits = weight.to_string();
    let first = digits.as_bytes()[0];
    digits.bytes().all(|digit| digit == first)
}

pub fn placeholder_weights<'a>(rows: &[Row<'a>], reference: Option<&Weights>) -> Vec<Row<'a>> {
    let mut found: Vec<Row> = rows
        .iter()
        .filter(|row| is_multi_character(row.word) && is_placeholder_weight(row.weight))
        .filter(|row| reference.is_none_or(|reference| !reference.contains_key(row.word)))
        .cloned()
        .collect();
    found.sort_by(|a, b| {
        b.weight
            .cmp(&a.weight)
            .then(a.word.cmp(b.word))
            .then(a.key.cmp(b.key))
    });
    found
}

/// 参照表不收、却压过同键常用词的一个词。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment<'a> {
    pub key: &'a str,
    pub word: &'a str,
    pub weight: i64,
    pub source: &'a str,
    /// 它压过的常用词（参照权重不低于门槛、本地权重比它低）有几个。
    pub outranked: usize,
    /// 其中参照权重最高的那个：词、本地权重、参照权重。
    pub strongest: (&'a str, i64, i64),
    /// 降到被压过的常用词里最低的本地权重减 1（不低于 0）。
    pub suggested: i64,
}

pub fn fragments_over_common<'a>(
    sources: &[(&'a str, &'a str)],
    curation: &'a PinyinCuration,
    reference: &Weights,
    minimum: i64,
) -> Vec<Fragment<'a>> {
    let mut found = Vec::new();
    for (key, words) in merged(sources, curation) {
        let common: Vec<(&str, i64, i64)> = words
            .iter()
            .filter_map(|(word, (weight, _))| {
                let reference = *reference.get(*word)?.get(key)?;
                (reference >= minimum).then_some((*word, *weight, reference))
            })
            .collect();
        if common.is_empty() {
            continue;
        }
        for (&word, &(weight, source)) in &words {
            if !is_multi_character(word)
                || reference.contains_key(word)
                || source == SOURCES[3]
                || curation.overrides.get(word, key).is_some()
            {
                continue;
            }
            let outranked: Vec<&(&str, i64, i64)> = common
                .iter()
                .filter(|(_, common_weight, _)| *common_weight < weight)
                .collect();
            let Some(strongest) = outranked
                .iter()
                .max_by(|a, b| a.2.cmp(&b.2).then(b.0.cmp(a.0)))
            else {
                continue;
            };
            let lowest = outranked
                .iter()
                .map(|(_, weight, _)| *weight)
                .min()
                .unwrap_or(0);
            found.push(Fragment {
                key,
                word,
                weight,
                source,
                outranked: outranked.len(),
                strongest: **strongest,
                suggested: (lowest - 1).max(0),
            });
        }
    }
    found.sort_by(|a, b| {
        b.weight
            .cmp(&a.weight)
            .then(a.key.cmp(b.key))
            .then(a.word.cmp(b.word))
    });
    found
}

/// 含粗俗词的一个词，连同它在被检查的行里的全部读音。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VulgarFragment<'a> {
    pub word: &'a str,
    pub readings: Vec<&'a str>,
    pub weight: i64,
    pub sources: Vec<&'a str>,
}

pub fn vulgar_fragments<'a>(
    rows: &[Row<'a>],
    table: &msime_engine::OffensiveWords,
    listed: &HashSet<String>,
) -> Vec<VulgarFragment<'a>> {
    let mut by_word: BTreeMap<&str, VulgarFragment> = BTreeMap::new();
    for row in rows {
        if listed.contains(row.word) || !table.is_offensive(row.word) {
            continue;
        }
        let entry = by_word.entry(row.word).or_insert_with(|| VulgarFragment {
            word: row.word,
            readings: Vec::new(),
            weight: row.weight,
            sources: Vec::new(),
        });
        if !entry.readings.contains(&row.key) {
            entry.readings.push(row.key);
        }
        if !entry.sources.contains(&row.source) {
            entry.sources.push(row.source);
        }
        entry.weight = entry.weight.max(row.weight);
    }
    let mut found: Vec<VulgarFragment> = by_word.into_values().collect();
    found.sort_by(|a, b| b.weight.cmp(&a.weight).then(a.word.cmp(b.word)));
    found
}

/// 命令行参数，见 `main.rs` 的 `CleanseReport`。
pub struct Options<'a> {
    pub repository: &'a Path,
    pub dictionary: &'a Path,
    pub out: &'a Path,
    pub upstream: Option<&'a Path>,
    pub ratio: f64,
    pub references: &'a [PathBuf],
    pub reference_min: i64,
    pub reference_scale: Option<f64>,
}

/// 写进每个文件表头的事实。
struct Header {
    lines: Vec<String>,
}

impl Header {
    fn render(&self, title: &str, columns: &str) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# {title}这是候选清单，维护者逐条审定后才写进 curation/ 的表；msime-dict-build cleanse-report 生成，不要手工编辑。");
        for line in &self.lines {
            let _ = writeln!(out, "# {line}");
        }
        let _ = writeln!(out, "# 列：{columns}");
        out
    }
}

fn digest_list(files: &[(String, String)]) -> String {
    files
        .iter()
        .map(|(path, sha256)| format!("{path}（SHA-256 {sha256}）"))
        .collect::<Vec<_>>()
        .join("、")
}

/// 跑报告，写出各个 TSV，返回每个文件写了几行，供命令行打印。
pub fn run(options: &Options, sources: &crate::sources::Sources) -> Result<Vec<(PathBuf, usize)>> {
    std::fs::create_dir_all(options.out)
        .with_context(|| format!("creating {}", options.out.display()))?;
    let curation = PinyinCuration::load(sources)?;
    let mut texts = Vec::new();
    let mut local_digests = Vec::new();
    for path in SOURCES {
        let resolved = sources.pinned(path)?;
        texts.push((path, text::read(&resolved)?));
        local_digests.push((path.to_owned(), sha256_file(&resolved)?));
    }
    for (path, sha256) in curation::present_digests(sources, &curation::PINYIN_TABLES)? {
        local_digests.push((path.to_owned(), sha256));
    }
    let views: Vec<(&str, &str)> = texts
        .iter()
        .map(|(path, text)| (*path, text.as_str()))
        .collect();
    let rows = examined_rows(&views, &curation);

    let mut reference = Weights::new();
    let mut reference_digests = Vec::new();
    for path in options.references {
        let text = text::read(path)?;
        parse_weights(&text, &mut reference);
        reference_digests.push((path.display().to_string(), sha256_file(path)?));
    }

    let mut header = Header {
        lines: vec![
            format!(
                "msime 提交 {}；msime-dictionary 提交 {}。",
                crate::product::builder_commit(options.repository)?,
                crate::product::git(options.dictionary, &["rev-parse", "HEAD"])?
            ),
            format!(
                "被检查的行：{}；已被 curation/ 否决或覆盖的行不列出。",
                digest_list(&local_digests)
            ),
        ],
    };
    if !reference_digests.is_empty() {
        header
            .lines
            .push(format!("参照表：{}。", digest_list(&reference_digests)));
    }

    let mut written = Vec::new();
    let mut write = |name: &str, contents: String, count: usize| -> Result<()> {
        let path = options.out.join(name);
        std::fs::write(&path, contents).with_context(|| format!("writing {}", path.display()))?;
        written.push((path, count));
        Ok(())
    };

    if let Some(upstream) = options.upstream {
        let mut weights = Weights::new();
        let mut digests = Vec::new();
        for table in UPSTREAM_TABLES {
            let path = upstream.join(table);
            parse_weights(&text::read(&path)?, &mut weights);
            digests.push((table.to_owned(), sha256_file(&path)?));
        }
        let commit = crate::product::git(upstream, &["rev-parse", "HEAD"])
            .unwrap_or_else(|_| "未知（不是 Git 检出）".to_owned());
        let rewrite = options.reference_scale.map(|scale| (&reference, scale));
        if rewrite.is_some() && options.references.is_empty() {
            bail!("--reference-scale needs a --reference to compare the rewritten weights with");
        }
        let found = weight_disagreements(&rows, &weights, options.ratio, rewrite);
        let mut out = header.render(
            &format!("本地权重与上游 rime-ice 相差 {} 倍以上的行。", options.ratio),
            "来源、词、全拼、本地权重、上游权重、倍数、建议写进 curation/pinyin-weight-overrides.txt 的行（恢复上游权重）",
        );
        let _ = writeln!(
            out,
            "# 上游：提交 {commit} 的 {}{}。",
            digest_list(&digests),
            options
                .reference_scale
                .map(|scale| format!(
                    "；权重等于参照权重乘 {scale}（四舍五入）的行算作有意按参照表改写，不列出"
                ))
                .unwrap_or_default()
        );
        for item in &found {
            let _ = writeln!(
                out,
                "{}\t{}\t{}\t{}\t{}\t{:.2}\t{}\\t{}\\t{}",
                item.row.source,
                item.row.word,
                item.row.key,
                item.row.weight,
                item.upstream,
                item.ratio,
                item.row.word,
                item.row.key,
                item.upstream
            );
        }
        write("weight-disagreement.tsv", out, found.len())?;
    }

    let placeholders = placeholder_weights(
        &rows,
        (!options.references.is_empty()).then_some(&reference),
    );
    let mut out = header.render(
        &format!(
            "多字词的占位权重：不小于 {PLACEHOLDER_MINIMUM} 且每位数字相同，或是梗数字 {}。{}",
            MEME_WEIGHTS.map(|weight| weight.to_string()).join("、"),
            if options.references.is_empty() {
                "没有给参照表，参照表收的词也列出。"
            } else {
                "参照表收了的词不列出。"
            }
        ),
        "来源、词、全拼、权重",
    );
    for row in &placeholders {
        let _ = writeln!(
            out,
            "{}\t{}\t{}\t{}",
            row.source, row.word, row.key, row.weight
        );
    }
    write("placeholder-weights.tsv", out, placeholders.len())?;

    if !options.references.is_empty() {
        let found = fragments_over_common(&views, &curation, &reference, options.reference_min);
        let mut out = header.render(
            &format!(
                "参照表任何读音都不收的多字词，在同一个全拼键上压过参照权重不低于 {} 的词。custom/words.txt 的行不列。",
                options.reference_min
            ),
            "全拼、词、本地权重、来源、压过的常用词个数、其中参照权重最高的词、它的本地权重、它的参照权重、建议写进 curation/pinyin-weight-overrides.txt 的行（降到被压过的词最低的权重之下）",
        );
        for item in &found {
            let _ = writeln!(
                out,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\\t{}\\t{}",
                item.key,
                item.word,
                item.weight,
                item.source,
                item.outranked,
                item.strongest.0,
                item.strongest.1,
                item.strongest.2,
                item.word,
                item.key,
                item.suggested
            );
        }
        write("fragment-over-common.tsv", out, found.len())?;
    }

    let (offensive_text, offensive_source) =
        match curation::read_optional(sources, curation::OFFENSIVE_WORDS)? {
            Some(text) => {
                let resolved = sources
                    .optional(curation::OFFENSIVE_WORDS)?
                    .context("the offensive word list was just read")?;
                let digest = sha256_file(&resolved)?;
                (text, format!("{}（SHA-256 {digest}）", curation::OFFENSIVE_WORDS))
            }
            None => (
                msime_engine::EMBEDDED_OFFENSIVE_WORDS.to_owned(),
                "引擎内嵌的 crates/engine/src/lattice/offensive_words.txt（msime-dictionary 还没有 curation/offensive-words.txt）".to_owned(),
            ),
        };
    let entries = curation::parse_offensive_words(&offensive_text)?;
    let listed: HashSet<String> = entries
        .iter()
        .filter(|entry| !entry.exempt)
        .map(|entry| entry.text.clone())
        .collect();
    let table = msime_engine::OffensiveWords::from_entries(entries)
        .context("the offensive word list has a listed word")?;
    let found = vulgar_fragments(&rows, &table, &listed);
    let mut out = header.render(
        "含粗俗词、本身不是表里的词、也没有被豁免写法盖住的词（引擎拼整句时拦的同一种）。",
        "词、权重最高的读音的权重、全部读音、来源、建议写进 curation/pinyin-vetoes.txt 的行（整词否决）",
    );
    let _ = writeln!(out, "# 粗俗词表：{offensive_source}。");
    for item in &found {
        let _ = writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}",
            item.word,
            item.weight,
            item.readings.join(","),
            item.sources.join(","),
            item.word
        );
    }
    write("vulgar-fragments.tsv", out, found.len())?;
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readings_are_normalised_to_the_build_spelling() {
        assert_eq!(normalize_reading("nǐ hǎo").as_deref(), Some("ni'hao"));
        assert_eq!(normalize_reading("lǜ sè").as_deref(), Some("lv'se"));
        assert_eq!(normalize_reading("lu\u{0308} se").as_deref(), Some("lv'se"));
        assert_eq!(normalize_reading("ni hao").as_deref(), Some("ni'hao"));
        assert_eq!(normalize_reading("ni'hao").as_deref(), Some("ni'hao"));
        assert_eq!(
            normalize_reading("Zhong  Guo ").as_deref(),
            Some("zhong'guo")
        );
        assert_eq!(normalize_reading("ni;ab"), None);
        assert_eq!(normalize_reading(""), None);
    }

    #[test]
    fn weight_lists_are_read_with_or_without_a_rime_header() {
        let mut weights = Weights::new();
        parse_weights(
            "# Rime dictionary\n---\nname: x\n...\n门的\tmén de\t4351\n门的\tmen de\t10\n# c\n无权重\twu quan zhong\n",
            &mut weights,
        );
        parse_weights("车检\tche'jian\t500\n", &mut weights);
        assert_eq!(weights["门的"]["men'de"], 4351);
        assert_eq!(weights["无权重"]["wu'quan'zhong"], 0);
        assert_eq!(weights["车检"]["che'jian"], 500);
        assert!(!weights.contains_key("name: x"));
    }

    const RIME_ICE: &str = "门的\tmen'de\t435135\n闷的\tmen'de\t300000\n在中\tzai'zhong\t130198\n中区\tzhong'qu\t10000\n中去\tzhong'qu\t218055\n别鸡巴\tbie'ji'ba\t100\n傻逼\tsha'bi\t108330\n占位\tzhan'wei\t9999\n梗\tgeng\t19260817\n梗数\tgeng'shu\t19260817\n";

    fn views<'a>(rime_ice: &'a str, custom: &'a str) -> Vec<(&'a str, &'a str)> {
        vec![
            (SOURCES[0], rime_ice),
            (SOURCES[1], ""),
            (SOURCES[2], ""),
            (SOURCES[3], custom),
        ]
    }

    #[test]
    fn rows_disagreeing_with_upstream_are_listed_unless_rewritten_from_the_reference() {
        let curation = PinyinCuration::default();
        let rows = examined_rows(&views(RIME_ICE, ""), &curation);
        let mut upstream = Weights::new();
        parse_weights(
            "门的\tmen de\t4351\n在中\tzai zhong\t1301\n闷的\tmen de\t200000\n",
            &mut upstream,
        );
        let found = weight_disagreements(&rows, &upstream, 5.0, None);
        assert_eq!(
            found
                .iter()
                .map(|item| (item.row.word, item.upstream))
                .collect::<Vec<_>>(),
            [("门的", 4351), ("在中", 1301)]
        );
        let mut reference = Weights::new();
        parse_weights("在中\tzài zhōng\t14152\n", &mut reference);
        let found = weight_disagreements(&rows, &upstream, 5.0, Some((&reference, 9.2)));
        assert_eq!(
            found.iter().map(|item| item.row.word).collect::<Vec<_>>(),
            ["门的"]
        );
        // 同词同音在两份对照文件里都有时只列构建留下的那一行（权重高的），建议的覆盖行不会重复。
        let rows = examined_rows(
            &[
                (SOURCES[0], "门的\tmen'de\t435135\n"),
                (SOURCES[1], "门的\tmen'de\t600000\n"),
                (SOURCES[2], ""),
                (SOURCES[3], ""),
            ],
            &curation,
        );
        let found = weight_disagreements(&rows, &upstream, 5.0, None);
        assert_eq!(
            found
                .iter()
                .map(|item| (item.row.source, item.row.weight))
                .collect::<Vec<_>>(),
            [(SOURCES[1], 600000)]
        );
        // 已经覆盖过的行不再列出。
        let curation = PinyinCuration::parse("", "门的\tmen'de\t4351\n").unwrap();
        let rows = examined_rows(&views(RIME_ICE, ""), &curation);
        assert!(weight_disagreements(&rows, &upstream, 5.0, None)
            .iter()
            .all(|item| item.row.word != "门的"));
    }

    /// 后续区划或人工词条已用更高权重覆盖读音时，上游行不再是构建留下的行；尤其不能建议覆盖人工提权，因为 `weight-overrides` 会拒绝它。
    #[test]
    fn upstream_comparisons_leave_out_rows_superseded_by_later_sources() {
        let curation = PinyinCuration::default();
        let mut upstream = Weights::new();
        parse_weights("甲乙\tjia yi\t1000\n", &mut upstream);
        for later in [SOURCES[2], SOURCES[3]] {
            let rows = examined_rows(
                &[
                    (SOURCES[0], "甲乙\tjia'yi\t100\n"),
                    (later, "甲乙\tjia'yi\t200\n"),
                ],
                &curation,
            );
            assert!(
                weight_disagreements(&rows, &upstream, 5.0, None).is_empty(),
                "{later} 已覆盖上游行，不应再建议改它的权重"
            );
        }

        // 人工词条没有提权时，上游行仍是构建的有效权重，照常给出候选。
        let rows = examined_rows(
            &views("甲乙\tjia'yi\t100\n", "甲乙\tjia'yi\t100\n"),
            &curation,
        );
        let found = weight_disagreements(&rows, &upstream, 5.0, None);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].row.source, SOURCES[0]);
    }

    #[test]
    fn placeholder_weights_are_repeated_digits_or_meme_numbers_on_multi_character_words() {
        assert!(is_placeholder_weight(9999));
        assert!(is_placeholder_weight(1111));
        assert!(is_placeholder_weight(33333));
        assert!(is_placeholder_weight(19_260_817));
        assert!(!is_placeholder_weight(999));
        assert!(!is_placeholder_weight(9998));
        let curation = PinyinCuration::default();
        let rows = examined_rows(&views(RIME_ICE, ""), &curation);
        let found = placeholder_weights(&rows, None);
        assert_eq!(
            found.iter().map(|row| row.word).collect::<Vec<_>>(),
            ["梗数", "占位"]
        );
        let mut reference = Weights::new();
        parse_weights("占位\tzhan wei\t5\n", &mut reference);
        let found = placeholder_weights(&rows, Some(&reference));
        assert_eq!(
            found.iter().map(|row| row.word).collect::<Vec<_>>(),
            ["梗数"]
        );
    }

    #[test]
    fn fragments_outranking_a_common_word_of_the_same_key_are_listed() {
        let mut reference = Weights::new();
        parse_weights(
            "闷的\tmen de\t3000\n门\tmen\t9\n中区\tzhong qu\t1000\n在中\tzai zhong\t50\n",
            &mut reference,
        );
        let curation = PinyinCuration::default();
        // custom 的 中去 不列：覆盖表不能改人工词条。
        let custom = "中去\tzhong'qu\t300000\n";
        let found = fragments_over_common(&views(RIME_ICE, custom), &curation, &reference, 100);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0],
            Fragment {
                key: "men'de",
                word: "门的",
                weight: 435135,
                source: SOURCES[0],
                outranked: 1,
                strongest: ("闷的", 300000, 3000),
                suggested: 299999,
            }
        );
        let found = fragments_over_common(&views(RIME_ICE, ""), &curation, &reference, 100);
        assert_eq!(
            found.iter().map(|item| item.word).collect::<Vec<_>>(),
            ["门的", "中去"]
        );
    }

    #[test]
    fn vulgar_fragments_are_longer_words_containing_a_listed_word() {
        let curation = PinyinCuration::default();
        let rows = examined_rows(&views(RIME_ICE, "是傻逼\tshi'sha'bi\t5\n"), &curation);
        let entries =
            curation::parse_offensive_words(msime_engine::EMBEDDED_OFFENSIVE_WORDS).unwrap();
        let listed: HashSet<String> = entries
            .iter()
            .filter(|entry| !entry.exempt)
            .map(|entry| entry.text.clone())
            .collect();
        let table = msime_engine::OffensiveWords::from_entries(entries).unwrap();
        let found = vulgar_fragments(&rows, &table, &listed);
        assert_eq!(
            found
                .iter()
                .map(|item| (item.word, item.sources.clone()))
                .collect::<Vec<_>>(),
            [("别鸡巴", vec![SOURCES[0]]), ("是傻逼", vec![SOURCES[3]])]
        );
        // 已经否决的不再列出。
        let curation = PinyinCuration::parse("别鸡巴\n", "").unwrap();
        let rows = examined_rows(&views(RIME_ICE, ""), &curation);
        assert!(vulgar_fragments(&rows, &table, &listed).is_empty());
    }
}
