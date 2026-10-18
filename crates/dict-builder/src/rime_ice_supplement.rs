//! 从明确检出的 rime-ice 提交生成中英文补充表，供固定版本重建和上游更新预览。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use msime_engine::{
    validate_personal_dictionary_entry, PersonalDictionaryEntry, PersonalDictionaryKind,
};

use crate::{product, sources, text};

const CHINESE: &[&str] = &[
    "cn_dicts/base.dict.yaml",
    "cn_dicts/ext.dict.yaml",
    "cn_dicts/8105.dict.yaml",
    "cn_dicts/others.dict.yaml",
];
const ENGLISH: &[&str] = &["en_dicts/en.dict.yaml", "en_dicts/en_ext.dict.yaml"];
const COMPARE_CHINESE: &[&str] = &[
    "sources/pinyin/rime-ice.txt",
    "sources/pinyin/single-chars.txt",
    "sources/pinyin/places.txt",
    "custom/words.txt",
];
const COMPARE_ENGLISH: &[&str] = &[
    "sources/english/rime-ice-en.txt",
    "sources/english/scowl-words.txt",
    "custom/english.txt",
];

fn body(source: &str) -> Result<&str> {
    let marker = source
        .lines()
        .position(|line| line.trim() == "...")
        .context("Rime dictionary has no data marker")?;
    let offset = source
        .split_inclusive('\n')
        .take(marker + 1)
        .map(str::len)
        .sum::<usize>();
    Ok(&source[offset..])
}

#[derive(Default)]
struct Chinese {
    words: BTreeMap<(String, String), i64>,
    unsupported_readings: BTreeSet<(String, String)>,
    unannotated_words: BTreeSet<String>,
}

fn chinese(sources: &[String], compare: &[String]) -> Result<Chinese> {
    let existing: BTreeSet<_> = compare
        .iter()
        .flat_map(|source| source.lines())
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let word = fields.next()?;
            let reading = fields.next()?;
            Some((word.to_owned(), reading.to_owned()))
        })
        .collect();
    let mut result = Chinese::default();
    for source in sources {
        for line in body(source)?
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        {
            let fields: Vec<_> = line.trim_end_matches('\r').split('\t').collect();
            let [word, reading, rest @ ..] = fields.as_slice() else {
                result.unannotated_words.insert(line.to_owned());
                continue;
            };
            if !word.chars().all(crate::places_supplement::is_han) {
                continue;
            }
            let key = reading.split_whitespace().collect::<Vec<_>>().join("'");
            let weight: i64 = rest
                .first()
                .copied()
                .filter(|weight| !weight.is_empty())
                .unwrap_or("0")
                .parse()
                .context("Rime dictionary has an invalid weight")?;
            let entry = PersonalDictionaryEntry {
                kind: PersonalDictionaryKind::Pinyin,
                key,
                value: (*word).to_owned(),
                weight: weight.max(1),
            };
            if weight < 0 {
                bail!("Rime dictionary has a negative weight");
            }
            let Ok(entry) = validate_personal_dictionary_entry(&entry) else {
                result.unsupported_readings.insert((entry.value, entry.key));
                continue;
            };
            let identity = (entry.value, entry.key);
            if !existing.contains(&identity) {
                let slot = result.words.entry(identity).or_insert(weight);
                *slot = (*slot).max(weight);
            }
        }
    }
    Ok(result)
}

fn english(sources: &[String], compare: &[String]) -> Result<BTreeSet<String>> {
    let mut existing = BTreeSet::new();
    for source in compare {
        for line in source
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
        {
            if let Some(word) = line.split_whitespace().next() {
                existing.insert(word.to_ascii_lowercase());
            }
        }
    }
    let mut words = BTreeSet::new();
    for source in sources {
        for line in body(source)?
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
        {
            let word = line.split('\t').next().unwrap_or_default();
            if !word.is_empty()
                && word.len() <= 64
                && word.bytes().all(|c| c.is_ascii_alphabetic())
                && !existing.contains(&word.to_ascii_lowercase())
            {
                words.insert(word.to_owned());
            }
        }
    }
    Ok(words)
}

pub fn build(upstream: &Path, dictionary: &Path, out: &Path, repository: &Path) -> Result<()> {
    let git = |args: &[&str]| -> Result<String> {
        let result = Command::new("git")
            .arg("-C")
            .arg(upstream)
            .args(args)
            .output()?;
        if !result.status.success() {
            bail!("cannot inspect rime-ice checkout");
        }
        Ok(String::from_utf8(result.stdout)?.trim().to_owned())
    };
    let commit = git(&["rev-parse", "HEAD"])?;
    if commit.len() != 40
        || !commit.bytes().all(|c| c.is_ascii_hexdigit())
        || !git(&["status", "--porcelain", "--untracked-files=no"])?.is_empty()
    {
        bail!("rime-ice checkout must have a clean, fixed commit");
    }
    let load = |root: &Path, paths: &[&str]| {
        paths
            .iter()
            .map(|p| text::read(&root.join(p)))
            .collect::<Result<Vec<_>>>()
    };
    let chinese = chinese(
        &load(upstream, CHINESE)?,
        &load(dictionary, COMPARE_CHINESE)?,
    )?;
    let english = english(
        &load(upstream, ENGLISH)?,
        &load(dictionary, COMPARE_ENGLISH)?,
    )?;
    let generator = product::builder_commit(repository)?;
    let header = |inputs: &[&str], compare: &[&str]| -> Result<String> {
        let mut result = format!("# 由 msime 提交 {generator} 的 crates/dict-builder/src/rime_ice_supplement.rs 生成。\n# 上游：https://github.com/iDvel/rime-ice 提交 {commit}；许可 GPL-3.0。\n# 命令：msime-dict-build rime-ice-supplement --upstream <该提交的干净检出> --dictionary <msime-dictionary> --out <输出目录>。\n# 去重对照：{}；中文按同词同音排除、重复取最大权重，英文按小写词形排除、保留纯 ASCII 显示词。\n", compare.join("、"));
        for (root, paths) in [(upstream, inputs), (dictionary, compare)] {
            for path in paths {
                writeln!(
                    result,
                    "# 输入：{path} SHA-256 {}",
                    sources::sha256_file(&root.join(path))?
                )?;
            }
        }
        Ok(result)
    };
    let mut cn = header(CHINESE, COMPARE_CHINESE)?;
    for ((word, key), weight) in &chinese.words {
        writeln!(cn, "{word}\t{key}\t{weight}")?;
    }
    let mut rejected =
        String::from("# 引擎暂不支持或不符合逐字读音校验的上游词音；不进入候选补充表。\n");
    for (word, key) in &chinese.unsupported_readings {
        writeln!(rejected, "{word}\t{key}")?;
    }
    writeln!(
        cn,
        "# 排除的不支持词音组合：{}；详见 unsupported-readings.txt。",
        chinese.unsupported_readings.len()
    )?;
    let mut unannotated = String::from("# 上游未注音词条；不猜读音，不进入候选补充表。\n");
    for word in &chinese.unannotated_words {
        writeln!(unannotated, "{word}")?;
    }
    writeln!(
        cn,
        "# 排除的未注音词条：{}；详见 unannotated-words.txt。",
        chinese.unannotated_words.len()
    )?;
    let mut en = header(ENGLISH, COMPARE_ENGLISH)?;
    for word in &english {
        writeln!(en, "{word}\t{}", word.to_ascii_lowercase())?;
    }
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join("rime-ice-supplement.txt"), cn)?;
    std::fs::write(out.join("rime-ice-en-supplement.txt"), en)?;
    std::fs::write(out.join("unsupported-readings.txt"), rejected)?;
    std::fs::write(out.join("unannotated-words.txt"), unannotated)?;
    std::fs::copy(upstream.join("LICENSE"), out.join("rime-ice-LICENSE"))?;
    let report = serde_json::json!({"repository":"https://github.com/iDvel/rime-ice.git", "commit":commit, "generator_commit":generator, "chinese_entries":chinese.words.len(), "unsupported_readings":chinese.unsupported_readings.len(), "unannotated_words":chinese.unannotated_words.len(), "english_entries":english.len(), "excluded_unannotated_sources":["cn_dicts/tencent.dict.yaml"], "review_required":true});
    std::fs::write(
        out.join("upstream-update.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    eprintln!("[report] {} Chinese entries, {} English entries from {commit}; review paired locks and licences before publishing", chinese.words.len(), english.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_deduplicates_every_comparison_and_preserves_readings() {
        let rows = chinese(&["---\nname: fixture\n...\n甲乙\tjia yi\t10\n甲乙\tjia yi\t20\n乙甲\tyi jia\t30\n# 乙乙\tyi yi\t99\n".into()], &["乙甲\tyi'jia\t1\n".into()]).unwrap();
        assert_eq!(
            rows.words.into_iter().collect::<Vec<_>>(),
            vec![(("甲乙".into(), "jia'yi".into()), 20)]
        );
        let rejected = chinese(&["...\n甲乙\tjia\t1\n嗯\thm\t1\n甲乙\n".into()], &[]).unwrap();
        assert!(rejected.words.is_empty());
        assert_eq!(rejected.unsupported_readings.len(), 2);
        assert_eq!(
            rejected.unannotated_words.into_iter().collect::<Vec<_>>(),
            ["甲乙"]
        );
        assert!(chinese(&["...\n甲乙\tjia yi\t-1\n".into()], &[]).is_err());
        assert!(chinese(&["missing header".into()], &[]).is_err());
    }

    #[test]
    fn english_keeps_ascii_variants_without_existing_words() {
        let rows = english(
            &[
                "...\nFixture\tFixture\nfixture\tfixture\nKnown\tKnown\nfile.txt\tfile.txt\n"
                    .into(),
            ],
            &["known known\n".into()],
        )
        .unwrap();
        assert_eq!(rows.into_iter().collect::<Vec<_>>(), ["Fixture", "fixture"]);
    }
}
