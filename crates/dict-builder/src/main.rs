//! `msime-dict-build`: builds the dictionary artifacts msime ships (see `resources/desktop-dictionary.lock.json`) from the inputs pinned in `resources/dictionary-sources.lock.json` and the hand-maintained files in `resources/dictionary-sources/`.
//!
//! ```text
//! msime-dict-build --cache <dir> --out <dir>                 every stage, then the manifest
//! msime-dict-build --cache <dir> --out <dir> --skip ngram    a quick local build without the corpus pass
//! msime-dict-build --list
//! msime-dict-build places --cache <dir> [--out <places.tsv>] [--offline]
//! msime-dict-build hanja --cache <dir> [--out <hanja.tsv>] [--offline]
//! msime-dict-build languages --cache <dir> [--out <dir>] [--offline]
//! msime-dict-build web --input <msime.db> --out-dir <dir> [--keep-multi 200000]
//! msime-dict-build check-words [--base <words.txt> --head <words.txt>] [--translations-base <translations.txt> --translations-head <translations.txt>] [--english-base <english.txt> --english-head <english.txt>] [--msime-db <msime.db>] [--english-db <english.db>] [--json <report.json>] [--markdown <summary.md>]
//! ```

mod cantonese;
mod check_words;
mod english;
mod hanja;
mod japanese;
mod languages;
mod licensing;
mod msime;
mod ngram;
mod others;
mod pinyin;
mod places;
mod product;
mod sources;
mod sqlite;
mod stroke;
mod text;
mod web;
mod zhuyin;

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::sources::{Lock, Sources};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Stage {
    /// Quanpin tables in msime.db (tbl_{1..7,others}_{initial})
    Quanpin,
    /// custom/words.txt (pinned from msime-dictionary) merged into the quanpin tables
    CustomWords,
    /// 86 wubi table in msime.db
    Wubi,
    /// 98 wubi table in msime.db
    Wubi98,
    /// Quick phrase table in msime.db, then msime.db's planner statistics
    QuickPhrases,
    /// english_words table in english.db, plus custom/english.txt (pinned from msime-dictionary)
    English,
    /// Bidirectional gloss tables in english.db, derived from ECDICT (reads msime.db)
    EnglishGlosses,
    /// custom/translations.txt (pinned from msime-dictionary) over the gloss tables
    CustomTranslations,
    /// emoji tables in others.db
    Emoji,
    /// kaomoji tables in others.db
    Kaomoji,
    /// symbol_catalog table in others.db
    Symbols,
    /// dict_japanese.dat from Mozc OSS data, plus its notice
    JapaneseModel,
    /// bigram.bin and trigram.bin over the pinned zhwiki dump (reads msime.db)
    Ngram,
}

/// Build order: later stages read what earlier ones wrote (glosses are weighted by the quanpin tables, the n-gram vocabulary is the finished quanpin tables).
const STAGES: [Stage; 13] = [
    Stage::Quanpin,
    Stage::CustomWords,
    Stage::Wubi,
    Stage::Wubi98,
    Stage::QuickPhrases,
    Stage::English,
    Stage::EnglishGlosses,
    Stage::CustomTranslations,
    Stage::Emoji,
    Stage::Kaomoji,
    Stage::Symbols,
    Stage::JapaneseModel,
    Stage::Ngram,
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[derive(Parser)]
#[command(
    name = "msime-dict-build",
    about = "Build msime's dictionary artifacts from pinned sources",
    subcommand_negates_reqs = true
)]
struct Arguments {
    #[command(subcommand)]
    command: Option<Command>,
    /// Where the artifacts are written.
    #[arg(long, required_unless_present = "list")]
    out: Option<PathBuf>,
    /// Where pinned sources are downloaded and reused from (about 500 MB for a full build).
    #[arg(long, required_unless_present = "list")]
    cache: Option<PathBuf>,
    /// Fail instead of downloading a source that is not cached.
    #[arg(long)]
    offline: bool,
    /// Run only these stages.
    #[arg(long, value_enum, num_args = 1..)]
    only: Vec<Stage>,
    /// Skip these stages.
    #[arg(long, value_enum, num_args = 1..)]
    skip: Vec<Stage>,
    /// Also read the inputs that have no redistribution grant. For local evaluation; never attach such a build to a release.
    #[arg(long)]
    include_unlicensed: bool,
    /// Print the stages and exit.
    #[arg(long)]
    list: bool,
    /// The msime checkout the manifest's provenance is read from.
    #[arg(long, default_value_os_t = repository_root())]
    repository: PathBuf,
}

#[derive(Subcommand)]
enum Command {
    /// Check a change to the shared custom dictionary's words.txt, translations.txt and english.txt; pass a base/head pair for each file to check. Exits 0 when every appended line is accepted, 1 when anything is rejected (the reports are still written), 2 when the check cannot run.
    CheckWords(CheckWords),
    /// Write the `@` mode place table (crates/engine/src/local/places.tsv) from the administrative divisions pinned under places/ in the sources lock.
    Places(Places),
    /// Write the Korean Hanja table (crates/engine/src/korean/hanja.tsv) from the libhangul hanja.txt pinned under hanja/ in the sources lock.
    Hanja(Hanja),
    /// Write the dictionaries that ship beside the resource set (cantonese.db, zhuyin.db, stroke.db) with their licence texts and checksums, from the sources pinned under yue/ and tw/ in the sources lock, rime-stroke's stroke.dict.yaml under stroke/ in the cache (checked against the commit stroke.rs records until the lock pins it) and the pinned cn/SingleCharsAllV1.txt frequencies.
    Languages(Languages),
    /// 从完整的 msime.db 裁出网页内置输入法用的 msime-pinyin.db（全部单字加按权重排名前 N 的多字词，不含五笔）和 msime-wubi86.db（只含 86 五笔），两者逐字节可复现。
    Web(WebArgs),
}

#[derive(Args)]
struct WebArgs {
    /// 完整的 msime.db，只读打开。
    #[arg(long)]
    input: PathBuf,
    /// 写出 msime-pinyin.db 和 msime-wubi86.db 的目录，不存在时创建；同名文件会被覆盖。
    #[arg(long)]
    out_dir: PathBuf,
    /// msime-pinyin.db 在全部多字表里保留的行数（单字表总是全部保留）。
    #[arg(long, default_value_t = web::DEFAULT_KEEP_MULTI)]
    keep_multi: usize,
}

fn build_web(arguments: &WebArgs) -> Result<()> {
    for summary in web::build(&arguments.input, &arguments.out_dir, arguments.keep_multi)? {
        eprintln!("[done] {summary}");
    }
    Ok(())
}

#[derive(Args)]
struct Places {
    /// Where pinned sources are downloaded and reused from.
    #[arg(long)]
    cache: PathBuf,
    /// The table to write; the engine's embedded copy by default.
    #[arg(long, default_value_os_t = repository_root().join("crates/engine/src/local/places.tsv"))]
    out: PathBuf,
    /// Fail instead of downloading a source that is not cached.
    #[arg(long)]
    offline: bool,
    /// The msime checkout the sources lock is read from.
    #[arg(long, default_value_os_t = repository_root())]
    repository: PathBuf,
}

fn build_places(arguments: &Places) -> Result<()> {
    let root = &arguments.repository;
    let sources = Sources {
        lock: Lock::load(&root.join("resources/dictionary-sources.lock.json"))?,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: arguments.cache.clone(),
        offline: arguments.offline,
    };
    let read = |path: &str| -> Result<String> { text::read(&sources.pinned(path)?) };
    let places = places::build(
        &read(places::PROVINCES)?,
        &read(places::CITIES)?,
        &read(places::AREAS)?,
    )?;
    places::write(&places, &arguments.out)?;
    eprintln!(
        "[done] {} places -> {}",
        places.len(),
        arguments.out.display()
    );
    Ok(())
}

#[derive(Args)]
struct Hanja {
    /// Where pinned sources are downloaded and reused from.
    #[arg(long)]
    cache: PathBuf,
    /// The table to write; the engine's embedded copy by default.
    #[arg(long, default_value_os_t = repository_root().join("crates/engine/src/korean/hanja.tsv"))]
    out: PathBuf,
    /// Fail instead of downloading a source that is not cached.
    #[arg(long)]
    offline: bool,
    /// The msime checkout the sources lock is read from.
    #[arg(long, default_value_os_t = repository_root())]
    repository: PathBuf,
}

fn build_hanja(arguments: &Hanja) -> Result<()> {
    let root = &arguments.repository;
    let sources = Sources {
        lock: Lock::load(&root.join("resources/dictionary-sources.lock.json"))?,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: arguments.cache.clone(),
        offline: arguments.offline,
    };
    let readings = hanja::build(&text::read(&sources.pinned(hanja::SOURCE)?)?)?;
    hanja::write(&readings, &arguments.out)?;
    let syllables = readings
        .iter()
        .map(|reading| reading.syllable)
        .collect::<std::collections::HashSet<_>>()
        .len();
    eprintln!(
        "[done] {} readings of {syllables} syllables -> {}",
        readings.len(),
        arguments.out.display()
    );
    Ok(())
}

#[derive(Args)]
struct Languages {
    /// Where pinned sources are downloaded and reused from.
    #[arg(long)]
    cache: PathBuf,
    /// Where the dictionaries, licence texts and checksums are written.
    #[arg(long, default_value_os_t = repository_root().join("target/language-dictionaries"))]
    out: PathBuf,
    /// Fail instead of downloading a source that is not cached.
    #[arg(long)]
    offline: bool,
    /// The msime checkout the sources lock and licence texts are read from.
    #[arg(long, default_value_os_t = repository_root())]
    repository: PathBuf,
}

fn build_languages(arguments: &Languages) -> Result<()> {
    let root = &arguments.repository;
    let sources = Sources {
        lock: Lock::load(&root.join("resources/dictionary-sources.lock.json"))?,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: arguments.cache.clone(),
        offline: arguments.offline,
    };
    for summary in languages::build(&sources, &root.join("resources/licenses"), &arguments.out)? {
        eprintln!("[done] {summary}");
    }
    eprintln!(
        "[done] wrote {} in {}",
        languages::SUMS,
        arguments.out.display()
    );
    Ok(())
}

#[derive(Args)]
#[command(group(
    clap::ArgGroup::new("files")
        .args(["base", "translations_base", "english_base"])
        .required(true)
        .multiple(true)
))]
struct CheckWords {
    /// custom/words.txt before the change.
    #[arg(long, requires = "head")]
    base: Option<PathBuf>,
    /// custom/words.txt after the change.
    #[arg(long, requires = "base")]
    head: Option<PathBuf>,
    /// custom/translations.txt before the change.
    #[arg(long, requires = "translations_head")]
    translations_base: Option<PathBuf>,
    /// custom/translations.txt after the change.
    #[arg(long, requires = "translations_base")]
    translations_head: Option<PathBuf>,
    /// custom/english.txt before the change.
    #[arg(long, requires = "english_head")]
    english_base: Option<PathBuf>,
    /// custom/english.txt after the change.
    #[arg(long, requires = "english_base")]
    english_head: Option<PathBuf>,
    /// A shipped msime.db; appended words already in its quanpin tables are rejected.
    #[arg(long)]
    msime_db: Option<PathBuf>,
    /// A shipped english.db; appended English words whose word and display are already in its english_words are rejected.
    #[arg(long)]
    english_db: Option<PathBuf>,
    /// Where the JSON report is written.
    #[arg(long)]
    json: Option<PathBuf>,
    /// Where the Markdown summary is written.
    #[arg(long)]
    markdown: Option<PathBuf>,
}

fn open_read_only(path: Option<&Path>) -> Result<Option<rusqlite::Connection>> {
    path.map(|path| {
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("opening {}", path.display()))
    })
    .transpose()
}

/// Runs `check-words` and writes its reports; `Ok(false)` when a line was rejected.
fn check_words(arguments: &CheckWords) -> Result<bool> {
    let msime = open_read_only(arguments.msime_db.as_deref())?;
    let english = open_read_only(arguments.english_db.as_deref())?;
    let mut contents = Vec::new();
    for (kind, base, head) in [
        (check_words::Kind::Words, &arguments.base, &arguments.head),
        (
            check_words::Kind::Translations,
            &arguments.translations_base,
            &arguments.translations_head,
        ),
        (
            check_words::Kind::English,
            &arguments.english_base,
            &arguments.english_head,
        ),
    ] {
        if let (Some(base), Some(head)) = (base, head) {
            contents.push((kind, text::read(base)?, text::read(head)?));
        }
    }
    let inputs: Vec<check_words::Input> = contents
        .iter()
        .map(|(kind, base, head)| check_words::Input {
            kind: *kind,
            base,
            head,
        })
        .collect();
    let shipped = check_words::Shipped {
        msime: msime.as_ref(),
        english: english.as_ref(),
    };
    let report = check_words::check(&inputs, shipped)?;
    if let Some(path) = &arguments.json {
        let mut json = serde_json::to_string_pretty(&report)?;
        json.push('\n');
        std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))?;
    }
    let summary = check_words::markdown(&report);
    if let Some(path) = &arguments.markdown {
        std::fs::write(path, &summary).with_context(|| format!("writing {}", path.display()))?;
    }
    eprint!("{summary}");
    Ok(report.accepted)
}

struct Build {
    sources: Sources,
    out: PathBuf,
    complete: bool,
}

impl Build {
    fn database(&self, name: &str) -> Result<rusqlite::Connection> {
        sqlite::open(&self.out.join(name))
    }

    fn pinyin(&self) -> Result<pinyin::Pinyin> {
        pinyin::Pinyin::with_overrides(&text::read(&self.sources.repository(pinyin::OVERRIDES)?)?)
    }

    fn run(&self, stage: Stage) -> Result<String> {
        match stage {
            Stage::Quanpin => {
                let single_chars = self.sources.pinned("cn/SingleCharsAllV1.txt")?;
                let (phrases, whitelist) = if self.complete {
                    let mut whitelist = msime::parse_whitelist(&text::read(
                        &self.sources.pinned(licensing::SINGLE_CHAR_WHITELIST)?,
                    )?);
                    whitelist.extend(msime::parse_whitelist(&text::read(
                        &self.sources.repository(msime::WHITELIST_ADDITIONS)?,
                    )?));
                    (
                        vec![
                            self.sources.pinned(licensing::BASE_DICT_PART1)?,
                            self.sources.pinned(licensing::BASE_DICT_PART2)?,
                        ],
                        Some(whitelist),
                    )
                } else {
                    // Without a provenance record the whitelist cannot be applied, so every single character of the licensed source is accepted.
                    (vec![self.sources.pinned("cn/BaseDictIceV1.txt")?], None)
                };
                let inputs = msime::QuanpinInputs {
                    single_chars: &single_chars,
                    whitelist,
                    phrases: phrases.iter().map(PathBuf::as_path).collect(),
                };
                let rows = msime::build_quanpin(&mut self.database("msime.db")?, &inputs)?;
                Ok(format!("{rows} rows"))
            }
            Stage::CustomWords => {
                let words = msime::parse_custom_words(&text::read(
                    &self.sources.pinned("custom/words.txt")?,
                )?)?;
                let counts = msime::apply_custom_words(&mut self.database("msime.db")?, &words)?;
                Ok(format!(
                    "{} entries: {} inserted, {} promoted, {} already at or above their weight",
                    words.len(),
                    counts.inserted,
                    counts.promoted,
                    counts.unchanged
                ))
            }
            Stage::Wubi => {
                let (imported, skipped) = msime::build_wubi(
                    &mut self.database("msime.db")?,
                    &self.sources.pinned("cn/Wubi86.txt")?,
                )?;
                Ok(format!("{imported} rows imported, {skipped} skipped"))
            }
            Stage::Wubi98 => {
                let (imported, skipped) = msime::build_wubi98(
                    &mut self.database("msime.db")?,
                    &self.sources.pinned("cn/Wubi98.txt")?,
                )?;
                Ok(format!("{imported} rows imported, {skipped} skipped"))
            }
            Stage::QuickPhrases => {
                let path = self.sources.repository("mix/quick_phrases.txt")?;
                let (imported, skipped) =
                    msime::build_quick_phrases(&mut self.database("msime.db")?, &path)?;
                Ok(format!(
                    "{imported} rows imported, {skipped} blank, comment or invalid lines"
                ))
            }
            Stage::English => {
                let oaldpe = if self.complete {
                    english::parse_oaldpe_words(&text::read(
                        &self.sources.pinned(licensing::OALDPE_WORDS)?,
                    )?)?
                } else {
                    Default::default()
                };
                let base = english::parse_base_dict_words(&text::read(
                    &self.sources.pinned("en/BaseDictIceEn.txt")?,
                )?)?;
                let counts = english::parse_google_counts(&text::read(
                    &self.sources.pinned("en/google_count_1_w.txt")?,
                )?);
                let custom = english::parse_custom_english(&text::read(
                    &self.sources.pinned(english::CUSTOM_ENGLISH)?,
                )?)?;
                let rows = english::build_english_words(
                    &mut self.database("english.db")?,
                    &oaldpe,
                    &base,
                    &counts,
                    &custom,
                )?;
                Ok(format!(
                    "{} words, {} custom rows: {} added, {} replacing a base row",
                    rows.base,
                    custom.len(),
                    rows.custom_added,
                    rows.custom_replaced
                ))
            }
            Stage::EnglishGlosses => {
                let msime_path = self.out.join("msime.db");
                if !msime_path.is_file() {
                    bail!("english-glosses weights Chinese terms by msime.db; build quanpin first");
                }
                let mut english_db = self.database("english.db")?;
                let glosses = english::derive_glosses(
                    &self.sources.pinned("ecdict/ecdict.csv")?,
                    &english_db,
                    &sqlite::open(&msime_path)?,
                )?;
                english::write_glosses(&mut english_db, &glosses)?;
                Ok(format!(
                    "{} English-to-Chinese, {} Chinese-to-English",
                    glosses.en_zh.len(),
                    glosses.zh_en.len()
                ))
            }
            Stage::CustomTranslations => {
                let entries = english::parse_custom_translations(&text::read(
                    &self.sources.pinned(english::CUSTOM_TRANSLATIONS)?,
                )?)?;
                english::apply_custom_translations(&mut self.database("english.db")?, &entries)?;
                Ok(format!("{} overrides", entries.len()))
            }
            Stage::Emoji => {
                let catalog = others::read_emoji_catalog(&text::read(
                    &self.sources.repository("emoji/emoji_catalog.txt")?,
                )?)?;
                let zh = others::load_keyword_map(&text::read(
                    &self.sources.repository("emoji/emoji.txt")?,
                )?)?;
                let en = others::load_keyword_map(&text::read(
                    &self.sources.repository("emoji/emoji_en.txt")?,
                )?)?;
                let (rows, keys) = others::emoji_rows(&self.pinyin()?, &catalog, &zh, &en);
                let pinyin_rows =
                    others::build_emoji(&mut self.database("others.db")?, &rows, &keys)?;
                Ok(format!("{} emoji, {pinyin_rows} search keys", rows.len()))
            }
            Stage::Kaomoji => {
                let mapping = others::load_keyword_map(&text::read(
                    &self.sources.repository("kaomoji/kaomoji.txt")?,
                )?)?;
                let (rows, entries) = others::build_kaomoji(
                    &mut self.database("others.db")?,
                    &self.pinyin()?,
                    &mapping,
                )?;
                Ok(format!("{entries} kaomoji, {rows} keyword rows"))
            }
            Stage::Symbols => {
                let categories = others::parse_piliapp(&text::read(
                    &self.sources.repository("symbols/piliapp_symbols.txt")?,
                )?);
                let rows = others::symbol_rows(&self.pinyin()?, &categories)?;
                others::build_symbols(&mut self.database("others.db")?, &rows)?;
                Ok(format!("{} symbols", rows.len()))
            }
            Stage::JapaneseModel => {
                let mut dictionaries = Vec::new();
                for name in japanese::DICTIONARY_FILES {
                    dictionaries.push((name, text::read(&self.sources.pinned(name)?)?));
                }
                let tokens = japanese::read_tokens(&dictionaries)?;
                let (size, costs) = japanese::read_connection(
                    &text::read(&self.sources.pinned(japanese::ID_DEF)?)?,
                    &text::read(&self.sources.pinned(japanese::CONNECTION)?)?,
                )?;
                japanese::write_model(
                    &self.out.join("dict_japanese.dat"),
                    &japanese::pack(&tokens, size, &costs)?,
                )?;
                std::fs::copy(
                    self.sources.pinned(japanese::NOTICE)?,
                    self.out.join(japanese::NOTICE_NAME),
                )?;
                Ok(format!("{} tokens, {size} context ids", tokens.len()))
            }
            Stage::Ngram => {
                let msime_path = self.out.join("msime.db");
                if !msime_path.is_file() {
                    bail!("ngram segments with msime.db's vocabulary; build quanpin first");
                }
                let vocabulary = ngram::Vocabulary::load(&sqlite::open(&msime_path)?)?;
                let corpus_file = self
                    .sources
                    .lock
                    .files
                    .iter()
                    .find(|file| file.path.starts_with("ngram-corpus/"))
                    .context("no ngram corpus is pinned")?;
                let corpus = self.sources.pinned(&corpus_file.path)?;
                let counts = ngram::count_corpus(&vocabulary, &corpus)?;
                std::fs::write(
                    self.out.join("trigram.bin"),
                    ngram::pack(&vocabulary, &counts, 3)?,
                )?;
                std::fs::write(
                    self.out.join("bigram.bin"),
                    ngram::pack(&vocabulary, &counts, 2)?,
                )?;
                Ok(format!(
                    "{} words, {} Han characters counted",
                    vocabulary.len(),
                    counts.characters
                ))
            }
        }
    }
}

fn stage_name(stage: Stage) -> String {
    stage
        .to_possible_value()
        .map(|value| value.get_name().to_owned())
        .unwrap_or_default()
}

fn main() -> Result<()> {
    let arguments = Arguments::parse();
    if let Some(Command::Places(places)) = &arguments.command {
        return build_places(places);
    }
    if let Some(Command::Hanja(hanja)) = &arguments.command {
        return build_hanja(hanja);
    }
    if let Some(Command::Languages(languages)) = &arguments.command {
        return build_languages(languages);
    }
    if let Some(Command::Web(web)) = &arguments.command {
        return build_web(web);
    }
    if let Some(Command::CheckWords(check)) = &arguments.command {
        match check_words(check) {
            Ok(true) => return Ok(()),
            Ok(false) => std::process::exit(1),
            Err(error) => {
                eprintln!("Error: {error:?}");
                std::process::exit(2);
            }
        }
    }
    if arguments.list {
        for stage in STAGES {
            let value = stage.to_possible_value().context("stage without a name")?;
            println!(
                "{:<20} {}",
                value.get_name(),
                value
                    .get_help()
                    .map(ToString::to_string)
                    .unwrap_or_default()
            );
        }
        return Ok(());
    }
    let (Some(out), Some(cache)) = (arguments.out, arguments.cache) else {
        bail!("--out and --cache are required");
    };
    let complete = arguments.include_unlicensed
        || licensing::env_requests_unlicensed(std::env::var(licensing::ENV_FLAG).ok().as_deref());
    if complete {
        eprintln!(
            "[licensing] including inputs with no redistribution grant; do not release this build"
        );
    } else {
        for line in licensing::describe_exclusions() {
            eprintln!("[licensing] {line}");
        }
    }
    let root = &arguments.repository;
    let lock = Lock::load(&root.join("resources/dictionary-sources.lock.json"))?;
    std::fs::create_dir_all(&out)?;
    let build = Build {
        sources: Sources {
            lock,
            repository_inputs: root.join("resources/dictionary-sources"),
            cache,
            offline: arguments.offline,
        },
        out,
        complete,
    };

    let selected: Vec<Stage> = STAGES
        .into_iter()
        .filter(|stage| {
            (arguments.only.is_empty() || arguments.only.contains(stage))
                && !arguments.skip.contains(stage)
        })
        .collect();
    for stage in selected {
        let started = Instant::now();
        eprintln!("[build] {}", stage_name(stage));
        let summary = build
            .run(stage)
            .with_context(|| format!("stage {}", stage_name(stage)))?;
        eprintln!(
            "[done] {} in {:.1}s: {summary}",
            stage_name(stage),
            started.elapsed().as_secs_f64()
        );
    }

    for name in product::SHIPPING_ARTIFACTS
        .iter()
        .filter(|name| name.ends_with(".db"))
    {
        let path = build.out.join(name);
        if path.is_file() {
            sqlite::freeze(&path)?;
        }
    }
    let missing: Vec<&str> = product::SHIPPING_ARTIFACTS
        .into_iter()
        .filter(|name| !build.out.join(name).is_file())
        .collect();
    if !missing.is_empty() {
        eprintln!(
            "[product] not writing {}: missing {}",
            product::MANIFEST,
            missing.join(", ")
        );
        return Ok(());
    }
    product::verify(&build.out, complete)?;
    product::write_manifest(&build.out, root, &build.sources.lock, complete)?;
    eprintln!(
        "[product] verified; wrote {} and SHA256SUMS.txt",
        product::MANIFEST
    );
    Ok(())
}
