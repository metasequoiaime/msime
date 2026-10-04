//! `languages`: the dictionaries that ship beside the resource set rather than inside it (`msime-cantonese.db`, `msime-zhuyin.db`, `msime-stroke.db`), each with its licence text, plus `msime-language-dictionaries-SHA256SUMS` over everything written. None of this touches the desktop dictionary product (`STAGES`, `product::SHIPPING_ARTIFACTS`, the manifest): a host ships these files only for the schemes it offers.

use std::path::Path;

use anyhow::{Context, Result};

use crate::cantonese;
use crate::sources::{sha256_file, Sources};
use crate::stroke;
use crate::text;
use crate::zhuyin;

pub const SUMS: &str = "msime-language-dictionaries-SHA256SUMS";

/// Builds every language dictionary into `out` with the licence texts from `licenses` (`resources/licenses/`), verifies each, and writes the checksums. Returns one summary line per dictionary.
pub fn build(sources: &Sources, licenses: &Path, out: &Path) -> Result<Vec<String>> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let mut written = Vec::new();
    let mut summaries = Vec::new();

    let commit = reference_commit(sources, cantonese::REFERENCE)?;
    let characters = text::read(&sources.pinned(cantonese::CHARACTERS)?)?;
    let words = text::read(&sources.pinned(cantonese::WORDS)?)?;
    let essay = text::read(&sources.pinned(cantonese::ESSAY)?)?;
    let dictionary = cantonese::build(
        &cantonese::parse_characters(&characters)?,
        &cantonese::parse_words(&words)?,
        &cantonese::parse_essay(&essay)?,
    );
    let database = out.join(cantonese::DATABASE);
    cantonese::write(&dictionary, &database, commit)?;
    let counts = cantonese::verify(&database, cantonese::FLOORS)?;
    copy_license(
        licenses,
        cantonese::LICENSE_SOURCE,
        out,
        cantonese::LICENSE_NAME,
    )?;
    written.extend([cantonese::DATABASE, cantonese::LICENSE_NAME]);
    summaries.push(format!(
        "{}: {} syllables, {} character and {} word entries ({} of them from the essay)",
        cantonese::DATABASE,
        counts.syllables,
        counts.characters,
        counts.words,
        dictionary.essay_words
    ));

    let commit = reference_commit(sources, zhuyin::REFERENCE)?;
    let mut rows = zhuyin::parse(
        zhuyin::PHRASES,
        &text::read(&sources.pinned(zhuyin::PHRASES)?)?,
    )?;
    rows.extend(zhuyin::parse(
        zhuyin::SUPPLEMENT,
        &text::read(&sources.pinned(zhuyin::SUPPLEMENT)?)?,
    )?);
    rows.extend(zhuyin::parse(
        zhuyin::CHARACTERS,
        &text::read(&sources.pinned(zhuyin::CHARACTERS)?)?,
    )?);
    let database = out.join(zhuyin::DATABASE);
    zhuyin::write(&zhuyin::build(&rows), &database, commit)?;
    let counts = zhuyin::verify(&database, zhuyin::FLOORS, &zhuyin::EXPECTED)?;
    copy_license(licenses, zhuyin::LICENSE_SOURCE, out, zhuyin::LICENSE_NAME)?;
    written.extend([zhuyin::DATABASE, zhuyin::LICENSE_NAME]);
    summaries.push(format!(
        "{}: {} syllables, {} character and {} phrase entries",
        zhuyin::DATABASE,
        counts.syllables,
        counts.characters,
        counts.phrases
    ));

    match stroke::source(sources)? {
        Some(source) => {
            let source = text::read(&source)?;
            let frequencies = text::read(&sources.pinned(stroke::FREQUENCIES)?)?;
            let database = out.join(stroke::DATABASE);
            stroke::write(
                &stroke::build(
                    &stroke::parse(&source)?,
                    &stroke::parse_frequencies(&frequencies)?,
                ),
                &database,
                stroke::source_commit(sources),
            )?;
            let counts = stroke::verify(&database, stroke::FLOORS, &stroke::EXPECTED)?;
            copy_license(licenses, stroke::LICENSE_SOURCE, out, stroke::LICENSE_NAME)?;
            written.extend([stroke::DATABASE, stroke::LICENSE_NAME]);
            summaries.push(format!(
                "{}: {} entries of {} characters, {} of them with a frequency",
                stroke::DATABASE,
                counts.entries,
                counts.characters,
                counts.weighted
            ));
        }
        // 笔画源文件既没被锁文件固定、也没放进缓存：只跳过笔画词库，不让粤拼与注音的发布跟着失败。
        None => summaries.push(format!(
            "{}: skipped, {} is not pinned in the sources lock and not in the cache",
            stroke::DATABASE,
            stroke::SOURCE
        )),
    }

    write_sums(out, &written)?;
    Ok(summaries)
}

/// The commit the sources lock pins `reference` at, recorded as a database's `source_commit`.
fn reference_commit<'a>(sources: &'a Sources, reference: &str) -> Result<&'a str> {
    Ok(&sources
        .lock
        .references
        .get(reference)
        .with_context(|| format!("{reference} is not pinned in the sources lock"))?
        .commit)
}

fn copy_license(licenses: &Path, source: &str, out: &Path, name: &str) -> Result<()> {
    let from = licenses.join(source);
    std::fs::copy(&from, out.join(name)).with_context(|| format!("copying {}", from.display()))?;
    Ok(())
}

/// `sha256  name` lines in name order, the `sha256sum` format of the desktop product's `msime-SHA256SUMS.txt`.
fn write_sums(out: &Path, names: &[&str]) -> Result<()> {
    let mut names = names.to_vec();
    names.sort_unstable();
    let mut sums = String::new();
    for name in names {
        sums.push_str(&format!("{}  {name}\n", sha256_file(&out.join(name))?));
    }
    std::fs::write(out.join(SUMS), sums).with_context(|| format!("writing {SUMS}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::Lock;

    #[test]
    fn sums_list_every_file_in_name_order() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.db"), b"b").unwrap();
        std::fs::write(dir.path().join("a.txt"), b"a").unwrap();
        write_sums(dir.path(), &["b.db", "a.txt"]).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join(SUMS)).unwrap(),
            format!(
                "{}  a.txt\n{}  b.db\n",
                sha256_file(&dir.path().join("a.txt")).unwrap(),
                sha256_file(&dir.path().join("b.db")).unwrap()
            )
        );
    }

    /// 用锁定的源文件完整构建一次，文件缓存在仓库的 `target/dict-cache`（首次使用时下载，约 12 MB）。锁文件固定 `stroke/stroke.dict.yaml` 之前，要先把 rime-stroke 的原文件放到 `target/dict-cache/stroke/`（见 `stroke::source`）。
    #[test]
    #[ignore = "首次使用时下载 msime-dictionary 附件里的粤拼、注音与字频源文件"]
    fn builds_from_the_pinned_sources() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let sources = Sources {
            lock: Lock::load(&root.join("resources/dictionary-sources.lock.json")).unwrap(),
            repository_inputs: root.join("resources/dictionary-sources"),
            cache: root.join("target/dict-cache"),
            offline: false,
        };
        let out = tempfile::tempdir().unwrap();
        let summaries = build(&sources, &root.join("resources/licenses"), out.path()).unwrap();
        assert_eq!(summaries.len(), 3, "{summaries:?}");
        let sums = std::fs::read_to_string(out.path().join(SUMS)).unwrap();
        let names: Vec<&str> = sums
            .lines()
            .map(|line| line.split_once("  ").unwrap().1)
            .collect();
        assert_eq!(
            names,
            [
                cantonese::DATABASE,
                zhuyin::LICENSE_NAME,
                cantonese::LICENSE_NAME,
                stroke::LICENSE_NAME,
                stroke::DATABASE,
                zhuyin::DATABASE
            ]
        );
        let license = std::fs::read_to_string(out.path().join(cantonese::LICENSE_NAME)).unwrap();
        assert!(license.contains("Attribution 4.0 International"));
        let license = std::fs::read_to_string(out.path().join(zhuyin::LICENSE_NAME)).unwrap();
        assert!(license.contains("GNU LESSER GENERAL PUBLIC LICENSE"));
        let license = std::fs::read_to_string(out.path().join(stroke::LICENSE_NAME)).unwrap();
        assert!(license.contains("Version 3, 29 June 2007"));
    }
}
