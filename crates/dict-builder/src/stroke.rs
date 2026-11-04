//! `msime-stroke.db`：笔画方案的笔顺码表，按 `msime_engine::language_dictionary` 定义的结构写出。笔顺码来自 rime-stroke（LGPL-3.0，见 `resources/licenses/rime-stroke-LGPL-3.0.txt`）在提交 `COMMIT` 的 `stroke.dict.yaml`，字频来自 msime-dictionary 的 `sources/pinyin/single-chars.txt`（rime-ice 字频，GPL-3.0；`dict-v2.0.30` 起其中与 rime-wanxiang 共有读音的字频改编自 rime-wanxiang，CC BY 4.0，见 `resources/licenses/rime-wanxiang-CC-BY-4.0.txt`；没有记录固定它的大小和 SHA-256，内容只由 `--dictionary` checkout 的提交决定）。
//!
//! `stroke.dict.yaml` 是 Rime 码表：YAML 头以 `...` 一行结束，之后每行 `字<TAB>笔顺码`，`#` 行是注释。码只用 h 横、s 竖、p 撇、n 点（捺）、z 折五个字母，与方案的按键一一对应，所以原样作为 `entries.key`，不加空格。一个字常有几个笔顺码（大陆规范与台湾 CNS11643 的笔顺并列收录，如「小」zpn 与 spn），每个码各成一条。上游没有权重列，Rime 用自己的八股文字频排序；这里改用 `single-chars.txt`：一个字在其中所有读音的权重之和就是它每个笔顺码的权重，表里没有的字权重为 0。
//!
//! 繁体字另从对应的简体字继承权重（`traditional_weights`）。`single-chars.txt` 是简体拼音的字频，繁体字只有万象对齐给的底分或零星的几千分（們 9、時 1592，而 时 849307），笔画方案又不做简繁转换、繁体用户直接打繁体字的笔顺，于是常用繁体字在前缀补全里排到 `COMPLETION_LIMIT` 之外，只有打满全部笔画才出得来（時 在 szhh 下排第 595）。繁简对照取 msime-dictionary 原样收录的 OpenCC `STCharacters.txt`（每个简体字取第一个繁体字形，即 OpenCC 默认的转换结果；不在表里的字取它自己），再经 `TWVariants.txt`、`HKVariants.txt` 补上台湾、香港的写法（爲 之外的 為、裏 之外的 裡，以及 着 → 著、污 → 汙 这类简体字本身就是 OpenCC 繁体字形、只有台湾写法不同的字）；这些字形的权重取自身权重与简体字频除以 `TRADITIONAL_DIVISOR` 中较大的一个。`STCharacters.txt` 第一个以外的字形不继承（鐘、複、髮、乾、曆），所以它们仍要多打几笔：OpenCC 只说明它们也是这个简体字的繁体，不给用法的比例；拿注音库的 `phrase.occ`（台湾语料）去分配也不可靠，它把 臺 记成 0 次、台 记成 53731 次，按它分会让 臺 什么也继承不到；给一个固定的折扣又没有数据依据。OpenCC 的表只决定哪个字继承哪个字的权重，库里不写入它的任何内容，所以 `LICENSE` 不加 OpenCC 的 Apache-2.0，与粤拼库用同两张异体表挑字形的做法相同。
//!
//! 基本区（U+4E00–9FFF）和扩展 A 区（U+3400–4DBF）的汉字都收，其余区段（扩展 B 区及以后、兼容汉字、部首、笔画符号、西夏文部件）只收权重达到 `EXTENSION_MIN_WEIGHT` 的字。上游收了扩展 B 区及以后的七万多个字，macOS 自带字体不覆盖扩展 B 区及以后，它们在候选窗里是方块，而笔数恰好打满时它们作为精确匹配排在常用字的补全前面。引擎把权重不大于 0 的字排到最后，这条判断不改：基本区有 4192 个字（們、與、裏）也只有底分 9，统一抬高门槛会把它们一起压下去，所以门槛只在构建时对基本区和扩展 A 区以外的字生效。
//!
//! `syllables` 表固定是五个笔画字母，引擎只拿它确认词典非空。上游有三个笔顺码超过引擎的 64 笔上限（最长 84 笔），它们照常写入，只能经前缀补全找到。
//!
//! `msime-stroke.db` 从 `--dictionary` checkout 读取 `sources/stroke/stroke.dict.yaml`，并按它的 `upstream.lock.json` 校验。`languages` 必须传 `--dictionary`，所以 `source` 里按常量校验 `--cache` 文件的兜底分支在命令行上已经走不到，只为不删代码而保留。

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use msime_engine::stroke::scheme::COMPLETION_LIMIT;

use crate::sources::{sha256_file, Sources};
use crate::text;

pub const SOURCE: &str = "sources/stroke/stroke.dict.yaml";
/// 字频来源，quanpin 阶段也读同一份文件（来自 `--dictionary` checkout，没有记录固定它的摘要）。
pub const FREQUENCIES: &str = "sources/pinyin/single-chars.txt";
/// OpenCC 的简转繁字表（msime-dictionary 原样收录、`upstream.lock.json` 固定），只用来决定繁体字继承哪个简体字的权重；台湾、香港的写法再按 `cantonese::TW_VARIANTS`、`cantonese::HK_VARIANTS` 补上。
pub const SIMPLIFIED_TO_TRADITIONAL: &str = "sources/opencc/STCharacters.txt";
/// 基本区和扩展 A 区以外的字收进库所需的最低权重。`single-chars.txt` 里这些字绝大多数只有万象对齐给的底分：每个读音 9 分，几个读音相加最多 38；真正有字频的（𰻝 3580、𬭤 2251 这类《通用规范汉字表》的字）最少也有 301，100 落在两者之间的空档里。
pub const EXTENSION_MIN_WEIGHT: i64 = 100;
/// 繁体字继承的权重是简体字频的几分之一。除数越小，繁体用户打得越少，简体用户的补全里挤进的繁体字也越多；按 `single-chars.txt` 字频最高的 4000 个简体字（以及它们经 OpenCC 换成的台湾、香港字形）模拟引擎的排序、每页 9 个候选，除数取 1 时简体用户平均每字要多打 0.08 笔，取 4 时只多 0.015 笔，而台湾用户每字少打 0.42 笔，是除数取 1 时能省下的 0.52 笔的八成；取 16 时又只省得下 0.36 笔。
pub const TRADITIONAL_DIVISOR: i64 = 4;
/// 锁文件里的上游引用名，它的提交就是 `source_commit`。
pub const REFERENCE: &str = "rime-stroke";
pub const REPOSITORY: &str = "https://github.com/rime/rime-stroke";
/// 许可证覆盖的上游提交，锁文件没有 `rime-stroke` 引用时作为 `source_commit`。
pub const COMMIT: &str = "1e8fff9b9494ddec23b0cbc526bcfd8171a6fd48";
/// `COMMIT` 处原样文件的大小与 SHA-256，msime-dictionary 的 `upstream.lock.json` 记录同样的值。`source` 的缓存兜底分支用它们校验手动放进缓存的文件，但这个分支在命令行上已经走不到。
pub const SOURCE_SIZE: u64 = 3_396_330;
pub const SOURCE_SHA256: &str = "b3e93dce89c185f45c3d6e189b86b3a8626913352cc85e1094c786579a665791";
/// 五种笔画，也是 `syllables` 表的全部内容。
pub const STROKES: [char; 5] = ['h', 's', 'p', 'n', 'z'];
/// 记入数据库 `license` 的 SPDX 表达式，每个进入库里的来源各一项：笔顺码是 rime-stroke 的 LGPL-3.0；权重是 `single-chars.txt` 里一个字各读音的字频之和，这些字频取自 rime-ice（GPL-3.0），与 rime-wanxiang 共有的读音改编自 rime-wanxiang（CC BY 4.0）。`single-chars.txt` 的读音按 MIT 的 pinyin-data 校正，但库里不存读音，所以不列 MIT。随库分发的 `LICENSE_NAME` 写着这三项的署名。
pub const LICENSE: &str = "LGPL-3.0-only AND GPL-3.0-only AND CC-BY-4.0";
pub const DATABASE: &str = "msime-stroke.db";
/// `resources/licenses/` 里的许可证文本，以及它在数据库旁边的文件名。
pub const LICENSE_SOURCE: &str = "rime-stroke-LGPL-3.0.txt";
pub const LICENSE_NAME: &str = "msime-rime_stroke_LICENSE.txt";

/// 发布构建时 `verify` 要求的下限。固定提交经 `kept` 过滤后的实际值是 47095 条、27588 个字、7678 个有字频的字。
pub const FLOORS: Floors = Floors {
    entries: 45_000,
    characters: 27_000,
    weighted: 7_000,
};

/// 发布构建时 `verify` 要求的上限，防的是扩展 B 区以后的生僻字又被放进来：`dict-v2.0.55` 的 `single-chars.txt` 给扩展 B 区以后 14736 个字都记了底分 9，`kept` 只看权重是否大于 0 时库里有 72800 条、42321 个字，按 `EXTENSION_MIN_WEIGHT` 过滤后是 47314 条、27765 个字。
pub const CEILINGS: Ceilings = Ceilings {
    entries: 50_000,
    characters: 29_500,
};

/// 这些前缀的前 `within` 个补全里必须有对应的繁体字，`within` 不超过引擎读的 `COMPLETION_LIMIT`，守住繁体字从简体字继承的权重：不继承时 時 在 szhh 下排第 595、國 在 szhs 下排第 190，都在引擎读的范围之外。著 守住「简体字本身就是 OpenCC 繁体字形、只有台湾写法不同」的继承（着 → 著）：只按 `STCharacters.txt` 继承时 著 只有自身的 32697 分，在 hsshs 下被继承了权重的 難、藍、葉 挤到第 8 位，继承 着 的四分之一后排第 3。
pub const EXPECTED_COMPLETIONS: [Expected; 5] = [
    Expected {
        key: "szhh",
        texts: &["時"],
        within: COMPLETION_LIMIT,
    },
    Expected {
        key: "szhs",
        texts: &["國"],
        within: COMPLETION_LIMIT,
    },
    Expected {
        key: "pssz",
        texts: &["們"],
        within: 10,
    },
    Expected {
        key: "nhsz",
        texts: &["裏"],
        within: 10,
    },
    Expected {
        key: "hsshs",
        texts: &["著"],
        within: 5,
    },
];

/// 这些码的前 `within` 条里必须有对应的字，用来发现排序上的大错。
pub const EXPECTED: [Expected; 8] = [
    Expected {
        key: "h",
        texts: &["一"],
        within: 1,
    },
    Expected {
        key: "hh",
        texts: &["二"],
        within: 1,
    },
    Expected {
        key: "hhh",
        texts: &["三"],
        within: 1,
    },
    Expected {
        key: "hs",
        texts: &["十"],
        within: 1,
    },
    Expected {
        key: "szh",
        texts: &["口"],
        within: 1,
    },
    Expected {
        key: "pn",
        texts: &["人"],
        within: 1,
    },
    // 「小」的大陆笔顺 spn 与台湾笔顺 zpn 都要排在首位。
    Expected {
        key: "spn",
        texts: &["小"],
        within: 1,
    },
    Expected {
        key: "zpn",
        texts: &["小"],
        within: 1,
    },
];

#[derive(Debug, Clone, Copy)]
pub struct Expected {
    pub key: &'static str,
    pub texts: &'static [&'static str],
    pub within: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct Floors {
    pub entries: usize,
    pub characters: usize,
    pub weighted: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct Ceilings {
    pub entries: usize,
    pub characters: usize,
}

/// `stroke.dict.yaml` 的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row<'a> {
    pub text: &'a str,
    /// 只含 `STROKES` 字母的笔顺码。
    pub code: &'a str,
}

#[derive(Debug, Default)]
pub struct Dictionary {
    /// `(key, text)` 到权重。
    pub entries: BTreeMap<(String, String), i64>,
    /// 权重因为从简体字继承而变大的字数。
    pub inherited: usize,
}

/// 构建读取的 `stroke.dict.yaml`：有 `--dictionary` checkout 时按 `Sources::pinned` 读（按 checkout 的 `upstream.lock.json` 校验），文件缺失或不符都报错。`languages` 必须传 `--dictionary`，所以命令行构建总是走这一支；后面的缓存兜底（只接受 `--cache` 下大小与 SHA-256 等于 `SOURCE_SIZE`、`SOURCE_SHA256` 的文件，都没有时返回 `None`）只有不带 checkout 的 `Sources` 才会走到，目前只有单元测试这样构造。
pub fn source(sources: &Sources) -> Result<Option<PathBuf>> {
    if sources.lock.files.iter().any(|file| file.path == SOURCE)
        || sources.checkout_file(SOURCE).is_some()
    {
        return sources.pinned(SOURCE).map(Some);
    }
    let cached = sources.cache.join(SOURCE);
    if !cached.exists() {
        return Ok(None);
    }
    check_cached(&cached, SOURCE_SIZE, SOURCE_SHA256)?;
    Ok(Some(cached))
}

fn check_cached(path: &Path, size: u64, sha256: &str) -> Result<()> {
    if !path.is_file() {
        bail!(
            "{SOURCE} is not in the cache at {} and no --dictionary checkout was given; download {REPOSITORY}/raw/{COMMIT}/stroke.dict.yaml there",
            path.display()
        );
    }
    let found_size = std::fs::metadata(path)
        .with_context(|| format!("reading {}", path.display()))?
        .len();
    let found_sha256 = sha256_file(path)?;
    if found_size != size || found_sha256 != sha256 {
        bail!(
            "{}: {found_size} bytes with sha256 {found_sha256}, rime-stroke {COMMIT} has {size} bytes with {sha256}",
            path.display()
        );
    }
    Ok(())
}

/// 记为 `source_commit` 的上游提交：锁文件的 `rime-stroke` 引用，未固定时是 `COMMIT`。
pub fn source_commit(sources: &Sources) -> &str {
    sources
        .lock
        .references
        .get(REFERENCE)
        .map_or(COMMIT, |reference| reference.commit.as_str())
}

/// YAML 头之后的 `字<TAB>笔顺码` 行。空行与 `#` 注释跳过；其他格式（权重列、多字词条、笔画以外的字母）都让构建失败，并写明行号。
pub fn parse(source: &str) -> Result<Vec<Row<'_>>> {
    let mut lines = source.lines().enumerate();
    if !lines.by_ref().any(|(_, line)| line == "...") {
        bail!("{SOURCE}: no `...` line ends the YAML header");
    }
    let mut rows = Vec::new();
    for (index, line) in lines {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let number = index + 1;
        let fields: Vec<&str> = line.split('\t').collect();
        let [text, code] = fields[..] else {
            bail!("{SOURCE}: line {number} is not character<TAB>strokes");
        };
        if text.chars().count() != 1 {
            bail!("{SOURCE}: line {number}: {text:?} is not one character");
        }
        if code.is_empty() || !code.chars().all(|stroke| STROKES.contains(&stroke)) {
            bail!("{SOURCE}: line {number}: {code:?} is not a sequence of h, s, p, n and z");
        }
        rows.push(Row { text, code });
    }
    Ok(rows)
}

/// `single-chars.txt` 的 `字<TAB>拼音<TAB>权重` 行（CRLF 换行，`#` 行跳过），每个字所有读音的权重之和。
pub fn parse_frequencies(source: &str) -> Result<HashMap<&str, i64>> {
    let mut weights: HashMap<&str, i64> = HashMap::new();
    for (index, line) in source.lines().enumerate() {
        let line = text::strip(line);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let number = index + 1;
        let fields: Vec<&str> = line.split('\t').collect();
        let [text, _, weight] = fields[..] else {
            bail!("{FREQUENCIES}: line {number} is not character<TAB>pinyin<TAB>weight");
        };
        let weight = weight
            .parse::<i64>()
            .ok()
            .filter(|weight| *weight >= 0)
            .with_context(|| format!("{FREQUENCIES}: line {number}: {weight:?} is not a weight"))?;
        let total = weights.entry(text).or_insert(0);
        *total = total.checked_add(weight).with_context(|| {
            format!("{FREQUENCIES}: line {number}: the weights of {text:?} overflow")
        })?;
    }
    Ok(weights)
}

/// 繁体字形从简体字继承的权重：`frequencies` 里每个有字频的字，取它在 `simplified_to_traditional`（`STCharacters.txt`）里的第一个繁体字形（不在表里、或第一个字形就是它自己时取它自己），再加上这个字形在 `tw_variants`、`hk_variants` 里的第一个异体，每个与这个字不同的字形都记它字频的 `TRADITIONAL_DIVISOR` 分之一；几个字落到同一个字形时取最大的。所以 着 这类不在 `STCharacters.txt` 里、台湾写法却不同的字（着 → 著）也带出台湾字形。字本身不在结果里，字频为 0 的字不带出任何字形。`STCharacters.txt` 第一个以外的字形（钟 → 鍾 鐘 的 鐘、复 → 復 複 覆 的 複）不继承：OpenCC 只给出它们是哪个字的繁体，不给用法的比例，见模块文档。
pub fn traditional_weights(
    frequencies: &HashMap<&str, i64>,
    simplified_to_traditional: &[(char, Vec<char>)],
    tw_variants: &[(char, Vec<char>)],
    hk_variants: &[(char, Vec<char>)],
) -> HashMap<char, i64> {
    let first = |pairs: &[(char, Vec<char>)]| -> HashMap<char, char> {
        pairs
            .iter()
            .filter_map(|(key, values)| Some((*key, *values.first()?)))
            .collect()
    };
    let (traditional, tw, hk) = (
        first(simplified_to_traditional),
        first(tw_variants),
        first(hk_variants),
    );
    let mut inherited: HashMap<char, i64> = HashMap::new();
    for (text, frequency) in frequencies {
        let mut characters = text.chars();
        let (Some(character), None) = (characters.next(), characters.next()) else {
            continue;
        };
        let weight = frequency / TRADITIONAL_DIVISOR;
        if weight <= 0 {
            continue;
        }
        let form = traditional.get(&character).copied().unwrap_or(character);
        let forms = [
            form,
            tw.get(&form).copied().unwrap_or(form),
            hk.get(&form).copied().unwrap_or(form),
        ];
        for form in forms {
            if form != character {
                let slot = inherited.entry(form).or_insert(0);
                *slot = (*slot).max(weight);
            }
        }
    }
    inherited
}

/// 每个 `(笔顺码, 字)` 一条，权重是该字的字频之和与 `inherited`（`traditional_weights`）给它的权重中较大的一个；同一行重复出现只留一条。`kept` 不收的字跳过，`cantonese` 是粤拼词表用到的字，扩展区里的这些字不受门槛限制。
pub fn build(
    rows: &[Row],
    frequencies: &HashMap<&str, i64>,
    inherited: &HashMap<char, i64>,
    cantonese: &HashSet<char>,
) -> Dictionary {
    let mut dictionary = Dictionary::default();
    let mut raised = BTreeSet::new();
    for row in rows {
        let own = frequencies.get(row.text).copied().unwrap_or(0);
        let weight = row
            .text
            .chars()
            .next()
            .and_then(|character| inherited.get(&character))
            .map_or(own, |inherited| own.max(*inherited));
        if !kept(row.text, weight, cantonese) {
            continue;
        }
        if weight > own {
            raised.insert(row.text);
        }
        dictionary
            .entries
            .insert((row.code.to_owned(), row.text.to_owned()), weight);
    }
    dictionary.inherited = raised.len();
    dictionary
}

/// 基本区和扩展 A 区的汉字都收；其他字符（扩展 B 区及以后、兼容汉字、部首、笔画符号、西夏文部件）只在权重达到 `EXTENSION_MIN_WEIGHT`、或者粤拼词表用到这个字（`cantonese`）时收。
fn kept(text: &str, weight: i64, cantonese: &HashSet<char>) -> bool {
    let Some(character) = text.chars().next() else {
        return false;
    };
    matches!(character, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}')
        || weight >= EXTENSION_MIN_WEIGHT
        || cantonese.contains(&character)
}

/// 粤拼词表（`cantonese::WORDS`）的词用到的字，给 `build` 豁免扩展区门槛：香港粤语用字很多在扩展 B 区及以后（𠺝、𢳂、𧨾），字频表只给它们底分，按门槛会被删掉，香港的笔画用户就打不出粤拼库里有的字。只取词表，不取字表：字表收了约两千个扩展区字，大半不在任何词里。
pub fn cantonese_characters(words: &[crate::cantonese::Row]) -> HashSet<char> {
    words.iter().flat_map(|row| row.text.chars()).collect()
}

/// 按引擎的语言词典结构把 `dictionary` 写到 `path`（先删掉旧文件），最后 freeze，同样的输入得到同样的字节。
pub fn write(dictionary: &Dictionary, path: &Path, source_commit: &str) -> Result<()> {
    let strokes: BTreeSet<String> = STROKES.iter().map(char::to_string).collect();
    crate::language_writer::write(
        path,
        source_commit,
        LICENSE,
        strokes.iter().map(String::as_str),
        &dictionary.entries,
    )
}

/// `verify` 在写好的数据库里数出的值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub syllables: usize,
    pub entries: usize,
    pub characters: usize,
    /// 有字频（权重大于 0）的字数。
    pub weighted: usize,
}

/// 像引擎那样打开写好的数据库；`syllables` 不是五个笔画、数量低于 `floors` 或高于 `ceilings`、`expected` 的码前几条里没有对应的字，或 `completions` 的前缀前几个补全里没有对应的字时失败。
pub fn verify(
    path: &Path,
    floors: Floors,
    ceilings: Ceilings,
    expected: &[Expected],
    completions: &[Expected],
) -> Result<Counts> {
    let (dictionary, connection) = crate::language_verify::open(path)?;
    let count = |sql| crate::language_verify::count(&connection, sql);
    let counts = Counts {
        syllables: count("SELECT count(*) FROM syllables")?,
        entries: count("SELECT count(*) FROM entries")?,
        characters: count("SELECT count(DISTINCT text) FROM entries")?,
        weighted: count("SELECT count(DISTINCT text) FROM entries WHERE weight > 0")?,
    };
    for stroke in STROKES {
        let stroke = stroke.to_string();
        let found = dictionary
            .has_syllable(&stroke)
            .map_err(|error| anyhow::anyhow!("{}: {error}", path.display()))?;
        if !found {
            bail!("{}: the stroke {stroke:?} is not listed", path.display());
        }
    }
    if counts.syllables != STROKES.len() {
        bail!(
            "{}: {} syllables, expected the {} strokes",
            path.display(),
            counts.syllables,
            STROKES.len()
        );
    }
    crate::language_verify::check_floors(
        path,
        &[
            ("entries", counts.entries, floors.entries),
            ("characters", counts.characters, floors.characters),
            ("weighted characters", counts.weighted, floors.weighted),
        ],
    )?;
    for (what, found, ceiling) in [
        ("entries", counts.entries, ceilings.entries),
        ("characters", counts.characters, ceilings.characters),
    ] {
        if found > ceiling {
            bail!(
                "{}: {found} {what}, above the ceiling of {ceiling}",
                path.display()
            );
        }
    }
    for check in completions {
        let top = dictionary
            .lookup_completions(check.key, check.within)
            .map_err(|error| anyhow::anyhow!("{}: {error}", path.display()))?;
        if !top
            .iter()
            .any(|(_, entry)| check.texts.contains(&entry.text.as_str()))
        {
            bail!(
                "{}: the first {} completions of {:?} lack {}",
                path.display(),
                check.within,
                check.key,
                check.texts.join(" or ")
            );
        }
    }
    for check in expected {
        let top = dictionary
            .lookup(check.key, check.within)
            .map_err(|error| anyhow::anyhow!("{}: {error}", path.display()))?;
        if !top
            .iter()
            .any(|entry| check.texts.contains(&entry.text.as_str()))
        {
            let found: Vec<&str> = top.iter().map(|entry| entry.text.as_str()).collect();
            bail!(
                "{}: {:?} offers {found:?} first, without {}",
                path.display(),
                check.key,
                check.texts.join(" or ")
            );
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_engine::language_dictionary;

    const HEADER: &str = "# Rime dictionary: stroke\n# encoding: utf-8\n#\n# h,s,p,n,z\n\n---\nname: stroke\nversion: \"2.0\"\nsort: by_weight\n...\n\n";

    fn stroke_source() -> String {
        format!(
            "{HEADER}甲\th\n乙\th\n丙\thh\n丁\thsp\n戊\tzpn\n戊\tspn\n# 注释行\n己\tzpn\n庚\tzpn\n"
        )
    }

    // 合成字频：乙有两个读音，权重相加；丁、庚不在表里，权重为 0。
    const FREQUENCIES_SOURCE: &str =
        "#  字表\r\n甲\tjia\t50\r\n乙\tyi\t30\r\n乙\tzhe\t40\r\n丙\tbing\t9\r\n戊\twu\t100\r\n己\tji\t60\r\n辛\txin\t5\r\n";

    fn sample() -> Dictionary {
        let source = stroke_source();
        build(
            &parse(&source).unwrap(),
            &parse_frequencies(FREQUENCIES_SOURCE).unwrap(),
            &HashMap::new(),
            &HashSet::new(),
        )
    }

    fn entries(dictionary: &Dictionary) -> Vec<(&str, &str, i64)> {
        dictionary
            .entries
            .iter()
            .map(|((key, text), weight)| (key.as_str(), text.as_str(), *weight))
            .collect()
    }

    #[test]
    fn rows_after_the_header_keep_their_stroke_letters() {
        let source = stroke_source();
        let rows = parse(&source).unwrap();
        assert_eq!(rows.len(), 8);
        assert_eq!(
            rows[0],
            Row {
                text: "甲",
                code: "h"
            }
        );
        // 同一个字的第二个笔顺码是独立的一行。
        assert_eq!(
            rows[5],
            Row {
                text: "戊",
                code: "spn"
            }
        );
    }

    #[test]
    fn frequencies_sum_every_reading_of_a_character() {
        let frequencies = parse_frequencies(FREQUENCIES_SOURCE).unwrap();
        assert_eq!(frequencies["乙"], 70);
        assert_eq!(frequencies["甲"], 50);
        assert_eq!(frequencies.len(), 6);
    }

    #[test]
    fn only_common_blocks_and_characters_with_a_frequency_are_kept() {
        let none = HashSet::new();
        // 基本区与扩展 A 区：没有字频也收。
        assert!(kept("口", 0, &none));
        assert!(kept("\u{3AD1}", 0, &none));
        // 扩展 B 区及以后、西夏文部件、部首、笔画符号、兼容汉字：没有字频，或者只有万象对齐给的底分（每个读音 9 分），都不收。
        for text in [
            "\u{20BB9}",
            "\u{31480}",
            "\u{18800}",
            "\u{2E8A}",
            "\u{31C0}",
            "\u{F900}",
        ] {
            for weight in [0, 9, 18, 38, EXTENSION_MIN_WEIGHT - 1] {
                assert!(!kept(text, weight, &none), "{text} {weight}");
            }
        }
        // 达到门槛的字照收，不论区段；基本区的底分字（們 9）照收。
        assert!(kept("\u{20BB9}", EXTENSION_MIN_WEIGHT, &none));
        assert!(kept("們", 9, &none));
        // 粤拼词表用到的扩展区字（𠺝）只有底分也收。
        let words = format!("{}𠺝\tlaa3\n", "---\nname: jyut6ping3.words\n...\n");
        let words = crate::cantonese::parse_words(&words).unwrap();
        let cantonese = cantonese_characters(&words);
        assert!(kept("\u{20E9D}", 9, &cantonese));
        assert!(!kept("\u{20BB9}", 9, &cantonese));
        let rows = [
            Row {
                text: "口",
                code: "szh",
            },
            Row {
                text: "\u{20BB9}",
                code: "szhhsp",
            },
            Row {
                text: "\u{3AD1}",
                code: "szhhzp",
            },
        ];
        let dictionary = build(&rows, &HashMap::new(), &HashMap::new(), &none);
        let texts: Vec<&str> = dictionary
            .entries
            .keys()
            .map(|(_, text)| text.as_str())
            .collect();
        assert_eq!(texts, ["口", "\u{3AD1}"]);
    }

    #[test]
    fn every_code_of_a_character_carries_its_frequency() {
        assert_eq!(
            entries(&sample()),
            [
                ("h", "乙", 70),
                ("h", "甲", 50),
                ("hh", "丙", 9),
                // 不在字频表里的字权重为 0。
                ("hsp", "丁", 0),
                ("spn", "戊", 100),
                ("zpn", "己", 60),
                ("zpn", "庚", 0),
                ("zpn", "戊", 100),
            ]
        );
    }

    /// 合成的繁简对照：么 的第一个繁体字形是它自己，也没有台湾、香港写法，不继承；为 → 爲，台湾写 為；里 → 裏，台湾写 裡、香港仍写 裏；发 的两个繁体字形只有第一个 發 继承；云 的字频为 0，什么也不带出；后 和（合成的）夂 都落到 後 上，取较大的；污 的第一个繁体字形是它自己，台湾写 汙；着 不在简转繁表里，台湾写 著。
    fn traditional_sample() -> HashMap<char, i64> {
        let frequencies = parse_frequencies(
            "么\tme\t4000\r\n为\twei\t9000\r\n里\tli\t801\r\n发\tfa\t40\r\n云\tyun\t0\r\n后\thou\t400\r\n夂\tzhi\t40\r\n污\twu\t120\r\n着\tzhe\t800\r\n",
        )
        .unwrap();
        let pairs = |name, source| crate::cantonese::parse_variants(name, source).unwrap();
        traditional_weights(
            &frequencies,
            &pairs(
                SIMPLIFIED_TO_TRADITIONAL,
                "么\t么 麼\n为\t爲\n里\t裏 里\n发\t發 髮\n云\t雲\n后\t後 后\n夂\t後\n污\t污\n",
            ),
            &pairs(
                crate::cantonese::TW_VARIANTS,
                "爲\t為\n裏\t裡\n污\t汙\n着\t著\n",
            ),
            &pairs(crate::cantonese::HK_VARIANTS, "爲\t為\n"),
        )
    }

    #[test]
    fn traditional_forms_inherit_a_share_of_their_simplified_weight() {
        let inherited = traditional_sample();
        let mut found: Vec<(char, i64)> = inherited.into_iter().collect();
        found.sort_unstable();
        // 9000 / 4 = 2250，801 / 4 = 200（整除），40 / 4 = 10，400 / 4 = 100，120 / 4 = 30，800 / 4 = 200。
        let mut expected = vec![
            ('爲', 2250),
            ('為', 2250),
            ('裏', 200),
            ('裡', 200),
            ('發', 10),
            ('後', 100),
            ('汙', 30),
            ('著', 200),
        ];
        expected.sort_unstable();
        assert_eq!(found, expected);
    }

    #[test]
    fn build_keeps_the_larger_of_the_own_and_the_inherited_weight() {
        let inherited = traditional_sample();
        let rows = [
            Row {
                text: "爲",
                code: "pnnppzzznnnn",
            },
            Row {
                text: "為",
                code: "npzzznnnn",
            },
            Row {
                text: "後",
                code: "ppszznzpn",
            },
            Row {
                text: "为",
                code: "nnzn",
            },
        ];
        // 後 自身有 200 分，比继承的 100 大，留自身的；為 自身 47 分，继承 2250。
        let frequencies =
            parse_frequencies("後\thou\t200\r\n為\twei\t47\r\n为\twei\t9000\r\n").unwrap();
        let dictionary = build(&rows, &frequencies, &inherited, &HashSet::new());
        assert_eq!(
            entries(&dictionary),
            [
                ("nnzn", "为", 9000),
                ("npzzznnnn", "為", 2250),
                ("pnnppzzznnnn", "爲", 2250),
                ("ppszznzpn", "後", 200),
            ]
        );
        assert_eq!(dictionary.inherited, 2);
        // 扩展 B 区以后的字继承到门槛以上的权重也照收：门槛看的是最终权重。
        let rare = [Row {
            text: "\u{2B7A9}",
            code: "zh",
        }];
        let inherited = HashMap::from([('\u{2B7A9}', EXTENSION_MIN_WEIGHT)]);
        assert_eq!(
            build(&rare, &HashMap::new(), &inherited, &HashSet::new())
                .entries
                .len(),
            1
        );
        assert!(
            build(&rare, &HashMap::new(), &HashMap::new(), &HashSet::new())
                .entries
                .is_empty()
        );
    }

    #[test]
    fn malformed_rows_name_their_line() {
        for (source, expected) in [
            ("name: stroke\n甲\th\n".to_owned(), "no `...` line"),
            (
                format!("{HEADER}甲\n"),
                "line 12 is not character<TAB>strokes",
            ),
            (
                format!("{HEADER}甲\th\t100\n"),
                "line 12 is not character<TAB>strokes",
            ),
            (
                format!("{HEADER}甲乙\th\n"),
                "line 12: \"甲乙\" is not one character",
            ),
            (format!("{HEADER}\th\n"), "\"\" is not one character"),
            (
                format!("{HEADER}甲\thx\n"),
                "line 12: \"hx\" is not a sequence of h, s, p, n and z",
            ),
            (format!("{HEADER}甲\t12\n"), "\"12\" is not a sequence"),
            (format!("{HEADER}甲\t\n"), "\"\" is not a sequence"),
        ] {
            let error = parse(&source).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
        for (source, expected) in [
            ("甲\tjia\n", "line 1 is not character<TAB>pinyin<TAB>weight"),
            ("甲\tjia\t-1\n", "line 1: \"-1\" is not a weight"),
            ("甲\tjia\tmany\n", "\"many\" is not a weight"),
            (
                "甲\tjia\t9223372036854775807\r\n甲\tjia\t1\r\n",
                "line 2: the weights of \"甲\" overflow",
            ),
        ] {
            let error = parse_frequencies(source).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn the_cached_source_must_match_the_upstream_digest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SOURCE);
        let error = check_cached(&path, 7, "0").unwrap_err().to_string();
        assert!(
            error.contains(&format!("{REPOSITORY}/raw/{COMMIT}/stroke.dict.yaml")),
            "{error}"
        );
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"fixture").unwrap();
        let digest = sha256_file(&path).unwrap();
        check_cached(&path, 7, &digest).unwrap();
        let error = check_cached(&path, 8, &digest).unwrap_err().to_string();
        assert!(error.contains("7 bytes with sha256"), "{error}");
        let error = check_cached(&path, 7, &"0".repeat(64))
            .unwrap_err()
            .to_string();
        assert!(error.contains(&digest), "{error}");
    }

    /// 锁文件没固定、缓存里也没有源文件时不构建笔画词库，而不是让整个 `languages` 失败；缓存里放了不对的文件仍然报错。
    #[test]
    fn an_unpinned_and_uncached_source_is_skipped() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut lock =
            crate::sources::Lock::load(&root.join("resources/dictionary-sources.lock.json"))
                .unwrap();
        lock.files.retain(|file| file.path != SOURCE);
        let dir = tempfile::tempdir().unwrap();
        let sources = Sources {
            lock,
            repository_inputs: root.join("resources/dictionary-sources"),
            cache: dir.path().to_path_buf(),
            offline: true,
            dictionary: None,
        };
        assert!(source(&sources).unwrap().is_none());
        let cached = dir.path().join(SOURCE);
        std::fs::create_dir_all(cached.parent().unwrap()).unwrap();
        std::fs::write(&cached, b"not rime-stroke").unwrap();
        assert!(source(&sources).is_err());
    }

    /// 锁文件的 `rime-stroke` 引用正是许可证覆盖的提交，锁文件不固定 `sources/stroke/` 下的任何文件，`stroke.dict.yaml` 是 msime 认定的 rime-stroke 上游数据。
    #[test]
    fn the_lock_references_the_covered_commit_and_pins_no_source() {
        let lock = crate::sources::Lock::load(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/dictionary-sources.lock.json"),
        )
        .unwrap();
        assert_eq!(lock.references[REFERENCE].commit, COMMIT);
        assert!(!lock
            .files
            .iter()
            .any(|file| file.path.starts_with("sources/stroke/")));
        assert_eq!(crate::sources::upstream_reference(SOURCE), Some(REFERENCE));
    }

    /// 库里的字频之和含有改编自 rime-wanxiang 的字频（「万」2702880 = 2701552 + 1328），所以 `license` 要写上 CC-BY-4.0，随库分发的许可证文本也要有 CC BY 4.0 第 3(a) 条要的署名：作者、上游与固定提交、许可证地址、改动说明和免责声明的指引。
    #[test]
    fn the_licence_names_every_source_the_weights_come_from() {
        assert_eq!(LICENSE, "LGPL-3.0-only AND GPL-3.0-only AND CC-BY-4.0");
        let text = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/licenses")
                .join(LICENSE_SOURCE),
        )
        .unwrap();
        for phrase in [
            "iDvel/rime-ice (GPL-3.0-only)",
            "amzxyz/rime-wanxiang",
            "7ab73ee0c7f80190004233e70f954833df64c80e",
            "Creator: amzxyz",
            "https://creativecommons.org/licenses/by/4.0/",
            "rime-wanxiang-CC-BY-4.0.txt",
            "disclaimer of warranties",
        ] {
            assert!(text.contains(phrase), "{LICENSE_SOURCE} lacks {phrase:?}");
        }
    }

    fn written(dictionary: &Dictionary) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DATABASE);
        write(dictionary, &path, COMMIT).unwrap();
        (dir, path)
    }

    #[test]
    fn the_engine_reads_what_is_written_and_the_bytes_repeat() {
        let dictionary = sample();
        let (_dir, path) = written(&dictionary);
        let engine = language_dictionary::open_read_only(&path).unwrap();
        let texts: Vec<String> = engine
            .lookup("zpn", 10)
            .unwrap()
            .into_iter()
            .map(|entry| entry.text)
            .collect();
        assert_eq!(texts, ["戊", "己", "庚"]);
        // 笔顺码没有空格，前缀补全覆盖以它开头的所有码。
        let completions: Vec<(String, String)> = engine
            .lookup_completions("h", 10)
            .unwrap()
            .into_iter()
            .map(|(key, entry)| (key, entry.text))
            .collect();
        assert_eq!(
            completions,
            [
                ("h".to_owned(), "乙".to_owned()),
                ("h".to_owned(), "甲".to_owned()),
                ("hh".to_owned(), "丙".to_owned()),
                ("hsp".to_owned(), "丁".to_owned()),
            ]
        );
        assert_eq!(engine.syllables().unwrap().len(), 5);
        assert!(engine.has_syllable("z").unwrap());
        assert!(!engine.has_syllable("x").unwrap());
        assert_eq!(
            engine
                .metadata(language_dictionary::METADATA_SOURCE_COMMIT)
                .unwrap()
                .as_deref(),
            Some(COMMIT)
        );
        assert_eq!(
            engine
                .metadata(language_dictionary::METADATA_LICENSE)
                .unwrap()
                .as_deref(),
            Some("LGPL-3.0-only AND GPL-3.0-only AND CC-BY-4.0")
        );
        let first = std::fs::read(&path).unwrap();
        write(&dictionary, &path, COMMIT).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), first);
    }

    #[test]
    fn verify_enforces_the_floors_and_the_expected_entries() {
        let (_dir, path) = written(&sample());
        let floors = Floors {
            entries: 8,
            characters: 7,
            weighted: 5,
        };
        let ceilings = Ceilings {
            entries: 8,
            characters: 7,
        };
        // h 的补全按权重是 乙 70、甲 50、丙 9、丁 0。
        let completions = [Expected {
            key: "h",
            texts: &["丙"],
            within: 3,
        }];
        let expected = [
            Expected {
                key: "h",
                texts: &["乙"],
                within: 1,
            },
            Expected {
                key: "zpn",
                texts: &["己", "庚"],
                within: 2,
            },
        ];
        assert_eq!(
            verify(&path, floors, ceilings, &expected, &completions).unwrap(),
            Counts {
                syllables: 5,
                entries: 8,
                characters: 7,
                weighted: 5
            }
        );
        for (below, what) in [
            (
                Floors {
                    entries: 9,
                    ..floors
                },
                "8 entries, below the floor of 9",
            ),
            (
                Floors {
                    characters: 8,
                    ..floors
                },
                "7 characters, below the floor of 8",
            ),
            (
                Floors {
                    weighted: 6,
                    ..floors
                },
                "5 weighted characters, below the floor of 6",
            ),
        ] {
            let error = verify(&path, below, ceilings, &expected, &completions)
                .unwrap_err()
                .to_string();
            assert!(error.contains(what), "{error}");
        }
        let error = verify(&path, FLOORS, ceilings, &expected, &completions)
            .unwrap_err()
            .to_string();
        assert!(error.contains("below the floor"), "{error}");
        for (above, what) in [
            (
                Ceilings {
                    entries: 7,
                    ..ceilings
                },
                "8 entries, above the ceiling of 7",
            ),
            (
                Ceilings {
                    characters: 6,
                    ..ceilings
                },
                "7 characters, above the ceiling of 6",
            ),
        ] {
            let error = verify(&path, floors, above, &expected, &completions)
                .unwrap_err()
                .to_string();
            assert!(error.contains(what), "{error}");
        }
        // 丙 是 h 的第 3 个补全，只看前 2 个时补全检查失败。
        let narrow = [Expected {
            within: 2,
            ..completions[0]
        }];
        let error = verify(&path, floors, ceilings, &expected, &narrow)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("the first 2 completions of \"h\" lack 丙"),
            "{error}"
        );

        // 甲 不是 h 的首选，首位检查失败。
        let check = [Expected {
            key: "h",
            texts: &["甲"],
            within: 1,
        }];
        let error = verify(&path, floors, ceilings, &check, &completions)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("\"h\" offers [\"乙\"] first, without 甲"),
            "{error}"
        );
    }
}
