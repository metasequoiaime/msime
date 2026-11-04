//! 86 五笔原表在 msime-dictionary 里的两种布局，以及 rime-wubi 那部分行的权重。构建的 Wubi 阶段、`wubi86-supplement`、`wubi98-supplement`、`wubixsj-supplement`、`wubi86-helpcode` 和 `word-vetoes` 的过期检查都经这里读 86 原表，看到的是同一组行、同一套权重。
//!
//! 新布局是两个文件：[`JIDIAN`] 是 KyleBing/rime-wubi86-jidian 的码表，`upstream.lock.json` 把它记在 [`JIDIAN_REFERENCE`] 下；[`RIME_WUBI`] 是 msime-dictionary #36 从 rime/rime-wubi 补进来、极点表没有的行，记在 [`RIME_WUBI_REFERENCE`] 下。两个文件首尾相接，就是拆分之前那个 `wubi86-jidian.txt` 的全部字节。旧布局只有那一个合并的文件，锁里也没有它。构建器只看 [`RIME_WUBI`] 在不在来决定按哪种布局读，所以拆分之前的提交（包括已经发布的 `dict-v*`）照常能构建。
//!
//! 极点表的权重是同码名次（10、20、30……；构建会写入的行里只有 蔻 `apfc`、个位数 `wwov` 两行是 5），rime-wubi 的权重是词频，从 0 到四亿多。原样并在一起，这些行会排到同码的极点首选前面（284 个编码），一码、二码的补全前列也会被它们占满。新布局下这些行按 [`tail`] 换算：一般记为 [`RIME_WUBI_WEIGHT`]，与极点表名次的最低一档相同，同权重按 rowid 排，所以它们排在同码的极点行之后，同码之间按上游词频的先后；又高于补充表占用的 0–9，所以仍排在生成的补充词之前。上游给 0 的部首、部件保持 0，排在最后；极点表已经收了的词组不再收 rime-wubi 给的另一个编码。旧布局分不出哪些行来自 rime-wubi，按原样读。

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

/// 一次构建读的 86 原表文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Files {
    pub jidian: PathBuf,
    /// 新布局下的 [`RIME_WUBI`]；旧布局为 `None`。
    pub rime_wubi: Option<PathBuf>,
}

impl Files {
    /// 在 checkout 里找 86 原表。有 [`RIME_WUBI`] 就是新布局，两个文件都按 checkout 的 `upstream.lock.json` 校验；没有就是旧布局，[`JIDIAN`] 不经记录读取。记录已经列出 [`JIDIAN`]、却没有 [`RIME_WUBI`] 的 checkout 是拆分之后又丢了文件，报错而不是按旧布局读出一张少了 rime-wubi 行的表。
    pub fn locate(sources: &Sources) -> Result<Self> {
        if sources.optional(RIME_WUBI)?.is_some() {
            return Ok(Self {
                jidian: sources.pinned(JIDIAN)?,
                rime_wubi: Some(sources.pinned(RIME_WUBI)?),
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
        })
    }

    /// 读到的每个文件和它在 msime-dictionary 里的路径，按读的先后；生成器表头按它记录输入的 SHA-256。
    pub fn inputs(&self) -> Vec<(&'static str, &Path)> {
        let mut inputs = vec![(JIDIAN, self.jidian.as_path())];
        inputs.extend(self.rime_wubi.as_deref().map(|path| (RIME_WUBI, path)));
        inputs
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
        Ok(Texts {
            jidian: text::read(&self.jidian)?,
            rime_wubi: self.rime_wubi.as_deref().map(text::read).transpose()?,
        })
    }
}

/// 86 原表的文本。
pub struct Texts {
    pub jidian: String,
    pub rime_wubi: Option<String>,
}

impl Texts {
    pub fn rows(&self) -> impl Iterator<Item = Option<(String, &str, i64)>> {
        rows(&self.jidian, self.rime_wubi.as_deref())
    }
}

/// 86 原表的行 `(code, value, weight)`，`None` 是空行、注释或无效行：先是 `jidian` 的行，原样；再是 `rime_wubi` 的行，按 [`tail`] 的规则换算、排序和取舍。
pub(crate) fn rows<'a>(
    jidian: &'a str,
    rime_wubi: Option<&'a str>,
) -> impl Iterator<Item = Option<(String, &'a str, i64)>> + 'a {
    let head: Vec<_> = parse(jidian).collect();
    let tail = rime_wubi.map_or_else(Vec::new, |source| tail(&head, source));
    head.into_iter().chain(tail)
}

/// 原表的一行 `(code, value, weight)`；`None` 是空行、注释或无效行。
type Row<'a> = Option<(String, &'a str, i64)>;

/// rime-wubi 的行，换算之后接在极点表 `head` 之后：
///
/// - 极点表已经有的多字词不收，不论编码。词组的编码由规则推出，一个词只有一个；极点表收了这个词，rime-wubi 再给的另一个编码就是错码。实际是 2 行：版权所有 `bqsy`（拼音首字母，极点表是 `tsrd`）和 T恤 `ttnt`（极点表是 `tntl`）。单字不在此列，rime-wubi 的单字行多是极点表没有的简码，例如 咪 `ko`、式 `aad`。
/// - 上游权重为 0 的行保持 0。rime-wubi 只给 12 个部首、部件（⺻ `vhgd`、龵 `rtgt` 等）0 权重，它们本该排在最后：补充表的同码上限不算它们（`wubi86_supplement::build`），构建把它们写在补充表之后（`msime::build_wubi`）。
/// - 其余的行记为 [`RIME_WUBI_WEIGHT`]；同码极点表构建会写入、有名次的行里有更低的权重（目前只有 蔻 `apfc`、个位数 `wwov` 是 5，这两个编码上没有 rime-wubi 的行），就取那个更低的权重，同权重按 rowid 排在极点行之后。
/// - 同一编码的行按上游权重从高到低稳定排序，换算成同一权重之后，rowid 的先后仍是上游词频的先后。文件里大体已是这个次序，只有 `qtwb`（犹他 626000 在 狗仔 5680000 之前）和 `zzxx`（θ 454 在 ι 463 之前）两处不是。
fn tail<'a>(head: &[Row<'a>], source: &'a str) -> Vec<Row<'a>> {
    let mut words = HashSet::new();
    let mut lowest: HashMap<&str, i64> = HashMap::new();
    for (code, value, weight) in head.iter().flatten() {
        if value.chars().nth(1).is_some() {
            words.insert(*value);
        }
        if *weight > 0 && !msime::unsupported_wubi_value(value) {
            let slot = lowest.entry(code.as_str()).or_insert(*weight);
            *slot = (*slot).min(*weight);
        }
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
        let weight = match upstream {
            0 => 0,
            _ => lowest
                .get(code.as_str())
                .map_or(RIME_WUBI_WEIGHT, |&lowest| lowest.min(RIME_WUBI_WEIGHT)),
        };
        let next = first_seen.len();
        let group = *first_seen.entry(code.clone()).or_insert(next);
        rows.push((group, upstream, Some((code, value, weight))));
    }
    rows.sort_by_key(|(group, upstream, _)| (*group, std::cmp::Reverse(*upstream)));
    rows.into_iter().map(|(_, _, row)| row).collect()
}

/// 生成器表头里称呼 86 原表的写法：`inputs` 是表头记录的输入路径，有 [`RIME_WUBI`] 时连同它和它的权重一起写，旧布局只写 [`JIDIAN`]，与拆分之前生成的表头一字不差。
pub(crate) fn described(inputs: &[(&str, &str)]) -> String {
    if inputs.iter().any(|(path, _)| *path == RIME_WUBI) {
        format!("{JIDIAN} 与 {RIME_WUBI}（后者的行按权重 {RIME_WUBI_WEIGHT} 算，上游权重为 0 的保持 0，{JIDIAN} 已有的词组不收）")
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

    /// rime-wubi 有词频的行换成 10：排在同码极点行之后（同权重按 rowid），又高于补充表的 0–9；上游的 0 保持 0；极点表自己的权重不动，无效行照样是 `None`。
    #[test]
    fn rime_wubi_rows_take_the_lowest_ranked_jidian_weight() {
        let parsed: Vec<_> = rows(
            "\u{feff}工\ta\t20\n戈\ta\t10\n# c\n",
            Some("咪\tko\t61900000\r\n版权所有\tbqsy\t444000000\r\n龴\tcny\t0\r\nx\t1a\t1\r\n"),
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
        let merged: Vec<_> = rows("工\ta\t20\n咪\tko\t61900000\r\n", None).collect();
        assert_eq!(merged[1], Some(("ko".into(), "咪", 61900000)));
    }

    /// 极点表已有的词组，rime-wubi 给的另一个编码不收（版权所有 bqsy、T恤 ttnt），单字的简码照收（咪 ko）；同码极点行有名次的最低权重低于 10 时取它（apfc 上的 5），扩展区字和 0 权重的行不算；同一编码按上游权重从高到低排（qtwb 上 狗仔 在 犹他 之前），不同编码之间保持文件里的先后。
    #[test]
    fn rime_wubi_rows_yield_to_the_jidian_table() {
        let jidian = "版权所有\ttsrd\t10\nT恤\ttntl\t10\n咪\tkoy\t10\n蔻\tapfc\t5\n\u{3400}\tqtwb\t0\n犭\tqtwb\t0\n";
        let rime_wubi = "版权所有\tbqsy\t444000000\r\nT恤\tttnt\t30300000\r\n咪\tko\t61900000\r\n犹他\tqtwb\t626000\r\n蔻丹\tapfc\t1000\r\n狗仔\tqtwb\t5680000\r\n";
        let tail: Vec<_> = rows(jidian, Some(rime_wubi)).skip(6).collect();
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
        json!({
            "version": 1,
            "upstreams": {
                JIDIAN_REFERENCE: {"repository": "https://github.com/KyleBing/rime-wubi86-jidian.git", "commit": JIDIAN_COMMIT},
                RIME_WUBI_REFERENCE: {"repository": "https://github.com/rime/rime-wubi.git", "commit": RIME_WUBI_COMMIT},
            },
            "files": [entry(JIDIAN, JIDIAN_REFERENCE, jidian), entry(RIME_WUBI, RIME_WUBI_REFERENCE, rime_wubi)],
        })
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
                rime_wubi: None
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
            &[(JIDIAN, jidian), (RIME_WUBI, rime_wubi)],
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
            [JIDIAN, RIME_WUBI]
        );
        let texts = files.read().unwrap();
        assert_eq!(
            texts.rows().flatten().map(|row| row.2).collect::<Vec<_>>(),
            [20, RIME_WUBI_WEIGHT]
        );

        let edited = checkout(
            &[(JIDIAN, jidian), (RIME_WUBI, "咪\tko\t1\r\n".as_bytes())],
            split_record(jidian, rime_wubi),
        );
        let error = format!("{:#}", Files::locate(&sources_at(&edited)).unwrap_err());
        assert!(error.contains(RIME_WUBI_REFERENCE), "{error}");

        let lost = checkout(&[(JIDIAN, jidian)], split_record(jidian, rime_wubi));
        let error = format!("{:#}", Files::locate(&sources_at(&lost)).unwrap_err());
        assert!(
            error.contains(RIME_WUBI) && error.contains("missing"),
            "{error}"
        );
    }
}
