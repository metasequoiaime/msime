//! 86 五笔原表在 msime-dictionary 里的两种布局，以及 rime-wubi 那部分行的权重。构建的 Wubi 阶段、`wubi86-supplement`、`wubi98-supplement`、`wubixsj-supplement`、`wubi86-helpcode` 和 `word-vetoes` 的过期检查都经这里读 86 原表，看到的是同一组行、同一套权重。
//!
//! 新布局是两个文件：[`JIDIAN`] 是 KyleBing/rime-wubi86-jidian 的码表，`upstream.lock.json` 把它记在 [`JIDIAN_REFERENCE`] 下；[`RIME_WUBI`] 是 msime-dictionary #36 从 rime/rime-wubi 补进来、极点表没有的行，记在 [`RIME_WUBI_REFERENCE`] 下。两个文件首尾相接，就是拆分之前那个 `wubi86-jidian.txt` 的全部字节。旧布局只有那一个合并的文件，锁里也没有它。构建器只看 [`RIME_WUBI`] 在不在来决定按哪种布局读，所以拆分之前的提交（包括已经发布的 `dict-v*`）照常能构建。
//!
//! 极点表的权重是同码名次（10、20、30……；构建会写入的行里只有 蔻 `apfc`、个位数 `wwov` 两行是 5），rime-wubi 的权重是词频，从 0 到四亿多。原样并在一起，这些行会排到同码的极点首选前面（284 个编码），一码、二码的补全前列也会被它们占满。新布局下这些行按 [`tail`] 换算：一般记为 [`RIME_WUBI_WEIGHT`]，与极点表名次的最低一档相同，同权重按 rowid 排，所以它们排在同码的极点行之后，同码之间按上游词频的先后；又高于补充表占用的 0–9，所以仍排在生成的补充词之前。两边的权重不能直接比，比的是拼音库上游四张表（[`FREQUENCY_TABLES`]）里的词频（[`Frequencies`]）：rime-wubi 的行在那里的词频不低于 [`LIFT_MINIMUM`]、又是同码某几行极点行的 [`LIFT_RATIO`] 倍以上时，排到这几行之前，所以极点表没收的常用词（的话 `rqyt`）不会落到极点表的冷僻词、冷僻字（魅族、塏）之后。上游给 0 的部首、部件保持 0，排在最后；极点表已经收了的词组不再收 rime-wubi 给的另一个编码。最后，不含汉字的行（假名、符号、拉丁字母）按 [`demote_symbols`] 降到同码下一行的权重，同码次序不变，只是不再靠同码名次挤进一码、二码的前缀补全前列。旧布局分不出哪些行来自 rime-wubi，按原样读。
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::msime;
use crate::sources::{Sources, UPSTREAM_LOCK};
use crate::text;

/// 极点 86 码表；旧布局下也包含 rime-wubi 的行。
pub const JIDIAN: &str = "sources/wubi/wubi86-jidian.txt";
/// 新布局下 rime-wubi 补进来的行；它在不在决定按哪种布局读。
pub const RIME_WUBI: &str = "sources/wubi/wubi86-rime-wubi.txt";
/// [`JIDIAN`] 在 `upstream.lock.json` 与 `resources/dictionary-sources.lock.json` 里的上游名。
pub const JIDIAN_REFERENCE: &str = "rime-wubi86-jidian";
/// [`RIME_WUBI`] 的上游名。
pub const RIME_WUBI_REFERENCE: &str = "rime-wubi";

/// 新布局下 [`RIME_WUBI`] 上游权重不为 0 的行的权重：极点表名次的最低一档，高于补充表的上限 `wubi86_supplement::SUPPLEMENT_CEILING`。同码极点行有更低的权重时取那个权重，见 [`tail`]。
pub(crate) const RIME_WUBI_WEIGHT: i64 = 10;
// 编译期保证 rime-wubi 的行排在补充词之前。
const _: () = assert!(RIME_WUBI_WEIGHT > crate::wubi86_supplement::SUPPLEMENT_CEILING);

/// rime-wubi 的行与极点行比词频时共用的尺度：拼音库的上游表，单字表、rime-ice 主表与补充表、行政区划地名表。极点表的权重是同码名次，rime-wubi 的是另一份语料的词频，两边都要换到这一个尺度上才能比；拼音库一个词有几个读音时取最高的那个（[`Frequencies`]）。
pub const FREQUENCY_TABLES: [&str; 4] = [
    crate::reweight::SINGLE_CHARS,
    crate::wubi86_supplement::BASE,
    crate::wubi86_supplement::PINYIN_SUPPLEMENT,
    crate::wubi86_supplement::PLACES,
];
/// rime-wubi 的行要排到极点行之前，它在 [`FREQUENCY_TABLES`] 里的词频至少要这么高；更低的词频只是噪声，比倍数没有意义。
pub(crate) const LIFT_MINIMUM: i64 = 100;
/// rime-wubi 的行要排到一行极点行之前，词频要高过那一行的这么多倍。极点表的名次是人工排的，倍数小的差别（老师 `ftjg` 对 考量 只有 1.24 倍）不推翻它；直接比词频会改 172 个编码的首选，其中有 老师 → 考量 这类新的退步。
pub(crate) const LIFT_RATIO: i64 = 10;

/// 一次构建读的 86 原表文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Files {
    pub jidian: PathBuf,
    /// 新布局下的 [`RIME_WUBI`]；旧布局为 `None`。
    pub rime_wubi: Option<PathBuf>,
    /// 新布局下 [`FREQUENCY_TABLES`] 每张表的路径，按表的先后；旧布局不比词频，为空。
    pub frequency_tables: Vec<(&'static str, PathBuf)>,
}

impl Files {
    /// 在 checkout 里找 86 原表。有 [`RIME_WUBI`] 就是新布局，两个文件都按 checkout 的 `upstream.lock.json` 校验，还要读 [`FREQUENCY_TABLES`]；没有就是旧布局，[`JIDIAN`] 不经记录读取。记录已经列出 [`JIDIAN`]、却没有 [`RIME_WUBI`] 的 checkout 是拆分之后又丢了文件，报错而不是按旧布局读出一张少了 rime-wubi 行的表。
    pub fn locate(sources: &Sources) -> Result<Self> {
        if sources.optional(RIME_WUBI)?.is_some() {
            return Ok(Self {
                jidian: sources.pinned(JIDIAN)?,
                rime_wubi: Some(sources.pinned(RIME_WUBI)?),
                frequency_tables: FREQUENCY_TABLES
                    .into_iter()
                    .map(|path| Ok((path, sources.pinned(path)?)))
                    .collect::<Result<_>>()?,
            });
        }
        if sources.records(JIDIAN) {
            bail!("{JIDIAN} is listed in the checkout's {UPSTREAM_LOCK}, so the checkout has the split 86 Wubi layout, but {RIME_WUBI} is missing");
        }
        let jidian = sources
            .optional(JIDIAN)?
            .with_context(|| format!("{JIDIAN} is not in the dictionary checkout"))?;
        Ok(Self {
            jidian,
            rime_wubi: None,
            frequency_tables: Vec::new(),
        })
    }

    /// 读到的每个文件和它在 msime-dictionary 里的路径，按读的先后（新布局最后是 [`FREQUENCY_TABLES`]）；生成器表头按它记录输入的 SHA-256。
    pub fn inputs(&self) -> Vec<(&'static str, &Path)> {
        let mut inputs = vec![(JIDIAN, self.jidian.as_path())];
        inputs.extend(self.rime_wubi.as_deref().map(|path| (RIME_WUBI, path)));
        inputs.extend(
            self.frequency_tables
                .iter()
                .map(|(name, path)| (*name, path.as_path())),
        );
        inputs
    }

    /// `path` 是不是 [`Files::inputs`] 之一；生成器另列自己读的文件时跳过这些，表头里每个文件只记一次。
    pub fn reads(&self, path: &str) -> bool {
        self.inputs().iter().any(|(input, _)| *input == path)
    }

    /// 生成器表头「上游」一行要列出的 86 原表 reference：新布局是两个文件各自的上游，旧布局的合并文件不在锁里，没有。
    pub fn references(&self) -> &'static [&'static str] {
        match self.rime_wubi {
            Some(_) => &[JIDIAN_REFERENCE, RIME_WUBI_REFERENCE],
            None => &[],
        }
    }

    /// [`Files::inputs`] 每个文件的 SHA-256，按实际读到的字节计算。
    pub fn digests(&self) -> Result<Vec<(&'static str, String)>> {
        self.inputs()
            .into_iter()
            .map(|(path, file)| Ok((path, crate::sources::sha256_file(file)?)))
            .collect()
    }

    pub fn read(&self) -> Result<Texts> {
        let frequency_tables = self
            .frequency_tables
            .iter()
            .map(|(_, path)| text::read(path))
            .collect::<Result<Vec<_>>>()?;
        Ok(Texts {
            jidian: text::read(&self.jidian)?,
            rime_wubi: self.rime_wubi.as_deref().map(text::read).transpose()?,
            frequencies: Frequencies::parse(frequency_tables.iter().map(String::as_str)),
        })
    }
}

/// 86 原表的文本，和新布局下比词频用的 [`Frequencies`]（旧布局为空表）。
pub struct Texts {
    pub jidian: String,
    pub rime_wubi: Option<String>,
    pub frequencies: Frequencies,
}

impl Texts {
    pub fn rows(&self) -> impl Iterator<Item = Option<(String, &str, i64)>> {
        rows(&self.jidian, self.rime_wubi.as_deref(), &self.frequencies)
    }
}

/// [`FREQUENCY_TABLES`] 里每个词的词频：同一个词所有读音、所有表里最高的权重。没收的词按 0 算。
#[derive(Debug, Default)]
pub struct Frequencies(HashMap<String, i64>);

impl Frequencies {
    /// `sources` 是 `词<TAB>拼音<TAB>权重` 格式的表的文本，`#` 开头的是注释。
    pub(crate) fn parse<'a>(sources: impl IntoIterator<Item = &'a str>) -> Self {
        let mut words: HashMap<String, i64> = HashMap::new();
        for (value, _, weight) in sources
            .into_iter()
            .flat_map(crate::places_supplement::weighted_rows)
        {
            let slot = words.entry(value.to_owned()).or_insert(weight);
            *slot = (*slot).max(weight);
        }
        Self(words)
    }

    pub(crate) fn get(&self, word: &str) -> i64 {
        self.0.get(word).copied().unwrap_or(0)
    }
}

/// 86 原表的行 `(code, value, weight)`，`None` 是空行、注释或无效行：先是 `jidian` 的行；再是 `rime_wubi` 的行，按 [`tail`] 的规则换算、排序和取舍；有 `rime_wubi` 时最后按 [`demote_symbols`] 调低不含汉字的行。旧布局（`rime_wubi` 为 `None`）原样给出极点表的行，`frequencies` 不用。
pub(crate) fn rows<'a>(
    jidian: &'a str,
    rime_wubi: Option<&'a str>,
    frequencies: &Frequencies,
) -> impl Iterator<Item = Option<(String, &'a str, i64)>> + 'a {
    let mut rows: Vec<_> = parse(jidian).collect();
    if let Some(source) = rime_wubi {
        let tail = tail(&rows, source, frequencies);
        rows.extend(tail);
        demote_symbols(&mut rows);
    }
    rows.into_iter()
}

/// 原表的一行 `(code, value, weight)`；`None` 是空行、注释或无效行。
type Row<'a> = Option<(String, &'a str, i64)>;

/// 构建会写入、有名次的行：权重大于 0，也不是 [`msime::unsupported_wubi_value`] 筛掉的字。
fn ranked(value: &str, weight: i64) -> bool {
    weight > 0 && !msime::unsupported_wubi_value(value)
}

/// rime-wubi 的行，换算之后接在极点表 `head` 之后：
///
/// - 极点表已经有的多字词不收，不论编码。词组的编码由规则推出，一个词只有一个；极点表收了这个词，rime-wubi 再给的另一个编码就是错码。实际是 2 行：版权所有 `bqsy`（拼音首字母，极点表是 `tsrd`）和 T恤 `ttnt`（极点表是 `tntl`）。单字不在此列，rime-wubi 的单字行多是极点表没有的简码，例如 咪 `ko`、式 `aad`。
/// - 上游权重为 0 的行保持 0。rime-wubi 只给 12 个部首、部件（⺻ `vhgd`、龵 `rtgt` 等）0 权重，它们本该排在最后：补充表的同码上限不算它们（`wubi86_supplement::build`），构建把它们写在补充表之后（`msime::build_wubi`）。
/// - 其余的行一般记为 [`RIME_WUBI_WEIGHT`]；同码极点表构建会写入、有名次的行里有更低的权重（目前只有 蔻 `apfc`、个位数 `wwov` 是 5，这两个编码上没有 rime-wubi 的行），就取那个更低的权重，同权重按 rowid 排在极点行之后。
/// - 例外是按 [`lifted`] 排到同码几行极点行之前的行：它在 [`Frequencies`] 里的词频不低于 [`LIFT_MINIMUM`]，又是同码排在最后的那几行极点行各自词频的 [`LIFT_RATIO`] 倍以上，就取这几行里第一行的权重加 1。
/// - 同一编码的行按上游权重从高到低稳定排序，换算成同一权重之后，rowid 的先后仍是上游词频的先后。文件里大体已是这个次序，只有 `qtwb`（犹他 626000 在 狗仔 5680000 之前）和 `zzxx`（θ 454 在 ι 463 之前）两处不是。
fn tail<'a>(head: &[Row<'a>], source: &'a str, frequencies: &Frequencies) -> Vec<Row<'a>> {
    let mut words = HashSet::new();
    // 每个编码上构建会写入、有名次的极点行 `(权重, 词)`，按引擎的次序（权重从高到低，同权重按 rowid）。
    let mut jidian: HashMap<&str, Vec<(i64, &str)>> = HashMap::new();
    for (code, value, weight) in head.iter().flatten() {
        if value.chars().nth(1).is_some() {
            words.insert(*value);
        }
        if ranked(value, *weight) {
            jidian
                .entry(code.as_str())
                .or_default()
                .push((*weight, *value));
        }
    }
    for rows in jidian.values_mut() {
        rows.sort_by_key(|(weight, _)| std::cmp::Reverse(*weight));
    }
    let mut first_seen: HashMap<String, usize> = HashMap::new();
    // （编码首次出现的次序，上游权重，行）
    let mut rows: Vec<(usize, i64, Row)> = Vec::new();
    for row in parse(source) {
        let Some((code, value, upstream)) = row else {
            rows.push((usize::MAX, 0, None));
            continue;
        };
        if value.chars().nth(1).is_some() && words.contains(value) {
            continue;
        }
        let same_code = jidian.get(code.as_str()).map_or(&[][..], Vec::as_slice);
        let weight = match upstream {
            0 => 0,
            _ => lifted(same_code, frequencies.get(value), frequencies).unwrap_or_else(|| {
                same_code.last().map_or(RIME_WUBI_WEIGHT, |&(lowest, _)| {
                    lowest.min(RIME_WUBI_WEIGHT)
                })
            }),
        };
        let next = first_seen.len();
        let group = *first_seen.entry(code.clone()).or_insert(next);
        rows.push((group, upstream, Some((code, value, weight))));
    }
    rows.sort_by_key(|(group, upstream, _)| (*group, std::cmp::Reverse(*upstream)));
    rows.into_iter().map(|(_, _, row)| row).collect()
}

/// 词频为 `frequency` 的 rime-wubi 行在同码极点行 `jidian`（按引擎的次序）之前该取的权重；不该排到任何极点行之前时为 `None`。
///
/// 它只越过排在最后、连续的那几行：这几行每一行的词频乘 [`LIFT_RATIO`] 都低于 `frequency`，再往前一行就不是了。越不过的行一行也不越，所以极点表的人工次序只在差距悬殊时让位，越过的那几行之间也保持原来的先后。权重取越过的第一行的权重加 1：极点表同码名次相差 10，这个权重落在它和前一行之间；前一行与它同权重时，权重分不出它们，就少越过这一行。
pub(crate) fn lifted(
    jidian: &[(i64, &str)],
    frequency: i64,
    frequencies: &Frequencies,
) -> Option<i64> {
    if frequency < LIFT_MINIMUM {
        return None;
    }
    let mut start = jidian.len();
    while start > 0
        && frequency
            > frequencies
                .get(jidian[start - 1].1)
                .saturating_mul(LIFT_RATIO)
    {
        start -= 1;
    }
    while start > 0 && start < jidian.len() && jidian[start - 1].0 == jidian[start].0 {
        start += 1;
    }
    jidian.get(start).map(|&(weight, _)| weight + 1)
}

/// 不含汉字的行（假名、注音、希腊字母、符号、拉丁字母）降到同码下一行的权重，同码次序不变。极点表的权重是同码名次，候选越多的编码权重越高，`cojp` 上 90 个片假名从 890 排起；引擎的前缀补全跨编码按权重排（`QUERY_SQL_86`），于是打一个 `c` 时补全首屏是 ァ ア ィ，`u` 的首屏有 ♩。尾段的行按 [`tail`] 降下来以后，这些名次就露了出来。
///
/// 从每个编码的最后一行往前看：不含汉字的行取「刚好仍排在下一行之前」的权重（下一行的权重，自己的 rowid 在后时再加 1），不高于原来的权重；同码最后一行不含汉字时取它和 [`RIME_WUBI_WEIGHT`] 中较低的那个。这样同码的候选次序一行不变，假名组整组落到最低一档、按 rowid 保持原来的先后，含汉字的行权重不动。只算构建会写入、有名次的行（[`ranked`]）；0 权重的部首、部件写在补充表之后，不参与。
fn demote_symbols(rows: &mut [Row]) {
    let mut codes: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        if let Some((code, value, weight)) = row {
            if ranked(value, *weight) {
                codes.entry(code.clone()).or_default().push(index);
            }
        }
    }
    let weight_of = |rows: &[Row], index: usize| rows[index].as_ref().map_or(0, |row| row.2);
    for mut indices in codes.into_values() {
        indices.sort_by_key(|&index| (std::cmp::Reverse(weight_of(rows, index)), index));
        let mut below: Option<(i64, usize)> = None;
        for &index in indices.iter().rev() {
            let Some((_, value, weight)) = rows[index].as_mut() else {
                continue;
            };
            if !value.chars().any(crate::places_supplement::is_han) {
                let floor = match below {
                    Some((next, after)) if index < after => next,
                    Some((next, _)) => next + 1,
                    None => RIME_WUBI_WEIGHT,
                };
                *weight = (*weight).min(floor);
            }
            below = Some((*weight, index));
        }
    }
}

/// 生成器表头里称呼 86 原表的写法：`inputs` 是表头记录的输入路径，有 [`RIME_WUBI`] 时连同它的权重规则、比词频用的 [`FREQUENCY_TABLES`] 一起写，旧布局只写 [`JIDIAN`]，与拆分之前生成的表头一字不差。
pub(crate) fn described(inputs: &[(&str, &str)]) -> String {
    if inputs.iter().any(|(path, _)| *path == RIME_WUBI) {
        let tables = FREQUENCY_TABLES.join("、");
        format!("{JIDIAN} 与 {RIME_WUBI}（后者的行按权重 {RIME_WUBI_WEIGHT} 算、排在同码极点行之后，但在 {tables} 里的最高词频不低于 {LIFT_MINIMUM}、又是同码最后几行极点行词频 {LIFT_RATIO} 倍以上的排到这几行之前；上游权重为 0 的保持 0，{JIDIAN} 已有的词组不收；不含汉字的行降到同码下一行的权重，同码次序不变）")
    } else {
        JIDIAN.to_owned()
    }
}

fn parse(source: &str) -> impl Iterator<Item = Option<(String, &str, i64)>> {
    text::universal_lines(text::without_bom(source))
        .into_iter()
        .map(|line| msime::parse_code_line(line, false))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::sources::{Dictionary, Lock, Reference};

    const JIDIAN_COMMIT: &str = "28f19e3affdabb37d11d38f457c7a08a4694b226";
    const RIME_WUBI_COMMIT: &str = "152a0d3f3efe40cae216d1e3b338242446848d07";
    const RIME_ICE_COMMIT: &str = "3aea6d3694fb3d94ec663641f021f788822897ad";
    /// 新布局 checkout 里的 [`FREQUENCY_TABLES`]：rime-ice 主表与补充表是上游数据，记在 `upstream.lock.json` 里；单字表和地名表不是。
    const FREQUENCY_FILES: [(&str, Option<&str>, &str); 4] = [
        (crate::reweight::SINGLE_CHARS, None, "咪\tmi\t5000\n"),
        (
            crate::wubi86_supplement::BASE,
            Some("rime-ice"),
            "咪咪\tmi'mi\t300\n",
        ),
        (
            crate::wubi86_supplement::PINYIN_SUPPLEMENT,
            Some("rime-ice-supplement"),
            "# empty\n",
        ),
        (crate::wubi86_supplement::PLACES, None, "# empty\n"),
    ];

    /// rime-wubi 有词频的行换成 10：排在同码极点行之后（同权重按 rowid），又高于补充表的 0–9；上游的 0 保持 0；极点表自己的权重不动，无效行照样是 `None`。
    #[test]
    fn rime_wubi_rows_take_the_lowest_ranked_jidian_weight() {
        let parsed: Vec<_> = rows(
            "\u{feff}工\ta\t20\n戈\ta\t10\n# c\n",
            Some("咪\tko\t61900000\r\n版权所有\tbqsy\t444000000\r\n龴\tcny\t0\r\nx\t1a\t1\r\n"),
            &Frequencies::default(),
        )
        .collect();
        assert_eq!(
            parsed,
            [
                Some(("a".into(), "工", 20)),
                Some(("a".into(), "戈", 10)),
                None,
                Some(("ko".into(), "咪", RIME_WUBI_WEIGHT)),
                Some(("bqsy".into(), "版权所有", RIME_WUBI_WEIGHT)),
                Some(("cny".into(), "龴", 0)),
                None,
            ]
        );
        // 旧布局原样读：合并文件里 rime-wubi 的行保留原来的权重。
        let merged: Vec<_> = rows(
            "工\ta\t20\n咪\tko\t61900000\r\n",
            None,
            &Frequencies::parse(["咪\tmi\t5000\n"]),
        )
        .collect();
        assert_eq!(merged[1], Some(("ko".into(), "咪", 61900000)));
    }

    /// 极点表已有的词组，rime-wubi 给的另一个编码不收（版权所有 bqsy、T恤 ttnt），单字的简码照收（咪 ko）；同码极点行有名次的最低权重低于 10 时取它（apfc 上的 5），扩展区字和 0 权重的行不算；同一编码按上游权重从高到低排（qtwb 上 狗仔 在 犹他 之前），不同编码之间保持文件里的先后。
    #[test]
    fn rime_wubi_rows_yield_to_the_jidian_table() {
        let jidian = "版权所有\ttsrd\t10\nT恤\ttntl\t10\n咪\tkoy\t10\n蔻\tapfc\t5\n\u{3400}\tqtwb\t0\n犭\tqtwb\t0\n";
        let rime_wubi = "版权所有\tbqsy\t444000000\r\nT恤\tttnt\t30300000\r\n咪\tko\t61900000\r\n犹他\tqtwb\t626000\r\n蔻丹\tapfc\t1000\r\n狗仔\tqtwb\t5680000\r\n";
        let tail: Vec<_> = rows(jidian, Some(rime_wubi), &Frequencies::default())
            .skip(6)
            .collect();
        assert_eq!(
            tail,
            [
                Some(("ko".into(), "咪", RIME_WUBI_WEIGHT)),
                Some(("qtwb".into(), "狗仔", RIME_WUBI_WEIGHT)),
                Some(("qtwb".into(), "犹他", RIME_WUBI_WEIGHT)),
                Some(("apfc".into(), "蔻丹", 5)),
            ]
        );
    }

    /// 在拼音库的词频上差距悬殊时，rime-wubi 的行越过同码排在最后的那几行极点行：的话 是 魅族 的一百多倍，排到首位（魅族 的权重 20 加 1）；式 只是 葚 的 6.6 倍，只越过 匞（权重 10 加 1）；考量 只是 老师 的 1.24 倍，一行也不越；词频不到 100 的不越，哪怕极点行没有词频；上游 0 权重的不越。
    #[test]
    fn rime_wubi_rows_pass_jidian_rows_far_rarer_in_the_pinyin_tables() {
        let frequencies = Frequencies::parse([
            "的话\tde'hua\t6317143\n魅族\tmei'zu\t47398\n鬼话\tgui'hua\t30000\n",
            "式\tshi\t66000\n葚\tshen\t10000\n葚\tren\t1\n考量\tkao'liang\t124\n老师\tlao'shi\t100\n教师\tjiao'shi\t100\n冷字\tleng'zi\t99\n部首\tbu'shou\t100000\n",
        ]);
        let jidian = "魅族\trqyt\t20\n鬼话\trqyt\t10\n葚\taad\t20\n匞\taad\t10\n老师\tftjg\t20\n教师\tftjg\t10\n甲\tabcd\t10\n";
        let rime_wubi = "的话\trqyt\t197000000\r\n式\taad\t5000000\r\n考量\tftjg\t300000\r\n冷字\tabcd\t9000000\r\n部首\tabcd\t0\r\n";
        let tail: Vec<_> = rows(jidian, Some(rime_wubi), &frequencies)
            .skip(7)
            .collect();
        assert_eq!(
            tail,
            [
                Some(("rqyt".into(), "的话", 21)),
                Some(("aad".into(), "式", 11)),
                Some(("ftjg".into(), "考量", RIME_WUBI_WEIGHT)),
                Some(("abcd".into(), "冷字", RIME_WUBI_WEIGHT)),
                Some(("abcd".into(), "部首", 0)),
            ]
        );
    }

    /// 越过的几行之前那一行与它们同权重时，权重分不出两行，就少越过一行；越过的位置看的是引擎的次序，不是文件里的先后。
    #[test]
    fn a_lifted_row_stops_at_a_tie() {
        let frequencies = Frequencies::parse(["常用\tchang'yong\t100000\n甲\tjia\t100000\n"]);
        let lift = |jidian: &[(i64, &str)]| lifted(jidian, 100000, &frequencies);
        assert_eq!(lift(&[(30, "甲"), (20, "乙"), (10, "丙")]), Some(21));
        assert_eq!(lift(&[(30, "乙"), (20, "丙")]), Some(31));
        assert_eq!(lift(&[(20, "甲"), (20, "乙"), (10, "丙")]), Some(11));
        assert_eq!(lift(&[(10, "甲"), (10, "乙")]), None);
        assert_eq!(lift(&[]), None);
        let tail: Vec<_> = rows(
            "乙\tabcd\t10\n甲\tabcd\t30\n",
            Some("常用\tabcd\t1\r\n"),
            &frequencies,
        )
        .skip(2)
        .collect();
        assert_eq!(tail, [Some(("abcd".into(), "常用", 11))]);
    }

    /// 不含汉字的行降到同码下一行的权重：片假名整组从 30、20、10 落到 10，按 rowid 保持原来的先后；夹在汉字行之间的符号（♩ 在 音符 与 童稚 之间）只降到下一行的权重，同码次序不变；首位的符号（Apple）也一样；含汉字的行（T恤）不算符号；0 权重的部首和旧布局都不动。
    #[test]
    fn symbol_rows_drop_to_the_next_row_of_their_code() {
        let jidian = "ァ\tcojp\t30\nア\tcojp\t20\nィ\tcojp\t10\n音符\tujtw\t50\n♩\tujtw\t40\n♪\tujtw\t30\n童稚\tujtw\t10\nApple\tappl\t20\n劳军\tappl\t10\nT恤\ttntl\t20\n甲\ttntl\t10\n";
        let rime_wubi = "θ\tzzxx\t454\r\n⺌\tihyt\t0\r\n";
        let ranked: Vec<_> = rows(jidian, Some(rime_wubi), &Frequencies::default())
            .flatten()
            .collect();
        let weights: Vec<i64> = ranked.iter().map(|row| row.2).collect();
        assert_eq!(weights, [10, 10, 10, 50, 10, 10, 10, 10, 10, 20, 10, 10, 0]);
        // 同码按引擎的次序（权重从高到低、同权重按 rowid）列出，与降权之前一样。
        for code in ["cojp", "ujtw", "appl", "tntl"] {
            let mut order: Vec<_> = ranked
                .iter()
                .enumerate()
                .filter(|(_, row)| row.0 == code)
                .collect();
            let before: Vec<&str> = order.iter().map(|(_, row)| row.1).collect();
            order.sort_by_key(|(index, row)| (std::cmp::Reverse(row.2), *index));
            let after: Vec<&str> = order.iter().map(|(_, row)| row.1).collect();
            assert_eq!(before, after, "{code}");
        }
        let merged: Vec<i64> = rows(jidian, None, &Frequencies::default())
            .flatten()
            .map(|row| row.2)
            .collect();
        assert_eq!(merged, [30, 20, 10, 50, 40, 30, 10, 20, 10, 20, 10]);
    }

    fn lock() -> Lock {
        let reference = |repository: &str, commit: &str| Reference {
            repository: repository.to_owned(),
            commit: commit.to_owned(),
        };
        Lock {
            references: BTreeMap::from([
                (
                    JIDIAN_REFERENCE.to_owned(),
                    reference(
                        "https://github.com/KyleBing/rime-wubi86-jidian.git",
                        JIDIAN_COMMIT,
                    ),
                ),
                (
                    RIME_WUBI_REFERENCE.to_owned(),
                    reference("https://github.com/rime/rime-wubi.git", RIME_WUBI_COMMIT),
                ),
                (
                    "rime-ice".to_owned(),
                    reference("https://github.com/iDvel/rime-ice.git", RIME_ICE_COMMIT),
                ),
                (
                    "rime-ice-supplement".to_owned(),
                    reference("https://github.com/iDvel/rime-ice.git", RIME_ICE_COMMIT),
                ),
            ]),
            mozc: reference("https://github.com/google/mozc.git", &"b".repeat(40)),
            files: Vec::new(),
        }
    }

    fn checkout(files: &[(&str, &[u8])], record: serde_json::Value) -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        for (path, content) in files {
            let target = directory.path().join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, content).unwrap();
        }
        std::fs::write(
            directory.path().join(UPSTREAM_LOCK),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        directory
    }

    fn sources_at(checkout: &tempfile::TempDir) -> Sources {
        let lock = lock();
        Sources {
            dictionary: Some(Dictionary::open(checkout.path().into(), &lock).unwrap()),
            lock,
            repository_inputs: PathBuf::new(),
            cache: PathBuf::new(),
            offline: true,
        }
    }

    fn entry(path: &str, upstream: &str, content: &[u8]) -> serde_json::Value {
        json!({"path": path, "upstream": upstream, "size": content.len(), "sha256": hex::encode(Sha256::digest(content))})
    }

    fn split_record(jidian: &[u8], rime_wubi: &[u8]) -> serde_json::Value {
        let mut files = vec![
            entry(JIDIAN, JIDIAN_REFERENCE, jidian),
            entry(RIME_WUBI, RIME_WUBI_REFERENCE, rime_wubi),
        ];
        for (path, upstream, content) in FREQUENCY_FILES {
            if let Some(upstream) = upstream {
                files.push(entry(path, upstream, content.as_bytes()));
            }
        }
        json!({
            "version": 1,
            "upstreams": {
                JIDIAN_REFERENCE: {"repository": "https://github.com/KyleBing/rime-wubi86-jidian.git", "commit": JIDIAN_COMMIT},
                RIME_WUBI_REFERENCE: {"repository": "https://github.com/rime/rime-wubi.git", "commit": RIME_WUBI_COMMIT},
                "rime-ice": {"repository": "https://github.com/iDvel/rime-ice.git", "commit": RIME_ICE_COMMIT},
                "rime-ice-supplement": {"repository": "https://github.com/iDvel/rime-ice.git", "commit": RIME_ICE_COMMIT},
            },
            "files": files,
        })
    }

    /// 新布局 checkout 的文件：两个 86 文件加 [`FREQUENCY_FILES`]。
    fn split_files<'a>(jidian: &'a [u8], rime_wubi: &'a [u8]) -> Vec<(&'static str, &'a [u8])> {
        let mut files = vec![(JIDIAN, jidian), (RIME_WUBI, rime_wubi)];
        files.extend(
            FREQUENCY_FILES
                .iter()
                .map(|(path, _, content)| (*path, content.as_bytes())),
        );
        files
    }

    /// 两种布局按 [`RIME_WUBI`] 在不在切换：拆分前的 checkout 锁里没有极点表，照常按合并文件读；拆分后两个文件都按记录校验，字节不符、或者记录列了极点表却缺了 rime-wubi 文件都报错。
    #[test]
    fn the_layout_follows_the_rime_wubi_file() {
        let jidian: &[u8] = "工\ta\t20\n".as_bytes();
        let rime_wubi: &[u8] = "咪\tko\t61900000\r\n".as_bytes();

        let merged = [jidian, rime_wubi].concat();
        let before = checkout(
            &[(JIDIAN, &merged)],
            json!({"version": 1, "upstreams": {}, "files": []}),
        );
        let files = Files::locate(&sources_at(&before)).unwrap();
        assert_eq!(
            files,
            Files {
                jidian: before.path().join(JIDIAN),
                rime_wubi: None,
                frequency_tables: Vec::new(),
            }
        );
        assert_eq!(
            files.inputs(),
            [(JIDIAN, before.path().join(JIDIAN).as_path())]
        );
        let texts = files.read().unwrap();
        assert_eq!(
            texts.rows().flatten().map(|row| row.2).collect::<Vec<_>>(),
            [20, 61900000]
        );

        let split = checkout(
            &split_files(jidian, rime_wubi),
            split_record(jidian, rime_wubi),
        );
        let files = Files::locate(&sources_at(&split)).unwrap();
        assert_eq!(files.rime_wubi, Some(split.path().join(RIME_WUBI)));
        assert_eq!(
            files
                .inputs()
                .iter()
                .map(|(path, _)| *path)
                .collect::<Vec<_>>(),
            [&[JIDIAN, RIME_WUBI][..], &FREQUENCY_TABLES].concat()
        );
        let texts = files.read().unwrap();
        assert_eq!(
            texts.rows().flatten().map(|row| row.2).collect::<Vec<_>>(),
            [20, RIME_WUBI_WEIGHT]
        );

        let edited = checkout(
            &split_files(jidian, "咪\tko\t1\r\n".as_bytes()),
            split_record(jidian, rime_wubi),
        );
        let error = format!("{:#}", Files::locate(&sources_at(&edited)).unwrap_err());
        assert!(error.contains(RIME_WUBI_REFERENCE), "{error}");

        // 新布局要比词频，比词频用的表缺了一张也报错，不当作没有词频。
        let mut files = split_files(jidian, rime_wubi);
        files.retain(|(path, _)| *path != crate::reweight::SINGLE_CHARS);
        let without_single_chars = checkout(&files, split_record(jidian, rime_wubi));
        let error = format!(
            "{:#}",
            Files::locate(&sources_at(&without_single_chars)).unwrap_err()
        );
        assert!(error.contains(crate::reweight::SINGLE_CHARS), "{error}");

        let lost = checkout(&[(JIDIAN, jidian)], split_record(jidian, rime_wubi));
        let error = format!("{:#}", Files::locate(&sources_at(&lost)).unwrap_err());
        assert!(
            error.contains(RIME_WUBI) && error.contains("missing"),
            "{error}"
        );
    }
}
