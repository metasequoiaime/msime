//! `msime-dict-build`: builds the dictionary artifacts msime ships (see `resources/desktop-dictionary.lock.json`) from the inputs pinned in `resources/dictionary-sources.lock.json` and the hand-maintained files in `resources/dictionary-sources/`.
//!
//! ```text
//! msime-dict-build --cache <dir> --out <dir>                 every stage, then the manifest
//! msime-dict-build --cache <dir> --out <dir> --skip ngram    a quick local build without the corpus pass
//! msime-dict-build --cache <dir> --out <dir> --dictionary <msime-dictionary checkout>
//! msime-dict-build --list
//! msime-dict-build places --cache <dir> [--out <places.tsv>] [--offline]
//! msime-dict-build places-supplement --cache <dir> --out <places.txt> [--offline]
//! msime-dict-build english-supplement --cache <dir> --out <scowl-words.txt> [--offline]
//! msime-dict-build hanja --cache <dir> [--out <hanja.tsv>] [--offline]
//! msime-dict-build hkcancor-counts --cache <dir> --out <hkcancor-word-counts.txt> [--offline]
//! msime-dict-build languages --cache <dir> [--out <dir>] [--offline] [--dictionary <msime-dictionary checkout>]
//! msime-dict-build check-words [--base <words.txt> --head <words.txt>] [--translations-base <translations.txt> --translations-head <translations.txt>] [--english-base <english.txt> --english-head <english.txt>] [--msime-db <msime-pinyin.db>] [--english-db <msime-english.db>] [--json <report.json>] [--markdown <summary.md>]
//! ```

mod cantonese;
mod check_words;
mod english;
mod english_supplement;
mod hanja;
mod hkcancor;
mod japanese;
mod languages;
mod licensing;
mod msime;
mod ngram;
mod others;
mod pinyin;
mod places;
mod places_supplement;
mod product;
mod sources;
mod sqlite;
mod stroke;
mod text;
mod zhuyin;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::sources::{Lock, Sources};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Stage {
    /// Quanpin tables in msime-pinyin.db (tbl_{1..7,others}_{initial})
    Quanpin,
    /// sources/pinyin/places.txt (pinned from msime-dictionary) merged into the quanpin tables, raising weights only
    PlacesSupplement,
    /// custom/words.txt (pinned from msime-dictionary) merged into the quanpin tables
    CustomWords,
    /// Wrong readings listed in resources/dictionary-sources/pinyin-reading-corrections.txt removed from the quanpin tables
    ReadingCorrections,
    /// 86 wubi table in msime-pinyin.db
    Wubi,
    /// 98 wubi table in msime-pinyin.db
    Wubi98,
    /// Quick phrase table in msime-pinyin.db, then msime-pinyin.db's planner statistics
    QuickPhrases,
    /// english_words table in msime-english.db from the rime-ice word lists and sources/english/scowl-words.txt, plus custom/english.txt (pinned from msime-dictionary)
    English,
    /// Bidirectional gloss tables in msime-english.db, derived from ECDICT (reads msime-pinyin.db)
    EnglishGlosses,
    /// custom/translations.txt (pinned from msime-dictionary) over the gloss tables
    CustomTranslations,
    /// emoji tables in msime-others.db
    Emoji,
    /// kaomoji tables in msime-others.db
    Kaomoji,
    /// symbol_catalog table in msime-others.db
    Symbols,
    /// msime-japanese.dat from Mozc OSS data, plus its notice
    JapaneseModel,
    /// msime-bigram.bin and msime-trigram.bin over the pinned zhwiki dump (reads msime-pinyin.db)
    Ngram,
}

/// Build order: later stages read what earlier ones wrote (glosses are weighted by the quanpin tables, the n-gram vocabulary is the finished quanpin tables).
const STAGES: [Stage; 15] = [
    Stage::Quanpin,
    Stage::PlacesSupplement,
    Stage::CustomWords,
    Stage::ReadingCorrections,
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
    /// A msime-dictionary checkout to read its files from instead of the lock: every file the lock pins from msime-dictionary, and any path under sources/ or custom/ the lock does not pin, is read from <path>/<lock path> without the lock's size and SHA-256 check (the checkout's Git commit pins the content, and the manifest records it); upstream data whose commit msime records (the rime-ice, rime-cantonese, libchewing-data, McBopomofo, rime-stroke, Mozc, libhangul and 98 Wubi files, and the HKCanCor counts and SCOWL supplement generated from upstream commits the lock records) must still match the lock.
    #[arg(long, value_name = "PATH")]
    dictionary: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Check a change to the shared custom dictionary's words.txt, translations.txt and english.txt; pass a base/head pair for each file to check. Exits 0 when every appended line is accepted, 1 when anything is rejected (the reports are still written), 2 when the check cannot run.
    CheckWords(CheckWords),
    /// Write the `@` mode place table (crates/engine/src/local/places.tsv) from the administrative divisions pinned under places/ in the sources lock.
    Places(Places),
    /// Write msime-dictionary's sources/pinyin/places.txt: the administrative place names (full and short) the pinned pinyin sources lack or rank below their level's floor, from the divisions pinned under places/.
    PlacesSupplement(PlacesSupplement),
    /// Write msime-dictionary's sources/english/scowl-words.txt: the words of SCOWL's size-60 Aspell dictionary (American plus British -ise spellings, pinned under scowl/) that the pinned rime-ice word lists (entries and commented-out entries) and custom/english.txt lack, less slurs, capitalised-only names spelling a pinyin key and words without a Google count (see english_supplement.rs).
    EnglishSupplement(EnglishSupplement),
    /// 从 sources lock 固定的 libhangul `sources/korean/hanja.txt` 生成韩文 Hanja 表（crates/engine/src/korean/hanja.tsv）。
    Hanja(Hanja),
    /// Write msime-dictionary's sources/cantonese/hkcancor-word-counts.txt: how often each word of two or more Han characters occurs in the HKCanCor transcriptions pinned under hkcancor/ in the sources lock.
    HkcancorCounts(HkcancorCounts),
    /// Write the dictionaries that ship beside the resource set (msime-cantonese.db, msime-zhuyin.db, msime-stroke.db) with their licence texts and checksums, from the sources pinned under sources/cantonese/ and sources/zhuyin/ in the sources lock, rime-stroke's stroke.dict.yaml pinned at sources/stroke/stroke.dict.yaml in the sources lock (stroke.rs's recorded commit, size and SHA-256 only check a file placed in the cache when the lock has no such entry) and the pinned sources/pinyin/single-chars.txt frequencies.
    Languages(Languages),
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
        dictionary: None,
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
struct PlacesSupplement {
    /// Where pinned sources are downloaded and reused from.
    #[arg(long)]
    cache: PathBuf,
    /// The supplement to write (msime-dictionary's sources/pinyin/places.txt).
    #[arg(long)]
    out: PathBuf,
    /// Fail instead of downloading a source that is not cached.
    #[arg(long)]
    offline: bool,
    /// The msime checkout the sources lock is read from.
    #[arg(long, default_value_os_t = repository_root())]
    repository: PathBuf,
}

fn build_places_supplement(arguments: &PlacesSupplement) -> Result<()> {
    let root = &arguments.repository;
    let sources = Sources {
        lock: Lock::load(&root.join("resources/dictionary-sources.lock.json"))?,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: arguments.cache.clone(),
        offline: arguments.offline,
        dictionary: None,
    };
    let read = |path: &str| -> Result<String> { text::read(&sources.pinned(path)?) };
    let (provinces, cities, areas) = (
        read(places::PROVINCES)?,
        read(places::CITIES)?,
        read(places::AREAS)?,
    );
    let (single_chars, base, rime_ice_supplement) = (
        read(places_supplement::SINGLE_CHARS)?,
        read(places_supplement::BASE)?,
        read(places_supplement::RIME_ICE_SUPPLEMENT)?,
    );
    let supplement = places_supplement::build(&places_supplement::Inputs {
        provinces: &provinces,
        cities: &cities,
        areas: &areas,
        single_chars: &single_chars,
        base: &base,
        rime_ice_supplement: &rime_ice_supplement,
    })?;
    let pinned = |path: &str| {
        sources
            .lock
            .files
            .iter()
            .find(|file| file.path == path)
            .with_context(|| format!("{path} is not pinned in the sources lock"))
    };
    // The places URLs name the upstream commit: .../Administrative-divisions-of-China/<commit>/dist/areas.csv.
    let areas_url = &pinned(places::AREAS)?.url;
    let upstream_commit = areas_url
        .split('/')
        .find(|segment| segment.len() == 40 && segment.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .with_context(|| format!("{areas_url} names no commit"))?;
    let compared = [
        (
            places_supplement::BASE,
            pinned(places_supplement::BASE)?.sha256.as_str(),
        ),
        (
            places_supplement::RIME_ICE_SUPPLEMENT,
            pinned(places_supplement::RIME_ICE_SUPPLEMENT)?
                .sha256
                .as_str(),
        ),
    ];
    let rendered = places_supplement::render(
        &supplement,
        &places_supplement::Provenance {
            upstream_commit,
            compared: &compared,
            single_chars: (
                places_supplement::SINGLE_CHARS,
                pinned(places_supplement::SINGLE_CHARS)?.sha256.as_str(),
            ),
        },
    );
    std::fs::write(&arguments.out, rendered)
        .with_context(|| format!("writing {}", arguments.out.display()))?;
    for line in places_supplement::report(&supplement) {
        eprintln!("[report] {line}");
    }
    let inserted = supplement
        .entries
        .iter()
        .filter(|entry| entry.existing.is_none())
        .count();
    eprintln!(
        "[done] {} entries ({inserted} new, {} raised; {} already ranked or at their floor) -> {}",
        supplement.entries.len(),
        supplement.entries.len() - inserted,
        supplement.already_ranked,
        arguments.out.display()
    );
    Ok(())
}

#[derive(Args)]
struct EnglishSupplement {
    /// Where pinned sources are downloaded and reused from.
    #[arg(long)]
    cache: PathBuf,
    /// The supplement to write (msime-dictionary's sources/english/scowl-words.txt).
    #[arg(long)]
    out: PathBuf,
    /// Fail instead of downloading a source that is not cached.
    #[arg(long)]
    offline: bool,
    /// The msime checkout the sources lock is read from.
    #[arg(long, default_value_os_t = repository_root())]
    repository: PathBuf,
}

fn build_english_supplement(arguments: &EnglishSupplement) -> Result<()> {
    let root = &arguments.repository;
    let sources = Sources {
        lock: Lock::load(&root.join("resources/dictionary-sources.lock.json"))?,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: arguments.cache.clone(),
        offline: arguments.offline,
        dictionary: None,
    };
    let read = |path: &str| -> Result<String> { text::read(&sources.pinned(path)?) };
    let [rime_ice_en, rime_ice_en_supplement, custom_english] = english_supplement::COMPARED;
    let pinyin = english_supplement::PINYIN
        .iter()
        .map(|path| read(path))
        .collect::<Result<Vec<_>>>()?;
    let filters = english_supplement::Filters {
        compared: english_supplement::compared_words(
            &read(rime_ice_en)?,
            &read(rime_ice_en_supplement)?,
            &read(custom_english)?,
        )?,
        pinyin_keys: english_supplement::pinyin_keys(
            &pinyin.iter().map(String::as_str).collect::<Vec<_>>(),
        ),
        counts: english::parse_google_counts(&read(english_supplement::COUNTS)?),
    };
    let archive = std::fs::read(sources.pinned(english_supplement::ARCHIVE)?)?;
    let package = english_supplement::read_package(&archive)?;
    let supplement = english_supplement::build(&package, &filters)?;
    let pinned = |path: &str| {
        sources
            .lock
            .files
            .iter()
            .find(|file| file.path == path)
            .with_context(|| format!("{path} is not pinned in the sources lock"))
    };
    let archive_file = pinned(english_supplement::ARCHIVE)?;
    let upstream = sources
        .lock
        .references
        .get(english_supplement::REFERENCE)
        .with_context(|| {
            format!(
                "{} is not a reference in the sources lock",
                english_supplement::REFERENCE
            )
        })?;
    let with_sha256 = |paths: &[&'static str]| {
        paths
            .iter()
            .map(|path| Ok((*path, pinned(path)?.sha256.as_str())))
            .collect::<Result<Vec<_>>>()
    };
    let compared_files = with_sha256(&english_supplement::COMPARED)?;
    let pinyin_files = with_sha256(&english_supplement::PINYIN)?;
    let rendered = english_supplement::render(
        &supplement,
        &english_supplement::Provenance {
            archive_url: &archive_file.url,
            archive_sha256: &archive_file.sha256,
            upstream_commit: &upstream.commit,
            compared: &compared_files,
            pinyin: &pinyin_files,
            counts_sha256: &pinned(english_supplement::COUNTS)?.sha256,
        },
    )?;
    std::fs::write(&arguments.out, rendered)
        .with_context(|| format!("writing {}", arguments.out.display()))?;
    for line in english_supplement::report(&supplement) {
        eprintln!("[report] {line}");
    }
    eprintln!(
        "[done] {} words -> {}",
        supplement.words.len(),
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
        dictionary: None,
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
struct HkcancorCounts {
    /// Where pinned sources are downloaded and reused from.
    #[arg(long)]
    cache: PathBuf,
    /// The table to write (msime-dictionary's sources/cantonese/hkcancor-word-counts.txt).
    #[arg(long)]
    out: PathBuf,
    /// Fail instead of downloading a source that is not cached.
    #[arg(long)]
    offline: bool,
    /// The msime checkout the sources lock is read from.
    #[arg(long, default_value_os_t = repository_root())]
    repository: PathBuf,
}

fn build_hkcancor_counts(arguments: &HkcancorCounts) -> Result<()> {
    let root = &arguments.repository;
    let sources = Sources {
        lock: Lock::load(&root.join("resources/dictionary-sources.lock.json"))?,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: arguments.cache.clone(),
        offline: arguments.offline,
        dictionary: None,
    };
    let upstream_commit = sources
        .lock
        .references
        .get(hkcancor::REFERENCE)
        .with_context(|| format!("{} is not pinned in the sources lock", hkcancor::REFERENCE))?
        .commit
        .clone();
    let mut texts = Vec::new();
    for file in sources
        .lock
        .files
        .iter()
        .filter(|file| file.path.starts_with(hkcancor::PREFIX))
    {
        texts.push((file.path.clone(), text::read(&sources.pinned(&file.path)?)?));
    }
    if texts.is_empty() {
        bail!(
            "no file is pinned under {} in the sources lock",
            hkcancor::PREFIX
        );
    }
    let files: Vec<(&str, &str)> = texts
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let counts = hkcancor::count(&files)?;
    let rendered = hkcancor::render(
        &counts,
        &hkcancor::Provenance {
            upstream_commit: &upstream_commit,
        },
    );
    std::fs::write(&arguments.out, rendered)
        .with_context(|| format!("writing {}", arguments.out.display()))?;
    eprintln!(
        "[done] {} words from {} tokens of {} files ({} lines skipped) -> {}",
        counts.words.len(),
        counts.tokens,
        counts.files,
        counts.skipped,
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
    /// A msime-dictionary checkout to read its files from instead of the lock: every file the lock pins from msime-dictionary, and any path under sources/ or custom/ the lock does not pin, is read from <path>/<lock path> without the lock's size and SHA-256 check (the checkout's Git commit pins the content, and the manifest records it); upstream data whose commit msime records (the rime-ice, rime-cantonese, libchewing-data, McBopomofo, rime-stroke, Mozc, libhangul and 98 Wubi files, and the HKCanCor counts and SCOWL supplement generated from upstream commits the lock records) must still match the lock.
    #[arg(long, value_name = "PATH")]
    dictionary: Option<PathBuf>,
}

fn build_languages(arguments: &Languages) -> Result<()> {
    let root = &arguments.repository;
    let sources = Sources {
        lock: Lock::load(&root.join("resources/dictionary-sources.lock.json"))?,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: arguments.cache.clone(),
        offline: arguments.offline,
        dictionary: arguments.dictionary.clone(),
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
    /// A shipped msime-pinyin.db; appended words already in its quanpin tables are rejected.
    #[arg(long)]
    msime_db: Option<PathBuf>,
    /// A shipped msime-english.db; appended English words whose word and display are already in its english_words are rejected.
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

    /// The OALDPE headwords the English stage adds, empty unless the build includes unlicensed inputs.
    fn oaldpe_words(&self) -> Result<std::collections::BTreeSet<String>> {
        if !self.complete {
            return Ok(Default::default());
        }
        english::parse_oaldpe_words(&text::read(&self.sources.pinned(licensing::OALDPE_WORDS)?)?)
    }

    fn scowl_words(&self) -> Result<english::EnglishWords> {
        english::parse_word_list(
            &text::read(&self.sources.pinned(english_supplement::OUTPUT)?)?,
            english_supplement::OUTPUT,
        )
    }

    fn run(&self, stage: Stage) -> Result<String> {
        match stage {
            Stage::Quanpin => {
                let single_chars = self.sources.pinned("sources/pinyin/single-chars.txt")?;
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
                    (
                        vec![self.sources.pinned("sources/pinyin/rime-ice.txt")?],
                        None,
                    )
                };
                let supplement = self
                    .sources
                    .pinned("sources/pinyin/rime-ice-supplement.txt")?;
                let mut phrases = phrases;
                phrases.push(supplement);
                let inputs = msime::QuanpinInputs {
                    single_chars: &single_chars,
                    whitelist,
                    phrases: phrases.iter().map(PathBuf::as_path).collect(),
                };
                let rows = msime::build_quanpin(&mut self.database("msime-pinyin.db")?, &inputs)?;
                Ok(format!("{rows} rows"))
            }
            Stage::PlacesSupplement => {
                let words = msime::parse_word_list(
                    &text::read(&self.sources.pinned(places_supplement::OUTPUT)?)?,
                    places_supplement::OUTPUT,
                )?;
                let counts =
                    msime::apply_custom_words(&mut self.database("msime-pinyin.db")?, &words)?;
                Ok(format!(
                    "{} entries: {} inserted, {} promoted, {} already at or above their weight",
                    words.len(),
                    counts.inserted,
                    counts.promoted,
                    counts.unchanged
                ))
            }
            Stage::CustomWords => {
                let words = msime::parse_custom_words(&text::read(
                    &self.sources.pinned("custom/words.txt")?,
                )?)?;
                let counts =
                    msime::apply_custom_words(&mut self.database("msime-pinyin.db")?, &words)?;
                Ok(format!(
                    "{} entries: {} inserted, {} promoted, {} already at or above their weight",
                    words.len(),
                    counts.inserted,
                    counts.promoted,
                    counts.unchanged
                ))
            }
            Stage::ReadingCorrections => {
                let entries = msime::parse_reading_corrections(&text::read(
                    &self.sources.repository(msime::READING_CORRECTIONS)?,
                )?)?;
                let removed = msime::apply_reading_corrections(
                    &mut self.database("msime-pinyin.db")?,
                    &entries,
                )?;
                Ok(format!("{} entries: {removed} rows removed", entries.len()))
            }
            Stage::Wubi => {
                let (imported, skipped) = msime::build_wubi(
                    &mut self.database("msime-pinyin.db")?,
                    &self.sources.pinned("sources/wubi/wubi86-jidian.txt")?,
                )?;
                Ok(format!("{imported} rows imported, {skipped} skipped"))
            }
            Stage::Wubi98 => {
                let supplement = self.sources.pinned("sources/wubi/wubi98-fcitx.txt")?;
                let (imported, skipped) = msime::build_wubi98_sources(
                    &mut self.database("msime-pinyin.db")?,
                    &self.sources.pinned("sources/wubi/wubi98.txt")?,
                    &[supplement.as_path()],
                )?;
                Ok(format!("{imported} rows imported, {skipped} skipped"))
            }
            Stage::QuickPhrases => {
                let path = self.sources.repository("mix/quick_phrases.txt")?;
                let (imported, skipped) =
                    msime::build_quick_phrases(&mut self.database("msime-pinyin.db")?, &path)?;
                Ok(format!(
                    "{imported} rows imported, {skipped} blank, comment or invalid lines"
                ))
            }
            Stage::English => {
                let oaldpe = self.oaldpe_words()?;
                let mut base = english::parse_base_dict_words(&text::read(
                    &self.sources.pinned("sources/english/rime-ice-en.txt")?,
                )?)?;
                let supplement = english::parse_base_dict_words(&text::read(
                    &self
                        .sources
                        .pinned("sources/english/rime-ice-en-supplement.txt")?,
                )?)?;
                let supplement_added = english::merge_words(&mut base, supplement);
                let scowl_added = english::merge_words(&mut base, self.scowl_words()?);
                // SCOWL's spelling dictionary decides which casing of a word leads (see english::leading_display).
                let attested = english_supplement::letter_forms(&english_supplement::read_package(
                    &std::fs::read(self.sources.pinned(english_supplement::ARCHIVE)?)?,
                )?);
                let counts = english::parse_google_counts(&text::read(
                    &self
                        .sources
                        .pinned("sources/english/google-word-counts.txt")?,
                )?);
                let custom = english::parse_custom_english(&text::read(
                    &self.sources.pinned(english::CUSTOM_ENGLISH)?,
                )?)?;
                let mut database = self.database("msime-english.db")?;
                let rows = english::build_english_words(
                    &mut database,
                    &oaldpe,
                    &base,
                    &counts,
                    &attested,
                    &custom,
                )?;
                english::write_notices(
                    &mut database,
                    &[(
                        english_supplement::NOTICE_SOURCE,
                        english_supplement::COPYRIGHT,
                    )],
                )?;
                std::fs::write(
                    self.out.join(english_supplement::NOTICE_NAME),
                    english_supplement::COPYRIGHT,
                )?;
                Ok(format!(
                    "{} words in {} rows ({} from rime-ice supplement, {} from SCOWL), {} custom rows: {} added, {} replacing a base row",
                    rows.base,
                    rows.base_rows,
                    supplement_added,
                    scowl_added,
                    custom.len(),
                    rows.custom_added,
                    rows.custom_replaced
                ))
            }
            Stage::EnglishGlosses => {
                let msime_path = self.out.join("msime-pinyin.db");
                if !msime_path.is_file() {
                    bail!("english-glosses weights Chinese terms by msime-pinyin.db; build quanpin first");
                }
                // The words only SCOWL brings (scowl-words.txt lacks every rime-ice and custom word; OALDPE headwords were candidates before SCOWL too) stay out of the Chinese-to-English index.
                let oaldpe = self.oaldpe_words()?;
                let reverse_excluded: HashSet<String> = self
                    .scowl_words()?
                    .into_keys()
                    .filter(|word| !oaldpe.contains(word))
                    .collect();
                let mut english_db = self.database("msime-english.db")?;
                let glosses = english::derive_glosses(
                    &self.sources.pinned("ecdict/ecdict.csv")?,
                    &english_db,
                    &sqlite::open(&msime_path)?,
                    &reverse_excluded,
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
                english::apply_custom_translations(
                    &mut self.database("msime-english.db")?,
                    &entries,
                )?;
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
                    others::build_emoji(&mut self.database("msime-others.db")?, &rows, &keys)?;
                Ok(format!("{} emoji, {pinyin_rows} search keys", rows.len()))
            }
            Stage::Kaomoji => {
                let mapping = others::load_keyword_map(&text::read(
                    &self.sources.repository("kaomoji/kaomoji.txt")?,
                )?)?;
                let (rows, entries) = others::build_kaomoji(
                    &mut self.database("msime-others.db")?,
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
                others::build_symbols(&mut self.database("msime-others.db")?, &rows)?;
                Ok(format!("{} symbols", rows.len()))
            }
            Stage::JapaneseModel => {
                let mut dictionaries = Vec::new();
                for name in japanese::DICTIONARY_FILES {
                    dictionaries.push((name, text::read(&self.sources.pinned(name)?)?));
                }
                let id_def = text::read(&self.sources.pinned(japanese::ID_DEF)?)?;
                let mut word_lists = Vec::new();
                for name in japanese::MANUAL_WORDS {
                    word_lists.push((name, text::read(&self.sources.pinned(name)?)?));
                }
                let filter = japanese::DictionaryFilter::parse(
                    japanese::DICTIONARY_FILTER,
                    &text::read(&self.sources.pinned(japanese::DICTIONARY_FILTER)?)?,
                )?;
                let (tokens, added, filtered) = japanese::system_tokens(
                    &dictionaries,
                    &id_def,
                    (
                        japanese::AUX_DICTIONARY,
                        &text::read(&self.sources.pinned(japanese::AUX_DICTIONARY)?)?,
                    ),
                    &word_lists,
                    &filter,
                )?;
                let (size, costs) = japanese::read_connection(
                    &id_def,
                    &text::read(&self.sources.pinned(japanese::CONNECTION)?)?,
                )?;
                japanese::write_model(
                    &self.out.join("msime-japanese.dat"),
                    &japanese::pack(&tokens, size, &costs)?,
                )?;
                std::fs::copy(
                    self.sources.pinned(japanese::NOTICE)?,
                    self.out.join(japanese::NOTICE_NAME),
                )?;
                std::fs::copy(
                    self.sources.pinned(japanese::LICENSE)?,
                    self.out.join(japanese::LICENSE_NAME),
                )?;
                Ok(format!(
                    "{} tokens ({added} added from the aux dictionary and manual word lists, {filtered} base lines filtered), {size} context ids",
                    tokens.len()
                ))
            }
            Stage::Ngram => {
                let msime_path = self.out.join("msime-pinyin.db");
                if !msime_path.is_file() {
                    bail!("ngram segments with msime-pinyin.db's vocabulary; build quanpin first");
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
                    self.out.join("msime-trigram.bin"),
                    ngram::pack(&vocabulary, &counts, 3)?,
                )?;
                std::fs::write(
                    self.out.join("msime-bigram.bin"),
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
    if let Some(Command::PlacesSupplement(supplement)) = &arguments.command {
        return build_places_supplement(supplement);
    }
    if let Some(Command::EnglishSupplement(supplement)) = &arguments.command {
        return build_english_supplement(supplement);
    }
    if let Some(Command::Hanja(hanja)) = &arguments.command {
        return build_hanja(hanja);
    }
    if let Some(Command::HkcancorCounts(counts)) = &arguments.command {
        return build_hkcancor_counts(counts);
    }
    if let Some(Command::Languages(languages)) = &arguments.command {
        return build_languages(languages);
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
            dictionary: arguments.dictionary.clone(),
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

    product::split_wubi_database(&build.out)?;

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
    product::write_manifest(
        &build.out,
        root,
        build.sources.dictionary.as_deref(),
        &build.sources.lock,
        complete,
    )?;
    eprintln!(
        "[product] verified; wrote {} and msime-SHA256SUMS.txt",
        product::MANIFEST
    );
    Ok(())
}
