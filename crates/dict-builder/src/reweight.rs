//! `reweight` 阶段：按万象拼音（rime-wanxiang）在构建期重算全拼表里与万象共有的行的权重，插在 `quanpin` 之后、`places-supplement` 之前。
//!
//! 万象的 17 份 `dicts/*.dict.yaml` 由 msime-dictionary 逐字节收在 `sources/pinyin/wanxiang/`，登记在它的 `upstream.lock.json`（上游名 [`REFERENCE`]），提交等于 msime 锁文件的同名 reference。checkout 的记录里没有这个上游时（登记之前的 msime-dictionary 提交，包括 `dict-v2.0.56` 及更早的版本）阶段什么也不做，产物与没有这个阶段时逐字节相同。
//!
//! 第一步总是做：复现 msime-dictionary#84 当时一次性写进源文件的换算。权重取自万象 `wanxiang.dict.yaml` 导入的 16 份词库里除错音表 `cuoyin` 以外的 15 份（[`IMPORTED`]，与 #84 相同；`cuoyin` 和不在导入之列的中英混写表 `mixed` 不提供权重，见 [`RECORDED_ONLY`]）。万象的读音去掉声调、ü 写成 v、音节之间用 `'`（[`normalize_reading`]）；只认显式写了非负整数权重的行，同一个「词＋全拼」出现多次时取最高的权重。源文件里一行的「词＋全拼」在万象里有权重时，新权重是 `round(万象权重 × 文件系数)`，四舍五入、用整数算（[`FILE_SCALES`]：`single-chars.txt` 9.42，`rime-ice.txt` 9.2，`rime-ice-supplement.txt` 9.28，就是 #84 用的系数）；其余行不动。源文件现在的权重正是这样算出来的（三份文件里与万象共有的 597343 行全部吻合），所以第一步不改变任何一行；它的用处是以后在原路径重新生成补充表、或者换新的上游提交时，对齐结果不会被冲掉。
//!
//! 第二步由 msime-dictionary 的 `curation/pinyin-reweight.txt`（[`SETTINGS`]）打开，文件不在时不做，见 [`Settings`]：`renming`（人名）和 `duoyin`（多音兼容读音）两份词库各用一个缩放比例，乘在文件系数之前；`unattested-cap` 让万象 17 份词库在任何读音下都不收的多字词，不高于同一全拼键上万象权重达到门槛（默认 100）的词（dict-cleanse 方案 2.1「碎片、虚词组合」）。`curation/pinyin-reweight-exempt.txt`（[`EXEMPT`]）列出第二步不碰的行。

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

/// 提供权重的万象词库，文件名是 `<名字>.dict.yaml`：`wanxiang.dict.yaml` 的 import_tables 一共 16 份，这里是除 `cuoyin` 以外的 15 份，按它的次序。不用 `cuoyin` 是 #84 的选择，不是上游方案的做法。
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

/// 不提供权重的两份：`cuoyin` 收的是错误读音（`wanxiang.dict.yaml` 导入它，#84 没有用它的权重），`mixed` 是中英混写的词（不在 import_tables 里）。第二步判断一个词是否被万象收录时也读它们：方案 2.1 的口径是全套 17 份。
pub const RECORDED_ONLY: [&str; 2] = ["cuoyin", "mixed"];

/// 第二步的开关和缩放比例，维护者审定的表，放在 `curation/`。
pub const SETTINGS: &str = "curation/pinyin-reweight.txt";

/// 第二步不碰的行，维护者审定的表：一行 `词<TAB>全拼`，见 [`Exemption`]。
pub const EXEMPT: &str = "curation/pinyin-reweight-exempt.txt";

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

/// `unattested-cap` 的门槛默认值：方案 2.1 的「万象权重 ≥100」，比的是万象原始权重，不乘缩放比例和文件系数。
pub const DEFAULT_CAP_LEADER_MINIMUM: i64 = 100;

/// `curation/pinyin-reweight-exempt.txt` 的一条：第二步不碰这个「词＋全拼」，它的权重停在第一步的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exemption {
    pub word: String,
    pub key: String,
    /// 1 起的行号，报错时用。
    pub line: usize,
}

/// `curation/pinyin-reweight.txt`：第二步的开关和参数。一行 `名字<TAB>值`，`#` 开头的行和空行不算：
///
/// - `unattested-cap`：`on` 或 `off`。打开时，万象 17 份词库（[`IMPORTED`] 加 [`RECORDED_ONLY`]）在任何读音下都没有收的多字词，如果权重不低于同一全拼键上「万象权重达到 `cap-leader-minimum`」的词里最高的那个，就压到它下面：同键被压的几行按原来的次序依次排在它下面（减 1、减 2……，最低到 1），原来权重相同的仍然相同。万象收了这个词、只是读音不同的行不压，那是方案 3.2「非标准读音」一类，要逐条审。单字不动，万象单字表的读音不全（嗯 只收 ng、n），单字的首选不能交给它。同键门槛以上的词换算后权重不到 2 时，这个键不压。
/// - `cap-leader-minimum`：正整数，默认 100。只在 `unattested-cap on` 时可以写。
/// - `renming`、`duoyin`：正的小数，最多两位，不大于 10，乘在这份词库的权重上、再乘文件系数。默认 1，即与其他词库同等对待（第一步的做法）。
///
/// 写了没有列出的名字、重复写同一个名字、值写错，构建失败。文件在、但每一项都等于默认值时也失败：那样的文件什么也不改，留着只会让人以为第二步开着。豁免表 [`EXEMPT`] 由 [`Settings::load`] 读进 `exempt`，见那里的规则。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub unattested_cap: bool,
    /// `unattested-cap` 的门槛：同键上万象原始权重达到它的词才能定上限。
    pub cap_leader_minimum: i64,
    /// `renming` 的缩放比例，百分之一。
    pub renming: i64,
    /// `duoyin` 的缩放比例，百分之一。
    pub duoyin: i64,
    /// 第二步不碰的行。
    pub exempt: Vec<Exemption>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            unattested_cap: false,
            cap_leader_minimum: DEFAULT_CAP_LEADER_MINIMUM,
            renming: 100,
            duoyin: 100,
            exempt: Vec::new(),
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

/// 一张 `curation/` 表里有内容的行：去掉首尾空白，跳过空行和 `#` 开头的行，带 1 起的行号。
fn table_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text::splitlines(text::without_bom(text))
        .into_iter()
        .enumerate()
        .map(|(index, line)| (index + 1, text::strip(line)))
        .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'))
}

impl Settings {
    pub fn parse(text: &str) -> Result<Self> {
        let mut settings = Self::default();
        let mut seen = HashSet::new();
        for (line_number, line) in table_lines(text) {
            let at = format!("{SETTINGS}:{line_number}");
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
                "cap-leader-minimum" => {
                    settings.cap_leader_minimum = value
                        .bytes()
                        .all(|b| b.is_ascii_digit())
                        .then(|| value.parse::<i64>().ok())
                        .flatten()
                        .filter(|minimum| *minimum >= 1)
                        .with_context(|| {
                            format!("{at}: cap-leader-minimum is a positive whole number, not {value:?}")
                        })?;
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
                _ => bail!("{at}: {name:?} is not a setting; the settings are unattested-cap, cap-leader-minimum, renming and duoyin"),
            }
        }
        if seen.contains("cap-leader-minimum") && !settings.unattested_cap {
            bail!("{SETTINGS}: cap-leader-minimum only applies with unattested-cap on");
        }
        if !settings.second_step() {
            bail!("{SETTINGS}: every setting is at its default, so the file changes nothing; delete it to leave the second step off");
        }
        Ok(settings)
    }

    /// 读豁免表 [`EXEMPT`]：一行 `词<TAB>全拼`，全拼要以小写字母开头；同一行写两次时失败。每一条是否真的被第二步改动，由 [`apply`] 核对。
    pub fn parse_exempt(text: &str) -> Result<Vec<Exemption>> {
        let mut seen = HashSet::new();
        let mut exempt = Vec::new();
        for (line_number, line) in table_lines(text) {
            let at = format!("{EXEMPT}:{line_number}");
            let fields: Vec<&str> = line.split('\t').map(text::strip).collect();
            let [word, key] = fields[..] else {
                bail!("{at}: expected a word and its quanpin separated by a TAB: {line:?}");
            };
            if word.is_empty() || !key.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
                bail!("{at}: expected a word and its quanpin separated by a TAB: {line:?}");
            }
            if !seen.insert((word.to_owned(), key.to_owned())) {
                bail!("{at}: {word} {key} is listed twice");
            }
            exempt.push(Exemption {
                word: word.to_owned(),
                key: key.to_owned(),
                line: line_number,
            });
        }
        Ok(exempt)
    }

    /// 第二步是否打开：开关或比例里至少有一项不是默认值。
    pub fn second_step(&self) -> bool {
        self.unattested_cap || self.renming != 100 || self.duoyin != 100
    }

    /// 从 `--dictionary` checkout 读。[`SETTINGS`] 不在时是默认值（只做第一步）。豁免表 [`EXEMPT`] 只能和打开第二步的 [`SETTINGS`] 一起存在：只删 [`SETTINGS`] 回滚第二步而留下豁免表时构建失败，两份要一起删。
    pub fn load(sources: &Sources) -> Result<Self> {
        let mut settings = match crate::curation::read_optional(sources, SETTINGS)? {
            Some(text) => Self::parse(&text)?,
            None => Self::default(),
        };
        if let Some(text) = crate::curation::read_optional(sources, EXEMPT)? {
            if !settings.second_step() {
                bail!("{EXEMPT} exempts rows from the second step, but {SETTINGS} does not turn it on; delete the two files together");
            }
            settings.exempt = Self::parse_exempt(&text)?;
        }
        Ok(settings)
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
        if !self.second_step() {
            return "first step only".to_owned();
        }
        let cap = if self.unattested_cap {
            format!("on (leaders weighted at least {})", self.cap_leader_minimum)
        } else {
            "off".to_owned()
        };
        format!(
            "second step: unattested-cap {cap}, renming {}, duoyin {}, {} exempt",
            format_hundredths(self.renming),
            format_hundredths(self.duoyin),
            self.exempt.len()
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

/// 提供权重的万象词库里一个「词＋全拼」的信息。
#[derive(Debug, Clone, Copy, Default)]
struct Attestation {
    /// 显式写了权重的行里最高的「权重 × 缩放比例」（百分之一）；没有一行写了权重时为 `None`。
    scaled: Option<i64>,
}

/// 一份万象 `.dict.yaml` 的词条行：`...` 一行结束文件头，之后不是空行、不以 `#` 开头的行。
fn wanxiang_lines(text: &str) -> impl Iterator<Item = &str> {
    let lines = text::universal_lines(text::without_bom(text));
    let start = lines
        .iter()
        .position(|line| *line == "...")
        .map_or(lines.len(), |end| end + 1);
    lines
        .into_iter()
        .skip(start)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
}

/// 万象词条行的词（第一列），读音读不读得懂都算：判断「万象任何读音都不收」用。
fn wanxiang_words(text: &str) -> impl Iterator<Item = &str> {
    wanxiang_lines(text).filter_map(|line| {
        let word = text::strip(line.split('\t').next()?);
        (!word.is_empty()).then_some(word)
    })
}

/// 万象词条行 `词<TAB>读音[<TAB>权重]`。读音读不懂（带辅助码、标点）的行和没有词的行跳过。权重只认非负整数，其余（没写、`-1`）为 `None`。
fn wanxiang_rows(text: &str) -> impl Iterator<Item = (&str, String, Option<i64>)> {
    wanxiang_lines(text).filter_map(|line| {
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

/// 提供权重的万象词库（[`IMPORTED`]）给每个「词＋全拼」显式写的最高原始权重，只收 `wanted` 认的「词＋全拼」；取权重的规则与第一步相同（读音按 [`normalize_reading`] 换算，只认非负整数权重，同一个「词＋全拼」取最高），不乘缩放比例和文件系数。`wanxiang` 是 [`load`] 读出的名字和内容，[`RECORDED_ONLY`] 的两份跳过。区划补充表的生成器用它给地名定万象下限。
pub fn imported_weights(
    wanxiang: &[(&str, &str)],
    wanted: impl Fn(&str, &str) -> bool,
) -> HashMap<(String, String), i64> {
    let mut weights: HashMap<(String, String), i64> = HashMap::new();
    for (name, text) in wanxiang {
        if !IMPORTED.contains(name) {
            continue;
        }
        for (word, key, weight) in wanxiang_rows(text) {
            let Some(weight) = weight else {
                continue;
            };
            if !wanted(word, &key) {
                continue;
            }
            let slot = weights.entry((word.to_owned(), key)).or_insert(weight);
            *slot = (*slot).max(weight);
        }
    }
    weights
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
    /// 文件系数换算（缩放比例都按 1）之后的权重，即第一步的结果；豁免的行停在这里。
    aligned: i64,
    /// 提供权重的万象词库给这个「词＋全拼」写的最高原始权重，没有写权重时为 `None`。
    raw: Option<i64>,
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
    /// 豁免表里的行，每一行都是第二步本来要改的。
    pub exempt: usize,
}

/// 照 `msime::insert_quanpin_file` 读一份全拼输入：按 `\n` 分行、`#` 开头的行跳过、去掉首尾空白后按 TAB 分列，白名单之外的单字跳过；没有第 2 列时失败，读音不以小写 ASCII 字母开头的行跳过，之后才要求有第 3 列。权重读不成整数时为 `None`。两边的检查和次序必须保持一致，`reweight` 阶段才只碰 `quanpin` 阶段写进去的那些行、在同样的行上失败。
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
            let missing = || {
                Some(Err(anyhow::anyhow!(
                    "{}:{}: expected value, pinyin and weight",
                    input.path,
                    number + 1
                )))
            };
            let Some(key) = fields.get(1) else {
                return missing();
            };
            if !key.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
                return None;
            }
            let Some(weight) = fields.get(2) else {
                return missing();
            };
            Some(Ok((value, *key, text::strip(weight).parse::<i64>().ok())))
        })
}

/// 按万象重算全拼表的权重。`inputs` 是 `quanpin` 阶段读的全部输入，按它读的次序；`wanxiang` 是万象词库的名字和内容，[`IMPORTED`] 里的提供权重，其余（[`RECORDED_ONLY`]）只用来判断一个词是否被收录。只改权重，不增删行、不改行序；与新权重相同的行不写。豁免表里的行匹配不到全拼表、或者第二步本来就不改它时失败。
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
    // 全拼表里的多字词：第二步只需要知道这些词万象收没收。
    let mut phrases: HashSet<&str> = HashSet::new();
    for input in inputs {
        let scale = file_scale(input.path);
        for row in quanpin_rows(input) {
            let (value, key, weight) = row?;
            lines.push((value, key, weight, scale));
            keys.entry(key).or_default().insert(value);
            if value.chars().count() >= 2 {
                phrases.insert(value);
            }
        }
    }
    // 每个「词＋全拼」两份万象权重：缩放比例都按 1 的（第一步）和按设置缩放的。
    let mut attestations: HashMap<(&str, &str), (Attestation, Attestation)> = HashMap::new();
    // 万象 17 份词库在任何读音下收了的多字词，只在打开 unattested-cap 时收集。
    let mut listed: HashSet<&str> = HashSet::new();
    for (name, text) in wanxiang {
        if settings.unattested_cap {
            listed.extend(wanxiang_words(text).filter_map(|word| phrases.get(word).copied()));
        }
        if !IMPORTED.contains(name) {
            continue;
        }
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
        let raw = attestation.and_then(|(aligned, _)| aligned.scaled.map(|w| w / 100));
        let Some(weight) = weight else {
            rows.entry((key, value))
                .or_insert(Row {
                    weight: 0,
                    original: 0,
                    aligned: 0,
                    raw,
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
                raw,
                unreadable: false,
            });
    }
    let shared = attestations
        .values()
        .filter(|(aligned, _)| aligned.scaled.is_some())
        .count();
    // 豁免的行退回第一步的结果；缩放比例改了它的，这条豁免算用上了。
    let mut exempt: HashMap<(&str, &str), (&Exemption, bool)> = HashMap::new();
    for exemption in &settings.exempt {
        let entry = (exemption.key.as_str(), exemption.word.as_str());
        let Some(row) = rows.get_mut(&entry).filter(|row| !row.unreadable) else {
            bail!(
                "{EXEMPT}:{}: {} {} is not a row of the quanpin table",
                exemption.line,
                exemption.word,
                exemption.key
            );
        };
        let rescaled = row.weight != row.aligned;
        row.weight = row.aligned;
        let (key, value) = rows.get_key_value(&entry).map(|(entry, _)| *entry).unwrap();
        exempt.insert((key, value), (exemption, rescaled));
    }
    let mut capped: HashSet<(&str, &str)> = HashSet::new();
    if settings.unattested_cap {
        let minimum = settings.cap_leader_minimum;
        let mut ceilings: HashMap<&str, i64> = HashMap::new();
        for ((key, _), row) in &rows {
            if !row.unreadable && row.raw.is_some_and(|raw| raw >= minimum) {
                let ceiling = ceilings.entry(key).or_insert(row.weight);
                *ceiling = (*ceiling).max(row.weight);
            }
        }
        // 全拼 → 要压的行；同键几行按原来的权重从高到低排，依次排在上限下面。
        let mut candidates: HashMap<&str, Vec<&str>> = HashMap::new();
        for ((key, value), row) in &rows {
            if row.unreadable || value.chars().count() < 2 || listed.contains(value) {
                continue;
            }
            let Some(&ceiling) = ceilings.get(key).filter(|ceiling| **ceiling > 1) else {
                continue;
            };
            if row.weight < ceiling {
                continue;
            }
            if let Some((_, used)) = exempt.get_mut(&(*key, *value)) {
                *used = true;
                continue;
            }
            candidates.entry(key).or_default().push(value);
        }
        for (key, values) in candidates {
            let ceiling = ceilings[key];
            let mut weights: Vec<i64> = values
                .iter()
                .map(|value| rows[&(key, *value)].weight)
                .collect();
            weights.sort_unstable_by(|a, b| b.cmp(a));
            weights.dedup();
            for value in values {
                let row = rows.get_mut(&(key, value)).unwrap();
                let rank = weights
                    .iter()
                    .position(|weight| *weight == row.weight)
                    .unwrap() as i64
                    + 1;
                row.weight = (ceiling - rank).max(1);
                capped.insert((key, value));
            }
        }
    }
    let mut unused: Vec<&Exemption> = exempt
        .values()
        .filter(|(_, used)| !used)
        .map(|(exemption, _)| *exemption)
        .collect();
    if !unused.is_empty() {
        unused.sort_by_key(|exemption| exemption.line);
        let listed: Vec<String> = unused
            .iter()
            .map(|exemption| {
                format!(
                    "{}:{} {} {}",
                    EXEMPT, exemption.line, exemption.word, exemption.key
                )
            })
            .collect();
        bail!(
            "the second step leaves these exempted rows alone, so the exemptions are stale; delete them: {}",
            listed.join(", ")
        );
    }
    let mut counts = Counts {
        shared,
        exempt: exempt.len(),
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

/// checkout 登记了万象时，17 份词库的名字和内容，[`IMPORTED`] 在前、[`RECORDED_ONLY`] 在后；没有登记时为 `None`，阶段不做。17 份都按 `upstream.lock.json` 校验字节：登记的是整套 `dicts/`，少了哪一份、哪一份被改过都让构建失败。
pub fn load(sources: &Sources) -> Result<Option<Vec<(&'static str, String)>>> {
    let Some(dictionary) = sources.dictionary.as_ref() else {
        bail!("the reweight stage reads msime-dictionary's {DIRECTORY}; pass --dictionary <msime-dictionary checkout>");
    };
    if !dictionary.records_upstream(REFERENCE) {
        return Ok(None);
    }
    IMPORTED
        .iter()
        .chain(RECORDED_ONLY.iter())
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
                ..Settings::default()
            }
        );
        assert_eq!(
            Settings::parse("unattested-cap\ton\ncap-leader-minimum\t1000\n")
                .unwrap()
                .cap_leader_minimum,
            1000
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
            (
                "unattested-cap\ton\ncap-leader-minimum\t0\n",
                "positive whole number",
            ),
            (
                "unattested-cap\ton\ncap-leader-minimum\t1e3\n",
                "positive whole number",
            ),
            (
                "renming\t0.41\ncap-leader-minimum\t100\n",
                "only applies with unattested-cap on",
            ),
        ] {
            let error = format!("{:#}", Settings::parse(text).unwrap_err());
            assert!(error.contains(needle), "{text:?}: {error}");
        }
        assert_eq!(Settings::default().describe(), "first step only");
        assert_eq!(
            Settings::parse("unattested-cap\ton\nrenming\t0.41\n")
                .unwrap()
                .describe(),
            "second step: unattested-cap on (leaders weighted at least 100), renming 0.41, duoyin 1.00, 0 exempt"
        );
        assert_eq!(
            Settings::parse("duoyin\t0.5\n").unwrap().describe(),
            "second step: unattested-cap off, renming 1.00, duoyin 0.50, 0 exempt"
        );
    }

    #[test]
    fn the_exempt_table_reads_word_and_quanpin_pairs_and_rejects_mistakes() {
        assert_eq!(
            Settings::parse_exempt("# 豁免\n李四\tli'si\n\n顾盼\tgu'pan\n").unwrap(),
            vec![
                Exemption {
                    word: "李四".to_owned(),
                    key: "li'si".to_owned(),
                    line: 2
                },
                Exemption {
                    word: "顾盼".to_owned(),
                    key: "gu'pan".to_owned(),
                    line: 4
                },
            ]
        );
        for (text, needle) in [
            ("李四\n", "separated by a TAB"),
            ("李四\tli'si\t1\n", "separated by a TAB"),
            ("李四\tLi'si\n", "separated by a TAB"),
            ("李四\tli'si\n李四\tli'si\n", "listed twice"),
        ] {
            let error = format!("{:#}", Settings::parse_exempt(text).unwrap_err());
            assert!(error.contains(needle), "{text:?}: {error}");
        }
    }

    fn cap() -> Settings {
        Settings {
            unattested_cap: true,
            ..Settings::default()
        }
    }

    fn phrases(rows: &[(&str, &str, i64)]) -> String {
        rows.iter()
            .map(|(value, key, weight)| format!("{value}\t{key}\t{weight}\n"))
            .collect()
    }

    #[test]
    fn only_a_leader_weighted_at_least_the_minimum_sets_the_ceiling() {
        // 万象 lianxiang 只收了 资讯有限公司，权重是底值 10（换算后 92）：不到 100，不给 咨询有限公司 定上限。
        let rows = [
            ("咨询有限公司", "zi'xun'you'xian'gong'si", 51835),
            ("资讯有限公司", "zi'xun'you'xian'gong'si", 92),
        ];
        let text = phrases(&rows);
        let lianxiang = format!("{HEADER}资讯有限公司\tzī xùn yǒu xiàn gōng sī\t10\n");
        let mut connection = database(&rows);
        let input = [Input {
            path: RIME_ICE,
            text: &text,
            accepted: None,
        }];
        let counts = apply(
            &mut connection,
            &input,
            &[("lianxiang", &lianxiang)],
            &cap(),
        )
        .unwrap();
        assert_eq!(counts.capped, 0);
        assert_eq!(
            weight(&connection, "咨询有限公司", "zi'xun'you'xian'gong'si"),
            Some(51835)
        );
        // 门槛写成 10 时才压。
        let mut connection = database(&rows);
        let settings = Settings {
            cap_leader_minimum: 10,
            ..cap()
        };
        let counts = apply(
            &mut connection,
            &input,
            &[("lianxiang", &lianxiang)],
            &settings,
        )
        .unwrap();
        assert_eq!(counts.capped, 1);
        assert_eq!(
            weight(&connection, "咨询有限公司", "zi'xun'you'xian'gong'si"),
            Some(91)
        );
    }

    #[test]
    fn a_word_wanxiang_lists_under_another_reading_or_only_in_cuoyin_is_not_capped() {
        // 万象 mingren 只收了 盖茨 gě cí，cuoyin 收了 一幢 的错音：两个词都算万象收录，读音不同的行留给方案 3.2 逐条审。
        let rows = [
            ("盖茨", "gai'ci", 52860),
            ("该词", "gai'ci", 48731),
            ("一幢", "yi'tong", 3000),
            ("一同", "yi'tong", 2000),
            ("碎片", "gai'ci", 50000),
        ];
        let text = phrases(&rows);
        let jichu = format!("{HEADER}该词\tgāi cí\t5297\n一同\tyī tóng\t217\n");
        let mingren = format!("{HEADER}盖茨\tgě cí\t1000\n");
        let cuoyin = format!("{HEADER}一幢\tyī tóng\n");
        let mut connection = database(&rows);
        let counts = apply(
            &mut connection,
            &[Input {
                path: RIME_ICE,
                text: &text,
                accepted: None,
            }],
            &[
                ("jichu", &jichu),
                ("mingren", &mingren),
                ("cuoyin", &cuoyin),
            ],
            &cap(),
        )
        .unwrap();
        // 碎片 是这里唯一一个万象任何读音都不收的词：压到 该词（5297 × 9.2 = 48732.4）下面。
        assert_eq!(counts.capped, 1);
        assert_eq!(weight(&connection, "碎片", "gai'ci"), Some(48731));
        assert_eq!(weight(&connection, "盖茨", "gai'ci"), Some(52860));
        assert_eq!(weight(&connection, "一幢", "yi'tong"), Some(3000));
    }

    #[test]
    fn rows_capped_on_one_key_keep_their_order_under_the_ceiling() {
        let rows = [
            ("真凉", "zhen'liang", 2550),
            ("振梁", "zhen'liang", 2520),
            ("贞良", "zhen'liang", 2520),
            ("甄亮", "zhen'liang", 920),
            ("振良", "zhen'liang", 790),
        ];
        let text = phrases(&rows);
        let jichu = format!("{HEADER}甄亮\tzhēn liàng\t100\n");
        let mut connection = database(&rows);
        let counts = apply(
            &mut connection,
            &[Input {
                path: RIME_ICE,
                text: &text,
                accepted: None,
            }],
            &[("jichu", &jichu)],
            &cap(),
        )
        .unwrap();
        assert_eq!(counts.capped, 3);
        assert_eq!(weight(&connection, "真凉", "zhen'liang"), Some(919));
        assert_eq!(weight(&connection, "振梁", "zhen'liang"), Some(918));
        assert_eq!(weight(&connection, "贞良", "zhen'liang"), Some(918));
        assert_eq!(weight(&connection, "甄亮", "zhen'liang"), Some(920));
        // 本来就在上限下面的不动。
        assert_eq!(weight(&connection, "振良", "zhen'liang"), Some(790));
    }

    #[test]
    fn a_ceiling_below_two_caps_nothing() {
        // renming 0.01 把 万象 10 换算成 round(0.1 × 9.2) = 1：压到 0 也排不到它下面，这个键不压。
        let rows = [("月的", "yue'de", 463120), ("约的", "yue'de", 1)];
        let text = phrases(&rows);
        let renming = format!("{HEADER}约的\tyuē de\t10\n");
        let settings = Settings {
            cap_leader_minimum: 10,
            renming: 1,
            ..cap()
        };
        let mut connection = database(&rows);
        let counts = apply(
            &mut connection,
            &[Input {
                path: RIME_ICE,
                text: &text,
                accepted: None,
            }],
            &[("renming", &renming)],
            &settings,
        )
        .unwrap();
        assert_eq!(counts.capped, 0);
        assert_eq!(weight(&connection, "月的", "yue'de"), Some(463120));
    }

    #[test]
    fn exempted_rows_keep_the_first_step_weight() {
        let rows = [
            ("李四", "li'si", 57666),
            ("李斯", "li'si", 36193),
            ("顾盼", "gu'pan", 9880),
            ("顾磐", "gu'pan", 1766),
        ];
        let text = phrases(&rows);
        let renming = format!("{HEADER}李四\tlǐ sì\t6268\n");
        let jichu = format!("{HEADER}李斯\tlǐ sī\t3934\n顾磐\tgù pán\t192\n");
        let exempt = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .enumerate()
                .map(|(index, (word, key))| Exemption {
                    word: (*word).to_owned(),
                    key: (*key).to_owned(),
                    line: index + 1,
                })
                .collect::<Vec<_>>()
        };
        let settings = Settings {
            renming: 41,
            exempt: exempt(&[("李四", "li'si"), ("顾盼", "gu'pan")]),
            ..cap()
        };
        let input = [Input {
            path: RIME_ICE,
            text: &text,
            accepted: None,
        }];
        let wanxiang = [("jichu", jichu.as_str()), ("renming", renming.as_str())];
        let mut connection = database(&rows);
        let counts = apply(&mut connection, &input, &wanxiang, &settings).unwrap();
        assert_eq!(
            counts,
            Counts {
                shared: 3,
                exempt: 2,
                ..Counts::default()
            }
        );
        // 6268 × 9.2 = 57665.6：李四 停在第一步，不乘 renming 的 0.41；顾盼 不被压。
        assert_eq!(weight(&connection, "李四", "li'si"), Some(57666));
        assert_eq!(weight(&connection, "顾盼", "gu'pan"), Some(9880));
        // 豁免的行第二步本来就不改，或者全拼表里没有这一行：失败。
        for (pairs, needle) in [
            (&[("李斯", "li'si")][..], "stale"),
            (
                &[("顾盼", "gu'pan"), ("顾盼", "gu'fan")][..],
                "not a row of the quanpin table",
            ),
        ] {
            let settings = Settings {
                renming: 41,
                exempt: exempt(pairs),
                ..cap()
            };
            let mut connection = database(&rows);
            let error = format!(
                "{:#}",
                apply(&mut connection, &input, &wanxiang, &settings).unwrap_err()
            );
            assert!(error.contains(needle), "{pairs:?}: {error}");
        }
    }

    #[test]
    fn a_two_column_row_with_a_non_letter_reading_is_skipped_like_the_quanpin_stage_does() {
        let mut connection = database(&[("车检", "che'jian", 1500)]);
        let jichu = format!("{HEADER}车检\tchē jiǎn\t163\n");
        let counts = apply(
            &mut connection,
            &[Input {
                path: RIME_ICE,
                text: "ê\tê\n车检\tche'jian\t1500\n",
                accepted: None,
            }],
            &[("jichu", &jichu)],
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(counts.shared, 1);
        // 读音是小写字母开头、却没有权重的行，两个阶段都失败。
        let error = apply(
            &mut connection,
            &[Input {
                path: RIME_ICE,
                text: "车检\tche'jian\n",
                accepted: None,
            }],
            &[("jichu", &jichu)],
            &Settings::default(),
        )
        .unwrap_err();
        assert!(
            format!("{error:#}").contains("expected value, pinyin and weight"),
            "{error:#}"
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
