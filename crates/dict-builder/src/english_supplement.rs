//! `english-supplement`: generates msime-dictionary's `sources/english/scowl-words.txt`, the English words the rime-ice word lists lack, from SCOWL's official Aspell dictionary.
//!
//! The input is the Aspell English package en-wl/wordlist publishes with a dictionary release (`ARCHIVE` in the sources lock). Its word lists are SCOWL up to size 60, already expanded to every inflected form and stored in Aspell's prezip format; the generator reads them from the tar.bz2 directly, so it needs no SCOWL database build and no affix expansion. `LISTS` are the American dictionary (`en-common.cwl` plus `en_US-wo_accents-only.cwl`) and the British -ise spellings (`en_GB-ise-wo_accents-only.cwl`): many users learnt British spelling at school (colour, centre, organise), Google counts both spellings, so the counts already rank the American form first where both share a prefix. The Canadian and Australian lists and the variant lists are left out: Canadian adds little beyond the two, the Australian data carries its own terms, and SCOWL's variant lists are the uncommon spellings its default dictionaries leave out on purpose.
//!
//! Only forms the English stage can index are kept: ASCII letters only, so possessives (aardvark's), abbreviations with periods and accented words are dropped. Words shorter than `MIN_LETTERS` are dropped too: completing one saves at most one keystroke, while the 400-odd one- and two-letter forms SCOWL has (mostly abbreviations, US state codes and chemical symbols: MB, NY, Ag) carry high Google counts and would crowd the top of every one-letter prefix; rime-ice comments out most of its own short entries for the same reason. Casing is SCOWL's: proper nouns and acronyms keep it (Aachen, NASA), and a word SCOWL lists in two casings (china and China) gets a line for each.
//!
//! A word is left out entirely, in every casing, when:
//! - `COMPARED` already has its lowercase form in any casing, either as an entry or as an entry rime-ice commented out (`# huang huang`, `# ads ads`): the comments are rime-ice's own curation (romanised Chinese syllables and names that collide with pinyin, abbreviations, misspellings), and SCOWL must not bring those words back;
//! - it is in `OFFENSIVE`, ethnic, racial and sexual-orientation slurs whose dictionary sense is mainly the slur: completion would offer them on ordinary prefixes and the Chinese-to-English glosses could pick them;
//! - SCOWL lists it only capitalised and its lowercase form is the full pinyin of a word in `PINYIN` (Guangzhou, Zhejiang, Wang, Mandela): mixed input offers the first English completion of five or more typed letters in the second slot, so such names would take that slot whenever their pinyin is typed. Lowercase words of the same shape (dieting, shaman, bayou) stay, as rime-ice keeps its own (bang, change, tuna);
//! - `COUNTS` has no count for it: such a word ranks after every counted word of its prefix, so it is reached only when nearly all of it has been typed, and the 20,000-odd of them (rare inflections such as retrenches, chanciness) would cost about a quarter of the supplement's size in `msime-english.db`, which ships inside the size-budgeted core dictionary.
//!
//! SCOWL's terms ask for its copyright notice in every copy of a list made from it and in the supporting documentation, so the output header repeats the notice, the package's `Copyright` file has to match the committed `resources/licenses/scowl-aspell6-en-Copyright.txt` byte for byte, and the English stage stores that file in `msime-english.db`.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::io::Read;

use anyhow::{bail, Context, Result};

use crate::english;
use crate::places_supplement;
use crate::text;

pub const OUTPUT: &str = "sources/english/scowl-words.txt";
pub const ARCHIVE: &str = "scowl/aspell6-en-2026.02.25-0.tar.bz2";
/// The sources lock reference naming the en-wl/wordlist commit the release was built from.
pub const REFERENCE: &str = "SCOWL";
pub const LISTS: [&str; 3] = [
    "en-common.cwl",
    "en_US-wo_accents-only.cwl",
    "en_GB-ise-wo_accents-only.cwl",
];
/// The shortest word kept.
pub const MIN_LETTERS: usize = 3;
pub const COMPARED: [&str; 3] = [
    "sources/english/rime-ice-en.txt",
    "sources/english/rime-ice-en-supplement.txt",
    english::CUSTOM_ENGLISH,
];
/// The pinyin word lists the quanpin, places-supplement and custom-words stages read, whose full pinyin keys a capitalised-only SCOWL word must not spell.
pub const PINYIN: [&str; 4] = [
    "sources/pinyin/rime-ice.txt",
    "sources/pinyin/rime-ice-supplement.txt",
    "sources/pinyin/places.txt",
    "custom/words.txt",
];
/// The Google unigram counts the English stage ranks words by; a SCOWL word without a count is left out.
pub const COUNTS: &str = "sources/english/google-word-counts.txt";
/// Slurs left out in every casing (lowercase forms; each must be a SCOWL form, so the list cannot go stale unnoticed). Words with a common neutral sense (spade, cracker, dyke, queer, gringo, spastic) are not listed.
pub const OFFENSIVE: [&str; 55] = [
    "chink",
    "chinks",
    "coolie",
    "coolies",
    "coon",
    "coons",
    "dago",
    "dagoes",
    "dagos",
    "darkie",
    "darkies",
    "fag",
    "faggot",
    "faggots",
    "fags",
    "golliwog",
    "golliwogs",
    "gook",
    "gooks",
    "gyp",
    "gypped",
    "gypper",
    "gypping",
    "gyps",
    "honkies",
    "honky",
    "jap",
    "japs",
    "kike",
    "kikes",
    "kraut",
    "krauts",
    "micks",
    "nigga",
    "niggas",
    "niggaz",
    "niggers",
    "poofter",
    "poofters",
    "redskin",
    "sheeny",
    "spic",
    "spics",
    "squaw",
    "squaws",
    "wetback",
    "wetbacks",
    "whitey",
    "whiteys",
    "wog",
    "wogs",
    "wop",
    "wops",
    "yid",
    "yids",
];
/// The package's `Copyright` file, which also ships in msime as the licence text of these words.
pub const COPYRIGHT: &str =
    include_str!("../../../resources/licenses/scowl-aspell6-en-Copyright.txt");
/// The `source` key the English stage stores `COPYRIGHT` under in `msime-english.db`.
pub const NOTICE_SOURCE: &str = "SCOWL";
/// The release file the English stage writes `COPYRIGHT` to, so SCOWL's notice travels in the supporting documentation beside `msime-english.db`.
pub const NOTICE_NAME: &str = "msime-scowl_Copyright.txt";

/// The members of an uncompressed POSIX tar archive: regular files only, by path.
fn tar_members(data: &[u8]) -> Result<BTreeMap<String, &[u8]>> {
    let field = |header: &[u8], range: std::ops::Range<usize>| -> String {
        let bytes = &header[range];
        let end = bytes
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).trim().to_owned()
    };
    let octal = |header: &[u8], range: std::ops::Range<usize>| -> Result<u64> {
        let text = field(header, range);
        u64::from_str_radix(&text, 8).with_context(|| format!("bad tar number {text:?}"))
    };
    let mut members = BTreeMap::new();
    let mut offset = 0;
    while offset + 512 <= data.len() {
        let header = &data[offset..offset + 512];
        if header.iter().all(|&byte| byte == 0) {
            return Ok(members);
        }
        // The checksum counts its own field as eight spaces.
        let checksum: u64 = header
            .iter()
            .enumerate()
            .map(|(index, &byte)| {
                if (148..156).contains(&index) {
                    u64::from(b' ')
                } else {
                    u64::from(byte)
                }
            })
            .sum();
        if octal(header, 148..156)? != checksum {
            bail!("tar header at byte {offset} fails its checksum");
        }
        let mut name = field(header, 0..100);
        if &header[257..262] == b"ustar" {
            let prefix = field(header, 345..500);
            if !prefix.is_empty() {
                name = format!("{prefix}/{name}");
            }
        }
        let size = usize::try_from(octal(header, 124..136)?)?;
        let start = offset + 512;
        let Some(body) = data.get(start..start + size) else {
            bail!("tar member {name} runs past the end of the archive");
        };
        if matches!(header[156], b'0' | 0) {
            members.insert(name, body);
        }
        offset = start + size.div_ceil(512) * 512;
    }
    bail!("tar archive ends without its end-of-archive block")
}

/// Aspell's prezip word lists (aspell `prog/prezip.c`): `0x02`, then per word a shared-prefix length (`0x00`–`0x1D`, or `0x1E` followed by `0xFF`* and a final byte, all summed) and the rest of the word, ending with `0x1F 0xFF`. Bytes below 0x20 inside a word are escaped as `0x1F` plus the byte + 0x20; the prefix length counts the escaped form.
fn prezip_words(data: &[u8]) -> Result<Vec<Vec<u8>>> {
    if data.first() != Some(&2) {
        bail!("not a prezip word list");
    }
    let mut words = Vec::new();
    let mut stored: Vec<u8> = Vec::new();
    let mut at = 1;
    loop {
        let Some(&first) = data.get(at) else {
            bail!("prezip word list ends without its terminator");
        };
        at += 1;
        let mut prefix = usize::from(first);
        if first == 30 {
            loop {
                let Some(&byte) = data.get(at) else {
                    bail!("prezip word list ends inside a prefix length");
                };
                at += 1;
                prefix += usize::from(byte);
                if byte != 255 {
                    break;
                }
            }
        }
        if prefix > stored.len() {
            bail!("prezip prefix length {prefix} exceeds the previous word");
        }
        stored.truncate(prefix);
        while let Some(&byte) = data.get(at).filter(|&&byte| byte > 30) {
            stored.push(byte);
            at += 1;
        }
        let mut word = Vec::with_capacity(stored.len());
        let mut index = 0;
        let mut finished = false;
        while index < stored.len() {
            if stored[index] != 31 {
                word.push(stored[index]);
                index += 1;
                continue;
            }
            match stored.get(index + 1) {
                Some(&escaped @ 32..=63) => word.push(escaped - 32),
                Some(&255) if index + 2 == stored.len() => {
                    finished = true;
                    break;
                }
                _ => bail!("corrupt prezip escape"),
            }
            index += 2;
        }
        if finished {
            if !word.is_empty() {
                words.push(word);
            }
            if at != data.len() {
                bail!("prezip word list has data after its terminator");
            }
            return Ok(words);
        }
        words.push(word);
    }
}

/// The package's word lists and `Copyright` file, from the tar.bz2 bytes.
pub struct Package {
    /// `LISTS` in order, each with its words as stored (Latin-1 bytes).
    pub lists: Vec<(&'static str, Vec<Vec<u8>>)>,
    pub copyright: Vec<u8>,
}

/// The one member whose file name is `name`, wherever the package puts its top directory.
fn member<'a>(members: &BTreeMap<String, &'a [u8]>, name: &str) -> Result<&'a [u8]> {
    let found: Vec<&[u8]> = members
        .iter()
        .filter(|(path, _)| path.rsplit('/').next() == Some(name))
        .map(|(_, body)| *body)
        .collect();
    match found[..] {
        [body] => Ok(body),
        [] => bail!("the Aspell package has no {name}"),
        _ => bail!("the Aspell package has several {name}"),
    }
}

pub fn read_package(compressed: &[u8]) -> Result<Package> {
    let mut tar = Vec::new();
    bzip2::read::MultiBzDecoder::new(compressed)
        .read_to_end(&mut tar)
        .context("decompressing the Aspell package")?;
    let members = tar_members(&tar)?;
    let mut lists = Vec::new();
    for name in LISTS {
        let words = prezip_words(member(&members, name)?).with_context(|| name.to_owned())?;
        lists.push((name, words));
    }
    Ok(Package {
        lists,
        copyright: member(&members, "Copyright")?.to_vec(),
    })
}

/// The package's forms made of ASCII letters only, as written: the casings SCOWL attests, which the English stage uses to pick a word's leading casing.
pub fn letter_forms(package: &Package) -> HashSet<String> {
    package
        .lists
        .iter()
        .flat_map(|(_, words)| words)
        .filter(|word| !word.is_empty() && word.iter().all(u8::is_ascii_alphabetic))
        .map(|word| String::from_utf8_lossy(word).into_owned())
        .collect()
}

/// What the generator kept and why it dropped the rest.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Supplement {
    /// The displays to write, ordered by lowercase word, the lowercase casing first.
    pub words: Vec<String>,
    /// Entries per list, before any filtering.
    pub list_entries: Vec<(&'static str, usize)>,
    /// Distinct forms across the lists.
    pub forms: usize,
    /// Forms that are not ASCII letters only (possessives, abbreviations with periods, accented words).
    pub not_letters: usize,
    /// Letter-only forms shorter than `MIN_LETTERS`.
    pub too_short: usize,
    /// Letter-only forms whose lowercase word a compared file already has, as an entry or a commented-out entry.
    pub already_present: usize,
    /// Forms whose lowercase word is in `OFFENSIVE`.
    pub offensive: usize,
    /// Forms of words SCOWL lists only capitalised whose lowercase form is a full pinyin key of `PINYIN`.
    pub pinyin_names: usize,
    /// Forms of words `COUNTS` has no count for.
    pub uncounted: usize,
}

impl Supplement {
    pub fn words_by_lowercase(&self) -> usize {
        self.words
            .iter()
            .map(|word| word.to_ascii_lowercase())
            .collect::<BTreeSet<_>>()
            .len()
    }
}

/// The lowercase words of rime-ice entries commented out as `# display code [weight]`, where the display is one ASCII word and the code spells it (`# huang huang`, `# ASI ASI`). Other comment lines (section titles, notes, `# July Jul`) do not have that shape.
fn commented_base_dict_words(text: &str) -> BTreeSet<String> {
    let mut words = BTreeSet::new();
    for line in text::universal_lines(text) {
        let Some(body) = text::strip(line).strip_prefix('#') else {
            continue;
        };
        let mut fields: Vec<&str> = text::split_whitespace(body).collect();
        if fields.len() == 3 && fields[2].bytes().all(|byte| byte.is_ascii_digit()) {
            fields.pop();
        }
        if let [display, code] = fields[..] {
            if !display.is_empty()
                && display.bytes().all(|byte| byte.is_ascii_alphabetic())
                && display.eq_ignore_ascii_case(code)
            {
                words.insert(display.to_ascii_lowercase());
            }
        }
    }
    words
}

/// The lowercase words of the compared files: the rime-ice word lists as the English stage parses them plus the entries rime-ice commented out, and the custom file's lookup keys.
pub fn compared_words(
    rime_ice_en: &str,
    rime_ice_en_supplement: &str,
    custom_english: &str,
) -> Result<HashSet<String>> {
    let mut words: HashSet<String> = english::parse_base_dict_words(rime_ice_en)?
        .into_keys()
        .collect();
    words.extend(english::parse_base_dict_words(rime_ice_en_supplement)?.into_keys());
    words.extend(commented_base_dict_words(rime_ice_en));
    words.extend(commented_base_dict_words(rime_ice_en_supplement));
    words.extend(
        english::parse_custom_english(custom_english)?
            .into_iter()
            .map(|entry| entry.word),
    );
    Ok(words)
}

/// The full pinyin keys of `PINYIN`'s rows with the syllable separators removed (`guang'zhou` becomes `guangzhou`), read as the quanpin stage reads them.
pub fn pinyin_keys(sources: &[&str]) -> HashSet<String> {
    sources
        .iter()
        .flat_map(|source| places_supplement::weighted_rows(source))
        .map(|(_, key, _)| key.replace('\'', ""))
        .collect()
}

/// The word sets the SCOWL forms are filtered against.
pub struct Filters {
    /// From [`compared_words`].
    pub compared: HashSet<String>,
    /// From [`pinyin_keys`].
    pub pinyin_keys: HashSet<String>,
    /// `COUNTS` as the English stage parses it.
    pub counts: HashMap<String, i64>,
}

pub fn build(package: &Package, filters: &Filters) -> Result<Supplement> {
    if package.copyright != COPYRIGHT.as_bytes() {
        bail!("the Aspell package's Copyright differs from resources/licenses/scowl-aspell6-en-Copyright.txt; review the new terms and update the licence file");
    }
    let mut supplement = Supplement::default();
    let mut forms: BTreeSet<&[u8]> = BTreeSet::new();
    for (name, words) in &package.lists {
        supplement.list_entries.push((*name, words.len()));
        forms.extend(words.iter().map(Vec::as_slice));
    }
    supplement.forms = forms.len();
    let lowercase_forms: HashSet<String> = forms
        .iter()
        .filter_map(|form| std::str::from_utf8(form).ok())
        .map(str::to_ascii_lowercase)
        .collect();
    if let Some(missing) = OFFENSIVE
        .iter()
        .find(|word| !lowercase_forms.contains(**word))
    {
        bail!("OFFENSIVE lists {missing}, which the Aspell package does not have; update the list");
    }
    // Lowercase word -> its displays, for the rules that look at every casing of a word.
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for form in forms {
        if form.is_empty() || !form.iter().all(u8::is_ascii_alphabetic) {
            supplement.not_letters += 1;
            continue;
        }
        if form.len() < MIN_LETTERS {
            supplement.too_short += 1;
            continue;
        }
        let display = String::from_utf8(form.to_vec())?;
        let word = display.to_ascii_lowercase();
        if filters.compared.contains(&word) {
            supplement.already_present += 1;
            continue;
        }
        if OFFENSIVE.contains(&word.as_str()) {
            supplement.offensive += 1;
            continue;
        }
        groups.entry(word).or_default().push(display);
    }
    let mut kept: Vec<String> = Vec::new();
    for (word, mut displays) in groups {
        if !displays.contains(&word) && filters.pinyin_keys.contains(&word) {
            supplement.pinyin_names += displays.len();
            continue;
        }
        if !filters.counts.contains_key(&word) {
            supplement.uncounted += displays.len();
            continue;
        }
        displays.sort_by_key(|display| (*display != word, display.clone()));
        kept.extend(displays);
    }
    if kept.is_empty() {
        bail!("SCOWL adds no word the compared files lack");
    }
    supplement.words = kept;
    Ok(supplement)
}

/// Where the inputs came from, for the header.
pub struct Provenance<'a> {
    pub archive_url: &'a str,
    pub archive_sha256: &'a str,
    pub upstream_commit: &'a str,
    /// Each compared file with its SHA-256.
    pub compared: &'a [(&'a str, &'a str)],
    /// Each `PINYIN` file with its SHA-256.
    pub pinyin: &'a [(&'a str, &'a str)],
    /// The SHA-256 of `COUNTS`.
    pub counts_sha256: &'a str,
    /// 运行生成器的 msime 提交；构建器有未提交改动时带 `-dirty` 后缀。
    pub generator_commit: &'a str,
}

/// The SCOWL copyright line and the permission paragraph after it, as single lines.
fn copyright_notice() -> Result<Vec<String>> {
    let paragraphs: Vec<String> = COPYRIGHT
        .split("\n\n")
        .map(|paragraph| paragraph.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    let at = paragraphs
        .iter()
        .position(|paragraph| {
            paragraph.starts_with("Copyright ") && paragraph.contains("Kevin Atkinson")
        })
        .context("the SCOWL Copyright file has no Kevin Atkinson copyright line")?;
    let permission = paragraphs
        .get(at + 1)
        .filter(|paragraph| paragraph.starts_with("Permission to use"))
        .context("the SCOWL copyright line is not followed by its permission notice")?;
    Ok(vec![paragraphs[at].clone(), permission.clone()])
}

pub fn render(supplement: &Supplement, provenance: &Provenance) -> Result<String> {
    let files = |files: &[(&str, &str)]| {
        files
            .iter()
            .map(|(path, sha256)| format!("{path}（SHA-256 {sha256}）"))
            .collect::<Vec<_>>()
            .join("、")
    };
    let compared = files(provenance.compared);
    let pinyin = files(provenance.pinyin);
    let counts_sha256 = provenance.counts_sha256;
    let offensive = OFFENSIVE.join("、");
    let mut out = String::new();
    let _ = writeln!(out, "# SCOWL 英文词补充表，由 msime 仓库提交 {} 的 crates/dict-builder/src/english_supplement.rs 以 `msime-dict-build english-supplement --dictionary <msime-dictionary checkout> --cache <dir> --out sources/english/scowl-words.txt` 生成；不要手工编辑。", provenance.generator_commit);
    let _ = writeln!(out, "# 上游：https://github.com/en-wl/wordlist 提交 {} 发布的 Aspell 英文词典 {}（SHA-256 {}），即 SCOWL 60 级的官方拼写检查词典；取其中 {}，也就是美式拼写词典加英式 -ise 拼写，不含加拿大、澳大利亚拼写和异体词表。", provenance.upstream_commit, provenance.archive_url, provenance.archive_sha256, LISTS.join("、"));
    for line in copyright_notice()? {
        let _ = writeln!(out, "# {line}");
    }
    let _ = writeln!(out, "# 收录：只收全由 ASCII 字母组成、至少 {MIN_LETTERS} 个字母的词形，带撇号（所有格）、点、连字符或非 ASCII 字母的词形和一两个字母的词形不收；大小写照 SCOWL，专有名词和缩写保留大写，同一个词有几种大小写就各占一行。");
    let _ = writeln!(out, "# 去重：对照集合是 {compared}；小写形式已在对照集合里出现的词（不论大小写）整词不收，rime-ice 英文词表里被注释掉的条目（形如 `# huang huang`，显示词是一个 ASCII 单词、编码与它只差大小写）也算在对照集合里，不让 SCOWL 把 rime-ice 有意去掉的词加回来。");
    let _ = writeln!(out, "# 排除：以下蔑称不论大小写整词不收：{offensive}；有常见中性义项的词（spade、cracker、dyke、queer、gringo、spastic）不在此列。SCOWL 只收了大写形式（没有全小写形式）、而小写形式恰好是 {pinyin} 里某个词去掉音节分隔符后的完整全拼的词（Guangzhou、Zhejiang、Wang、Mandela）整词不收，因为拼音输入时中英混排会把它们插到第二位；全小写的普通词（dieting、shaman）照收。在 {COUNTS}（SHA-256 {counts_sha256}）里没有词频的词整词不收。");
    let _ = writeln!(out, "# 构建的 english 阶段把本文件与 rime-ice 英文词表合并，每种大小写各占一行，按 {COUNTS} 的词频排序。");
    let _ = writeln!(
        out,
        "# 每行一个词，按小写形式排序，同一个词的全小写形式在前。"
    );
    for word in &supplement.words {
        let _ = writeln!(out, "{word}");
    }
    Ok(out)
}

/// The generator's account of what it read and dropped, for review.
pub fn report(supplement: &Supplement) -> Vec<String> {
    let mut lines: Vec<String> = supplement
        .list_entries
        .iter()
        .map(|(name, entries)| format!("{name}: {entries} entries"))
        .collect();
    lines.push(format!(
        "{} distinct forms: {} not ASCII letters only, {} shorter than {MIN_LETTERS} letters, {} already in the compared files (entries or commented-out entries), {} offensive, {} capitalised-only words spelling a pinyin key, {} without a Google count, {} written ({} lowercase words)",
        supplement.forms,
        supplement.not_letters,
        supplement.too_short,
        supplement.already_present,
        supplement.offensive,
        supplement.pinyin_names,
        supplement.uncounted,
        supplement.words.len(),
        supplement.words_by_lowercase()
    ));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// prezip.c's compressor, for fixtures.
    fn prezip(words: &[&[u8]]) -> Vec<u8> {
        let mut out = vec![2];
        let mut previous: Vec<u8> = Vec::new();
        for word in words {
            let mut stored = Vec::new();
            for &byte in *word {
                if byte >= 32 {
                    stored.push(byte);
                } else {
                    stored.extend([31, byte + 32]);
                }
            }
            let shared = previous
                .iter()
                .zip(&stored)
                .take_while(|(a, b)| a == b)
                .count();
            if shared < 30 {
                out.push(u8::try_from(shared).unwrap());
            } else {
                out.push(30);
                let mut rest = shared - 30;
                while rest >= 255 {
                    out.push(255);
                    rest -= 255;
                }
                out.push(u8::try_from(rest).unwrap());
            }
            out.extend(&stored[shared..]);
            previous = stored;
        }
        // The compressor reads one empty line at the end of its input.
        out.extend([0, 31, 255]);
        out
    }

    fn tar(members: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (name, body) in members {
            let mut header = [0u8; 512];
            header[..name.len()].copy_from_slice(name.as_bytes());
            header[100..108].copy_from_slice(b"0000644\0");
            header[124..136].copy_from_slice(format!("{:011o}\0", body.len()).as_bytes());
            header[156] = b'0';
            header[257..263].copy_from_slice(b"ustar\0");
            header[148..156].copy_from_slice(b"        ");
            let sum: u32 = header.iter().map(|&byte| u32::from(byte)).sum();
            header[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
            out.extend(header);
            out.extend(*body);
            out.resize(out.len().div_ceil(512) * 512, 0);
        }
        out.resize(out.len() + 1024, 0);
        out
    }

    #[test]
    fn prezip_lists_decode_as_aspell_writes_them() {
        let long = [b'a'; 300];
        let words: Vec<&[u8]> = vec![
            b"A",
            b"AA",
            b"AA's",
            b"Aachen",
            b"a\x01b",
            &long,
            &long[..299],
            b"zebra",
        ];
        assert_eq!(prezip_words(&prezip(&words)).unwrap(), words);
        assert!(prezip_words(b"\x01A").is_err(), "wrong header");
        assert!(
            prezip_words(b"\x02\x00A\x05B\x1f\xff").is_err(),
            "prefix past the previous word"
        );
        assert!(prezip_words(b"\x02\x00A").is_err(), "no terminator");
        let mut trailing = prezip(&words);
        trailing.push(b'x');
        assert!(
            prezip_words(&trailing).is_err(),
            "data after the terminator"
        );
    }

    #[test]
    fn commented_entries_are_read_only_in_the_entry_shape() {
        let words = commented_base_dict_words("# 键盘按键\n# huang huang\r\n# ASI ASI 3\n# July Jul\n# Legal High ligouhai\n#ads ads\nhub hub\n");
        assert_eq!(
            words.into_iter().collect::<Vec<_>>(),
            ["ads", "asi", "huang"]
        );
    }

    fn filters(rime_ice_en: &str) -> Filters {
        Filters {
            compared: compared_words(rime_ice_en, "organize organize\n", "nasa\tNASA\t1\n").unwrap(),
            pinyin_keys: pinyin_keys(&["广州\tguang'zhou\t100\n# 黄\thuang\t1\n得体\tdie'ting\t5\n"]),
            counts: english::parse_google_counts("aachen\t5\naardvark\t9\nchina\t70\ncolour\t8\norganise\t4\ndieting\t6\nguangzhou\t30\nnigga\t50\n"),
        }
    }

    #[test]
    fn package_lists_are_read_and_filtered_against_the_compared_words() {
        let mut common: Vec<&[u8]> = vec![
            b"Aachen",
            b"aardvark",
            b"aardvark's",
            b"ab",
            b"China",
            b"china",
            b"dieting",
            b"Guangzhou",
            b"hello",
            b"Huang",
            b"NASA",
            b"NY",
            b"vis-a-vis",
            b"zebra",
        ];
        common.extend(OFFENSIVE.iter().map(|word| word.as_bytes()));
        let common = prezip(&common);
        let us = prezip(&[b"color", b"organize"]);
        let gb = prezip(&[b"colour", b"organise", b"caf\xe9"]);
        let archive = tar(&[
            ("aspell6-en/en-common.cwl", &common),
            ("aspell6-en/en_US-wo_accents-only.cwl", &us),
            ("aspell6-en/en_GB-ise-wo_accents-only.cwl", &gb),
            ("aspell6-en/Copyright", COPYRIGHT.as_bytes()),
        ]);
        let mut members = tar_members(&archive).unwrap();
        assert_eq!(members.len(), 4);
        let mut package = Package {
            lists: LISTS
                .iter()
                .map(|name| {
                    (
                        *name,
                        prezip_words(member(&members, name).unwrap()).unwrap(),
                    )
                })
                .collect(),
            copyright: member(&members, "Copyright").unwrap().to_vec(),
        };
        let filters = filters("Hello hello\nColor color\n# huang huang\n");
        let supplement = build(&package, &filters).unwrap();
        // Huang is commented out in rime-ice, Guangzhou spells 广州's pinyin, zebra has no count; dieting spells 得体's pinyin but is lowercase.
        assert_eq!(
            supplement.words,
            ["Aachen", "aardvark", "china", "China", "colour", "dieting", "organise"]
        );
        assert_eq!(
            (
                supplement.forms,
                supplement.not_letters,
                supplement.too_short,
                supplement.already_present,
                supplement.offensive,
                supplement.pinyin_names,
                supplement.uncounted
            ),
            (19 + OFFENSIVE.len(), 3, 2, 5, OFFENSIVE.len(), 1, 1)
        );
        assert_eq!(supplement.words_by_lowercase(), 6);

        let rendered = render(
            &supplement,
            &Provenance {
                archive_url: "https://example.invalid/aspell6-en.tar.bz2",
                archive_sha256: "00",
                upstream_commit: "c0ffee",
                compared: &[("sources/english/rime-ice-en.txt", "11")],
                pinyin: &[("sources/pinyin/rime-ice.txt", "22")],
                counts_sha256: "33",
                generator_commit: "0123456789abcdef0123456789abcdef01234567",
            },
        )
        .unwrap();
        let first = rendered.lines().next().unwrap();
        assert!(
            first.contains("msime 仓库提交 0123456789abcdef0123456789abcdef01234567"),
            "{first}"
        );
        assert!(!first.contains("dictionary-sources.lock.json"));
        assert!(rendered.contains("# Copyright 2000-2026 by Kevin Atkinson\n# Permission to use, copy, modify, distribute, and sell any part of SCOWLv2, or word lists created from it,"));
        let body: Vec<&str> = rendered
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect();
        assert_eq!(body, supplement.words);
        let parsed = english::parse_word_list(&rendered, OUTPUT).unwrap();
        assert_eq!(parsed["china"], ["china", "China"]);

        let mut stale = Package {
            lists: package.lists.clone(),
            copyright: package.copyright.clone(),
        };
        stale.lists[0].1.retain(|word| word.as_slice() != b"yids");
        assert!(
            build(&stale, &filters).is_err(),
            "an OFFENSIVE word missing from SCOWL stops the generator"
        );
        package.copyright.push(b'\n');
        assert!(
            build(&package, &filters).is_err(),
            "changed terms stop the generator"
        );
        members.clear();
        assert!(member(&members, "Copyright").is_err());
    }
}
