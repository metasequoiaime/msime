//! `msime-dict-build`：构建 msime 发布的词库产物（见 `resources/desktop-dictionary.lock.json`），输入是 `resources/dictionary-sources.lock.json` 固定的第三方文件、`--dictionary` 给出的 msime-dictionary checkout，以及 `resources/dictionary-sources/` 里人工维护的文件。
//!
//! ```text
//! msime-dict-build --cache <dir> --out <dir> --dictionary <msime-dictionary checkout>               全部阶段，然后写 manifest
//! msime-dict-build --cache <dir> --out <dir> --dictionary <msime-dictionary checkout> --skip ngram  跳过语料统计的快速本地构建
//! msime-dict-build --cache <dir> --out <dir> --only emoji                                           只跑不读 msime-dictionary 数据的阶段，不写 manifest 并删掉旧的
//! msime-dict-build --list
//! msime-dict-build places --cache <dir> [--out <places.tsv>] [--offline]
//! msime-dict-build places-supplement --dictionary <msime-dictionary checkout> --cache <dir> --out <places.txt> [--offline]
//! msime-dict-build english-supplement --dictionary <msime-dictionary checkout> --cache <dir> --out <scowl-words.txt> [--offline]
//! msime-dict-build wubi86-supplement --dictionary <msime-dictionary checkout> --cache <dir> --out <wubi86-supplement.txt> [--offline]
//! msime-dict-build wubi98-supplement --dictionary <msime-dictionary checkout> --cache <dir> --out <wubi98-supplement.txt> [--offline]
//! msime-dict-build wubixsj-supplement --dictionary <msime-dictionary checkout> --cache <dir> --out <wubixsj-supplement.txt> [--offline]
//! msime-dict-build wubi86-helpcode --dictionary <msime-dictionary checkout> --out <wubi86_helpcode.txt>
//! msime-dict-build hanja --dictionary <msime-dictionary checkout> --cache <dir> [--out <hanja.tsv>] [--offline]
//! msime-dict-build hkcancor-counts --cache <dir> --out <hkcancor-word-counts.txt> [--offline]
//! msime-dict-build languages --dictionary <msime-dictionary checkout> --cache <dir> [--out <dir>] [--offline]
//! msime-dict-build syllable-frequency --pinyin <msime-pinyin.db> [--out <syllable_frequency.tsv>]
//! msime-dict-build web --pinyin <msime-pinyin.db> --wubi <msime-wubi.db> --out-dir <dir> [--keep-multi 200000]
//! msime-dict-build check-words [--base <words.txt> --head <words.txt>] [--translations-base <translations.txt> --translations-head <translations.txt>] [--english-base <english.txt> --english-head <english.txt>] [--cantonese-base <cantonese.txt> --cantonese-head <cantonese.txt> --cantonese-characters <jyut6ping3.chars.dict.yaml>] [--msime-db <msime-pinyin.db>] [--english-db <msime-english.db>] [--cantonese-db <msime-cantonese.db>] [--pinyin-vetoes <curation/pinyin-vetoes.txt>] [--english-vetoes <curation/english-vetoes.txt>] [--json <report.json>] [--markdown <summary.md>]
//! msime-dict-build cleanse-report --dictionary <msime-dictionary checkout> --out <dir> [--upstream <rime-ice checkout>] [--ratio 5] [--reference <file>...] [--reference-min 100] [--reference-scale <factor>] [--nonstandard-min 1000]
//! ```

mod cantonese;
mod cantonese_curation;
mod check_words;
mod cleanse_report;
mod curation;
mod custom_ceiling;
mod english;
mod english_supplement;
mod hanja;
mod hkcancor;
mod japanese;
mod language_verify;
mod language_writer;
mod languages;
mod licensing;
mod msime;
mod ngram;
mod others;
mod pinyin;
mod places;
mod places_supplement;
mod private_file;
mod product;
mod reweight;
mod rime_ice_supplement;
mod sources;
mod sqlite;
mod stroke;
mod syllable_frequency;
mod text;
mod web;
mod wubi86_helpcode;
mod wubi86_supplement;
mod wubi86_table;
mod wubi98_supplement;
mod wubi_supplement;
mod wubixsj_supplement;
mod zhuyin;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::sources::{sha256_file, Dictionary, Lock, Sources};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Stage {
    /// Quanpin tables in msime-pinyin.db (tbl_{1..7,others}_{initial})
    Quanpin,
    /// 按 --dictionary checkout 登记的万象词库（sources/pinyin/wanxiang/）重算全拼表里与万象共有的行的权重；curation/pinyin-reweight.txt 打开第二步。checkout 没有登记 rime-wanxiang 时什么也不做。
    Reweight,
    /// 从 --dictionary checkout 读取的 sources/pinyin/places.txt 并入全拼表，只升不降
    PlacesSupplement,
    /// 从 --dictionary checkout 读取的 custom/words.txt 并入全拼表；新插入的词不超过同全拼、同九键码上权重不低于 1000 的常用词减 1
    CustomWords,
    /// Wrong readings listed in resources/dictionary-sources/pinyin-reading-corrections.txt removed from the quanpin tables
    ReadingCorrections,
    /// 从全拼表删掉 --dictionary checkout 的 curation/pinyin-vetoes.txt 否决的词（整词）或读音（词+全拼）；没有这个文件时什么也不做。
    WordVetoes,
    /// 把 --dictionary checkout 的 curation/pinyin-weight-overrides.txt 里每个读音的权重设成给定的值，可降可升，不能覆盖 custom/words.txt 从已有行提权的读音；没有这个文件时什么也不做。
    WeightOverrides,
    /// 把 --dictionary checkout 的 curation/offensive-words.txt 写进 msime-pinyin.db 的 sentence_excluded 表，引擎拼整句时按它拦粗俗词；没有这个文件时不建这张表，引擎用内嵌表。
    SentenceExcluded,
    /// 86 五笔表：读取固定原表，再从本次 --dictionary 的拼音、人工词条与另一版原表派生补充数据。
    Wubi,
    /// 98 五笔表：读取两张固定原表，再从本次 --dictionary 的拼音、人工词条与 86 原表派生补充数据。
    Wubi98,
    /// 新世纪五笔表：只取固定原表的基本区单字，词组全部从本次 --dictionary 的 86、98 原表、拼音与人工词条按新世纪单字全码派生；--dictionary 还没收 sources/wubi/wubixsj-supplement.txt 时跳过，不建这张表。
    #[value(name = "wubixsj")]
    WubiXsj,
    /// Quick phrase table in msime-pinyin.db, then msime-pinyin.db's planner statistics
    QuickPhrases,
    /// msime-english.db 的 english_words 表，来自 rime-ice 英文词表和 sources/english/scowl-words.txt，再加 custom/english.txt（都从 --dictionary checkout 读取）
    English,
    /// Bidirectional gloss tables in msime-english.db, derived from ECDICT (reads msime-pinyin.db)
    EnglishGlosses,
    /// 从 --dictionary checkout 读取的 custom/translations.txt 覆盖释义表
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
const STAGES: [Stage; 20] = [
    Stage::Quanpin,
    Stage::Reweight,
    Stage::PlacesSupplement,
    Stage::CustomWords,
    Stage::ReadingCorrections,
    Stage::WordVetoes,
    Stage::WeightOverrides,
    Stage::SentenceExcluded,
    Stage::Wubi,
    Stage::Wubi98,
    Stage::WubiXsj,
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
    /// msime-dictionary checkout，`sources/` 和 `custom/` 下的每个路径都从这里读（msime 的锁文件不固定其中任何文件）。它的 Git 提交固定内容，manifest 会记下这个提交；msime 认定为上游数据的文件必须与 checkout 的 `upstream.lock.json` 一致，记录里的提交必须等于锁文件的 references。不传时，读 msime-dictionary 数据的阶段会失败，也不写 manifest，`--out` 里旧的 manifest 和校验和文件会被删掉。
    #[arg(long, value_name = "PATH")]
    dictionary: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// 从干净的 rime-ice 检出生成中英文补充表及来源报告；不修改任何已发布输入或锁文件。
    RimeIceSupplement(RimeIceSupplement),
    /// Check a change to the shared custom dictionary's words.txt, translations.txt, english.txt and cantonese.txt; pass a base/head pair for each file to check. Exits 0 when every appended line is accepted, 1 when anything is rejected (the reports are still written), 2 when the check cannot run.
    CheckWords(Box<CheckWords>),
    /// Write the `@` mode place table (crates/engine/src/local/places.tsv) from the administrative divisions pinned under places/ in the sources lock.
    Places(Places),
    /// 写出 msime-dictionary 的 sources/pinyin/places.txt：msime-dictionary 的拼音词库没有、或排名低于所在层级下限的行政区划地名（全称和简称），区划数据取自锁文件在 places/ 下固定的文件。
    PlacesSupplement(PlacesSupplement),
    /// 写出 msime-dictionary 的 sources/english/scowl-words.txt：SCOWL 60 级 Aspell 词典（美式拼写加英式 -ise 拼写，固定在 scowl/ 下）里 msime-dictionary 的 rime-ice 英文词表（含被注释掉的条目）和 custom/english.txt 都没有的词，去掉蔑称、只有大写形式且拼出某个全拼的名字，以及没有 Google 词频的词（见 english_supplement.rs）。
    EnglishSupplement(EnglishSupplement),
    /// 写出 msime-dictionary 的 sources/wubi/wubi86-supplement.txt：msime-dictionary 的 86 码表（sources/wubi/wubi86-jidian.txt）缺少、而 98 码表列有或 rime-ice 主表及补充表里权重不低于 100 的多字词，或 custom/words.txt、places.txt 的词语，限两个及以上汉字；编码按 86 版词组规则由 86 码表的单字编码推出，权重低于 86 码表同一编码下的行（见 wubi86_supplement.rs）。
    Wubi86Supplement(Wubi86Supplement),
    /// 写出 msime-dictionary 的 sources/wubi/wubi98-supplement.txt：两张 98 码表（sources/wubi/wubi98.txt、sources/wubi/wubi98-fcitx.txt）缺少、而 86 码表列有或 rime-ice 主表及补充表里权重不低于 100 的多字词，或 custom/words.txt、places.txt 的词语，限两个及以上基本区汉字；编码按词组规则由 98 码表的单字编码推出，权重低于 98 码表同一编码下的行（见 wubi98_supplement.rs）。
    Wubi98Supplement(Wubi98Supplement),
    /// 写出 msime-dictionary 的 sources/wubi/wubixsj-supplement.txt：新世纪五笔的全部词组。候选是 86、98 码表里的多字词、rime-ice 主表及补充表里权重不低于 100 的多字词，以及 custom/words.txt、places.txt 的词语，限两个及以上基本区汉字；编码按词组规则由新世纪码表（sources/wubi/wubixsj-byujiang.txt）的单字编码推出；86 码表里有的词沿用它在那里的权重，其余的权重低于同一编码下的单字（见 wubixsj_supplement.rs）。
    #[command(name = "wubixsj-supplement")]
    WubiXsjSupplement(WubiXsjSupplement),
    /// 写出 msime 的 resources/helpcodes/wubi86_helpcode.txt：辅助码方案 wubi86 的码表，每个基本区汉字取它在 msime-dictionary 的 86 码表（sources/wubi/wubi86-jidian.txt）里全码的前两码（见 wubi86_helpcode.rs）。
    Wubi86Helpcode(Wubi86Helpcode),
    /// 从 --dictionary checkout 的 libhangul `sources/korean/hanja.txt` 生成韩文 Hanja 表（crates/engine/src/korean/hanja.tsv），同一 checkout 的 OpenCC `sources/opencc/STCharacters.txt` 决定哪些字只作简体字形。
    Hanja(Hanja),
    /// Write msime-dictionary's sources/cantonese/hkcancor-word-counts.txt: how often each word of two or more Han characters occurs in the HKCanCor transcriptions pinned under hkcancor/ in the sources lock.
    HkcancorCounts(HkcancorCounts),
    /// 写出随资源集一起发布的词库（msime-cantonese.db、msime-zhuyin.db、msime-stroke.db）及其许可证文本和校验和，数据来自 --dictionary checkout 的 sources/cantonese/、sources/zhuyin/、sources/stroke/stroke.dict.yaml 和 sources/pinyin/single-chars.txt。
    Languages(Languages),
    /// 从词库 release 的 msime-pinyin.db 和 msime-wubi.db 裁出网页内置输入法用的 msime-pinyin.db（全部单字加按权重排名前 N 的多字词，不含五笔）和 msime-wubi86.db（只含 86 五笔），给了 --japanese 时再从 msime-japanese.dat 裁出成本最低的 N 条词，都逐字节可复现。
    Web(WebArgs),
    /// 给 msime-dictionary 的清洗出候选清单（本地权重与上游 rime-ice 相差过大的行、占位权重、压过同键常用词的碎片词、压过同键规范读音的非规范读音、含粗俗词的长词条和五笔原表里的同类词、压过同键常用词的人工词条、来源对不上上游的行），每类一个 TSV，不改任何库和源文件；维护者审定后再写进 curation/ 的否决表和权重覆盖表（见 cleanse_report.rs）。
    CleanseReport(CleanseReport),
    /// 从词库 release 的 msime-pinyin.db 写出九宫格切分用的音节频度表（crates/engine/src/nine_key/syllable_frequency.tsv）：单字行的权重，加上音节在多字词里每出现一次计一次的词的权重。
    SyllableFrequency(SyllableFrequency),
}

#[derive(Args)]
struct CleanseReport {
    /// 读取 `sources/`、`custom/` 和 `curation/` 的 msime-dictionary checkout（按其 `upstream.lock.json` 校验）。
    #[arg(long, value_name = "PATH")]
    dictionary: PathBuf,
    /// 写候选清单的目录。
    #[arg(long)]
    out: PathBuf,
    /// 上游 rime-ice 检出：给了才出 weight-disagreement.tsv，比的是它的 cn_dicts/base.dict.yaml 和 cn_dicts/ext.dict.yaml。
    #[arg(long, value_name = "PATH")]
    upstream: Option<PathBuf>,
    /// 本地权重与上游相差几倍以上才列出。
    #[arg(long, default_value_t = 5.0)]
    ratio: f64,
    /// 参照频率表（Rime 的 .dict.yaml，或 `词<TAB>读音[<TAB>权重]`），可以给多份，同词同音取最大权重。给了才出 fragment-over-common.tsv，占位权重也按它排除参照表收了的词。
    #[arg(long, value_name = "PATH")]
    reference: Vec<PathBuf>,
    /// 参照权重不低于这个值的词算常用词。
    #[arg(long, default_value_t = 100)]
    reference_min: i64,
    /// weight-disagreement.tsv 里，权重恰好等于参照权重乘这个系数（四舍五入）的行算作有意按参照表改写过的，不列出。
    #[arg(long)]
    reference_scale: Option<f64>,
    /// nonstandard-readings.tsv 里，读音是参照表收了的、本地权重不低于这个值的词才算被压过的规范读音。
    #[arg(long, default_value_t = 1000)]
    nonstandard_min: i64,
    /// 读取来源锁文件的 msime checkout；表头记下它的提交。
    #[arg(long, default_value_os_t = repository_root())]
    repository: PathBuf,
}

fn build_cleanse_report(arguments: &CleanseReport) -> Result<()> {
    let root = &arguments.repository;
    let lock = Lock::load(&root.join("resources/dictionary-sources.lock.json"))?;
    let dictionary = Dictionary::open(arguments.dictionary.clone(), &lock)?;
    // 只读 msime-dictionary 里的文件，不经过下载缓存。
    let sources = Sources {
        lock,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: PathBuf::new(),
        offline: true,
        dictionary: Some(dictionary),
    };
    let written = cleanse_report::run(
        &cleanse_report::Options {
            repository: root,
            dictionary: &arguments.dictionary,
            out: &arguments.out,
            upstream: arguments.upstream.as_deref(),
            ratio: arguments.ratio,
            references: &arguments.reference,
            reference_min: arguments.reference_min,
            reference_scale: arguments.reference_scale,
            nonstandard_min: arguments.nonstandard_min,
        },
        &sources,
    )?;
    for (path, count) in written {
        eprintln!("[done] {count} candidates -> {}", path.display());
    }
    Ok(())
}

#[derive(Args)]
struct SyllableFrequency {
    /// 词库 release 的 msime-pinyin.db，只读打开。
    #[arg(long)]
    pinyin: PathBuf,
    /// 写出的表；默认是引擎内嵌的那份。
    #[arg(long, default_value_os_t = repository_root().join("crates/engine/src/nine_key/syllable_frequency.tsv"))]
    out: PathBuf,
}

fn build_syllable_frequency(arguments: &SyllableFrequency) -> Result<()> {
    let table = syllable_frequency::build(&arguments.pinyin)?;
    std::fs::write(&arguments.out, &table)
        .with_context(|| format!("write {}", arguments.out.display()))?;
    eprintln!(
        "[done] {} syllables -> {}",
        table.lines().filter(|line| !line.starts_with('#')).count(),
        arguments.out.display()
    );
    Ok(())
}

#[derive(Args)]
struct RimeIceSupplement {
    #[arg(long)]
    upstream: PathBuf,
    #[arg(long)]
    dictionary: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value_os_t = repository_root())]
    repository: PathBuf,
}

#[derive(Args)]
struct WebArgs {
    /// 词库 release 的 msime-pinyin.db（全拼表与快捷短语），只读打开。
    #[arg(long)]
    pinyin: PathBuf,
    /// 词库 release 的 msime-wubi.db（wubi86、wubi98，以及较新发布里的 wubixsj），只读打开。
    #[arg(long)]
    wubi: PathBuf,
    /// 写出 msime-pinyin.db 和 msime-wubi86.db 的目录，不存在时创建；同名文件会被覆盖。
    #[arg(long)]
    out_dir: PathBuf,
    /// msime-pinyin.db 在全部多字表里保留的行数（单字表总是全部保留）。
    #[arg(long, default_value_t = web::DEFAULT_KEEP_MULTI)]
    keep_multi: usize,
    /// 词库 release 的 msime-japanese.dat；给了就在 --out-dir 里再写出裁剪后的 msime-japanese.dat。
    #[arg(long)]
    japanese: Option<PathBuf>,
    /// 网页日语模型保留的词条数，按词条成本从低到高。
    #[arg(long, default_value_t = web::DEFAULT_KEEP_JAPANESE)]
    keep_japanese: usize,
}

fn build_web(arguments: &WebArgs) -> Result<()> {
    let inputs = web::Inputs {
        pinyin: &arguments.pinyin,
        wubi: &arguments.wubi,
    };
    for summary in web::build(inputs, &arguments.out_dir, arguments.keep_multi)? {
        eprintln!("[done] {summary}");
    }
    if let Some(japanese) = &arguments.japanese {
        let summary = web::build_japanese(japanese, &arguments.out_dir, arguments.keep_japanese)?;
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
    /// 读取 `sources/` 和 `custom/` 文件的 msime-dictionary checkout（按其 `upstream.lock.json` 校验）。
    #[arg(long, value_name = "PATH")]
    dictionary: PathBuf,
}

fn build_places_supplement(arguments: &PlacesSupplement) -> Result<()> {
    let root = &arguments.repository;
    let lock = Lock::load(&root.join("resources/dictionary-sources.lock.json"))?;
    let dictionary = Dictionary::open(arguments.dictionary.clone(), &lock)?;
    let sources = Sources {
        lock,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: arguments.cache.clone(),
        offline: arguments.offline,
        dictionary: Some(dictionary),
    };
    let read = |path: &str| -> Result<String> { text::read(&sources.pinned(path)?) };
    let curation = curation::PinyinCuration::load(&sources)?;
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
        curation: &curation,
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
    // 表头里的 SHA-256 按实际读到的字节计算。
    let digest = |path: &str| -> Result<String> { sha256_file(&sources.pinned(path)?) };
    let base_sha256 = digest(places_supplement::BASE)?;
    let rime_ice_supplement_sha256 = digest(places_supplement::RIME_ICE_SUPPLEMENT)?;
    let single_chars_sha256 = digest(places_supplement::SINGLE_CHARS)?;
    let curation_digests = curation::present_digests(&sources, &curation::PINYIN_TABLES)?;
    let mut compared = vec![
        (places_supplement::BASE, base_sha256.as_str()),
        (
            places_supplement::RIME_ICE_SUPPLEMENT,
            rime_ice_supplement_sha256.as_str(),
        ),
    ];
    compared.extend(
        curation_digests
            .iter()
            .map(|(path, sha256)| (*path, sha256.as_str())),
    );
    let generator_commit = product::builder_commit(root)?;
    let rendered = places_supplement::render(
        &supplement,
        &places_supplement::Provenance {
            upstream_commit,
            compared: &compared,
            single_chars: (
                places_supplement::SINGLE_CHARS,
                single_chars_sha256.as_str(),
            ),
            generator_commit: &generator_commit,
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
    /// 读取 `sources/` 和 `custom/` 文件的 msime-dictionary checkout（按其 `upstream.lock.json` 校验）。
    #[arg(long, value_name = "PATH")]
    dictionary: PathBuf,
}

fn build_english_supplement(arguments: &EnglishSupplement) -> Result<()> {
    let root = &arguments.repository;
    let lock = Lock::load(&root.join("resources/dictionary-sources.lock.json"))?;
    let dictionary = Dictionary::open(arguments.dictionary.clone(), &lock)?;
    let sources = Sources {
        lock,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: arguments.cache.clone(),
        offline: arguments.offline,
        dictionary: Some(dictionary),
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
    let archive = crate::sources::read_private(&sources.pinned(english_supplement::ARCHIVE)?)?;
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
    // 表头里的 SHA-256 按实际读到的字节计算。
    let with_sha256 = |paths: &[&'static str]| {
        paths
            .iter()
            .map(|path| Ok((*path, sha256_file(&sources.pinned(path)?)?)))
            .collect::<Result<Vec<(&'static str, String)>>>()
    };
    let compared_files = with_sha256(&english_supplement::COMPARED)?;
    let pinyin_files = with_sha256(&english_supplement::PINYIN)?;
    let compared_refs: Vec<(&str, &str)> = compared_files
        .iter()
        .map(|(path, sha256)| (*path, sha256.as_str()))
        .collect();
    let pinyin_refs: Vec<(&str, &str)> = pinyin_files
        .iter()
        .map(|(path, sha256)| (*path, sha256.as_str()))
        .collect();
    let counts_sha256 = sha256_file(&sources.pinned(english_supplement::COUNTS)?)?;
    let generator_commit = product::builder_commit(root)?;
    let rendered = english_supplement::render(
        &supplement,
        &english_supplement::Provenance {
            archive_url: &archive_file.url,
            archive_sha256: &archive_file.sha256,
            upstream_commit: &upstream.commit,
            compared: &compared_refs,
            pinyin: &pinyin_refs,
            counts_sha256: &counts_sha256,
            generator_commit: &generator_commit,
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
struct Wubi86Supplement {
    /// Where pinned sources are downloaded and reused from.
    #[arg(long)]
    cache: PathBuf,
    /// The supplement to write (msime-dictionary's sources/wubi/wubi86-supplement.txt).
    #[arg(long)]
    out: PathBuf,
    /// Fail instead of downloading a source that is not cached.
    #[arg(long)]
    offline: bool,
    /// The msime checkout the sources lock is read from.
    #[arg(long, default_value_os_t = repository_root())]
    repository: PathBuf,
    /// 读取 `sources/` 和 `custom/` 文件的 msime-dictionary checkout（按其 `upstream.lock.json` 校验）。
    #[arg(long, value_name = "PATH")]
    dictionary: PathBuf,
}

fn build_wubi86_supplement(arguments: &Wubi86Supplement) -> Result<()> {
    let root = &arguments.repository;
    let lock = Lock::load(&root.join("resources/dictionary-sources.lock.json"))?;
    let dictionary = Dictionary::open(arguments.dictionary.clone(), &lock)?;
    let sources = Sources {
        lock,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: arguments.cache.clone(),
        offline: arguments.offline,
        dictionary: Some(dictionary),
    };
    let read = |path: &str| -> Result<String> { text::read(&sources.pinned(path)?) };
    let curation = curation::PinyinCuration::load(&sources)?;
    let wubi98_path = sources.pinned(wubi86_supplement::WUBI98)?;
    let wubi98 = msime::decode_utf16le(&crate::sources::read_private(&wubi98_path)?)
        .with_context(|| format!("decoding {}", wubi98_path.display()))?;
    let wubi86 = wubi86_table::Files::locate(&sources)?;
    let table = wubi86.read()?;
    let (wubi98_fcitx, base) = (
        read(wubi86_supplement::WUBI98_FCITX)?,
        read(wubi86_supplement::BASE)?,
    );
    let supplement = wubi86_supplement::build(&wubi86_supplement::Inputs {
        jidian: &table.jidian,
        rime_wubi: table.rime_wubi.as_deref(),
        wubi98: &wubi98,
        wubi98_fcitx: &wubi98_fcitx,
        base: &base,
        pinyin_supplement: &read(wubi86_supplement::PINYIN_SUPPLEMENT)?,
        places: &read(wubi86_supplement::PLACES)?,
        custom: &read(wubi86_supplement::CUSTOM)?,
        curation: &curation,
    })?;
    // 表头里的 SHA-256 按实际读到的字节计算。
    let digests = wubi86
        .digests()?
        .into_iter()
        .map(Ok)
        .chain(
            [
                wubi86_supplement::WUBI98,
                wubi86_supplement::WUBI98_FCITX,
                wubi86_supplement::BASE,
                wubi86_supplement::PINYIN_SUPPLEMENT,
                wubi86_supplement::PLACES,
                wubi86_supplement::CUSTOM,
            ]
            .into_iter()
            .map(|path| Ok((path, sha256_file(&sources.pinned(path)?)?))),
        )
        .chain(
            curation::present_digests(&sources, &curation::PINYIN_TABLES)?
                .into_iter()
                .map(Ok),
        )
        .collect::<Result<Vec<_>>>()?;
    let inputs: Vec<(&str, &str)> = digests
        .iter()
        .map(|(path, sha256)| (*path, sha256.as_str()))
        .collect();
    let generator_commit = product::builder_commit(root)?;
    let rendered = wubi86_supplement::render(
        &supplement,
        &wubi86_supplement::Provenance {
            inputs: &inputs,
            generator_commit: &generator_commit,
            upstreams: &wubi_supplement::upstreams(
                &sources.lock,
                &[wubi86.references(), &wubi_supplement::UPSTREAMS].concat(),
            ),
        },
    );
    std::fs::write(&arguments.out, rendered)
        .with_context(|| format!("writing {}", arguments.out.display()))?;
    for line in wubi86_supplement::report(&supplement) {
        eprintln!("[report] {line}");
    }
    eprintln!(
        "[done] {} words -> {}",
        supplement.entries.len(),
        arguments.out.display()
    );
    Ok(())
}

#[derive(Args)]
struct Wubi86Helpcode {
    /// 要写出的码表（msime 的 `resources/helpcodes/wubi86_helpcode.txt`）。
    #[arg(long)]
    out: PathBuf,
    /// 读取来源锁文件（`resources/dictionary-sources.lock.json`）的 msime checkout。
    #[arg(long, default_value_os_t = repository_root())]
    repository: PathBuf,
    /// 读取 86 原表（`sources/wubi/wubi86-jidian.txt`，拆分之后连同 `sources/wubi/wubi86-rime-wubi.txt`）的 msime-dictionary checkout（新布局按其 `upstream.lock.json` 校验）。
    #[arg(long, value_name = "PATH")]
    dictionary: PathBuf,
}

fn build_wubi86_helpcode(arguments: &Wubi86Helpcode) -> Result<()> {
    let root = &arguments.repository;
    let lock = Lock::load(&root.join("resources/dictionary-sources.lock.json"))?;
    let dictionary = Dictionary::open(arguments.dictionary.clone(), &lock)?;
    // 只读 msime-dictionary 里的文件，不经过下载缓存，所以没有 --cache，也不会下载任何东西。
    let sources = Sources {
        lock,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: PathBuf::new(),
        offline: true,
        dictionary: Some(dictionary),
    };
    let wubi86 = wubi86_table::Files::locate(&sources)?;
    let table = wubi86.read()?;
    let helpcode = wubi86_helpcode::build(&table.jidian, table.rime_wubi.as_deref())?;
    // 表头里的 SHA-256 按实际读到的字节计算。
    let digests = wubi86.digests()?;
    let inputs: Vec<(&str, &str)> = digests
        .iter()
        .map(|(path, sha256)| (*path, sha256.as_str()))
        .collect();
    let rendered = wubi86_helpcode::render(&helpcode, &inputs);
    std::fs::write(&arguments.out, rendered)
        .with_context(|| format!("writing {}", arguments.out.display()))?;
    for line in wubi86_helpcode::report(&helpcode) {
        eprintln!("[report] {line}");
    }
    eprintln!(
        "[done] {} characters -> {}",
        helpcode.codes.len(),
        arguments.out.display()
    );
    Ok(())
}

#[derive(Args)]
struct Wubi98Supplement {
    /// Where pinned sources are downloaded and reused from.
    #[arg(long)]
    cache: PathBuf,
    /// The supplement to write (msime-dictionary's sources/wubi/wubi98-supplement.txt).
    #[arg(long)]
    out: PathBuf,
    /// Fail instead of downloading a source that is not cached.
    #[arg(long)]
    offline: bool,
    /// The msime checkout the sources lock is read from.
    #[arg(long, default_value_os_t = repository_root())]
    repository: PathBuf,
    /// 读取 `sources/` 和 `custom/` 文件的 msime-dictionary checkout（按其 `upstream.lock.json` 校验）。
    #[arg(long, value_name = "PATH")]
    dictionary: PathBuf,
}

fn build_wubi98_supplement(arguments: &Wubi98Supplement) -> Result<()> {
    let root = &arguments.repository;
    let lock = Lock::load(&root.join("resources/dictionary-sources.lock.json"))?;
    let dictionary = Dictionary::open(arguments.dictionary.clone(), &lock)?;
    let sources = Sources {
        lock,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: arguments.cache.clone(),
        offline: arguments.offline,
        dictionary: Some(dictionary),
    };
    let read = |path: &str| -> Result<String> { text::read(&sources.pinned(path)?) };
    let curation = curation::PinyinCuration::load(&sources)?;
    let wubi98_path = sources.pinned(wubi98_supplement::WUBI98)?;
    let wubi98 = msime::decode_utf16le(&crate::sources::read_private(&wubi98_path)?)
        .with_context(|| format!("decoding {}", wubi98_path.display()))?;
    let wubi86 = wubi86_table::Files::locate(&sources)?;
    let table = wubi86.read()?;
    let (wubi98_fcitx, base) = (
        read(wubi98_supplement::WUBI98_FCITX)?,
        read(wubi98_supplement::BASE)?,
    );
    let supplement = wubi98_supplement::build(&wubi98_supplement::Inputs {
        wubi98: &wubi98,
        wubi98_fcitx: &wubi98_fcitx,
        jidian: &table.jidian,
        rime_wubi: table.rime_wubi.as_deref(),
        base: &base,
        pinyin_supplement: &read(wubi98_supplement::PINYIN_SUPPLEMENT)?,
        places: &read(wubi98_supplement::PLACES)?,
        custom: &read(wubi98_supplement::CUSTOM)?,
        curation: &curation,
    })?;
    // 表头里的 SHA-256 按实际读到的字节计算。
    let pinned_digest = |path: &'static str| Ok((path, sha256_file(&sources.pinned(path)?)?));
    let digests = [wubi98_supplement::WUBI98, wubi98_supplement::WUBI98_FCITX]
        .into_iter()
        .map(pinned_digest)
        .chain(wubi86.digests()?.into_iter().map(Ok))
        .chain(
            [
                wubi98_supplement::BASE,
                wubi98_supplement::PINYIN_SUPPLEMENT,
                wubi98_supplement::PLACES,
                wubi98_supplement::CUSTOM,
            ]
            .into_iter()
            .map(pinned_digest),
        )
        .chain(
            curation::present_digests(&sources, &curation::PINYIN_TABLES)?
                .into_iter()
                .map(Ok),
        )
        .collect::<Result<Vec<_>>>()?;
    let inputs: Vec<(&str, &str)> = digests
        .iter()
        .map(|(path, sha256)| (*path, sha256.as_str()))
        .collect();
    let generator_commit = product::builder_commit(root)?;
    let rendered = wubi98_supplement::render(
        &supplement,
        &wubi98_supplement::Provenance {
            inputs: &inputs,
            generator_commit: &generator_commit,
            upstreams: &wubi_supplement::upstreams(
                &sources.lock,
                &[wubi86.references(), &wubi_supplement::UPSTREAMS].concat(),
            ),
        },
    );
    std::fs::write(&arguments.out, rendered)
        .with_context(|| format!("writing {}", arguments.out.display()))?;
    for line in wubi98_supplement::report(&supplement) {
        eprintln!("[report] {line}");
    }
    eprintln!(
        "[done] {} words -> {}",
        supplement.entries.len(),
        arguments.out.display()
    );
    Ok(())
}

#[derive(Args)]
struct WubiXsjSupplement {
    /// Where pinned sources are downloaded and reused from.
    #[arg(long)]
    cache: PathBuf,
    /// The table to write (msime-dictionary's sources/wubi/wubixsj-supplement.txt).
    #[arg(long)]
    out: PathBuf,
    /// Fail instead of downloading a source that is not cached.
    #[arg(long)]
    offline: bool,
    /// The msime checkout the sources lock is read from.
    #[arg(long, default_value_os_t = repository_root())]
    repository: PathBuf,
    /// 读取 `sources/` 和 `custom/` 文件的 msime-dictionary checkout（按其 `upstream.lock.json` 校验）。
    #[arg(long, value_name = "PATH")]
    dictionary: PathBuf,
}

fn build_wubixsj_supplement(arguments: &WubiXsjSupplement) -> Result<()> {
    let root = &arguments.repository;
    let lock = Lock::load(&root.join("resources/dictionary-sources.lock.json"))?;
    let dictionary = Dictionary::open(arguments.dictionary.clone(), &lock)?;
    let sources = Sources {
        lock,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: arguments.cache.clone(),
        offline: arguments.offline,
        dictionary: Some(dictionary),
    };
    let read = |path: &str| -> Result<String> { text::read(&sources.pinned(path)?) };
    let curation = curation::PinyinCuration::load(&sources)?;
    let wubi98_path = sources.pinned(wubixsj_supplement::WUBI98)?;
    let wubi98 = msime::decode_utf16le(&crate::sources::read_private(&wubi98_path)?)
        .with_context(|| format!("decoding {}", wubi98_path.display()))?;
    let wubi86 = wubi86_table::Files::locate(&sources)?;
    let table = wubi86.read()?;
    let (wubixsj, wubi98_fcitx, base) = (
        read(wubixsj_supplement::WUBIXSJ)?,
        read(wubixsj_supplement::WUBI98_FCITX)?,
        read(wubixsj_supplement::BASE)?,
    );
    let supplement = wubixsj_supplement::build(&wubixsj_supplement::Inputs {
        wubixsj: &wubixsj,
        jidian: &table.jidian,
        rime_wubi: table.rime_wubi.as_deref(),
        wubi98: &wubi98,
        wubi98_fcitx: &wubi98_fcitx,
        base: &base,
        pinyin_supplement: &read(wubixsj_supplement::PINYIN_SUPPLEMENT)?,
        places: &read(wubixsj_supplement::PLACES)?,
        custom: &read(wubixsj_supplement::CUSTOM)?,
        curation: &curation,
    })?;
    // 表头里的 SHA-256 按实际读到的字节计算。
    let pinned_digest = |path: &'static str| Ok((path, sha256_file(&sources.pinned(path)?)?));
    let digests = [wubixsj_supplement::WUBIXSJ]
        .into_iter()
        .map(pinned_digest)
        .chain(wubi86.digests()?.into_iter().map(Ok))
        .chain(
            [
                wubixsj_supplement::WUBI98,
                wubixsj_supplement::WUBI98_FCITX,
                wubixsj_supplement::BASE,
                wubixsj_supplement::PINYIN_SUPPLEMENT,
                wubixsj_supplement::PLACES,
                wubixsj_supplement::CUSTOM,
            ]
            .into_iter()
            .map(pinned_digest),
        )
        .chain(
            curation::present_digests(&sources, &curation::PINYIN_TABLES)?
                .into_iter()
                .map(Ok),
        )
        .collect::<Result<Vec<_>>>()?;
    let inputs: Vec<(&str, &str)> = digests
        .iter()
        .map(|(path, sha256)| (*path, sha256.as_str()))
        .collect();
    let generator_commit = product::builder_commit(root)?;
    let rendered = wubixsj_supplement::render(
        &supplement,
        &wubixsj_supplement::Provenance {
            inputs: &inputs,
            generator_commit: &generator_commit,
            upstreams: &wubi_supplement::upstreams(
                &sources.lock,
                &[wubi86.references(), &wubixsj_supplement::UPSTREAMS].concat(),
            ),
        },
    );
    std::fs::write(&arguments.out, rendered)
        .with_context(|| format!("writing {}", arguments.out.display()))?;
    for line in wubixsj_supplement::report(&supplement) {
        eprintln!("[report] {line}");
    }
    eprintln!(
        "[done] {} words -> {}",
        supplement.entries.len(),
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
    /// 读取 `sources/` 和 `custom/` 文件的 msime-dictionary checkout（按其 `upstream.lock.json` 校验）。
    #[arg(long, value_name = "PATH")]
    dictionary: PathBuf,
}

fn build_hanja(arguments: &Hanja) -> Result<()> {
    let root = &arguments.repository;
    let lock = Lock::load(&root.join("resources/dictionary-sources.lock.json"))?;
    let dictionary = Dictionary::open(arguments.dictionary.clone(), &lock)?;
    let sources = Sources {
        lock,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: arguments.cache.clone(),
        offline: arguments.offline,
        dictionary: Some(dictionary),
    };
    let simplified_to_traditional = cantonese::parse_variants(
        stroke::SIMPLIFIED_TO_TRADITIONAL,
        &text::read(&sources.pinned(stroke::SIMPLIFIED_TO_TRADITIONAL)?)?,
    )?;
    let readings = hanja::build(
        &text::read(&sources.pinned(hanja::SOURCE)?)?,
        &hanja::simplified_only_forms(&simplified_to_traditional),
    )?;
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
    let generator_commit = product::builder_commit(root)?;
    let rendered = hkcancor::render(
        &counts,
        &hkcancor::Provenance {
            upstream_commit: &upstream_commit,
            generator_commit: &generator_commit,
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
    /// msime-dictionary checkout，`sources/` 和 `custom/` 下的每个路径都从这里读（msime 的锁文件不固定其中任何文件）。它的 Git 提交固定内容；msime 认定为上游数据的文件必须与 checkout 的 `upstream.lock.json` 一致，记录里的提交必须等于锁文件的 references。
    #[arg(long, value_name = "PATH")]
    dictionary: PathBuf,
}

fn build_languages(arguments: &Languages) -> Result<()> {
    let root = &arguments.repository;
    let lock = Lock::load(&root.join("resources/dictionary-sources.lock.json"))?;
    let dictionary = Dictionary::open(arguments.dictionary.clone(), &lock)?;
    let sources = Sources {
        lock,
        repository_inputs: root.join("resources/dictionary-sources"),
        cache: arguments.cache.clone(),
        offline: arguments.offline,
        dictionary: Some(dictionary),
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
        .args(["base", "translations_base", "english_base", "cantonese_base"])
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
    /// custom/cantonese.txt before the change.
    #[arg(long, requires_all = ["cantonese_head", "cantonese_characters"])]
    cantonese_base: Option<PathBuf>,
    /// custom/cantonese.txt after the change.
    #[arg(long, requires = "cantonese_base")]
    cantonese_head: Option<PathBuf>,
    /// msime-dictionary 的 sources/cantonese/jyut6ping3.chars.dict.yaml：新行每个字的读音都要是这里给的读音之一，与构建同一个检查。
    #[arg(long, requires = "cantonese_base")]
    cantonese_characters: Option<PathBuf>,
    /// A shipped msime-pinyin.db; appended words already in its quanpin tables are rejected.
    #[arg(long)]
    msime_db: Option<PathBuf>,
    /// A shipped msime-english.db; appended English words whose word and display are already in its english_words are rejected.
    #[arg(long)]
    english_db: Option<PathBuf>,
    /// 已发布的 msime-cantonese.db：同一去声调粤拼下已有同一个词的新行被拒绝。
    #[arg(long)]
    cantonese_db: Option<PathBuf>,
    /// msime-dictionary 的 curation/pinyin-vetoes.txt：custom/words.txt 新加的词被整词否决或按读音否决时拒绝。文件不在时按空表。
    #[arg(long)]
    pinyin_vetoes: Option<PathBuf>,
    /// msime-dictionary 的 curation/english-vetoes.txt：custom/english.txt 新加的词在其中时拒绝；文件不在时按空表。构建器内置的蔑称不给这个参数也拒绝。
    #[arg(long)]
    english_vetoes: Option<PathBuf>,
    /// Where the JSON report is written.
    #[arg(long)]
    json: Option<PathBuf>,
    /// Where the Markdown summary is written.
    #[arg(long)]
    markdown: Option<PathBuf>,
}

fn open_read_only(path: Option<&Path>) -> Result<Option<rusqlite::Connection>> {
    path.map(|path| {
        sqlite::open_read_only(path).with_context(|| format!("opening {}", path.display()))
    })
    .transpose()
}

/// Runs `check-words` and writes its reports; `Ok(false)` when a line was rejected.
fn check_words(arguments: &CheckWords) -> Result<bool> {
    let msime = open_read_only(arguments.msime_db.as_deref())?;
    let english = open_read_only(arguments.english_db.as_deref())?;
    let cantonese = open_read_only(arguments.cantonese_db.as_deref())?;
    let characters = arguments
        .cantonese_characters
        .as_deref()
        .map(text::read)
        .transpose()?;
    let characters = characters
        .as_deref()
        .map(cantonese::parse_characters)
        .transpose()?;
    let readings = characters.as_deref().map(cantonese::character_readings);
    // 否决表和构建一样，文件不在就是空表；调用方应传 PR 基准提交的表，防止同一个 PR 撤掉否决后加回词条。
    let optional = |path: &Path| -> Result<String> {
        if path.exists() {
            text::read(path)
        } else {
            Ok(String::new())
        }
    };
    let pinyin_vetoes = arguments
        .pinyin_vetoes
        .as_deref()
        .map(|path| curation::PinyinVetoes::parse(&optional(path)?))
        .transpose()?;
    let english_vetoes = arguments
        .english_vetoes
        .as_deref()
        .map(|path| curation::EnglishVetoes::parse(&optional(path)?))
        .transpose()?;
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
        (
            check_words::Kind::Cantonese,
            &arguments.cantonese_base,
            &arguments.cantonese_head,
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
        cantonese: cantonese.as_ref(),
        cantonese_readings: readings.as_ref(),
        pinyin_vetoes: pinyin_vetoes.as_ref(),
        english_vetoes: english_vetoes.as_ref(),
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

/// [`Build::quanpin_inputs`] 的结果：单字表、白名单、带路径的词组表。
type QuanpinFiles = (
    PathBuf,
    Option<HashSet<String>>,
    Vec<(&'static str, PathBuf)>,
);

struct Build {
    sources: Sources,
    out: PathBuf,
    complete: bool,
}

/// msime-dictionary checkout 是否发布新世纪五笔表：只看它有没有收新世纪的词组快照 `sources/wubi/wubixsj-supplement.txt`。构建器不读这份快照的内容（词组每次从 checkout 重新派生，见 [`Build::fresh_wubi_supplement`]），只拿它在不在当开关。这样第一个带 `wubixsj` 的 `dict-v` 由收快照的那个 msime-dictionary PR 发出，它同时补上 NOTICE 与发布说明，维护者在合入它之前查专利和商标；在那之前的 checkout，包括还没收上游码表、`upstream.lock.json` 也没有记录它的旧提交（例如 `dict-v2.0.41`），照常构建，只是没有这张表。快照由 msime-dictionary 的 `check_upstream.py` 列为必须存在的生成表，删掉它要连那份清单一起改。
fn wubixsj_published(dictionary: &Path) -> bool {
    dictionary.join(wubixsj_supplement::OUTPUT).is_file()
}

/// 构建时重新派生词组的五笔表。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WubiTable {
    Wubi86,
    Wubi98,
    WubiXsj,
}

impl Build {
    /// 每次从本次构建的固定输入派生，不依赖词库仓上一次生成的快照。
    fn fresh_wubi_supplement(&self, table: WubiTable) -> Result<PathBuf> {
        let repository = self
            .sources
            .repository_inputs
            .parent()
            .and_then(Path::parent)
            .context("missing builder repository root")?
            .to_path_buf();
        let dictionary = self
            .sources
            .dictionary
            .as_ref()
            .context("missing dictionary checkout")?
            .root
            .clone();
        let output = match table {
            WubiTable::Wubi86 => wubi86_supplement::OUTPUT,
            WubiTable::Wubi98 => wubi98_supplement::OUTPUT,
            WubiTable::WubiXsj => wubixsj_supplement::OUTPUT,
        };
        let out = self.out.join(
            Path::new(output)
                .file_name()
                .context("missing supplement file name")?,
        );
        let (cache, offline) = (self.sources.cache.clone(), self.sources.offline);
        match table {
            WubiTable::Wubi86 => build_wubi86_supplement(&Wubi86Supplement {
                out: out.clone(),
                repository,
                dictionary,
                cache,
                offline,
            })?,
            WubiTable::Wubi98 => build_wubi98_supplement(&Wubi98Supplement {
                out: out.clone(),
                repository,
                dictionary,
                cache,
                offline,
            })?,
            WubiTable::WubiXsj => build_wubixsj_supplement(&WubiXsjSupplement {
                out: out.clone(),
                repository,
                dictionary,
                cache,
                offline,
            })?,
        }
        Ok(out)
    }

    fn database(&self, name: &str) -> Result<rusqlite::Connection> {
        sqlite::open(&self.out.join(name))
    }

    /// 全拼阶段的输入：单字表、许可记录允许时使用的单字白名单，以及按加载次序排列的词组表（带它们在 msime-dictionary 或锁文件里的路径，`reweight` 按它找文件系数）。
    fn quanpin_inputs(&self) -> Result<QuanpinFiles> {
        let single_chars = self.sources.pinned(reweight::SINGLE_CHARS)?;
        let (mut phrases, whitelist) = if self.complete {
            let mut whitelist = msime::parse_whitelist(&text::read(
                &self.sources.pinned(licensing::SINGLE_CHAR_WHITELIST)?,
            )?);
            whitelist.extend(msime::parse_whitelist(&text::read(
                &self.sources.repository(msime::WHITELIST_ADDITIONS)?,
            )?));
            (
                vec![
                    (
                        licensing::BASE_DICT_PART1,
                        self.sources.pinned(licensing::BASE_DICT_PART1)?,
                    ),
                    (
                        licensing::BASE_DICT_PART2,
                        self.sources.pinned(licensing::BASE_DICT_PART2)?,
                    ),
                ],
                Some(whitelist),
            )
        } else {
            // 没有来源记录时不能应用白名单，接受许可来源里的全部单字。
            (
                vec![(reweight::RIME_ICE, self.sources.pinned(reweight::RIME_ICE)?)],
                None,
            )
        };
        phrases.push((
            reweight::RIME_ICE_SUPPLEMENT,
            self.sources.pinned(reweight::RIME_ICE_SUPPLEMENT)?,
        ));
        Ok((single_chars, whitelist, phrases))
    }

    /// 覆盖表也列了的 custom 词里，哪些是 custom-words 阶段从已有的行提了权的（[`curation::custom_promotions`]）：按 `quanpin` 和 `places-supplement` 两个阶段读的输入重新取 custom 之前的权重。
    fn custom_promotions<'a>(
        &self,
        listed: impl IntoIterator<Item = &'a msime::CustomWord>,
    ) -> Result<HashMap<(String, String), (i64, i64)>> {
        let (single_chars, whitelist, phrases) = self.quanpin_inputs()?;
        let mut texts = vec![(text::read(&single_chars)?, whitelist.as_ref())];
        for path in phrases
            .iter()
            .map(|(_, path)| path)
            .chain([&self.sources.pinned(places_supplement::OUTPUT)?])
        {
            texts.push((text::read(path)?, None));
        }
        let earlier: Vec<(&str, Option<&HashSet<String>>)> = texts
            .iter()
            .map(|(text, accepted)| (text.as_str(), *accepted))
            .collect();
        Ok(curation::custom_promotions(listed, &earlier))
    }

    /// 有 `curation/offensive-words.txt` 时把五笔表 `table` 里的粗俗词挪到同码最后（[`curation::place_offensive_last_in_wubi`]），返回接在阶段摘要后面的说明；没有这个文件时什么也不做，返回空串。
    fn place_offensive_last_in_wubi(
        &self,
        connection: &mut rusqlite::Connection,
        table: &str,
    ) -> Result<String> {
        let Some(offensive) = curation::load_offensive_words(&self.sources)? else {
            return Ok(String::new());
        };
        let moved = curation::place_offensive_last_in_wubi(connection, table, &offensive)?;
        Ok(format!(
            ", {moved} offensive rows moved after the other rows of their code"
        ))
    }

    /// 五笔原表（86 极点表、两张 98 表）里的全部词，`word-vetoes` 判断只在五笔里出现的整词否决是否过期时用，见 [`curation::wubi_source_rows`]。
    fn wubi_source_words(&self) -> Result<HashSet<String>> {
        Ok(curation::wubi_source_rows(&self.sources)?
            .into_iter()
            .map(|(_, _, value)| value)
            .collect())
    }

    fn pinyin(&self) -> Result<pinyin::Pinyin> {
        pinyin::Pinyin::with_overrides(&text::read(&self.sources.repository(pinyin::OVERRIDES)?)?)
    }

    /// 表情与颜文字的三份关键词表过一遍拒绝表（`others::check_keyword_vetoes`），命中就让构建失败。
    fn check_keyword_vetoes(&self) -> Result<()> {
        let mut files = Vec::new();
        for path in others::KEYWORD_FILES {
            files.push((path, text::read(&self.sources.repository(path)?)?));
        }
        let files: Vec<(&str, &str)> = files
            .iter()
            .map(|(path, source)| (*path, source.as_str()))
            .collect();
        others::check_keyword_vetoes(
            &text::read(&self.sources.repository(others::KEYWORD_VETOES)?)?,
            &files,
        )
    }

    /// The OALDPE headwords the English stage adds, empty unless the build includes unlicensed inputs.
    fn oaldpe_words(&self) -> Result<std::collections::BTreeSet<String>> {
        if !self.complete {
            return Ok(Default::default());
        }
        english::parse_oaldpe_words(&text::read(&self.sources.pinned(licensing::OALDPE_WORDS)?)?)
    }

    /// SCOWL 拼写词典（固定的 Aspell 包）里全由字母组成的词形，英文阶段和释义阶段都靠它定大小写。
    fn scowl_attested(&self) -> Result<HashSet<String>> {
        Ok(english_supplement::letter_forms(
            &english_supplement::read_package(&crate::sources::read_private(
                &self.sources.pinned(english_supplement::ARCHIVE)?,
            )?)?,
        ))
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
                let (single_chars, whitelist, phrases) = self.quanpin_inputs()?;
                let inputs = msime::QuanpinInputs {
                    single_chars: &single_chars,
                    whitelist,
                    phrases: phrases.iter().map(|(_, path)| path.as_path()).collect(),
                };
                let rows = msime::build_quanpin(&mut self.database("msime-pinyin.db")?, &inputs)?;
                Ok(format!("{rows} rows"))
            }
            Stage::Reweight => {
                let Some(wanxiang) = reweight::load(&self.sources)? else {
                    return Ok(format!(
                        "left out: the checkout's upstream.lock.json records no {}",
                        reweight::REFERENCE
                    ));
                };
                let settings = reweight::Settings::load(&self.sources)?;
                let (single_chars, whitelist, phrases) = self.quanpin_inputs()?;
                let mut texts = vec![(reweight::SINGLE_CHARS, text::read(&single_chars)?)];
                for (label, path) in &phrases {
                    texts.push((*label, text::read(path)?));
                }
                let inputs: Vec<reweight::Input> = texts
                    .iter()
                    .enumerate()
                    .map(|(index, (path, text))| reweight::Input {
                        path,
                        text: text.as_str(),
                        accepted: if index == 0 { whitelist.as_ref() } else { None },
                    })
                    .collect();
                let wanxiang: Vec<(&str, &str)> = wanxiang
                    .iter()
                    .map(|(name, text)| (*name, text.as_str()))
                    .collect();
                let counts = reweight::apply(
                    &mut self.database("msime-pinyin.db")?,
                    &inputs,
                    &wanxiang,
                    &settings,
                )?;
                Ok(format!(
                    "{}: {} rows shared with {}, {} realigned, {} rescaled, {} capped",
                    settings.describe(),
                    counts.shared,
                    reweight::REFERENCE,
                    counts.aligned,
                    counts.scaled,
                    counts.capped
                ))
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
                // 新插入的词不压过同全拼、同九键码的常用词（H11），竞争者按之后的纠错、否决、覆盖算。
                let curation = curation::PinyinCuration::load(&self.sources)?;
                let corrections = msime::parse_reading_corrections(&text::read(
                    &self.sources.repository(msime::READING_CORRECTIONS)?,
                )?)?;
                let (counts, capped) = custom_ceiling::apply_custom_words_with_ceiling(
                    &mut self.database("msime-pinyin.db")?,
                    &words,
                    &custom_ceiling::curated_weight(&curation, &corrections),
                )?;
                for word in &capped {
                    eprintln!("[custom-words] capped {}", word.describe());
                }
                Ok(format!(
                    "{} entries: {} inserted ({} capped below a common word), {} promoted, {} already at or above their weight",
                    words.len(),
                    counts.inserted,
                    capped.len(),
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
            Stage::WordVetoes => {
                let vetoes = curation::PinyinCuration::load(&self.sources)?.vetoes;
                if vetoes.is_empty() {
                    return Ok(format!("no entries in {}", curation::PINYIN_VETOES));
                }
                let counts = curation::apply_word_vetoes(
                    &mut self.database("msime-pinyin.db")?,
                    &vetoes,
                    || self.wubi_source_words(),
                )?;
                Ok(format!(
                    "{} entries: {} rows removed, {} whole-word entries only in the Wubi tables",
                    vetoes.entries.len(),
                    counts.removed,
                    counts.wubi_only
                ))
            }
            Stage::WeightOverrides => {
                let overrides = curation::PinyinCuration::load(&self.sources)?.overrides;
                if overrides.is_empty() {
                    return Ok(format!(
                        "no entries in {}",
                        curation::PINYIN_WEIGHT_OVERRIDES
                    ));
                }
                let custom = msime::parse_custom_words(&text::read(
                    &self.sources.pinned("custom/words.txt")?,
                )?)?;
                let listed: Vec<&msime::CustomWord> = custom
                    .iter()
                    .filter(|entry| overrides.get(&entry.value, &entry.key).is_some())
                    .collect();
                let promoted = if listed.is_empty() {
                    HashMap::new()
                } else {
                    self.custom_promotions(listed)?
                };
                let counts = curation::apply_weight_overrides(
                    &mut self.database("msime-pinyin.db")?,
                    &overrides,
                    &promoted,
                )?;
                Ok(format!(
                    "{} entries: {} rows changed, {} already at their weight",
                    overrides.entries.len(),
                    counts.changed,
                    counts.unchanged
                ))
            }
            Stage::SentenceExcluded => {
                let Some(text) = curation::read_optional(&self.sources, curation::OFFENSIVE_WORDS)?
                else {
                    let path = self.out.join("msime-pinyin.db");
                    let dropped =
                        path.is_file() && curation::drop_sentence_excluded(&sqlite::open(&path)?)?;
                    return Ok(format!(
                        "left out: no {}, so the engine uses its embedded list{}",
                        curation::OFFENSIVE_WORDS,
                        if dropped {
                            " (dropped the table an earlier build left in --out)"
                        } else {
                            ""
                        }
                    ));
                };
                let entries = curation::parse_offensive_words(&text)?;
                let (listed, exempt) = curation::write_sentence_excluded(
                    &mut self.database("msime-pinyin.db")?,
                    &entries,
                )?;
                Ok(format!("{listed} words and {exempt} exempt patterns"))
            }
            Stage::Wubi => {
                let table = wubi86_table::Files::locate(&self.sources)?;
                let supplement = self.fresh_wubi_supplement(WubiTable::Wubi86)?;
                let custom = self.sources.optional(msime::CUSTOM_WUBI86)?;
                let mut later = vec![supplement.as_path()];
                later.extend(custom.as_deref());
                let vetoes = curation::PinyinCuration::load(&self.sources)?.vetoes;
                let mut connection = self.database("msime-pinyin.db")?;
                let (imported, skipped, filtered, vetoed) =
                    msime::build_wubi(&mut connection, &table, &later, &vetoes)?;
                let offensive = self.place_offensive_last_in_wubi(&mut connection, "wubi86")?;
                Ok(format!(
                    "{imported} rows imported, {skipped} skipped, {filtered} unsupported Wubi values left out, {vetoed} vetoed{offensive}; {}{}",
                    if table.rime_wubi.is_some() {
                        format!("{} at weight {} (rows upstream weighs 0 stay at 0, after the supplement; phrases {} already codes left out)", wubi86_table::RIME_WUBI, wubi86_table::RIME_WUBI_WEIGHT, wubi86_table::JIDIAN)
                    } else {
                        format!("no {} in the checkout, so {} is read as one table", wubi86_table::RIME_WUBI, wubi86_table::JIDIAN)
                    },
                    if custom.is_some() {
                        ""
                    } else {
                        "; no custom/wubi86.txt in the checkout"
                    }
                ))
            }
            Stage::Wubi98 => {
                let supplement = self.sources.pinned(wubi98_supplement::WUBI98_FCITX)?;
                let generated = self.fresh_wubi_supplement(WubiTable::Wubi98)?;
                let vetoes = curation::PinyinCuration::load(&self.sources)?.vetoes;
                let mut connection = self.database("msime-pinyin.db")?;
                let (imported, skipped, vetoed) = msime::build_wubi98_sources(
                    &mut connection,
                    &self.sources.pinned(wubi98_supplement::WUBI98)?,
                    &[supplement.as_path()],
                    &[generated.as_path()],
                    &vetoes,
                )?;
                let offensive = self.place_offensive_last_in_wubi(&mut connection, "wubi98")?;
                Ok(format!(
                    "{imported} rows imported, {skipped} skipped, {vetoed} vetoed{offensive}"
                ))
            }
            Stage::WubiXsj => {
                let dictionary = &self
                    .sources
                    .dictionary
                    .as_ref()
                    .context("missing dictionary checkout")?
                    .root;
                if !wubixsj_published(dictionary) {
                    return Ok(format!(
                        "left out: the msime-dictionary checkout has no {}, so it does not publish the wubixsj table yet",
                        wubixsj_supplement::OUTPUT
                    ));
                }
                let generated = self.fresh_wubi_supplement(WubiTable::WubiXsj)?;
                let vetoes = curation::PinyinCuration::load(&self.sources)?.vetoes;
                let mut connection = self.database("msime-pinyin.db")?;
                let counts = msime::build_wubixsj(
                    &mut connection,
                    &self.sources.pinned(wubixsj_supplement::WUBIXSJ)?,
                    &[generated.as_path()],
                    &vetoes,
                )?;
                let offensive = self.place_offensive_last_in_wubi(&mut connection, "wubixsj")?;
                Ok(format!(
                    "{} rows imported, {} skipped, {} single-character rows outside the basic CJK block or with an invalid code left out, {} upstream words left out, {} vetoed{offensive}",
                    counts.imported, counts.skipped, counts.filtered, counts.phrases, counts.vetoed
                ))
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
                let mut supplement = english::parse_base_dict_words(&text::read(
                    &self
                        .sources
                        .pinned("sources/english/rime-ice-en-supplement.txt")?,
                )?)?;
                // SCOWL's spelling dictionary decides which casing of a word leads (see english::leading_display).
                let attested = self.scowl_attested()?;
                let lowercase_added = english::add_attested_lowercase(&mut supplement, &attested);
                let supplement_added = english::merge_words(&mut base, supplement);
                let scowl_added = english::merge_words(&mut base, self.scowl_words()?);
                let counts = english::parse_google_counts(&text::read(
                    &self
                        .sources
                        .pinned("sources/english/google-word-counts.txt")?,
                )?);
                let custom = english::parse_custom_english(&text::read(
                    &self.sources.pinned(english::CUSTOM_ENGLISH)?,
                )?)?;
                let mut database = self.database("msime-english.db")?;
                let vetoes = curation::EnglishVetoes::load(&self.sources)?;
                let rows = english::build_english_words(
                    &mut database,
                    &oaldpe,
                    &base,
                    &counts,
                    &attested,
                    &custom,
                    &vetoes,
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
                    "{} words in {} rows ({} from rime-ice supplement, {} of them given a lowercase display SCOWL attests; {} from SCOWL), {} custom rows: {} added, {} replacing a base row; {} words and custom rows vetoed",
                    rows.base,
                    rows.base_rows,
                    supplement_added,
                    lowercase_added,
                    scowl_added,
                    custom.len(),
                    rows.custom_added,
                    rows.custom_replaced,
                    rows.vetoed
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
                let vetoes = curation::GlossVetoes::load(&self.sources)?;
                let mut english_db = self.database("msime-english.db")?;
                let glosses = english::derive_glosses(
                    &self.sources.pinned("ecdict/ecdict.csv")?,
                    &english_db,
                    &sqlite::open(&msime_path)?,
                    &reverse_excluded,
                    &self.scowl_attested()?,
                    &vetoes,
                )?;
                english::write_glosses(&mut english_db, &glosses)?;
                Ok(format!(
                    "{} English-to-Chinese, {} Chinese-to-English, {} sense pairs vetoed",
                    glosses.en_zh.len(),
                    glosses.zh_en.len(),
                    vetoes.pairs.len()
                ))
            }
            Stage::CustomTranslations => {
                let entries = english::parse_custom_translations(&text::read(
                    &self.sources.pinned(english::CUSTOM_TRANSLATIONS)?,
                )?)?;
                let vetoes = curation::EnglishVetoes::load(&self.sources)?;
                let (vetoed, kept): (Vec<_>, Vec<_>) = entries.into_iter().partition(|entry| {
                    vetoes
                        .vetoed_translation(&entry.source, &entry.gloss)
                        .is_some()
                });
                let deleted = english::apply_custom_translations(
                    &mut self.database("msime-english.db")?,
                    &kept,
                )?;
                Ok(format!(
                    "{} overrides ({} deletion lines removing {deleted} glosses), {} left out for a vetoed English word",
                    kept.len(),
                    kept.iter().filter(|entry| entry.deletes()).count(),
                    vetoed.len()
                ))
            }
            Stage::Emoji => {
                self.check_keyword_vetoes()?;
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
                self.check_keyword_vetoes()?;
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
                    &self.sources.pinned(others::PILIAPP_SYMBOLS)?,
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
    if let Some(Command::RimeIceSupplement(arguments)) = &arguments.command {
        return rime_ice_supplement::build(
            &arguments.upstream,
            &arguments.dictionary,
            &arguments.out,
            &arguments.repository,
        );
    }
    if let Some(Command::Places(places)) = &arguments.command {
        return build_places(places);
    }
    if let Some(Command::PlacesSupplement(supplement)) = &arguments.command {
        return build_places_supplement(supplement);
    }
    if let Some(Command::EnglishSupplement(supplement)) = &arguments.command {
        return build_english_supplement(supplement);
    }
    if let Some(Command::Wubi98Supplement(supplement)) = &arguments.command {
        return build_wubi98_supplement(supplement);
    }
    if let Some(Command::WubiXsjSupplement(supplement)) = &arguments.command {
        return build_wubixsj_supplement(supplement);
    }
    if let Some(Command::Wubi86Supplement(supplement)) = &arguments.command {
        return build_wubi86_supplement(supplement);
    }
    if let Some(Command::Wubi86Helpcode(helpcode)) = &arguments.command {
        return build_wubi86_helpcode(helpcode);
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
    if let Some(Command::Web(web)) = &arguments.command {
        return build_web(web);
    }
    if let Some(Command::SyllableFrequency(frequency)) = &arguments.command {
        return build_syllable_frequency(frequency);
    }
    if let Some(Command::CleanseReport(report)) = &arguments.command {
        return build_cleanse_report(report);
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
    let dictionary = arguments
        .dictionary
        .clone()
        .map(|checkout| Dictionary::open(checkout, &lock))
        .transpose()?;
    std::fs::create_dir_all(&out)?;
    let build = Build {
        sources: Sources {
            lock,
            repository_inputs: root.join("resources/dictionary-sources"),
            cache,
            offline: arguments.offline,
            dictionary,
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
    product::stamp_format_versions(&build.out)?;

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
    // 不写 manifest 时删掉 `--out` 里旧的 manifest 和校验和：上面刚重新构建或冻结过的数据库已经和它们记下的大小、SHA-256 不符。
    if !missing.is_empty() {
        eprintln!(
            "[product] not writing {}: missing {}{}",
            product::MANIFEST,
            missing.join(", "),
            removed_note(&product::remove_stale_manifest(&build.out)?)
        );
        return Ok(());
    }
    let Some(dictionary) = build.sources.dictionary.as_ref() else {
        eprintln!(
            "[product] not writing {}: it records the msime-dictionary commit the build read; pass --dictionary{}",
            product::MANIFEST,
            removed_note(&product::remove_stale_manifest(&build.out)?)
        );
        return Ok(());
    };
    product::verify(&build.out, complete, wubixsj_published(&dictionary.root))?;
    product::write_manifest(
        &build.out,
        root,
        &dictionary.root,
        &build.sources.lock,
        complete,
    )?;
    eprintln!(
        "[product] verified; wrote {} and {}",
        product::MANIFEST,
        product::SUMS
    );
    Ok(())
}

/// 不写 manifest 时附在提示后面，说明删掉了哪些旧文件。
fn removed_note(removed: &[&str]) -> String {
    if removed.is_empty() {
        String::new()
    } else {
        format!(" (removed the stale {})", removed.join(" and "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The msime-dictionary checkout a wubi stage test builds from, at one of the three points of the 新世纪 rollout.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum XsjCheckout {
        /// Before it took the upstream table: `upstream.lock.json` does not list it (dict-v2.0.41 and older).
        WithoutSource,
        /// It has the upstream table but not the phrase snapshot yet.
        SourceOnly,
        /// It has both, so it publishes the table.
        Published,
    }

    /// Builds a checkout and a builder repository in a temporary directory; returns them with the `Build` reading them.
    fn wubi_fixture(checkout: XsjCheckout) -> (tempfile::TempDir, PathBuf, PathBuf, Build) {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("builder");
        let dictionary = temporary.path().join("dictionary");
        let out = temporary.path().join("out");
        std::fs::create_dir_all(root.join("resources/dictionary-sources")).unwrap();
        // custom-words 阶段读纠错表来判断哪些行之后会被删掉（H11 封顶），真实仓库总有这个文件。
        std::fs::write(
            root.join("resources/dictionary-sources")
                .join(msime::READING_CORRECTIONS),
            "# empty\n",
        )
        .unwrap();
        std::fs::create_dir_all(&out).unwrap();
        let write = |path: &str, bytes: &[u8]| {
            let file = dictionary.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, bytes).unwrap();
        };
        write(
            wubi86_supplement::JIDIAN,
            "甲\tabcd\t100\n乙\tefgh\t100\n乙乙\tefef\t10\n".as_bytes(),
        );
        let mut utf16 = vec![0xff, 0xfe];
        utf16.extend(
            "甲\tijkl\n乙\tmnop\n"
                .encode_utf16()
                .flat_map(u16::to_le_bytes),
        );
        write(wubi86_supplement::WUBI98, &utf16);
        write(wubi86_supplement::WUBI98_FCITX, b"# empty\n");
        write(wubi86_supplement::BASE, "甲\tjia\t100\n".as_bytes());
        write(wubi86_supplement::PINYIN_SUPPLEMENT, b"# empty\n");
        write(wubi86_supplement::PLACES, b"# empty\n");
        write(wubi86_supplement::CUSTOM, "甲乙\tjia'yi\t1\n".as_bytes());
        let mut upstream_files = vec![
            wubi86_supplement::WUBI98,
            wubi86_supplement::WUBI98_FCITX,
            wubi86_supplement::BASE,
            wubi86_supplement::PINYIN_SUPPLEMENT,
        ];
        if checkout != XsjCheckout::WithoutSource {
            write(
                wubixsj_supplement::WUBIXSJ,
                "---\nname: wubi06\n...\n甲\tqrst\t10\n乙\tuvwx\t10\n乙乙\tuvwu\t500\n".as_bytes(),
            );
            upstream_files.push(wubixsj_supplement::WUBIXSJ);
        }
        if checkout == XsjCheckout::Published {
            // 快照只当开关，内容不读：这一行要是被读进去，下面的断言会看到「丙丙」。
            write(
                wubixsj_supplement::OUTPUT,
                "# stale snapshot\n丙丙\tzzzz\t99\n".as_bytes(),
            );
        }
        // msime 的锁总是登记 wubi06-rime；旧 checkout 的 upstream.lock.json 没有这一项。
        let reference = |name: &str| {
            (
                name.to_owned(),
                serde_json::json!({"repository": format!("https://example.invalid/{name}"), "commit":"a".repeat(40)}),
            )
        };
        let names = [
            "rime-ice",
            "rime-ice-supplement",
            "98wubi-tables",
            "fcitx5-table-extra",
            "wubi06-rime",
        ];
        let references: serde_json::Map<String, serde_json::Value> =
            names.into_iter().map(reference).collect();
        let upstreams: serde_json::Map<String, serde_json::Value> = names
            .into_iter()
            .filter(|name| checkout != XsjCheckout::WithoutSource || *name != "wubi06-rime")
            .map(reference)
            .collect();
        let files: Vec<_> = upstream_files.into_iter().map(|path| serde_json::json!({"path":path, "upstream":sources::upstream_reference(path).unwrap(), "size":std::fs::metadata(dictionary.join(path)).unwrap().len(), "sha256":sources::sha256_file(&dictionary.join(path)).unwrap()})).collect();
        write(
            sources::UPSTREAM_LOCK,
            &serde_json::to_vec(
                &serde_json::json!({"version":1,"upstreams":upstreams,"files":files}),
            )
            .unwrap(),
        );
        let lock = serde_json::json!({"references":references, "mozc":{"repository":"https://example.invalid/mozc", "commit":"b".repeat(40)}, "files":[]});
        std::fs::write(
            root.join("resources/dictionary-sources.lock.json"),
            serde_json::to_vec(&lock).unwrap(),
        )
        .unwrap();
        for args in [
            vec!["init", "-q"],
            vec!["add", "resources/dictionary-sources.lock.json"],
            vec![
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "fixture",
            ],
        ] {
            assert!(std::process::Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(args)
                .status()
                .unwrap()
                .success());
        }
        let lock = Lock::load(&root.join("resources/dictionary-sources.lock.json")).unwrap();
        let build = Build {
            sources: Sources {
                dictionary: Some(Dictionary::open(dictionary.clone(), &lock).unwrap()),
                lock,
                repository_inputs: root.join("resources/dictionary-sources"),
                cache: temporary.path().join("cache"),
                offline: true,
            },
            out: out.clone(),
            complete: false,
        };
        (temporary, dictionary, out, build)
    }

    /// Emoji、Kaomoji 两个阶段都先过拒绝表：关键词表里有被拒绝的行时，两个阶段都在读目录、写数据库之前失败，并指出命中的行。这里只放拒绝表和三份关键词表，目录不在，阶段要是不先查拒绝表，报的就是缺目录。
    #[test]
    fn emoji_and_kaomoji_stages_check_the_keyword_vetoes() {
        let temporary = tempfile::tempdir().unwrap();
        let inputs = temporary.path().join("resources/dictionary-sources");
        let write = |path: &str, text: &str| {
            let file = inputs.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(others::KEYWORD_VETOES, "草泥马\n");
        write("emoji/emoji.txt", "羊驼\t🦙\n草泥马\t🦙\n");
        write("emoji/emoji_en.txt", "llama\t🦙\n");
        write("kaomoji/kaomoji.txt", "开心\t(^_^)\n");
        let lock: Lock = serde_json::from_value(serde_json::json!({
            "references": {},
            "mozc": {"repository": "https://example.invalid/mozc", "commit": "b".repeat(40)},
            "files": [],
        }))
        .unwrap();
        let out = temporary.path().join("out");
        std::fs::create_dir_all(&out).unwrap();
        let build = Build {
            sources: Sources {
                dictionary: None,
                lock,
                repository_inputs: inputs.clone(),
                cache: temporary.path().join("cache"),
                offline: true,
            },
            out: out.clone(),
            complete: false,
        };
        for stage in [Stage::Emoji, Stage::Kaomoji] {
            let error = format!("{:#}", build.run(stage).unwrap_err());
            assert!(
                error.contains("emoji/emoji.txt:2: 草泥马 -> 🦙 is vetoed"),
                "{error}"
            );
        }
        assert!(!out.join("msime-others.db").exists());
    }

    fn has_table(db: &rusqlite::Connection, table: &str) -> bool {
        db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .unwrap()
    }

    /// 没有 `curation/` 的 checkout：三个新阶段什么也不做，`msime-pinyin.db` 逐字节不变，也不建 `sentence_excluded`。有表时按表删行、改权重、写粗俗词表，五笔阶段按粗俗词表给五笔表排位；整词否决只在五笔原表里出现也算匹配到，什么也匹配不到时失败；覆盖表能改 custom 新插入的词，不能改 custom 提过权的行；整词否决的词不进五笔表。
    #[test]
    fn curation_stages_are_no_ops_without_tables_and_apply_them_when_present() {
        let (_temporary, dictionary, out, build) = wubi_fixture(XsjCheckout::Published);
        std::fs::write(
            dictionary.join("sources/pinyin/single-chars.txt"),
            "甲\tjia\t1000\n乙\tyi\t1000\n",
        )
        .unwrap();
        for stage in [Stage::Quanpin, Stage::CustomWords] {
            build.run(stage).unwrap();
        }
        let path = out.join("msime-pinyin.db");
        let before = sha256_file(&path).unwrap();
        for stage in [
            Stage::WordVetoes,
            Stage::WeightOverrides,
            Stage::SentenceExcluded,
        ] {
            build.run(stage).unwrap();
        }
        assert_eq!(sha256_file(&path).unwrap(), before);
        assert!(!has_table(
            &rusqlite::Connection::open(&path).unwrap(),
            "sentence_excluded"
        ));

        let curation = |name: &str, text: &str| {
            std::fs::create_dir_all(dictionary.join("curation")).unwrap();
            std::fs::write(dictionary.join("curation").join(name), text).unwrap();
        };
        curation("pinyin-vetoes.txt", "乙\tyi\n乙乙\n");
        curation("pinyin-weight-overrides.txt", "甲\tjia\t50\n");
        curation("offensive-words.txt", "甲乙\n");
        let summary = build.run(Stage::WordVetoes).unwrap();
        assert!(
            summary.contains("1 whole-word entries only in the Wubi tables"),
            "{summary}"
        );
        build.run(Stage::WeightOverrides).unwrap();
        build.run(Stage::SentenceExcluded).unwrap();
        let db = rusqlite::Connection::open(&path).unwrap();
        let weight = |value: &str| -> Option<i64> {
            db.query_row(
                "SELECT max(weight) FROM tbl_1_j WHERE value = ?1",
                [value],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(weight("甲"), Some(50));
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM tbl_1_y WHERE value = '乙'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert_eq!(
            db.query_row("SELECT value FROM sentence_excluded", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            "甲乙"
        );
        drop(db);
        let summary = build.run(Stage::Wubi).unwrap();
        assert!(
            summary.contains("offensive rows moved after the other rows of their code"),
            "{summary}"
        );
        let db = rusqlite::Connection::open(&path).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM wubi86 WHERE value = '乙乙'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        drop(db);

        curation("pinyin-vetoes.txt", "丙丙\n");
        let error = format!("{:#}", build.run(Stage::WordVetoes).unwrap_err());
        assert!(error.contains("丙丙 (line 1)"), "{error}");
        // custom 新插入的 甲乙 可以覆盖；custom 把 甲 从单字表的 1000 提到 5000 之后，覆盖 甲 失败。
        curation("pinyin-vetoes.txt", "");
        curation("pinyin-weight-overrides.txt", "甲乙\tjia'yi\t5\n");
        build.run(Stage::WeightOverrides).unwrap();
        let db = rusqlite::Connection::open(&path).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT weight FROM tbl_2_j WHERE value = '甲乙'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            5
        );
        drop(db);
        std::fs::write(
            dictionary.join(wubi86_supplement::CUSTOM),
            "甲乙\tjia'yi\t1\n甲\tjia\t5000\n",
        )
        .unwrap();
        for stage in [Stage::Quanpin, Stage::CustomWords] {
            build.run(stage).unwrap();
        }
        curation("pinyin-weight-overrides.txt", "甲\tjia\t50\n");
        let error = format!("{:#}", build.run(Stage::WeightOverrides).unwrap_err());
        assert!(
            error.contains("甲 jia (line 1, raised from 1000 to 5000 by custom/words.txt)"),
            "{error}"
        );

        // 表删掉以后，--out 里上一次留下的 sentence_excluded 也删掉，引擎退回内嵌表。
        std::fs::remove_file(dictionary.join("curation/offensive-words.txt")).unwrap();
        let summary = build.run(Stage::SentenceExcluded).unwrap();
        assert!(summary.contains("dropped"), "{summary}");
        assert!(!has_table(
            &rusqlite::Connection::open(&path).unwrap(),
            "sentence_excluded"
        ));
    }

    /// custom-words 阶段接上 H11 封顶：custom 新插入的词压在同全拼的常用词之下，竞争者来自前面阶段建好的全拼表（这里由 places.txt 并入），按 curation/ 的覆盖表、否决表和仓库纠错表施加之后的权重算。
    #[test]
    fn custom_words_are_capped_below_curated_competitors() {
        let (_temporary, dictionary, out, build) = wubi_fixture(XsjCheckout::Published);
        let write = |path: &str, text: &str| {
            let file = dictionary.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "sources/pinyin/single-chars.txt",
            "甲\tjia\t1000\n乙\tyi\t1000\n",
        );
        write(
            places_supplement::OUTPUT,
            "万州\twan'zhou\t3000\n门的\tmen'de\t435135\n闷的\tmen'de\t2000\n车间\tche'jian\t9000\n车检\tche'jian\t5000\n血型\txie'xing\t8000\n歇性\txie'xing\t1500\n",
        );
        write(
            wubi86_supplement::CUSTOM,
            "挽昼\twan'zhou\t3333\n们得\tmen'de\t3855\n车坚\tche'jian\t6000\n斜星\txie'xing\t3855\n",
        );
        // 门的 被覆盖到 100，不再是常用词；车间 被否决；血型 xie'xing 是之后要纠正掉的错误读音。
        write(
            "curation/pinyin-weight-overrides.txt",
            "门的\tmen'de\t100\n",
        );
        write("curation/pinyin-vetoes.txt", "车间\tche'jian\n");
        std::fs::write(
            build
                .sources
                .repository_inputs
                .join(msime::READING_CORRECTIONS),
            "血型\txie'xing\txue'xing\n",
        )
        .unwrap();
        for stage in [Stage::Quanpin, Stage::PlacesSupplement] {
            build.run(stage).unwrap();
        }
        let summary = build.run(Stage::CustomWords).unwrap();
        assert!(
            summary.contains("4 inserted (4 capped below a common word)"),
            "{summary}"
        );
        let db = rusqlite::Connection::open(out.join("msime-pinyin.db")).unwrap();
        let weight = |value: &str, key: &str| -> i64 {
            db.query_row(
                &format!(
                    "SELECT weight FROM {} WHERE key = ?1 AND value = ?2",
                    msime::pinyin_table(key).unwrap()
                ),
                [key, value],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(weight("挽昼", "wan'zhou"), 2999);
        assert_eq!(weight("们得", "men'de"), 1999);
        assert_eq!(weight("车坚", "che'jian"), 4999);
        assert_eq!(weight("斜星", "xie'xing"), 1499);
    }

    #[test]
    fn wubi_stages_derive_current_custom_words_instead_of_reading_snapshot_files() {
        let (_temporary, dictionary, out, build) = wubi_fixture(XsjCheckout::Published);
        assert!(wubixsj_published(&dictionary));
        for stage in [Stage::Wubi, Stage::Wubi98, Stage::WubiXsj] {
            build.run(stage).unwrap();
        }
        assert!(!dictionary.join(wubi86_supplement::OUTPUT).exists());
        assert!(!dictionary.join(wubi98_supplement::OUTPUT).exists());
        let db = rusqlite::Connection::open(out.join("msime-pinyin.db")).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM wubixsj WHERE value='丙丙' OR key='zzzz'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        // 新世纪表不收上游的词组行「乙乙 uvwu」，而是把极点表里的「乙乙」按新世纪单字码重新编码，沿用极点表的权重。
        assert_eq!(
            db.query_row(
                "SELECT group_concat(key), max(weight) FROM wubixsj WHERE value='乙乙'",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            )
            .unwrap(),
            ("uvuv".to_owned(), 10)
        );
        assert_eq!(stage_name(Stage::WubiXsj), "wubixsj");
        for (table, expected) in [("wubi86", "abef"), ("wubi98", "ijmn"), ("wubixsj", "qruv")] {
            assert_eq!(
                db.query_row(
                    &format!("SELECT key FROM {table} WHERE value='甲乙'"),
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
                expected
            );
        }
    }

    /// checkout 里有 `custom/wubi86.txt` 时，它的行接在极点表和补充表之后写进 `wubi86`：同码的原有候选在前（甲 之后才是 ①）。没有这个文件的 checkout 照常构建，见上一个测试。
    #[test]
    fn the_custom_wubi86_table_is_appended_after_the_upstream_rows() {
        let (_temporary, dictionary, out, build) = wubi_fixture(XsjCheckout::Published);
        std::fs::write(
            dictionary.join(msime::CUSTOM_WUBI86),
            "# 带圈数字\n①\tabcd\t1\n",
        )
        .unwrap();
        let summary = build.run(Stage::Wubi).unwrap();
        assert!(!summary.contains("no custom/wubi86.txt"), "{summary}");
        let db = rusqlite::Connection::open(out.join("msime-pinyin.db")).unwrap();
        let mut rows = db
            .prepare(
                "SELECT value, weight FROM wubi86 WHERE key='abcd' ORDER BY weight DESC, rowid",
            )
            .unwrap();
        let rows: Vec<(String, i64)> = rows
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(rows, [("甲".to_owned(), 100), ("①".to_owned(), 1)]);
    }

    /// 拆分之后的 checkout：86 原表是极点表加 `wubi86-rime-wubi.txt`，两者都按 `upstream.lock.json` 校验；rime-wubi 的行（甲甲 带着五百万的词频）按 10 写入，排在同码极点行之后、补充行之前，整词否决、三张表的补充词照常。记录列了极点表而 rime-wubi 文件丢了时，Wubi 阶段报错。
    #[test]
    fn the_split_86_layout_ranks_rime_wubi_rows_below_jidian_rows() {
        let (temporary, dictionary, out, _) = wubi_fixture(XsjCheckout::Published);
        let root = temporary.path().join("builder");
        std::fs::write(
            dictionary.join(wubi86_table::JIDIAN),
            "甲\tabcd\t100\n乙\tefgh\t100\n",
        )
        .unwrap();
        std::fs::write(
            dictionary.join(wubi86_table::RIME_WUBI),
            "乙乙\tefef\t10\r\n甲甲\tabcd\t5000000\r\n",
        )
        .unwrap();
        let read_json = |path: &Path| -> serde_json::Value {
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
        };
        let lock_path = root.join("resources/dictionary-sources.lock.json");
        let record_path = dictionary.join(sources::UPSTREAM_LOCK);
        let (mut lock, mut record) = (read_json(&lock_path), read_json(&record_path));
        for (name, path) in [
            (wubi86_table::JIDIAN_REFERENCE, wubi86_table::JIDIAN),
            (wubi86_table::RIME_WUBI_REFERENCE, wubi86_table::RIME_WUBI),
        ] {
            let reference = serde_json::json!({"repository": format!("https://example.invalid/{name}"), "commit": "a".repeat(40)});
            lock["references"][name] = reference.clone();
            record["upstreams"][name] = reference;
            record["files"].as_array_mut().unwrap().push(serde_json::json!({"path": path, "upstream": name, "size": std::fs::metadata(dictionary.join(path)).unwrap().len(), "sha256": sha256_file(&dictionary.join(path)).unwrap()}));
        }
        std::fs::write(&lock_path, serde_json::to_vec(&lock).unwrap()).unwrap();
        std::fs::write(&record_path, serde_json::to_vec(&record).unwrap()).unwrap();
        let lock = Lock::load(&lock_path).unwrap();
        let build = Build {
            sources: Sources {
                dictionary: Some(Dictionary::open(dictionary.clone(), &lock).unwrap()),
                lock,
                repository_inputs: root.join("resources/dictionary-sources"),
                cache: temporary.path().join("cache"),
                offline: true,
            },
            out: out.clone(),
            complete: false,
        };
        let summary = build.run(Stage::Wubi).unwrap();
        assert!(
            summary.contains("wubi86-rime-wubi.txt at weight 10"),
            "{summary}"
        );
        for stage in [Stage::Wubi98, Stage::WubiXsj] {
            build.run(stage).unwrap();
        }
        // 三份生成表的表头「上游」一行都列出两个 86 原表的上游提交。
        for output in [
            wubi86_supplement::OUTPUT,
            wubi98_supplement::OUTPUT,
            wubixsj_supplement::OUTPUT,
        ] {
            let rendered =
                std::fs::read_to_string(out.join(Path::new(output).file_name().unwrap())).unwrap();
            let upstreams = rendered
                .lines()
                .find(|line| line.starts_with("# 上游："))
                .unwrap();
            for name in [
                wubi86_table::JIDIAN_REFERENCE,
                wubi86_table::RIME_WUBI_REFERENCE,
            ] {
                assert!(
                    upstreams.contains(&format!(
                        "https://example.invalid/{name} 提交 {}",
                        "a".repeat(40)
                    )),
                    "{output}: {upstreams}"
                );
            }
        }
        let db = rusqlite::Connection::open(out.join("msime-pinyin.db")).unwrap();
        let rows: Vec<(String, i64)> = db
            .prepare(
                "SELECT value, weight FROM wubi86 WHERE key='abcd' ORDER BY weight DESC, rowid",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(rows, [("甲".to_owned(), 100), ("甲甲".to_owned(), 10)]);
        // 新世纪表的第一层沿用换算后的权重；甲乙 照常由补充表给出 86 词组码。
        assert_eq!(
            db.query_row(
                "SELECT max(weight) FROM wubixsj WHERE value='甲甲'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            10
        );
        assert_eq!(
            db.query_row("SELECT key FROM wubi86 WHERE value='甲乙'", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            "abef"
        );
        drop(db);

        std::fs::remove_file(dictionary.join(wubi86_table::RIME_WUBI)).unwrap();
        let error = format!("{:#}", build.run(Stage::Wubi).unwrap_err());
        assert!(error.contains("missing"), "{error}");
    }

    /// 收快照之前的 checkout 都不发布新世纪表：只收了上游码表的（PR 合入顺序的中间状态，这时合入的日常词条也会触发发布），和连上游码表都没有、按旧 `upstream.lock.json` 打开的（重建 `dict-v2.0.41` 这类旧提交）。两者的 86、98 照常构建，新世纪阶段跳过而不是失败。
    #[test]
    fn a_checkout_without_the_wubixsj_snapshot_builds_without_the_table() {
        for checkout in [XsjCheckout::SourceOnly, XsjCheckout::WithoutSource] {
            let (_temporary, dictionary, out, build) = wubi_fixture(checkout);
            assert!(!wubixsj_published(&dictionary));
            for stage in [Stage::Wubi, Stage::Wubi98] {
                build.run(stage).unwrap();
            }
            let summary = build.run(Stage::WubiXsj).unwrap();
            assert!(summary.starts_with("left out"), "{summary}");
            let db = rusqlite::Connection::open(out.join("msime-pinyin.db")).unwrap();
            assert!(has_table(&db, "wubi86") && has_table(&db, "wubi98"));
            assert!(!has_table(&db, "wubixsj"));
        }
        // 没收上游码表的 checkout 一旦被要求读它（例如 wubixsj-supplement 子命令）仍然报错，开关不会把缺文件掩盖掉。
        let (_temporary, _, _, build) = wubi_fixture(XsjCheckout::WithoutSource);
        assert!(build.sources.pinned(wubixsj_supplement::WUBIXSJ).is_err());
    }

    /// 七个生成器和 `languages` 必须传 `--dictionary`，不能退回到静默跳过笔画词库之类的路径；`hkcancor-counts` 不读 msime-dictionary，不需要它。
    #[test]
    fn generators_and_languages_require_a_dictionary_checkout() {
        let parses = |arguments: &[&str]| {
            Arguments::try_parse_from(
                std::iter::once("msime-dict-build").chain(arguments.iter().copied()),
            )
            .is_ok()
        };
        for command in [
            &["languages", "--cache", "c"][..],
            &["hanja", "--cache", "c"],
            &["places-supplement", "--cache", "c", "--out", "o"],
            &["english-supplement", "--cache", "c", "--out", "o"],
            &["wubi86-supplement", "--cache", "c", "--out", "o"],
            &["wubi98-supplement", "--cache", "c", "--out", "o"],
            &["wubixsj-supplement", "--cache", "c", "--out", "o"],
            &["wubi86-helpcode", "--out", "o"],
        ] {
            assert!(!parses(command), "{command:?} parsed without --dictionary");
            let with: Vec<&str> = command
                .iter()
                .copied()
                .chain(["--dictionary", "d"])
                .collect();
            assert!(parses(&with), "{with:?}");
        }
        assert!(parses(&["hkcancor-counts", "--cache", "c", "--out", "o"]));
    }

    /// 检查 custom/cantonese.txt 必须同时给出前后两版和字表，单给字表不算要检查的文件。
    #[test]
    fn check_words_needs_the_characters_file_for_cantonese() {
        let parses = |arguments: &[&str]| {
            Arguments::try_parse_from(
                ["msime-dict-build", "check-words"]
                    .into_iter()
                    .chain(arguments.iter().copied()),
            )
            .is_ok()
        };
        assert!(parses(&[
            "--cantonese-base",
            "b",
            "--cantonese-head",
            "h",
            "--cantonese-characters",
            "c",
            "--cantonese-db",
            "d"
        ]));
        assert!(!parses(&["--cantonese-base", "b", "--cantonese-head", "h"]));
        assert!(!parses(&[
            "--cantonese-base",
            "b",
            "--cantonese-characters",
            "c"
        ]));
        assert!(!parses(&["--cantonese-characters", "c"]));
        assert!(parses(&["--base", "b", "--head", "h"]));
    }
}

#[cfg(test)]
#[path = "reweight_stage_tests.rs"]
mod reweight_stage_tests;
