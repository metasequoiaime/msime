//! `reweight` 阶段：按万象拼音（rime-wanxiang）在构建期重算全拼表里与万象共有的行的权重，插在 `quanpin` 之后、`places-supplement` 之前。
//!
//! 万象的 17 份 `dicts/*.dict.yaml` 由 msime-dictionary 逐字节收在 `sources/pinyin/wanxiang/`，登记在它的 `upstream.lock.json`（上游名 [`REFERENCE`]），提交等于 msime 锁文件的同名 reference。checkout 的记录里没有这个上游时（登记之前的 msime-dictionary 提交，包括 `dict-v2.0.56` 及更早的版本）阶段什么也不做，产物与没有这个阶段时逐字节相同。
//!
//! 第一步总是做：复现 msime-dictionary#84 当时一次性写进源文件的换算。参与对照的是万象 `wanxiang.dict.yaml` 导入的 15 份词库（[`IMPORTED`]；错音表 `cuoyin` 和 `mixed` 只登记、不参与）。万象的读音去掉声调、ü 写成 v、音节之间用 `'`（[`normalize_reading`]）；只认显式写了非负整数权重的行，同一个「词＋全拼」出现多次时取最高的权重。源文件里一行的「词＋全拼」在万象里有权重时，新权重是 `round(万象权重 × 文件系数)`，四舍五入、用整数算（[`FILE_SCALES`]：`single-chars.txt` 9.42，`rime-ice.txt` 9.2，`rime-ice-supplement.txt` 9.28，就是 #84 用的系数）；其余行不动。源文件现在的权重正是这样算出来的（三份文件里与万象共有的 597343 行全部吻合），所以第一步不改变任何一行；它的用处是以后在原路径重新生成补充表、或者换新的上游提交时，对齐结果不会被冲掉。
//!
//! 第二步由 msime-dictionary 的 `curation/pinyin-reweight.txt`（[`SETTINGS`]）打开，文件不在时不做，见 [`Settings`]：`renming`（人名）和 `duoyin`（多音兼容读音）两份词库各用一个缩放比例，乘在文件系数之前；`unattested-cap` 让万象没有收录的多字词不高于同一全拼键上万象收录的最高的词。

use std::collections::{HashMap, HashSet};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};

use crate::cleanse_report::normalize_reading;
use crate::msime;
use crate::sources::Sources;
use crate::text;

/// msime 锁文件和 msime-dictionary `upstream.lock.json` 里万象的上游名。
pub const REFERENCE: &str = "rime-wanxiang";

/// 万象词库在 msime-dictionary 里的目录，结尾带 `/`，与 `UPSTREAM_FILES` 的目录前缀写法相同。
pub const DIRECTORY: &str = "sources/pinyin/wanxiang/";

/// 参与对照的万象词库（`wanxiang.dict.yaml` 的 import_tables，按它的次序），文件名是 `<名字>.dict.yaml`。
pub const IMPORTED: [&str; 15] = [
    "zi",
    "jichu",
    "lianxiang",
    "duoyin",
    "shici",
    "diming",
    "yixue",
    "huaxue",
    "yaopin",
    "mingren",
    "yiren",
    "wuzhong",
    "renming",
    "taifeng",
    "fangyan",
];

/// 只登记、不参与对照的两份：`cuoyin` 收的是错误读音，`mixed` 是中英混写的词，万象自己的方案也不把它们当词频用。
pub const RECORDED_ONLY: [&str; 2] = ["cuoyin", "mixed"];

/// 第二步的开关和缩放比例，维护者审定的表，放在 `curation/`。
pub const SETTINGS: &str = "curation/pinyin-reweight.txt";

/// 单字表的路径。
pub const SINGLE_CHARS: &str = "sources/pinyin/single-chars.txt";
/// 主词组表的路径。
pub const RIME_ICE: &str = "sources/pinyin/rime-ice.txt";
/// rime-ice 补充表的路径。
pub const RIME_ICE_SUPPLEMENT: &str = "sources/pinyin/rime-ice-supplement.txt";

/// 每份源文件的系数，以百分之一为单位：9.42、9.2、9.28，是 msime-dictionary#84 按「该文件与万象共有、两边权重都大于 10 的行上本仓权重之和 ÷ 万象权重之和」算出的值。没有列出的输入（只在 `--include-unlicensed` 的本地构建里出现的 CustomPinyinDictionary 两份）不换算。
pub const FILE_SCALES: [(&str, i64); 3] = [
    (SINGLE_CHARS, 942),
    (RIME_ICE, 920),
    (RIME_ICE_SUPPLEMENT, 928),
];

/// 一份源文件的系数（百分之一），不换算的输入为 `None`。
pub fn file_scale(path: &str) -> Option<i64> {
    FILE_SCALES
        .iter()
        .find(|(listed, _)| *listed == path)
        .map(|(_, scale)| *scale)
}

/// 一份万象词库在 checkout 里的路径。
pub fn dictionary_path(name: &str) -> String {
    format!("{DIRECTORY}{name}.dict.yaml")
}

/// 缩放比例的上限（百分之一）：比例只用来把某一份词库的量级调回其他词库的量级，大于 10 倍不会是本意。
const MAXIMUM_SCALE: i64 = 1000;

/// `curation/pinyin-reweight.txt`：第二步的开关和两份词库的缩放比例。一行 `名字<TAB>值`，`#` 开头的行和空行不算：
///
/// - `unattested-cap`：`on` 或 `off`。打开时，万象没有收录（任何一份参与对照的词库都没有这个「词＋全拼」，没写权重的行也算收录）的多字词，权重不高于同一全拼键上万象收录的词里最高的那个减 1（最低到 1），排到它后面；单字不动，万象单字表的读音不全（嗯 只收 ng、n），单字的首选不能交给它。
/// - `renming`、`duoyin`：正的小数，最多两位，不大于 10，乘在这份词库的权重上、再乘文件系数。默认 1，即与其他词库同等对待（第一步的做法）。
///
/// 写了没有列出的名字、重复写同一个名字、值写错，构建失败。文件在、但每一项都等于默认值时也失败：那样的文件什么也不改，留着只会让人以为第二步开着。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub unattested_cap: bool,
    /// `renming` 的缩放比例，百分之一。
    pub renming: i64,
    /// `duoyin` 的缩放比例，百分之一。
    pub duoyin: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            unattested_cap: false,
            renming: 100,
            duoyin: 100,
        }
    }
}

/// 把 `0.41`、`1`、`1.5` 这样最多两位小数的正数读成百分之一的整数。
fn parse_hundredths(value: &str) -> Option<i64> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > 2
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || (value.contains('.') && fraction.is_empty())
    {
        return None;
    }
    let whole: i64 = whole.parse().ok()?;
    let fraction: i64 = format!("{fraction:0<2}").parse().ok()?;
    whole.checked_mul(100)?.checked_add(fraction)
}

impl Settings {
    pub fn parse(text: &str) -> Result<Self> {
        let mut settings = Self::default();
        let mut seen = HashSet::new();
        for (index, line) in text::splitlines(text::without_bom(text))
            .into_iter()
            .enumerate()
        {
            let line = text::strip(line);
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let at = format!("{SETTINGS}:{}", index + 1);
            let fields: Vec<&str> = line.split('\t').map(text::strip).collect();
            let [name, value] = fields[..] else {
                bail!("{at}: expected a name and a value separated by a TAB: {line:?}");
            };
            if !seen.insert(name.to_owned()) {
                bail!("{at}: {name} is listed twice");
            }
            match name {
                "unattested-cap" => {
                    settings.unattested_cap = match value {
                        "on" => true,
                        "off" => false,
                        _ => bail!("{at}: unattested-cap is on or off, not {value:?}"),
                    }
                }
                "renming" | "duoyin" => {
                    let scale = parse_hundredths(value)
                        .filter(|scale| (1..=MAXIMUM_SCALE).contains(scale))
                        .with_context(|| {
                            format!("{at}: {name} is a positive number with at most two decimals, at most 10, not {value:?}")
                        })?;
                    if name == "renming" {
                        settings.renming = scale;
                    } else {
                        settings.duoyin = scale;
                    }
                }
                _ => bail!("{at}: {name:?} is not a setting; the settings are unattested-cap, renming and duoyin"),
            }
        }
        if settings == Self::default() {
            bail!("{SETTINGS}: every setting is at its default, so the file changes nothing; delete it to leave the second step off");
        }
        Ok(settings)
    }

    /// 从 `--dictionary` checkout 读，文件不在时是默认值（只做第一步）。
    pub fn load(sources: &Sources) -> Result<Self> {
        match crate::curation::read_optional(sources, SETTINGS)? {
            Some(text) => Self::parse(&text),
            None => Ok(Self::default()),
        }
    }

    /// 一份万象词库的缩放比例，百分之一。
    fn scale(&self, name: &str) -> i64 {
        match name {
            "renming" => self.renming,
            "duoyin" => self.duoyin,
            _ => 100,
        }
    }

    /// 写进阶段摘要的一句话。
    pub fn describe(&self) -> String {
        if *self == Self::default() {
            return "first step only".to_owned();
        }
        format!(
            "second step: unattested-cap {}, renming {}, duoyin {}",
            if self.unattested_cap { "on" } else { "off" },
            format_hundredths(self.renming),
            format_hundredths(self.duoyin)
        )
    }
}

fn format_hundredths(value: i64) -> String {
    format!("{}.{:02}", value / 100, value % 100)
}

/// `round(weight × scale / 10⁴)`，四舍五入。`scaled` 是已经乘过词库缩放比例的万象权重（百分之一），`file_scale` 是文件系数（百分之一）。用整数算：#84 用浮点数乘 9.42 这类系数，597343 行里没有一行落在需要分辨 .5 的边界上，整数算法与它逐行相同，又不依赖浮点误差。
pub fn round_scaled(scaled: i64, file_scale: i64) -> i64 {
    (scaled * file_scale + 5_000) / 10_000
}

/// 参与对照的万象词库里一个「词＋全拼」的信息。
#[derive(Debug, Clone, Copy, Default)]
struct Attestation {
    /// 显式写了权重的行里最高的「权重 × 缩放比例」（百分之一）；没有一行写了权重时为 `None`。
    scaled: Option<i64>,
}

/// 一份万象 `.dict.yaml` 的词条行：`...` 一行结束文件头，之后是 `词<TAB>读音[<TAB>权重]`。读音读不懂（带辅助码、标点）的行和没有词的行跳过。权重只认非负整数，其余（没写、`-1`）为 `None`。
fn wanxiang_rows(text: &str) -> impl Iterator<Item = (&str, String, Option<i64>)> {
    let lines = text::universal_lines(text::without_bom(text));
    let start = lines
        .iter()
        .position(|line| *line == "...")
        .map_or(lines.len(), |end| end + 1);
    lines.into_iter().skip(start).filter_map(|line| {
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let mut fields = line.split('\t');
        let word = text::strip(fields.next()?);
        let key = normalize_reading(text::strip(fields.next()?))?;
        if word.is_empty() {
            return None;
        }
        let weight = fields.next().map(text::strip).and_then(|weight| {
            (!weight.is_empty() && weight.bytes().all(|b| b.is_ascii_digit()))
                .then(|| weight.parse::<i64>().ok())
                .flatten()
        });
        Some((word, key, weight))
    })
}

/// 一份全拼输入：路径（报错和查文件系数用）、内容，以及只对单字表生效的白名单。
pub struct Input<'a> {
    pub path: &'a str,
    pub text: &'a str,
    pub accepted: Option<&'a HashSet<String>>,
}

/// 全拼表里一行在阶段里的状态。
#[derive(Debug, Clone)]
struct Row {
    /// 按第一步、第二步算出的新权重，多份输入都有这一行时取最高的，与 `quanpin` 阶段去重的规则相同。
    weight: i64,
    /// 换算之前的权重（同样取最高），只用来分类计数。
    original: i64,
    /// 文件系数换算（缩放比例都按 1）之后的权重，只用来分类计数。
    aligned: i64,
    attested: bool,
    /// 有一份输入的这一行权重不是整数：这一行不动，由 SQLite 按 `quanpin` 阶段写入的原样保留。
    unreadable: bool,
}

/// `reweight` 阶段的结果。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// 全拼表里与万象共有（万象写了权重）的行。
    pub shared: usize,
    /// 文件系数换算改了权重的行。源文件已经是对齐后的样子时为 0。
    pub aligned: usize,
    /// `renming`、`duoyin` 的缩放比例又改了权重的行。
    pub scaled: usize,
    /// `unattested-cap` 压下去的行。
    pub capped: usize,
}

/// 照 `msime::insert_quanpin_file` 读一份全拼输入：按 `\n` 分行、`#` 开头的行跳过、去掉首尾空白后按 TAB 分列，白名单之外的单字跳过，读音不以小写 ASCII 字母开头的行跳过。权重读不成整数时为 `None`。两边必须保持一致，`reweight` 阶段才只碰 `quanpin` 阶段写进去的那些行。
fn quanpin_rows<'a>(
    input: &'a Input<'a>,
) -> impl Iterator<Item = Result<(&'a str, &'a str, Option<i64>)>> + 'a {
    input
        .text
        .split_terminator('\n')
        .enumerate()
        .filter(|(_, line)| !line.starts_with('#'))
        .filter_map(move |(number, line)| {
            let fields: Vec<&str> = text::strip(line).split('\t').collect();
            let value = fields[0];
            if input
                .accepted
                .is_some_and(|accepted| !accepted.contains(value))
            {
                return None;
            }
            let (Some(key), Some(weight)) = (fields.get(1), fields.get(2)) else {
                return Some(Err(anyhow::anyhow!(
                    "{}:{}: expected value, pinyin and weight",
                    input.path,
                    number + 1
                )));
            };
            if !key.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
                return None;
            }
            Some(Ok((value, *key, text::strip(weight).parse::<i64>().ok())))
        })
}

/// 按万象重算全拼表的权重。`inputs` 是 `quanpin` 阶段读的全部输入，按它读的次序；`wanxiang` 是 [`IMPORTED`] 里每份词库的名字和内容。只改权重，不增删行、不改行序；与新权重相同的行不写。
pub fn apply(
    connection: &mut Connection,
    inputs: &[Input],
    wanxiang: &[(&str, &str)],
    settings: &Settings,
) -> Result<Counts> {
    // 先记下全拼表里的每一行，再只为这些行收集万象的信息：万象有 220 万个「词＋全拼」，全拼表不到 100 万行。
    let mut lines: Vec<(&str, &str, Option<i64>, Option<i64>)> = Vec::new();
    // 全拼 → 这个全拼下的词；分两层是为了能拿万象那边临时拼出来的全拼查表。
    let mut keys: HashMap<&str, HashSet<&str>> = HashMap::new();
    for input in inputs {
        let scale = file_scale(input.path);
        for row in quanpin_rows(input) {
            let (value, key, weight) = row?;
            lines.push((value, key, weight, scale));
            keys.entry(key).or_default().insert(value);
        }
    }
    // 每个「词＋全拼」两份万象权重：缩放比例都按 1 的（第一步）和按设置缩放的。
    let mut attestations: HashMap<(&str, &str), (Attestation, Attestation)> = HashMap::new();
    for (name, text) in wanxiang {
        let scale = settings.scale(name);
        for (word, key, weight) in wanxiang_rows(text) {
            let Some((&row_key, &row_value)) = keys
                .get_key_value(key.as_str())
                .and_then(|(row_key, values)| Some((row_key, values.get(word)?)))
            else {
                continue;
            };
            let (aligned, scaled) = attestations.entry((row_key, row_value)).or_default();
            if let Some(weight) = weight {
                aligned.scaled = Some(aligned.scaled.unwrap_or(0).max(weight * 100));
                scaled.scaled = Some(scaled.scaled.unwrap_or(0).max(weight * scale));
            }
        }
    }
    let mut rows: HashMap<(&str, &str), Row> = HashMap::new();
    for (value, key, weight, file_scale) in lines {
        let attestation = attestations.get(&(key, value));
        let Some(weight) = weight else {
            rows.entry((key, value))
                .or_insert(Row {
                    weight: 0,
                    original: 0,
                    aligned: 0,
                    attested: attestation.is_some(),
                    unreadable: true,
                })
                .unreadable = true;
            continue;
        };
        let (aligned, scaled) = match (attestation, file_scale) {
            (Some((aligned, scaled)), Some(file_scale)) => (
                aligned
                    .scaled
                    .map_or(weight, |w| round_scaled(w, file_scale)),
                scaled
                    .scaled
                    .map_or(weight, |w| round_scaled(w, file_scale)),
            ),
            _ => (weight, weight),
        };
        rows.entry((key, value))
            .and_modify(|row| {
                row.original = row.original.max(weight);
                row.aligned = row.aligned.max(aligned);
                row.weight = row.weight.max(scaled);
            })
            .or_insert(Row {
                weight: scaled,
                original: weight,
                aligned,
                attested: attestation.is_some(),
                unreadable: false,
            });
    }
    let shared = attestations
        .values()
        .filter(|(aligned, _)| aligned.scaled.is_some())
        .count();
    let mut capped: HashSet<(&str, &str)> = HashSet::new();
    if settings.unattested_cap {
        let mut ceilings: HashMap<&str, i64> = HashMap::new();
        for ((key, _), row) in &rows {
            if row.attested && !row.unreadable {
                let ceiling = ceilings.entry(key).or_insert(row.weight);
                *ceiling = (*ceiling).max(row.weight);
            }
        }
        for ((key, value), row) in rows.iter_mut() {
            if row.attested || row.unreadable || value.chars().count() < 2 {
                continue;
            }
            let Some(&ceiling) = ceilings.get(key) else {
                continue;
            };
            if row.weight >= ceiling {
                let lowered = row.weight.min((ceiling - 1).max(1));
                if lowered != row.weight {
                    row.weight = lowered;
                    capped.insert((key, value));
                }
            }
        }
    }
    let mut counts = Counts {
        shared,
        ..Counts::default()
    };
    let transaction = connection.transaction()?;
    for ((key, value), row) in &rows {
        if row.unreadable || row.weight == row.original {
            continue;
        }
        let table = msime::pinyin_table(key)
            .with_context(|| format!("{key:?} maps to no quanpin table"))?;
        let changed = transaction
            .prepare_cached(&format!(
                "update {table} set weight = ?1 where key = ?2 and value = ?3 and weight != ?1"
            ))?
            .execute(params![row.weight, key, value])?;
        if changed == 0 {
            continue;
        }
        if capped.contains(&(*key, *value)) {
            counts.capped += 1;
        } else if row.weight != row.aligned {
            counts.scaled += 1;
        } else {
            counts.aligned += 1;
        }
    }
    transaction.commit()?;
    Ok(counts)
}

/// checkout 登记了万象时，[`IMPORTED`] 每份词库的名字和内容；没有登记时为 `None`，阶段不做。17 份都按 `upstream.lock.json` 校验字节，[`RECORDED_ONLY`] 的两份只校验不读：登记的是整套 `dicts/`，少了哪一份、哪一份被改过都让构建失败。
pub fn load(sources: &Sources) -> Result<Option<Vec<(&'static str, String)>>> {
    let Some(dictionary) = sources.dictionary.as_ref() else {
        bail!("the reweight stage reads msime-dictionary's {DIRECTORY}; pass --dictionary <msime-dictionary checkout>");
    };
    if !dictionary.records_upstream(REFERENCE) {
        return Ok(None);
    }
    for name in RECORDED_ONLY {
        sources.pinned(&dictionary_path(name))?;
    }
    IMPORTED
        .iter()
        .map(|name| {
            let path = sources.pinned(&dictionary_path(name))?;
            Ok((*name, text::read(&path)?))
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database(rows: &[(&str, &str, i64)]) -> Connection {
        let directory = tempfile::tempdir().unwrap();
        let single = directory.path().join("single.txt");
        let phrases = directory.path().join("phrases.txt");
        let mut single_text = String::new();
        let mut phrase_text = String::new();
        for (value, key, weight) in rows {
            let line = format!("{value}\t{key}\t{weight}\n");
            if value.chars().count() == 1 {
                single_text.push_str(&line);
            } else {
                phrase_text.push_str(&line);
            }
        }
        std::fs::write(&single, single_text).unwrap();
        std::fs::write(&phrases, phrase_text).unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        msime::build_quanpin(
            &mut connection,
            &msime::QuanpinInputs {
                single_chars: &single,
                whitelist: None,
                phrases: vec![phrases.as_path()],
            },
        )
        .unwrap();
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
            .ok()
    }

    const HEADER: &str = "# Rime dictionary\n---\nname: test\n...\n";

    #[test]
    fn the_first_step_reproduces_the_alignment_already_in_the_sources() {
        // 万象 163 × 9.2 = 1499.6，四舍五入是 1500；车件 万象不收，保持原样。
        let rows = [
            ("车间", "che'jian", 33948),
            ("车件", "che'jian", 5895),
            ("车检", "che'jian", 1500),
        ];
        let mut connection = database(&rows);
        let text = "车间\tche'jian\t33948\n车件\tche'jian\t5895\n车检\tche'jian\t1500\n";
        let jichu = format!("{HEADER}车检\tchē jiǎn\t163\n车间\tchē jiān\t3690\n");
        let counts = apply(
            &mut connection,
            &[Input {
                path: RIME_ICE,
                text,
                accepted: None,
            }],
            &[("jichu", &jichu)],
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(
            counts,
            Counts {
                shared: 2,
                ..Counts::default()
            }
        );
        assert_eq!(weight(&connection, "车检", "che'jian"), Some(1500));
        assert_eq!(weight(&connection, "车件", "che'jian"), Some(5895));
        assert_eq!(weight(&connection, "车间", "che'jian"), Some(33948));
    }

    #[test]
    fn a_regenerated_source_is_aligned_again() {
        // 补充表重新生成后回到上游的权重：阶段按文件系数 9.28 改回去。
        let mut connection = database(&[("阿贝", "a'bei", 4351), ("阿宾", "a'bin", 7)]);
        let text = "阿贝\ta'bei\t4351\n阿宾\ta'bin\t7\n";
        let zi_free = format!("{HEADER}阿贝\tā bèi\t1052\n");
        let counts = apply(
            &mut connection,
            &[Input {
                path: RIME_ICE_SUPPLEMENT,
                text,
                accepted: None,
            }],
            &[("jichu", &zi_free)],
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(
            counts,
            Counts {
                shared: 1,
                aligned: 1,
                ..Counts::default()
            }
        );
        // 1052 × 9.28 = 9762.56 → 9763，就是 rime-ice-supplement.txt 现在的值。
        assert_eq!(weight(&connection, "阿贝", "a'bei"), Some(9763));
        assert_eq!(weight(&connection, "阿宾", "a'bin"), Some(7));
    }

    #[test]
    fn rows_without_a_weight_or_with_a_negative_one_are_not_used_and_lv_is_read_as_v() {
        // 万象 diming 的 柏香镇 写 -1、fangyan 的 闹热 没写权重，#84 都没有换算；绿 lǜ 是 lv，不是 lu。
        let rows = [
            ("柏香镇", "bai'xiang'zhen", 149),
            ("闹热", "nao're", 1435),
            ("绿", "lv", 1),
            ("绿", "lu", 1526),
        ];
        let mut connection = database(&rows);
        let phrases = "柏香镇\tbai'xiang'zhen\t149\n闹热\tnao're\t1435\n";
        let single = "绿\tlv\t1\n绿\tlu\t1526\n";
        let diming = format!("{HEADER}柏香镇\tbǎi xiāng zhèn\t-1\n");
        let fangyan = format!("{HEADER}闹热\tnào rè\n");
        let zi = format!("{HEADER}绿\tlǜ\t1643\n");
        let counts = apply(
            &mut connection,
            &[
                Input {
                    path: SINGLE_CHARS,
                    text: single,
                    accepted: None,
                },
                Input {
                    path: RIME_ICE,
                    text: phrases,
                    accepted: None,
                },
            ],
            &[("zi", &zi), ("diming", &diming), ("fangyan", &fangyan)],
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(counts.shared, 1);
        assert_eq!(counts.aligned, 1);
        // 1643 × 9.42 = 15477.06。
        assert_eq!(weight(&connection, "绿", "lv"), Some(15477));
        assert_eq!(weight(&connection, "绿", "lu"), Some(1526));
        assert_eq!(weight(&connection, "柏香镇", "bai'xiang'zhen"), Some(149));
        assert_eq!(weight(&connection, "闹热", "nao're"), Some(1435));
    }

    #[test]
    fn a_reading_two_inputs_list_keeps_the_higher_aligned_weight() {
        // 同一个单字读音在单字表（9.42）和补充表（9.28）里都有：quanpin 阶段留下较高的一行，这里也取较高的换算结果。
        let mut connection = database(&[("嬢", "niang", 11229)]);
        let zi = format!("{HEADER}嬢\tniáng\t1192\n");
        let counts = apply(
            &mut connection,
            &[
                Input {
                    path: SINGLE_CHARS,
                    text: "嬢\tniang\t11229\n",
                    accepted: None,
                },
                Input {
                    path: RIME_ICE_SUPPLEMENT,
                    text: "嬢\tniang\t11062\n",
                    accepted: None,
                },
            ],
            &[("zi", &zi)],
            &Settings::default(),
        )
        .unwrap();
        // max(1192 × 9.42, 1192 × 9.28) = 11228.64 → 11229。
        assert_eq!(
            counts,
            Counts {
                shared: 1,
                ..Counts::default()
            }
        );
        assert_eq!(weight(&connection, "嬢", "niang"), Some(11229));
    }

    #[test]
    fn the_unattested_cap_puts_a_fragment_under_the_attested_leader_of_its_key() {
        let rows = [
            ("门的", "men'de", 435135),
            ("闷的", "men'de", 208546),
            ("们的", "men'de", 9),
            ("嗯", "en", 9179528),
            ("恩", "en", 299754),
        ];
        let mut connection = database(&rows);
        let phrases = "门的\tmen'de\t435135\n闷的\tmen'de\t208546\n们的\tmen'de\t9\n";
        let single = "嗯\ten\t9179528\n恩\ten\t299754\n";
        let jichu = format!("{HEADER}闷的\tmèn de\t22668\n");
        let zi = format!("{HEADER}恩\tēn\t31821\n");
        let settings = Settings {
            unattested_cap: true,
            ..Settings::default()
        };
        let counts = apply(
            &mut connection,
            &[
                Input {
                    path: SINGLE_CHARS,
                    text: single,
                    accepted: None,
                },
                Input {
                    path: RIME_ICE,
                    text: phrases,
                    accepted: None,
                },
            ],
            &[("zi", &zi), ("jichu", &jichu)],
            &settings,
        )
        .unwrap();
        assert_eq!(counts.capped, 1);
        assert_eq!(weight(&connection, "门的", "men'de"), Some(208545));
        assert_eq!(weight(&connection, "闷的", "men'de"), Some(208546));
        // 本来就在下面的不动；单字不受这条规则管。
        assert_eq!(weight(&connection, "们的", "men'de"), Some(9));
        assert_eq!(weight(&connection, "嗯", "en"), Some(9179528));
    }

    #[test]
    fn a_word_listed_without_a_weight_still_counts_as_attested() {
        let rows = [("闹热", "nao're", 1435), ("脑热", "nao're", 920)];
        let mut connection = database(&rows);
        let text = "闹热\tnao're\t1435\n脑热\tnao're\t920\n";
        let fangyan = format!("{HEADER}闹热\tnào rè\n");
        let jichu = format!("{HEADER}脑热\tnǎo rè\t100\n");
        let settings = Settings {
            unattested_cap: true,
            ..Settings::default()
        };
        let counts = apply(
            &mut connection,
            &[Input {
                path: RIME_ICE,
                text,
                accepted: None,
            }],
            &[("jichu", &jichu), ("fangyan", &fangyan)],
            &settings,
        )
        .unwrap();
        assert_eq!(counts.capped, 0);
        assert_eq!(weight(&connection, "闹热", "nao're"), Some(1435));
    }

    #[test]
    fn renming_and_duoyin_scales_apply_before_the_file_scale_and_only_where_they_lead() {
        let rows = [
            ("张勇", "zhang'yong", 42982),
            ("张开", "zhang'kai", 211655),
            ("多重", "duo'zhong", 335920),
        ];
        let mut connection = database(&rows);
        let text = "张勇\tzhang'yong\t42982\n张开\tzhang'kai\t211655\n多重\tduo'zhong\t335920\n";
        let renming = format!("{HEADER}张勇\tzhāng yǒng\t4672\n张开\tzhāng kāi\t23006\n");
        let jichu = format!("{HEADER}张开\tzhāng kāi\t20000\n");
        let duoyin = format!("{HEADER}多重\tduō zhòng\t36513\n");
        let settings = Settings {
            renming: 41,
            duoyin: 50,
            ..Settings::default()
        };
        let counts = apply(
            &mut connection,
            &[Input {
                path: RIME_ICE,
                text,
                accepted: None,
            }],
            &[
                ("jichu", &jichu),
                ("duoyin", &duoyin),
                ("renming", &renming),
            ],
            &settings,
        )
        .unwrap();
        assert_eq!(counts.scaled, 3);
        // 4672 × 0.41 × 9.2 = 17622.78。
        assert_eq!(weight(&connection, "张勇", "zhang'yong"), Some(17623));
        // max(20000, 23006 × 0.41) × 9.2：jichu 的权重更高，用它。
        assert_eq!(weight(&connection, "张开", "zhang'kai"), Some(184000));
        // 36513 × 0.5 × 9.2 = 167959.8。
        assert_eq!(weight(&connection, "多重", "duo'zhong"), Some(167960));
    }

    #[test]
    fn inputs_without_a_file_scale_are_left_alone() {
        let mut connection = database(&[("车检", "che'jian", 3000)]);
        let jichu = format!("{HEADER}车检\tchē jiǎn\t163\n");
        let counts = apply(
            &mut connection,
            &[Input {
                path: crate::licensing::BASE_DICT_PART1,
                text: "车检\tche'jian\t3000\n",
                accepted: None,
            }],
            &[("jichu", &jichu)],
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(
            counts,
            Counts {
                shared: 1,
                ..Counts::default()
            }
        );
        assert_eq!(weight(&connection, "车检", "che'jian"), Some(3000));
    }

    #[test]
    fn the_whitelist_and_comment_lines_are_read_as_the_quanpin_stage_reads_them() {
        let mut connection = database(&[("恩", "en", 100)]);
        let zi = format!("{HEADER}恩\tēn\t31821\n嗯\tèn\t1\n");
        let accepted: HashSet<String> = ["恩".to_owned()].into();
        let counts = apply(
            &mut connection,
            &[Input {
                path: SINGLE_CHARS,
                text: "# 注释\n恩\ten\t100\n嗯\ten\t9179528\n",
                accepted: Some(&accepted),
            }],
            &[("zi", &zi)],
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(
            counts,
            Counts {
                shared: 1,
                aligned: 1,
                ..Counts::default()
            }
        );
        assert_eq!(weight(&connection, "恩", "en"), Some(299754));
    }

    #[test]
    fn settings_parse_names_values_and_reject_mistakes() {
        assert_eq!(
            Settings::parse("# 第二步\nunattested-cap\ton\nrenming\t0.41\nduoyin\t1\n").unwrap(),
            Settings {
                unattested_cap: true,
                renming: 41,
                duoyin: 100
            }
        );
        assert_eq!(Settings::parse("duoyin\t0.5\n").unwrap().duoyin, 50);
        for (text, needle) in [
            ("unattested-cap\tyes\n", "on or off"),
            ("renming\t0\n", "positive"),
            ("renming\t0.415\n", "positive"),
            ("renming\t11\n", "positive"),
            ("renming\t-1\n", "positive"),
            ("renming\t1.\n", "positive"),
            ("renming\t0.4\nrenming\t0.5\n", "listed twice"),
            ("zi\t0.5\n", "not a setting"),
            ("renming 0.4\n", "TAB"),
            ("unattested-cap\toff\nrenming\t1\n", "changes nothing"),
            ("", "changes nothing"),
        ] {
            let error = format!("{:#}", Settings::parse(text).unwrap_err());
            assert!(error.contains(needle), "{text:?}: {error}");
        }
        assert_eq!(Settings::default().describe(), "first step only");
        assert_eq!(
            Settings::parse("unattested-cap\ton\nrenming\t0.41\n")
                .unwrap()
                .describe(),
            "second step: unattested-cap on, renming 0.41, duoyin 1.00"
        );
    }

    #[test]
    fn round_scaled_rounds_half_up_with_integers() {
        assert_eq!(round_scaled(163 * 100, 920), 1500);
        assert_eq!(round_scaled(10 * 100, 920), 92);
        // 25 × 9.42 = 235.5 正好落在边界上，四舍五入是 236。
        assert_eq!(round_scaled(25 * 100, 942), 236);
        assert_eq!(round_scaled(1, 100), 0);
    }

    #[test]
    fn every_wanxiang_dictionary_is_imported_or_recorded_only_once() {
        let mut names: Vec<&str> = IMPORTED
            .iter()
            .chain(RECORDED_ONLY.iter())
            .copied()
            .collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), IMPORTED.len() + RECORDED_ONLY.len());
        assert_eq!(
            dictionary_path("zi"),
            "sources/pinyin/wanxiang/zi.dict.yaml"
        );
        assert_eq!(
            crate::sources::upstream_reference(&dictionary_path("renming")),
            Some(REFERENCE)
        );
    }
}
