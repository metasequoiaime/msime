//! `languages`: the dictionaries that ship beside the resource set rather than inside it (`msime-cantonese.db`, `msime-zhuyin.db`, `msime-stroke.db`), each with its licence text, plus `msime-language-dictionaries-SHA256SUMS` over everything written. None of this touches the desktop dictionary product (`STAGES`, `product::SHIPPING_ARTIFACTS`, the manifest): a host ships these files only for the schemes it offers.

use std::path::Path;

use anyhow::{Context, Result};

use crate::cantonese;
use crate::cantonese_curation;
use crate::hkcancor;
use crate::sources::{sha256_file, Sources};
use crate::stroke;
use crate::text;
use crate::zhuyin;

pub const SUMS: &str = "msime-language-dictionaries-SHA256SUMS";

/// 把各语言词库及 `licenses`（`resources/licenses/`）中的许可文本写入 `out`，逐库验证并写出校验和。每个词库返回一行摘要，另列出构建压低的粤语 essay 补词，供维护者审查。
pub fn build(sources: &Sources, licenses: &Path, out: &Path) -> Result<Vec<String>> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let mut written = Vec::new();
    let mut summaries = Vec::new();

    let commit = reference_commit(sources, cantonese::REFERENCE)?;
    let characters = text::read(&sources.pinned(cantonese::CHARACTERS)?)?;
    let words = text::read(&sources.pinned(cantonese::WORDS)?)?;
    let essay = text::read(&sources.pinned(cantonese::ESSAY)?)?;
    let corpus = text::read(&sources.pinned(cantonese::CORPUS)?)?;
    let custom = cantonese::custom_source(sources)?
        .map(|path| text::read(&path))
        .transpose()?;
    let characters = cantonese::parse_characters(&characters)?;
    let custom_words = match &custom {
        Some(custom) => {
            cantonese::parse_custom(custom, &cantonese::character_readings(&characters))?
        }
        None => Vec::new(),
    };
    let corpus = hkcancor::parse(&corpus)?;
    // 两张异体表和否决表都必须在 checkout 里，缺了就直接失败，不退回旧的替换清单：只有 msime-dictionary 的发布跑 `languages`，它总是构建 `main` 的当前提交，碰到缺文件只可能是两个仓库的 PR 合反了顺序，报出缺哪个文件比悄悄发布另一份字形好查。
    let tw_variants = text::read(&sources.pinned(cantonese::TW_VARIANTS)?)?;
    let hk_variants = text::read(&sources.pinned(cantonese::HK_VARIANTS)?)?;
    let vetoes =
        cantonese::parse_variant_vetoes(&text::read(&sources.pinned(cantonese::VARIANT_VETOES)?)?)?;
    let forms = cantonese::hong_kong_forms(
        &cantonese::candidate_variants(&tw_variants, &hk_variants, &vetoes)?,
        &corpus,
    );
    let curation = cantonese_curation_inputs(sources)?;
    let word_rows = cantonese::parse_words(&words)?;
    let dictionary = cantonese::build(
        &characters,
        &word_rows,
        &cantonese::parse_essay(&essay)?,
        &corpus,
        &custom_words,
        &forms,
        &curation,
    )?;
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
    let custom_summary = match custom {
        Some(_) => format!(
            "{} of the {} reviewed words in {} added or raised",
            dictionary.custom_words,
            custom_words.len(),
            cantonese::CUSTOM
        ),
        None => format!("{} is not in the checkout", cantonese::CUSTOM),
    };
    let forms_summary = forms
        .iter()
        .map(|(opencc, form)| format!("{opencc}→{} {}", form.hong_kong, form.corpus_count))
        .collect::<Vec<_>>()
        .join(", ");
    let vetoes_summary = vetoes.iter().collect::<String>();
    summaries.push(format!(
        "{}: {} syllables, {} character and {} word entries ({} of them from the essay, {} of those held below the evidenced words-file words of their key for want of HKCanCor, reviewed or pinyin corroboration and {} kept by {}; {} words weighed by the HKCanCor counts at {} each, {} of them held below an essay word of their key; {custom_summary}; {} entries removed by {}; {} character entries raised toward {} by the words they are written in; {} OpenCC forms spelled the Hong Kong way, with their HKCanCor counts: {forms_summary}; kept by {}: {vetoes_summary})",
        cantonese::DATABASE,
        counts.syllables,
        counts.characters,
        counts.words,
        dictionary.essay_words,
        dictionary.essay_capped.len(),
        dictionary.essay_kept,
        cantonese_curation::ESSAY_KEEPS,
        dictionary.corpus_words,
        dictionary.corpus_scale,
        dictionary.corpus_capped,
        dictionary.vetoed,
        cantonese_curation::WORD_VETOES,
        dictionary.characters_raised,
        cantonese::WORD_FLOOR_CEILING,
        forms.len(),
        cantonese::VARIANT_VETOES
    ));
    // 压低的 essay 补词逐条列出，维护者审过之后把该留的写进豁免表。
    summaries.push(format!(
        "{}: essay words held below the evidenced words-file words of their key (key text old→new): {}",
        cantonese::DATABASE,
        dictionary
            .essay_capped
            .iter()
            .map(|capped| format!("{} {} {}→{}", capped.key, capped.text, capped.from, capped.to))
            .collect::<Vec<_>>()
            .join(", ")
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
    let occurrences = text::read(&sources.pinned(zhuyin::OCCURRENCES)?)?;
    let dictionary = zhuyin::build(&rows, &zhuyin::parse_occurrences(&occurrences)?);
    let database = out.join(zhuyin::DATABASE);
    zhuyin::write(&dictionary, &database, commit)?;
    let counts = zhuyin::verify(&database, zhuyin::FLOORS, &zhuyin::EXPECTED)?;
    copy_license(licenses, zhuyin::LICENSE_SOURCE, out, zhuyin::LICENSE_NAME)?;
    written.extend([zhuyin::DATABASE, zhuyin::LICENSE_NAME]);
    summaries.push(format!(
        "{}: {} syllables, {} character and {} phrase entries ({} phrases weighed by phrase.occ at {} each, {} of them capped below a counted entry; {} keys whose tied single-reading characters phrase.occ reordered)",
        zhuyin::DATABASE,
        counts.syllables,
        counts.characters,
        counts.phrases,
        dictionary.filled,
        dictionary.occurrence_scale,
        dictionary.capped,
        dictionary.reordered
    ));

    match stroke::source(sources)? {
        Some(source) => {
            let source = text::read(&source)?;
            let frequencies = text::read(&sources.pinned(stroke::FREQUENCIES)?)?;
            let frequencies = stroke::parse_frequencies(&frequencies)?;
            let simplified_to_traditional =
                text::read(&sources.pinned(stroke::SIMPLIFIED_TO_TRADITIONAL)?)?;
            let inherited = stroke::traditional_weights(
                &frequencies,
                &cantonese::parse_variants(
                    stroke::SIMPLIFIED_TO_TRADITIONAL,
                    &simplified_to_traditional,
                )?,
                &cantonese::parse_variants(cantonese::TW_VARIANTS, &tw_variants)?,
                &cantonese::parse_variants(cantonese::HK_VARIANTS, &hk_variants)?,
            );
            let dictionary = stroke::build(
                &stroke::parse(&source)?,
                &frequencies,
                &inherited,
                &stroke::cantonese_characters(&word_rows),
            );
            let database = out.join(stroke::DATABASE);
            stroke::write(&dictionary, &database, stroke::source_commit(sources))?;
            let counts = stroke::verify(
                &database,
                stroke::FLOORS,
                stroke::CEILINGS,
                &stroke::EXPECTED,
                &stroke::EXPECTED_COMPLETIONS,
            )?;
            copy_license(licenses, stroke::LICENSE_SOURCE, out, stroke::LICENSE_NAME)?;
            written.extend([stroke::DATABASE, stroke::LICENSE_NAME]);
            summaries.push(format!(
                "{}: {} entries of {} characters, {} of them with a frequency; {} traditional characters raised toward 1/{} of their simplified form's frequency",
                stroke::DATABASE,
                counts.entries,
                counts.characters,
                counts.weighted,
                dictionary.inherited,
                stroke::TRADITIONAL_DIVISOR
            ));
        }
        // 没有 `--dictionary` checkout、缓存里也没有笔画源文件：只跳过笔画词库，不让粤拼与注音的发布跟着失败。
        None => summaries.push(format!(
            "{}: skipped, no --dictionary checkout and {} is not in the cache",
            stroke::DATABASE,
            stroke::SOURCE
        )),
    }

    write_sums(out, &written)?;
    Ok(summaries)
}

/// 粤拼库的清洗输入：拼音库三张上游词表（按 `curation/pinyin-vetoes.txt` 去掉否决了的读音；权重覆盖表不影响一个词在不在库，见 `cantonese_curation` 的模块文档）、繁转简字表，以及 `curation/cantonese-essay-keep.txt`、`curation/cantonese-word-vetoes.txt`；`curation/` 的表不在时按空表。
fn cantonese_curation_inputs(sources: &Sources) -> Result<cantonese_curation::Curation> {
    let mut tables = Vec::with_capacity(cantonese_curation::PINYIN_TABLES.len());
    for path in cantonese_curation::PINYIN_TABLES {
        tables.push(text::read(&sources.pinned(path)?)?);
    }
    let pinyin_vetoes = crate::curation::PinyinCuration::load(sources)?.vetoes;
    let traditional_to_simplified = cantonese::parse_variants(
        cantonese_curation::TRADITIONAL_TO_SIMPLIFIED,
        &text::read(&sources.pinned(cantonese_curation::TRADITIONAL_TO_SIMPLIFIED)?)?,
    )?;
    let keeps = cantonese_curation::EssayKeeps::parse(
        &crate::curation::read_optional(sources, cantonese_curation::ESSAY_KEEPS)?
            .unwrap_or_default(),
    )?;
    let vetoes = cantonese_curation::WordVetoes::parse(
        &crate::curation::read_optional(sources, cantonese_curation::WORD_VETOES)?
            .unwrap_or_default(),
    )?;
    Ok(cantonese_curation::Curation {
        pinyin: cantonese_curation::PinyinWords::new(
            &tables.iter().map(String::as_str).collect::<Vec<_>>(),
            &traditional_to_simplified,
            &pinyin_vetoes,
        ),
        keeps,
        vetoes,
    })
}

/// The commit the sources lock pins `reference` at, recorded as a database's `source_commit`.
fn reference_commit<'a>(sources: &'a Sources, reference: &str) -> Result<&'a str> {
    Ok(&sources
        .lock
        .references
        .get(reference)
        .with_context(|| format!("{reference} is not a reference in the sources lock"))?
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
    use crate::sources::{Dictionary, Lock};

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

    /// 从 `MSIME_DICTIONARY` 指向的 msime-dictionary checkout 完整构建一次，不下载任何文件。
    #[test]
    #[ignore = "needs MSIME_DICTIONARY=<msime-dictionary checkout> and downloads nothing"]
    fn builds_from_a_dictionary_checkout() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let lock = Lock::load(&root.join("resources/dictionary-sources.lock.json")).unwrap();
        let dictionary = Dictionary::open(
            std::path::PathBuf::from(std::env::var("MSIME_DICTIONARY").expect("MSIME_DICTIONARY")),
            &lock,
        )
        .unwrap();
        let sources = Sources {
            lock,
            repository_inputs: root.join("resources/dictionary-sources"),
            cache: root.join("target/dict-cache"),
            offline: true,
            dictionary: Some(dictionary),
        };
        let out = tempfile::tempdir().unwrap();
        let summaries = build(&sources, &root.join("resources/licenses"), out.path()).unwrap();
        assert_eq!(summaries.len(), 4, "{summaries:?}");
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
