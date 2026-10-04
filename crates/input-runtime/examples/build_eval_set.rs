//! Freeze the MIT-licensed SampleIME quanpin word list into a committed evaluation set.
//!
//! The source is `SampleIMESimplifiedQuanPin.txt` from microsoft/Windows-classic-samples (MIT). No build reads it and it is not part of `msime-pinyin.db`, so the evaluation set is frozen into `resources/eval/quanpin-words-v1.tsv` once and committed; rerunning this needs a copy of that file, not anything in this repository.
//!
//! Truncation is filtered with the Engine's own segmenter rather than a length threshold: the source format truncates keys at twelve characters, but shorter keys are truncated too (`chulufengma`, `shumenshul`), so only "does this key segment into exactly as many syllables as the value has characters" rejects them all.
//!
//! usage: build_eval_set <SampleIMESimplifiedQuanPin.txt> <output.tsv> <engine-commit>

use std::collections::BTreeMap;
use std::fmt::Write as _;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: build_eval_set <source.txt> <output.tsv> <engine-commit>".into());
    }
    let (source, output, commit) = (&args[0], &args[1], &args[2]);
    let text = std::fs::read_to_string(source)?;

    let (mut unparsed, mut single_char, mut truncated) = (0usize, 0usize, 0usize);
    // Keyed by (input, gold) so a repeated pair collapses; the source lists homophones separately.
    let mut kept: BTreeMap<(String, String), usize> = BTreeMap::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = parse(line) else {
            unparsed += 1;
            continue;
        };
        let syllables = value.chars().count();
        if syllables < 2 {
            single_char += 1;
            continue;
        }
        let key = key.to_ascii_lowercase();
        let normalized = msime_engine::host::normalize_full_pinyin(&key, syllables);
        if normalized.is_empty() {
            truncated += 1;
            continue;
        }
        kept.insert((key, value.to_string()), syllables);
    }

    let mut out = String::new();
    writeln!(
        out,
        "# source: microsoft/Windows-classic-samples SampleIMESimplifiedQuanPin.txt (MIT)"
    )?;
    writeln!(out, "# engine-commit: {commit}")?;
    writeln!(
        out,
        "# generated-by: cargo run -p msime-input-runtime --example build_eval_set"
    )?;
    writeln!(
        out,
        "# kept={} unparsed={unparsed} single_char={single_char} truncated={truncated}",
        kept.len()
    )?;
    writeln!(out, "id\tinput\tgold\tsyllables\ttags")?;
    for (index, ((input, gold), syllables)) in kept.iter().enumerate() {
        writeln!(out, "w-{:05}\t{input}\t{gold}\t{syllables}\t", index + 1)?;
    }
    std::fs::write(output, out)?;

    println!(
        "kept={} unparsed={unparsed} single_char={single_char} truncated={truncated} -> {output}",
        kept.len()
    );
    Ok(())
}

/// Parses one `"pinyin"="漢字"` line. Returns None for anything else.
fn parse(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix('"')?;
    let (key, rest) = rest.split_once("\"=\"")?;
    let value = rest.strip_suffix('"')?;
    if key.is_empty() || value.is_empty() || !key.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    Some((key, value))
}
