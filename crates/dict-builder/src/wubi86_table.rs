//! 86 五笔原表在 msime-dictionary 里的两种布局，以及 rime-wubi 那部分行的权重。构建的 Wubi 阶段、`wubi86-supplement`、`wubi98-supplement`、`wubixsj-supplement`、`wubi86-helpcode` 和 `word-vetoes` 的过期检查都经这里读 86 原表，看到的是同一组行、同一套权重。
//!
//! 新布局是两个文件：[`JIDIAN`] 是 KyleBing/rime-wubi86-jidian 的码表，`upstream.lock.json` 把它记在 [`JIDIAN_REFERENCE`] 下；[`RIME_WUBI`] 是 msime-dictionary #36 从 rime/rime-wubi 补进来、极点表没有的行，记在 [`RIME_WUBI_REFERENCE`] 下。两个文件首尾相接，就是拆分之前那个 `wubi86-jidian.txt` 的全部字节。旧布局只有那一个合并的文件，锁里也没有它。构建器只看 [`RIME_WUBI`] 在不在来决定按哪种布局读，所以拆分之前的提交（包括已经发布的 `dict-v*`）照常能构建。
//!
//! 极点表的权重是同码名次（10、20、30……，有名次的行最低是 10），rime-wubi 的权重是词频，从几百到四亿多。原样并在一起，这些行会排到同码的极点首选前面（284 个编码），一码、二码的补全前列也会被它们占满。新布局下这些行一律记为 [`RIME_WUBI_WEIGHT`]：和极点表有名次的行的最低一档相同，同权重按 rowid 排，所以它们排在同码的极点行之后，彼此之间保持文件里的先后（也就是 rime-wubi 权重从高到低的次序）；又高于补充表占用的 0–9，所以仍排在生成的补充词之前。旧布局分不出哪些行来自 rime-wubi，按原样读。

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

/// 新布局下 [`RIME_WUBI`] 每一行的权重：极点表有名次的行的最低权重，高于补充表的上限 `wubi86_supplement::SUPPLEMENT_CEILING`。
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

/// 86 原表的行 `(code, value, weight)`，`None` 是空行、注释或无效行：先是 `jidian` 的行，原样；再是 `rime_wubi` 的行，权重换成 [`RIME_WUBI_WEIGHT`]。
pub(crate) fn rows<'a>(
    jidian: &'a str,
    rime_wubi: Option<&'a str>,
) -> impl Iterator<Item = Option<(String, &'a str, i64)>> + 'a {
    parse(jidian).chain(rime_wubi.into_iter().flat_map(|source| {
        parse(source).map(|row| row.map(|(code, value, _)| (code, value, RIME_WUBI_WEIGHT)))
    }))
}

/// 生成器表头里称呼 86 原表的写法：`inputs` 是表头记录的输入路径，有 [`RIME_WUBI`] 时连同它和它的权重一起写，旧布局只写 [`JIDIAN`]，与拆分之前生成的表头一字不差。
pub(crate) fn described(inputs: &[(&str, &str)]) -> String {
    if inputs.iter().any(|(path, _)| *path == RIME_WUBI) {
        format!("{JIDIAN} 与 {RIME_WUBI}（后者的行一律按权重 {RIME_WUBI_WEIGHT} 算）")
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

    /// rime-wubi 的行一律换成 10：排在同码极点行之后（同权重按 rowid），又高于补充表的 0–9；极点表自己的权重不动，无效行照样是 `None`。
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
                Some(("cny".into(), "龴", RIME_WUBI_WEIGHT)),
                None,
            ]
        );
        // 旧布局原样读：合并文件里 rime-wubi 的行保留原来的权重。
        let merged: Vec<_> = rows("工\ta\t20\n咪\tko\t61900000\r\n", None).collect();
        assert_eq!(merged[1], Some(("ko".into(), "咪", 61900000)));
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
