//! `places-supplement`: generates msime-dictionary's `sources/pinyin/places.txt`, the administrative place names the shipped pinyin dictionary lacks or ranks too low, from the modood/Administrative-divisions-of-China tables `places` also reads.
//!
//! Every province, prefecture-level and county-level division contributes its full name and, where one remains, its short name: the name without its administrative suffix (省, 自治区, 市, 自治州, 地区, 盟, 区, 县, 旗, 自治县, 自治旗, 特区, 林区, 矿区) and the ethnic names an autonomous division carries (广西壮族自治区 → 广西, 双江拉祜族佤族布朗族傣族自治县 → 双江). A short name must keep at least two Han characters, must not be an ethnic name itself, and a banner or autonomous county whose name ends in a position qualifier (土默特左旗, 喀喇沁左翼蒙古族自治县, 达尔罕茂明安联合旗) gets none, since the bare stem is not what the place is called.
//!
//! Left out entirely: functional zones (county codes 71–79, which the statistics code assigns to development zones rather than to divisions), names containing one of `EXCLUDED_FRAGMENTS` (bar the real divisions in `FRAGMENT_EXCEPTIONS`), and the generic names in `GENERIC_NAMES`, which are ordinary words first.
//!
//! The reading of a name is the `places` key (`READINGS` plus per-character pinyin), and a short name takes the first syllables of its full name's key. The per-character reading is a guess wherever a character has more than one reading, so a name is kept only when the guess is confirmed: every character that `sources/pinyin/single-chars.txt` lists with several readings, or does not list at all, has to sit inside a span of at least two characters of the name that the comparison set (`sources/pinyin/rime-ice.txt` and `sources/pinyin/rime-ice-supplement.txt`) contains with the same syllables, as a word of its own or inside a longer word, or belong to a full name's administrative suffix read the fixed way (区 qu, 县 xian, 市 shi). A syllable `single-chars.txt` does not list for its character rejects the name outright. A name the comparison set already has under a different reading is left out as well, whether it also has ours or not: either the per-character reading is wrong for the place (中牟, 召陵) or the name is also an ordinary word read another way (民乐), and in both cases raising it would put a place reading ahead of the word people mean. The one exception is a name whose reading, apart from a full name's administrative suffix, comes entirely from `READINGS` entries of two or more characters (六合 lu'he, 宕昌 tan'chang), when the comparison set has the name under that reading too: such a reading was checked for the place, and raising its row changes nothing under the other key. Names in `UNSOURCED_NAMES` are left out although the comparison set confirms them, because the place's reading is disputed and no dictionary or 民政部 source settles it.
//!
//! Weights are floors computed from the shipped data, not chosen: for provinces and prefectures, the median weight of the names of the same level and form (full or short) that the comparison set has at a weight above 10; for county-level names, the median weight of all `sources/pinyin/rime-ice.txt` rows. A name used at several levels takes the highest floor. Only the names the comparison set lacks or holds at the unranked weight of `RANKED_WEIGHT` or below are written, each at its floor: a name the comparison set already ranks above that keeps its weight even below the floor, since that weight is rime-ice's frequency signal and raising it would push ordinary words of the same reading down (伊犁 over 屹立, 山西 and 陕西 flattened to one weight).
//!
//! A floor must not take the first candidate of a key away from a common word either. The common weight is the 90th percentile (lower nearest rank) of the `sources/pinyin/rime-ice.txt` weights, computed from the data like the floors. When another word of the comparison set has the same full pinyin at a weight above it, the name's weight is capped one below that word's (盐城 under 严惩, 辽阳 under 疗养, 崇左 under 重做); a name whose capped weight is not above the weight it already has is not written. Words of the same key at or below the common weight do not cap the floor, since a rarer word should not keep a place name off the first position. Nor does a word whose weight is a placeholder: among the `sources/pinyin/rime-ice.txt` rows above the common weight, a single weight value held by more than `PLACEHOLDER_PERCENT`% of them is a fill-in value rather than a frequency (9999 holds thousands of such rows: 紧皱, 爆闪, 吃粥 ...), detected from the data like the common weight. Such a word is skipped and the highest other word of the key with a real weight decides the cap (锦州 is not held under 紧皱, while 盐城 stays under 严惩).
//!
//! The build merges the file before `custom/words.txt` with the same raise-only rule (`msime::apply_custom_words`): a missing row is inserted, a lower one raised, a higher one left alone.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

use anyhow::{bail, Context, Result};

use crate::places::{self, GROUPING_CITIES};
use crate::text;

pub const OUTPUT: &str = "sources/pinyin/places.txt";
pub const SINGLE_CHARS: &str = "sources/pinyin/single-chars.txt";
pub const BASE: &str = "sources/pinyin/rime-ice.txt";
pub const RIME_ICE_SUPPLEMENT: &str = "sources/pinyin/rime-ice-supplement.txt";

/// Names containing one of these are economic, administrative or service zones rather than places people type (石家庄高新技术产业开发区, 大柴旦行政委员会), or a region that is not a place name (中沙群岛的岛礁及其海域). 新区 also drops the few real districts named so (浦东新区); their short names are ordinary entries of the base dictionary already.
const EXCLUDED_FRAGMENTS: &[&str] = &[
    "开发区",
    "经济技术",
    "高新",
    "新区",
    "管理区",
    "示范区",
    "实验区",
    "委员会",
    "海域",
];

/// Real divisions whose name only looks like it contains a fragment: 清新区 is 清新 plus 区, not a 新区. Every entry must match a name of the pinned data.
const FRAGMENT_EXCEPTIONS: &[&str] = &["清新区"];

/// Full or short names that are ordinary words before they are places, so a place floor would only distort their ranking. Every entry must match a candidate of the pinned data; an unused one fails the generator so the list does not rot.
const GENERIC_NAMES: &[&str] = &[
    "城关", "新华", "和平", "城区", "郊区", "矿区", "东区", "西区", "城东", "城西", "城北", "城厢",
    "北关",
];

/// Names whose reading only the comparison set confirms, while the place's reading is disputed and no dictionary or 民政部 source found settles it: 称多 (青海) is given as both Chēngduō and Chènduō, and 汉典 lists no place reading of 称. Every entry must match a candidate of the pinned data.
const UNSOURCED_NAMES: &[&str] = &["称多", "称多县"];

/// The 55 ethnic minorities as autonomous division names spell them, with and without 族, plus 各族 (龙胜各族自治县).
const ETHNIC_NAMES: &[&str] = &[
    "蒙古",
    "回",
    "藏",
    "维吾尔",
    "苗",
    "彝",
    "壮",
    "布依",
    "朝鲜",
    "满",
    "侗",
    "瑶",
    "白",
    "土家",
    "哈尼",
    "哈萨克",
    "傣",
    "黎",
    "傈僳",
    "佤",
    "畲",
    "高山",
    "拉祜",
    "水",
    "东乡",
    "纳西",
    "景颇",
    "柯尔克孜",
    "土",
    "达斡尔",
    "仫佬",
    "羌",
    "布朗",
    "撒拉",
    "毛南",
    "仡佬",
    "锡伯",
    "阿昌",
    "普米",
    "塔吉克",
    "怒",
    "乌孜别克",
    "俄罗斯",
    "鄂温克",
    "德昂",
    "保安",
    "裕固",
    "京",
    "塔塔尔",
    "独龙",
    "鄂伦春",
    "赫哲",
    "门巴",
    "珞巴",
    "基诺",
];

/// Administrative suffixes per level, longest first where one ends another, with their fixed readings. A full name's suffix counts as a confirmed reading when its key ends in these syllables, since a suffix like 区 is read qu whatever other readings the character has.
const PROVINCE_SUFFIXES: &[(&str, &str)] =
    &[("自治区", "zi'zhi'qu"), ("省", "sheng"), ("市", "shi")];
const PREFECTURE_SUFFIXES: &[(&str, &str)] = &[
    ("自治州", "zi'zhi'zhou"),
    ("地区", "di'qu"),
    ("盟", "meng"),
    ("市", "shi"),
];
const COUNTY_SUFFIXES: &[(&str, &str)] = &[
    ("自治县", "zi'zhi'xian"),
    ("自治旗", "zi'zhi'qi"),
    ("特区", "te'qu"),
    ("林区", "lin'qu"),
    ("矿区", "kuang'qu"),
    ("区", "qu"),
    ("县", "xian"),
    ("市", "shi"),
    ("旗", "qi"),
];
/// Suffixes after which the stem can still end in ethnic names (管城回族区, 梅里斯达斡尔族区).
const ETHNIC_SUFFIXES: &[&str] = &["自治区", "自治州", "自治县", "自治旗", "区"];
/// Suffixes whose stem gets no short name when it ends in a position qualifier.
const QUALIFIED_SUFFIXES: &[&str] = &["旗", "自治旗", "自治县"];
const POSITION_QUALIFIERS: &[&str] = &["左翼", "右翼", "左", "右", "前", "后", "中", "合"];

/// The percentile of the `sources/pinyin/rime-ice.txt` weights above which a word of the same key caps a place name's weight.
const COMMON_PERCENTILE: usize = 90;

/// Among the `sources/pinyin/rime-ice.txt` rows weighted above the common weight, a weight value more than this percentage of them share is a placeholder, not a frequency, and does not cap a place name.
const PLACEHOLDER_PERCENT: usize = 1;

/// A weight above this counts as a real ranking when the level floors are measured; rows at 1–10 are the unranked tail the supplement exists to lift.
const RANKED_WEIGHT: i64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Province,
    Prefecture,
    County,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Form {
    Full,
    Short,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Province => "省级",
            Level::Prefecture => "地级",
            Level::County => "县级",
        }
    }
}

impl Form {
    fn label(self) -> &'static str {
        match self {
            Form::Full => "全称",
            Form::Short => "简称",
        }
    }
}

/// One name a division contributes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    level: Level,
    form: Form,
    word: String,
    key: String,
}

/// A line of the supplement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry {
    pub word: String,
    pub key: String,
    pub weight: i64,
    /// The weight the comparison set has, `None` when the row is new.
    pub existing: Option<i64>,
}

/// What was left out and why, for the generator's report.
#[derive(Debug, Default)]
pub struct Exclusions {
    pub functional_zones: Vec<String>,
    pub fragments: Vec<String>,
    pub generic: Vec<String>,
    pub unsourced: Vec<String>,
    /// `(word, our key, the comparison set's keys)`.
    pub other_reading: Vec<(String, String, Vec<String>)>,
    /// `(word, key, the characters left unconfirmed)`.
    pub unconfirmed: Vec<(String, String, String)>,
    /// `(word, key, character, syllable)`.
    pub unlisted_syllable: Vec<(String, String, char, String)>,
}

/// A name whose floor was lowered below a common word of the same key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capped {
    pub word: String,
    pub key: String,
    pub floor: i64,
    /// One below `competitor_weight`.
    pub weight: i64,
    pub competitor: String,
    pub competitor_weight: i64,
    /// The weight the comparison set has, `None` when the row is new.
    pub existing: Option<i64>,
    /// False when the capped weight is not above `existing`, so the name is not written.
    pub written: bool,
}

#[derive(Debug)]
pub struct Supplement {
    pub entries: Vec<Entry>,
    pub floors: BTreeMap<(Level, Form), i64>,
    /// The weight above which a word of the same key caps a name: the `COMMON_PERCENTILE` of the base weights.
    pub common: i64,
    /// Placeholder weights, ascending: a word at one of these does not cap a name.
    pub placeholders: Vec<i64>,
    pub capped: Vec<Capped>,
    pub exclusions: Exclusions,
    /// Accepted names the comparison set already ranks above `RANKED_WEIGHT` or holds at or above their floor, so not written.
    pub already_ranked: usize,
}

/// The pinned inputs, as text.
pub struct Inputs<'a> {
    pub provinces: &'a str,
    pub cities: &'a str,
    pub areas: &'a str,
    pub single_chars: &'a str,
    pub base: &'a str,
    pub rime_ice_supplement: &'a str,
}

/// `value<TAB>key<TAB>weight` rows as the quanpin stage reads them: `#` lines skipped, the line stripped, rows without three fields ignored.
pub(crate) fn weighted_rows(source: &str) -> impl Iterator<Item = (&str, &str, i64)> {
    source.split_terminator('\n').filter_map(|line| {
        if line.starts_with('#') {
            return None;
        }
        let mut fields = text::strip(line).split('\t');
        let (value, key, weight) = (fields.next()?, fields.next()?, fields.next()?);
        Some((value, key, text::strip(weight).parse().ok()?))
    })
}

/// word → key → highest weight.
type Shipped<'a> = HashMap<&'a str, HashMap<&'a str, i64>>;

fn shipped<'a>(sources: &[&'a str]) -> Shipped<'a> {
    let mut words: Shipped = HashMap::new();
    for source in sources {
        for (value, key, weight) in weighted_rows(source) {
            let slot = words.entry(value).or_default().entry(key).or_insert(weight);
            *slot = (*slot).max(weight);
        }
    }
    words
}

/// The lower nearest-rank percentile, so the result is always a weight some row really has.
fn percentile(mut values: Vec<i64>, percent: usize) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some(values[(values.len() - 1) * percent / 100])
}

/// The lower median.
fn median(values: Vec<i64>) -> Option<i64> {
    percentile(values, 50)
}

/// The weight values that more than `PLACEHOLDER_PERCENT`% of the weights above `common` share, ascending.
fn placeholders(weights: &[i64], common: i64) -> Vec<i64> {
    let mut counts: BTreeMap<i64, usize> = BTreeMap::new();
    for weight in weights.iter().filter(|weight| **weight > common) {
        *counts.entry(*weight).or_default() += 1;
    }
    let total: usize = counts.values().sum();
    counts
        .into_iter()
        .filter(|(_, count)| count * 100 > total * PLACEHOLDER_PERCENT)
        .map(|(weight, _)| weight)
        .collect()
}

/// key → the two highest-weighted distinct words whose weight is not a placeholder, highest first, so the best real competitor of a given word is at hand.
fn leaders<'a>(
    shipped: &Shipped<'a>,
    placeholders: &[i64],
) -> HashMap<&'a str, Vec<(&'a str, i64)>> {
    let mut leaders: HashMap<&str, Vec<(&str, i64)>> = HashMap::new();
    for (word, keys) in shipped {
        for (key, weight) in keys {
            if placeholders.contains(weight) {
                continue;
            }
            let slot = leaders.entry(key).or_default();
            slot.push((word, *weight));
            // Ties broken by word so the competitor a report names does not depend on hash order.
            slot.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
            slot.truncate(2);
        }
    }
    leaders
}

pub(crate) fn is_han(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{3134f}')
}

fn is_ethnic(text: &str) -> bool {
    ETHNIC_NAMES
        .iter()
        .any(|name| text == *name || text.strip_suffix('族') == Some(*name))
        || text == "各族"
}

/// Removes trailing ethnic names, longest first, while something remains before them.
fn strip_ethnic(mut stem: &str) -> &str {
    let mut names: Vec<String> = ETHNIC_NAMES
        .iter()
        .flat_map(|name| [format!("{name}族"), (*name).to_owned()])
        .collect();
    names.push("各族".to_owned());
    names.sort_by_key(|name| std::cmp::Reverse(name.chars().count()));
    'strip: loop {
        for name in &names {
            if let Some(rest) = stem.strip_suffix(name.as_str()) {
                if !rest.is_empty() {
                    stem = rest;
                    continue 'strip;
                }
            }
        }
        return stem;
    }
}

/// The short name of a division, if it has one.
/// The divisions whose name ends in the suffix 林区. Other names ending so are districts named …林 (碑林区, 万柏林区), whose suffix is 区.
const FOREST_DISTRICTS: &[&str] = &["神农架林区"];

/// The administrative suffix of a division's name and its reading.
fn suffix(name: &str, level: Level) -> Option<(&'static str, &'static str)> {
    let suffixes = match level {
        Level::Province => PROVINCE_SUFFIXES,
        Level::Prefecture => PREFECTURE_SUFFIXES,
        Level::County => COUNTY_SUFFIXES,
    };
    suffixes
        .iter()
        .filter(|(suffix, _)| *suffix != "林区" || FOREST_DISTRICTS.contains(&name))
        .find(|(suffix, _)| name.len() > suffix.len() && name.ends_with(suffix))
        .copied()
}

fn short_name(name: &str, level: Level) -> Option<String> {
    let (suffix, _) = suffix(name, level)?;
    let suffix = &suffix;
    let mut stem = &name[..name.len() - suffix.len()];
    if ETHNIC_SUFFIXES.contains(suffix) {
        stem = strip_ethnic(stem);
    }
    if QUALIFIED_SUFFIXES.contains(suffix)
        && POSITION_QUALIFIERS
            .iter()
            .any(|qualifier| stem.ends_with(qualifier))
    {
        return None;
    }
    (stem.chars().count() >= 2 && stem.chars().all(is_han) && !is_ethnic(stem))
        .then(|| stem.to_owned())
}

/// The first `count` syllables of `key`.
fn key_prefix(key: &str, count: usize) -> String {
    key.split('\'').take(count).collect::<Vec<_>>().join("'")
}

/// Which characters of each candidate the comparison set confirms: a span of two or more characters of the word, found in a shipped word (on its own or inside a longer one) with the same syllables at the same positions.
fn confirmed_positions(
    candidates: &[(String, String)],
    shipped: &Shipped,
) -> HashMap<(String, String), Vec<bool>> {
    // span text → (span key → [(candidate index, start)])
    let mut spans: HashMap<String, HashMap<String, Vec<(usize, usize)>>> = HashMap::new();
    let mut longest = 0;
    for (index, (word, key)) in candidates.iter().enumerate() {
        let characters: Vec<char> = word.chars().collect();
        let syllables: Vec<&str> = key.split('\'').collect();
        longest = longest.max(characters.len());
        for start in 0..characters.len() {
            for end in start + 2..=characters.len() {
                spans
                    .entry(characters[start..end].iter().collect())
                    .or_default()
                    .entry(syllables[start..end].join("'"))
                    .or_default()
                    .push((index, start));
            }
        }
    }
    let mut confirmed: Vec<Vec<bool>> = candidates
        .iter()
        .map(|(word, _)| vec![false; word.chars().count()])
        .collect();
    for (word, keys) in shipped {
        let characters: Vec<char> = word.chars().collect();
        if characters.len() < 2 {
            continue;
        }
        for key in keys.keys() {
            let syllables: Vec<&str> = key.split('\'').collect();
            if syllables.len() != characters.len() {
                continue;
            }
            for start in 0..characters.len() {
                for end in start + 2..=characters.len().min(start + longest) {
                    let span: String = characters[start..end].iter().collect();
                    let Some(readings) = spans.get(&span) else {
                        continue;
                    };
                    let Some(owners) = readings.get(&syllables[start..end].join("'")) else {
                        continue;
                    };
                    for &(index, offset) in owners {
                        for flag in &mut confirmed[index][offset..offset + end - start] {
                            *flag = true;
                        }
                    }
                }
            }
        }
    }
    candidates.iter().cloned().zip(confirmed).collect()
}

/// Every candidate name of the pinned divisions, with the exclusions by name.
fn candidates(inputs: &Inputs, exclusions: &mut Exclusions) -> Result<Vec<Candidate>> {
    let keys: HashMap<String, String> =
        places::build(inputs.provinces, inputs.cities, inputs.areas)?
            .into_iter()
            .map(|place| (place.name, place.key))
            .collect();
    let provinces = places::records(inputs.provinces, places::PROVINCES, 2)?;
    let cities = places::records(inputs.cities, places::CITIES, 3)?;
    let areas = places::records(inputs.areas, places::AREAS, 4)?;
    let mut divisions: Vec<(Level, &str)> = Vec::new();
    divisions.extend(
        provinces
            .iter()
            .map(|row| (Level::Province, row[1].as_str())),
    );
    divisions.extend(
        cities
            .iter()
            .filter(|row| !GROUPING_CITIES.contains(&row[1].as_str()))
            .map(|row| (Level::Prefecture, row[1].as_str())),
    );
    for row in &areas {
        let code = &row[0];
        let county = code
            .get(4..6)
            .with_context(|| format!("{}: area code {code} is not six digits", row[1]))?;
        if ("71"..="79").contains(&county) {
            exclusions.functional_zones.push(row[1].clone());
            continue;
        }
        divisions.push((Level::County, row[1].as_str()));
    }

    let mut generic_used = vec![false; GENERIC_NAMES.len()];
    let mut generic = |word: &str| -> bool {
        match GENERIC_NAMES.iter().position(|name| *name == word) {
            Some(index) => {
                generic_used[index] = true;
                true
            }
            None => false,
        }
    };
    let mut unsourced_used = vec![false; UNSOURCED_NAMES.len()];
    let mut unsourced = |word: &str| -> bool {
        match UNSOURCED_NAMES.iter().position(|name| *name == word) {
            Some(index) => {
                unsourced_used[index] = true;
                true
            }
            None => false,
        }
    };
    let mut exceptions_used = vec![false; FRAGMENT_EXCEPTIONS.len()];
    let mut result = Vec::new();
    for (level, name) in divisions {
        let exception = FRAGMENT_EXCEPTIONS
            .iter()
            .position(|exception| *exception == name);
        if let Some(index) = exception {
            exceptions_used[index] = true;
        }
        if exception.is_none()
            && EXCLUDED_FRAGMENTS
                .iter()
                .any(|fragment| name.contains(fragment))
        {
            exclusions.fragments.push(name.to_owned());
            continue;
        }
        let key = keys
            .get(name)
            .with_context(|| format!("{name}: no key in the place table"))?;
        if generic(name) {
            exclusions.generic.push(name.to_owned());
        } else if unsourced(name) {
            exclusions.unsourced.push(name.to_owned());
        } else {
            result.push(Candidate {
                level,
                form: Form::Full,
                word: name.to_owned(),
                key: key.clone(),
            });
        }
        if let Some(short) = short_name(name, level) {
            if generic(&short) {
                exclusions.generic.push(short);
                continue;
            }
            if unsourced(&short) {
                exclusions.unsourced.push(short);
                continue;
            }
            let key = key_prefix(key, short.chars().count());
            result.push(Candidate {
                level,
                form: Form::Short,
                word: short,
                key,
            });
        }
    }
    for forest in FOREST_DISTRICTS {
        if !keys.contains_key(*forest) {
            bail!("forest district {forest} matches no place");
        }
    }
    if let Some(index) = exceptions_used.iter().position(|used| !used) {
        bail!(
            "fragment exception {} matches no place",
            FRAGMENT_EXCEPTIONS[index]
        );
    }
    let unused: Vec<&str> = GENERIC_NAMES
        .iter()
        .zip(&generic_used)
        .filter(|(_, used)| !**used)
        .map(|(name, _)| *name)
        .collect();
    if !unused.is_empty() {
        bail!("generic names no place uses: {}", unused.join(", "));
    }
    let unused: Vec<&str> = UNSOURCED_NAMES
        .iter()
        .zip(&unsourced_used)
        .filter(|(_, used)| !**used)
        .map(|(name, _)| *name)
        .collect();
    if !unused.is_empty() {
        bail!("unsourced names no place uses: {}", unused.join(", "));
    }
    exclusions.generic.sort();
    exclusions.generic.dedup();
    exclusions.unsourced.sort();
    exclusions.unsourced.dedup();
    Ok(result)
}

pub fn build(inputs: &Inputs) -> Result<Supplement> {
    let mut exclusions = Exclusions::default();
    let candidates = candidates(inputs, &mut exclusions)?;

    let mut readings: HashMap<char, HashSet<&str>> = HashMap::new();
    for (value, key, _) in weighted_rows(inputs.single_chars) {
        let mut characters = value.chars();
        if let (Some(character), None) = (characters.next(), characters.next()) {
            readings.entry(character).or_default().insert(key);
        }
    }
    let shipped = shipped(&[inputs.base, inputs.rime_ice_supplement]);

    let mut distinct: Vec<(String, String)> = candidates
        .iter()
        .map(|candidate| (candidate.word.clone(), candidate.key.clone()))
        .collect();
    distinct.sort();
    distinct.dedup();
    let confirmed = confirmed_positions(&distinct, &shipped);
    // Trailing characters of a full name whose key reads its administrative suffix the fixed way.
    let mut suffix_lengths: HashMap<(String, String), usize> = HashMap::new();
    for candidate in candidates
        .iter()
        .filter(|candidate| candidate.form == Form::Full)
    {
        if let Some((suffix, reading)) = suffix(&candidate.word, candidate.level) {
            if candidate.key == reading || candidate.key.ends_with(&format!("'{reading}")) {
                suffix_lengths.insert(
                    (candidate.word.clone(), candidate.key.clone()),
                    suffix.chars().count(),
                );
            }
        }
    }

    let mut rejected: HashSet<(String, String)> = HashSet::new();
    for (word, key) in &distinct {
        let pair = (word.clone(), key.clone());
        let known = shipped.get(word.as_str());
        let others: Vec<String> = known
            .map(|keys| {
                let mut others: Vec<String> = keys
                    .keys()
                    .filter(|other| **other != key)
                    .map(|other| (*other).to_owned())
                    .collect();
                others.sort();
                others
            })
            .unwrap_or_default();
        let suffix_start = word.chars().count() - suffix_lengths.get(&pair).copied().unwrap_or(0);
        // A reading `READINGS` checked for this place, which the comparison set also has: raising that row leaves the other key's word where it is.
        let reviewed = known.is_some_and(|keys| keys.contains_key(key.as_str()))
            && places::reviewed_positions(word)[..suffix_start]
                .iter()
                .all(|reviewed| *reviewed);
        if !others.is_empty() && !reviewed {
            exclusions
                .other_reading
                .push((word.clone(), key.clone(), others));
            rejected.insert(pair);
            continue;
        }
        if known.is_some_and(|keys| keys.contains_key(key.as_str())) {
            continue;
        }
        let mut unconfirmed = String::new();
        let mut unlisted = None;
        for (position, ((character, syllable), confirmed)) in word
            .chars()
            .zip(key.split('\''))
            .zip(&confirmed[&pair])
            .enumerate()
        {
            let is_confirmed = *confirmed || position >= suffix_start;
            match readings.get(&character) {
                Some(listed) if !listed.contains(syllable) => {
                    unlisted = Some((character, syllable.to_owned()));
                    break;
                }
                Some(listed) if listed.len() == 1 => {}
                _ if is_confirmed => {}
                _ => unconfirmed.push(character),
            }
        }
        if let Some((character, syllable)) = unlisted {
            exclusions
                .unlisted_syllable
                .push((word.clone(), key.clone(), character, syllable));
            rejected.insert(pair);
        } else if !unconfirmed.is_empty() {
            exclusions
                .unconfirmed
                .push((word.clone(), key.clone(), unconfirmed));
            rejected.insert(pair);
        }
    }

    let existing = |candidate: &Candidate| -> Option<i64> {
        shipped
            .get(candidate.word.as_str())
            .and_then(|keys| keys.get(candidate.key.as_str()))
            .copied()
    };
    let accepted: Vec<&Candidate> = candidates
        .iter()
        .filter(|candidate| !rejected.contains(&(candidate.word.clone(), candidate.key.clone())))
        .collect();

    let base_weights: Vec<i64> = weighted_rows(inputs.base)
        .map(|(_, _, weight)| weight)
        .collect();
    let base_median =
        median(base_weights.clone()).with_context(|| format!("{BASE} has no rows"))?;
    let common = percentile(base_weights.clone(), COMMON_PERCENTILE)
        .with_context(|| format!("{BASE} has no rows"))?;
    let placeholders = placeholders(&base_weights, common);
    let leaders = leaders(&shipped, &placeholders);
    let mut floors = BTreeMap::new();
    for level in [Level::Province, Level::Prefecture, Level::County] {
        for form in [Form::Full, Form::Short] {
            let floor = if level == Level::County {
                base_median
            } else {
                median(
                    accepted
                        .iter()
                        .filter(|candidate| candidate.level == level && candidate.form == form)
                        .filter_map(|candidate| existing(candidate))
                        .filter(|weight| *weight > RANKED_WEIGHT)
                        .collect(),
                )
                .with_context(|| {
                    format!(
                        "no {}{} is ranked above {RANKED_WEIGHT} in the comparison set",
                        level.label(),
                        form.label()
                    )
                })?
            };
            floors.insert((level, form), floor);
        }
    }

    let mut targets: BTreeMap<(String, String), i64> = BTreeMap::new();
    for candidate in &accepted {
        let floor = floors[&(candidate.level, candidate.form)];
        let slot = targets
            .entry((candidate.word.clone(), candidate.key.clone()))
            .or_insert(floor);
        *slot = (*slot).max(floor);
    }
    let mut entries = Vec::new();
    let mut capped = Vec::new();
    let mut already_ranked = 0;
    for ((word, key), floor) in targets {
        let existing = shipped
            .get(word.as_str())
            .and_then(|keys| keys.get(key.as_str()))
            .copied();
        if existing.is_some_and(|existing| existing > RANKED_WEIGHT || existing >= floor) {
            already_ranked += 1;
            continue;
        }
        let competitor = leaders
            .get(key.as_str())
            .and_then(|words| words.iter().find(|(other, _)| *other != word))
            .copied();
        let mut weight = floor;
        if let Some((competitor, competitor_weight)) = competitor {
            if competitor_weight > common && competitor_weight - 1 < floor {
                weight = competitor_weight - 1;
                let written = existing.is_none_or(|existing| weight > existing);
                capped.push(Capped {
                    word: word.clone(),
                    key: key.clone(),
                    floor,
                    weight,
                    competitor: competitor.to_owned(),
                    competitor_weight,
                    existing,
                    written,
                });
                if !written {
                    continue;
                }
            }
        }
        entries.push(Entry {
            word,
            key,
            weight,
            existing,
        });
    }
    exclusions.other_reading.sort();
    exclusions.unconfirmed.sort();
    exclusions.unlisted_syllable.sort();
    Ok(Supplement {
        entries,
        floors,
        common,
        placeholders,
        capped,
        exclusions,
        already_ranked,
    })
}

/// Facts the header records, so a reader can reproduce the file.
pub struct Provenance<'a> {
    /// The modood/Administrative-divisions-of-China commit the `places/` files are pinned at.
    pub upstream_commit: &'a str,
    /// `(path, sha256)` of the comparison files.
    pub compared: &'a [(&'a str, &'a str)],
    /// `(path, sha256)` of the single-character readings.
    pub single_chars: (&'a str, &'a str),
    /// 运行生成器的 msime 提交；构建器有未提交改动时带 `-dirty` 后缀。
    pub generator_commit: &'a str,
}

pub fn render(supplement: &Supplement, provenance: &Provenance) -> String {
    let floors = supplement
        .floors
        .iter()
        .map(|((level, form), floor)| format!("{}{} {floor}", level.label(), form.label()))
        .collect::<Vec<_>>()
        .join("，");
    let compared = provenance
        .compared
        .iter()
        .map(|(path, sha256)| format!("{path}（SHA-256 {sha256}）"))
        .collect::<Vec<_>>()
        .join("、");
    let (single_chars, single_chars_sha256) = provenance.single_chars;
    let common = supplement.common;
    let placeholders = placeholder_list(&supplement.placeholders);
    let mut out = String::new();
    let _ = writeln!(out, "# 行政区划地名补充表，由 msime 仓库提交 {} 的 crates/dict-builder/src/places_supplement.rs 以 `msime-dict-build places-supplement --dictionary <msime-dictionary checkout> --cache <dir> --out sources/pinyin/places.txt` 生成；不要手工编辑。", provenance.generator_commit);
    let _ = writeln!(out, "# 上游：https://github.com/modood/Administrative-divisions-of-China 提交 {} 的 dist/provinces.csv、dist/cities.csv、dist/areas.csv（WTFPL）。", provenance.upstream_commit);
    let _ = writeln!(out, "# 收录：省级、地级、县级区划的全称，以及去掉行政后缀（省、自治区、市、自治州、地区、盟、区、县、旗、自治县、自治旗、特区、林区（只用于神农架林区）、矿区）和民族名后至少两个汉字的简称；名称以左、右、前、后、中、合、左翼、右翼结尾的旗与自治县不取简称。排除区划代码末两位为 71–79 的功能区，名称含 {} 的单位（其中含 新区 的正式区划如 浦东新区 也一并排除；{} 是 清新+区，保留），泛名 {}，以及读音有争议、找不到权威出处的 {}。", EXCLUDED_FRAGMENTS.join("、"), FRAGMENT_EXCEPTIONS.join("、"), GENERIC_NAMES.join("、"), UNSOURCED_NAMES.join("、"));
    let _ = writeln!(out, "# 读音：msime places.rs 的 READINGS 加逐字拼音，简称取全称读音的前几个音节。在 sources/pinyin/single-chars.txt 里有多个读音或没有收录的字，必须落在对照集合里同音的至少两字片段中，或是按固定读音（区 qu、县 xian、市 shi 等）读出的全称行政后缀，否则整词不收；音节不在该字读音之列的不收；对照集合里已有同词异音的不收（不论是否也有同音行），例外是除全称行政后缀外整词读音都来自 READINGS 里两字及以上的条目、且对照集合也有这个读音的词（六合 lu'he、宕昌 tan'chang）。");
    let _ = writeln!(out, "# 去重与权重：对照集合是 {compared}；字音表是 {single_chars}（SHA-256 {single_chars_sha256}）。权重下限由生成器从对照集合算出：省级、地级取同层同形（全称或简称）在对照集合里权重大于 {RANKED_WEIGHT} 的已有词的权重中位数，县级取 sources/pinyin/rime-ice.txt 全部行权重的中位数；本次为 {floors}。一个词出现在多个层级时取最高的下限。只写出对照集合里没有、或权重不超过 {RANKED_WEIGHT} 且低于下限的词，权重写下限；对照集合里权重已大于 {RANKED_WEIGHT} 的词保留原权重，不抬升。下限不抢常用词的首位：常用线取 sources/pinyin/rime-ice.txt 全部行权重的第 {COMMON_PERCENTILE} 百分位（本次为 {common}），对照集合里同一全拼的其他词最高权重超过常用线时，权重降到比它低 1（不超过下限），降后不高于已有权重的词不写出；比较时跳过权重为占位值的词，占位值是 rime-ice.txt 里权重超过常用线的行中被超过 {PLACEHOLDER_PERCENT}% 的行共用的单个权重值（本次为 {placeholders}），这类词不触发封顶，由同一全拼下其余非占位权重的最高者决定；构建的 places-supplement 阶段在 custom/words.txt 之前按只升不降并入：没有的插入，更低的调高到这里的权重，更高的不变。");
    for entry in &supplement.entries {
        let _ = writeln!(out, "{}\t{}\t{}", entry.word, entry.key, entry.weight);
    }
    out
}

fn placeholder_list(placeholders: &[i64]) -> String {
    if placeholders.is_empty() {
        return "无".to_owned();
    }
    placeholders
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join("、")
}

/// The generator's account of what it left out, one line per group, for review.
pub fn report(supplement: &Supplement) -> Vec<String> {
    let exclusions = &supplement.exclusions;
    let mut lines: Vec<String> = supplement
        .floors
        .iter()
        .map(|((level, form), floor)| format!("floor {}{}: {floor}", level.label(), form.label()))
        .collect();
    lines.push(format!(
        "common weight (p{COMMON_PERCENTILE} of {BASE}): {}; placeholder weights ignored as competitors (>{PLACEHOLDER_PERCENT}% of the rows above it): {}",
        supplement.common,
        if supplement.placeholders.is_empty() {
            "none".to_owned()
        } else {
            supplement
                .placeholders
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        }
    ));
    let not_written = supplement
        .capped
        .iter()
        .filter(|capped| !capped.written)
        .count();
    lines.push(format!(
        "capped below a common word of the same key, {} ({not_written} not written): {}",
        supplement.capped.len(),
        supplement
            .capped
            .iter()
            .map(|capped| format!(
                "{}({}; {} -> {} under {} {}{}{})",
                capped.word,
                capped.key,
                capped.floor,
                capped.weight,
                capped.competitor,
                capped.competitor_weight,
                capped
                    .existing
                    .map(|existing| format!(", has {existing}"))
                    .unwrap_or_default(),
                if capped.written { "" } else { ", not written" }
            ))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    lines.push(format!(
        "functional zones (codes 71-79), {}: {}",
        exclusions.functional_zones.len(),
        exclusions.functional_zones.join(" ")
    ));
    lines.push(format!(
        "excluded fragments, {}: {}",
        exclusions.fragments.len(),
        exclusions.fragments.join(" ")
    ));
    lines.push(format!(
        "generic names, {}: {}",
        exclusions.generic.len(),
        exclusions.generic.join(" ")
    ));
    lines.push(format!(
        "unsourced readings, {}: {}",
        exclusions.unsourced.len(),
        exclusions.unsourced.join(" ")
    ));
    lines.push(format!(
        "another reading in the comparison set, {}: {}",
        exclusions.other_reading.len(),
        exclusions
            .other_reading
            .iter()
            .map(|(word, key, others)| format!("{word}({key}; has {})", others.join(",")))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    lines.push(format!(
        "unconfirmed readings, {}: {}",
        exclusions.unconfirmed.len(),
        exclusions
            .unconfirmed
            .iter()
            .map(|(word, key, characters)| format!("{word}({key}; {characters})"))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    lines.push(format!(
        "syllables single-chars.txt does not list, {}: {}",
        exclusions.unlisted_syllable.len(),
        exclusions
            .unlisted_syllable
            .iter()
            .map(|(word, key, character, syllable)| format!(
                "{word}({key}; {character} {syllable})"
            ))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROVINCES: &str = "code,name\n33,\"浙江省\"\n45,\"广西壮族自治区\"\n";
    const CITIES: &str = "code,name,provinceCode\n3301,\"杭州市\",33\n3307,\"金华市\",33\n3305,\"湖州市\",33\n3306,\"和平市\",33\n3302,\"宁波市\",33\n3303,\"温州市\",33\n4590,\"自治区直辖县级行政区划\",45\n";

    fn areas() -> String {
        let mut areas = String::from("code,name,cityCode,provinceCode\n330702,\"婺城区\",3307,33\n330771,\"金华经济技术开发区\",3307,33\n330110,\"余杭新区\",3301,33\n459001,\"龙胜各族自治县\",4590,45\n459002,\"土默特左旗\",4590,45\n459003,\"乐陵市\",4590,45\n459004,\"中牟县\",4590,45\n459005,\"城关区\",4590,45\n459006,\"长子县\",4590,45\n459007,\"清新区\",4590,45\n459008,\"神农架林区\",4590,45\n330703,\"金东区\",3307,33\n459009,\"称多县\",4590,45\n459010,\"民乐县\",4590,45\n");
        // places::build requires every READINGS entry and this module every generic name to be used.
        let mut code = 459100;
        for name in places_readings_names()
            .into_iter()
            .chain(GENERIC_NAMES.iter().map(|name| (*name).to_owned()))
        {
            code += 1;
            let _ = writeln!(areas, "{code},\"{name}\",4590,45");
        }
        areas
    }

    fn places_readings_names() -> Vec<String> {
        // The fixture of places.rs does the same: one district per reading.
        let source = include_str!("places.rs");
        let start = source.find("const READINGS").unwrap();
        let end = start + source[start..].find("];").unwrap();
        source[start..end]
            .lines()
            .filter_map(|line| line.trim().strip_prefix("(\""))
            .map(|rest| format!("{}区", &rest[..rest.find('"').unwrap()]))
            .collect()
    }

    const SINGLE_CHARS: &str = "#\r\n金\tjin\t1\r\n华\thua\t1\r\n湖\thu\t1\r\n州\tzhou\t1\r\n杭\thang\t1\r\n婺\twu\t1\r\n城\tcheng\t1\r\n区\tqu\t1\r\n区\tou\t1\r\n乐\tle\t1\r\n乐\tyue\t1\r\n乐\tlao\t1\r\n陵\tling\t1\r\n长\tchang\t1\r\n长\tzhang\t1\r\n子\tzi\t1\r\n东\tdong\t1\r\n市\tshi\t1\r\n县\txian\t1\r\n";

    const BASE_DICT: &str = "# c\r\n杭州\thang'zhou\t50000\r\n杭州市\thang'zhou'shi\t20000\r\n金华市\tjin'hua'shi\t20000\r\n湖州市\thu'zhou'shi\t10\r\n湖州\thu'zhou\t3\r\n浙江\tzhe'jiang\t90000\r\n浙江省\tzhe'jiang'sheng\t90000\r\n广西\tguang'xi\t90000\r\n广西壮族自治区\tguang'xi'zhuang'zu'zi'zhi'qu\t90000\r\n乐陵\tlao'ling\t2000\r\n宁波\tning'bo\t40000\r\n温州\twen'zhou\t60000\r\n六合\tliu'he\t44535\r\n六合\tlu'he\t2\r\n称多县\tcheng'duo'xian\t795\r\n民乐\tmin'yue\t6020\r\n长子\tzhang'zi\t9000\r\n长子\tchang'zi\t50\r\n城区\tcheng'qu\t500\r\n";

    fn supplement() -> Supplement {
        let areas = areas();
        build(&Inputs {
            provinces: PROVINCES,
            cities: CITIES,
            areas: &areas,
            single_chars: SINGLE_CHARS,
            base: BASE_DICT,
            rime_ice_supplement: "# s\n",
        })
        .unwrap()
    }

    /// `BASE_DICT` plus 6000 filler rows at 5, which put the 90th percentile, the common weight, at 5, and 500 rows at distinct weights above it, so no weight of the fixture is shared by over 1% of the rows above common and none is a placeholder.
    fn padded_base() -> String {
        let mut base = String::from(BASE_DICT);
        for index in 0..6000 {
            let _ = write!(base, "填充{index}\tfiller'{index}\t5\r\n");
        }
        for index in 0..500 {
            let _ = write!(base, "实词{index}\treal'{index}\t{}\r\n", 1000 + index);
        }
        base
    }

    fn entry<'a>(supplement: &'a Supplement, word: &str) -> Option<&'a Entry> {
        supplement.entries.iter().find(|entry| entry.word == word)
    }

    #[test]
    fn short_names_drop_suffixes_and_ethnic_names() {
        assert_eq!(
            short_name("广西壮族自治区", Level::Province).as_deref(),
            Some("广西")
        );
        assert_eq!(
            short_name("双江拉祜族佤族布朗族傣族自治县", Level::County).as_deref(),
            Some("双江")
        );
        assert_eq!(
            short_name("管城回族区", Level::County).as_deref(),
            Some("管城")
        );
        assert_eq!(
            short_name("龙胜各族自治县", Level::County).as_deref(),
            Some("龙胜")
        );
        assert_eq!(
            short_name("察布查尔锡伯自治县", Level::County).as_deref(),
            Some("察布查尔")
        );
        assert_eq!(
            short_name("金华市", Level::Prefecture).as_deref(),
            Some("金华")
        );
        assert_eq!(
            short_name("晋中市", Level::Prefecture).as_deref(),
            Some("晋中")
        );
        assert_eq!(
            short_name("万柏林区", Level::County).as_deref(),
            Some("万柏林")
        );
        assert_eq!(
            short_name("神农架林区", Level::County).as_deref(),
            Some("神农架")
        );
        assert_eq!(
            short_name("井陉矿区", Level::County).as_deref(),
            Some("井陉")
        );
        assert_eq!(short_name("鄂温克族自治旗", Level::County), None);
        assert_eq!(short_name("土默特左旗", Level::County), None);
        assert_eq!(short_name("喀喇沁左翼蒙古族自治县", Level::County), None);
        assert_eq!(short_name("景县", Level::County), None);
        assert_eq!(short_name("矿区", Level::County), None);
    }

    #[test]
    fn missing_and_low_names_are_written_at_their_level_floor() {
        let supplement = supplement();
        // Prefecture short floor: the lower median of the ranked prefecture short names 杭州 (50000), 宁波 (40000) and 温州 (60000).
        assert_eq!(supplement.floors[&(Level::Prefecture, Form::Short)], 50000);
        // County floor: the lower median of the base rows.
        let mut weights: Vec<i64> = weighted_rows(BASE_DICT).map(|row| row.2).collect();
        weights.sort_unstable();
        assert_eq!(
            supplement.floors[&(Level::County, Form::Full)],
            weights[(weights.len() - 1) / 2]
        );
        let jinhua = entry(&supplement, "金华").unwrap();
        assert_eq!(
            (jinhua.key.as_str(), jinhua.weight, jinhua.existing),
            ("jin'hua", 50000, None)
        );
        let huzhou = entry(&supplement, "湖州").unwrap();
        assert_eq!((huzhou.weight, huzhou.existing), (50000, Some(3)));
        assert_eq!(entry(&supplement, "湖州市").unwrap().weight, 20000);
        // Already at or above the floor: not written.
        assert!(entry(&supplement, "杭州").is_none());
        assert!(entry(&supplement, "金华市").is_none());
        // Ranked above RANKED_WEIGHT though below the floor: rime-ice's weight stands.
        assert!(entry(&supplement, "宁波").is_none());
    }

    #[test]
    fn a_common_word_of_the_same_key_caps_the_floor() {
        let mut base = padded_base();
        // 胡诌 is common and below 湖州's floor; 进化 sits at the common weight; 胡州事 is common but one above 湖州市's existing weight.
        base.push_str("胡诌\thu'zhou\t30000\r\n进化\tjin'hua\t5\r\n胡州事\thu'zhou'shi\t11\r\n");
        let areas = areas();
        let supplement = build(&Inputs {
            provinces: PROVINCES,
            cities: CITIES,
            areas: &areas,
            single_chars: SINGLE_CHARS,
            base: &base,
            rime_ice_supplement: "# s\n",
        })
        .unwrap();
        assert_eq!(supplement.common, 5);
        assert_eq!(supplement.floors[&(Level::Prefecture, Form::Short)], 50000);
        // Capped one below 胡诌 and still written, since 29999 is above its existing 3.
        let huzhou = entry(&supplement, "湖州").unwrap();
        assert_eq!((huzhou.weight, huzhou.existing), (29999, Some(3)));
        // A word at the common weight does not cap: 金华 keeps its floor ahead of 进化.
        assert_eq!(entry(&supplement, "金华").unwrap().weight, 50000);
        // Capped to 10, which is not above the existing 10: not written.
        assert!(entry(&supplement, "湖州市").is_none());
        let capped: Vec<(&str, i64, &str, bool)> = supplement
            .capped
            .iter()
            .map(|capped| {
                (
                    capped.word.as_str(),
                    capped.weight,
                    capped.competitor.as_str(),
                    capped.written,
                )
            })
            .collect();
        assert_eq!(
            capped,
            [
                ("湖州", 29999, "胡诌", true),
                ("湖州市", 10, "胡州事", false)
            ]
        );
    }

    #[test]
    fn a_supplement_competitor_caps_a_new_name_and_one_above_the_floor_does_not() {
        // The padding puts both the common weight and the county floor (the base median) at 5.
        let base = padded_base();
        // 金化 is common and only in rime-ice-supplement.txt; 无城区 is common but at the county floor plus one.
        let rime_ice_supplement = "# s\n金化\tjin'hua\t40000\n无城区\twu'cheng'qu\t6\n";
        let areas = areas();
        let supplement = build(&Inputs {
            provinces: PROVINCES,
            cities: CITIES,
            areas: &areas,
            single_chars: SINGLE_CHARS,
            base: &base,
            rime_ice_supplement,
        })
        .unwrap();
        assert_eq!(supplement.common, 5);
        assert_eq!(supplement.floors[&(Level::Prefecture, Form::Short)], 50000);
        assert_eq!(supplement.floors[&(Level::County, Form::Full)], 5);
        // 金华 is not in the comparison set: capped one below the supplement's 金化 and written as a new name.
        let jinhua = entry(&supplement, "金华").unwrap();
        assert_eq!(
            (jinhua.key.as_str(), jinhua.weight, jinhua.existing),
            ("jin'hua", 39999, None)
        );
        // 无城区 is above the common weight but not above the floor of 婺城区: the floor stands and nothing is capped.
        let wuchengqu = entry(&supplement, "婺城区").unwrap();
        assert_eq!((wuchengqu.weight, wuchengqu.existing), (5, None));
        let capped: Vec<(&str, i64, &str, Option<i64>, bool)> = supplement
            .capped
            .iter()
            .map(|capped| {
                (
                    capped.word.as_str(),
                    capped.weight,
                    capped.competitor.as_str(),
                    capped.existing,
                    capped.written,
                )
            })
            .collect();
        assert_eq!(capped, [("金华", 39999, "金化", None, true)]);
    }

    #[test]
    fn placeholders_are_weights_shared_by_over_one_percent_of_the_rows_above_common() {
        // 200 distinct weights above the common weight 10, 9999 five times more, 150 once more: 9999 holds 5 of 206 rows (over 1%), 150 holds 2 (under). The many rows at 10 are not above common and do not count.
        let mut weights: Vec<i64> = (11..211).collect();
        weights.extend([9999; 5]);
        weights.push(150);
        weights.extend([10; 1000]);
        assert_eq!(placeholders(&weights, 10), [9999]);
        assert!(placeholders(&(11..211).collect::<Vec<_>>(), 10).is_empty());
    }

    #[test]
    fn a_placeholder_competitor_does_not_cap_but_a_real_one_behind_it_does() {
        // 30 rows at 9999 make it a placeholder.
        let mut base = padded_base();
        for index in 0..30 {
            let _ = write!(base, "占位{index}\tzhan'wei'{index}\t9999\r\n");
        }
        // 胡诌 is the only other word of hu'zhou and sits at the placeholder; 进化 leads jin'hua at the placeholder with the real 近华 behind it.
        base.push_str("胡诌\thu'zhou\t9999\r\n进化\tjin'hua\t9999\r\n近华\tjin'hua\t8000\r\n");
        let areas = areas();
        let supplement = build(&Inputs {
            provinces: PROVINCES,
            cities: CITIES,
            areas: &areas,
            single_chars: SINGLE_CHARS,
            base: &base,
            rime_ice_supplement: "# s\n",
        })
        .unwrap();
        assert_eq!(supplement.common, 5);
        assert_eq!(supplement.placeholders, [9999]);
        assert_eq!(supplement.floors[&(Level::Prefecture, Form::Short)], 50000);
        // 胡诌 at the placeholder does not cap 湖州.
        assert_eq!(entry(&supplement, "湖州").unwrap().weight, 50000);
        // 进化 is skipped, and 近华 behind it caps 金华.
        assert_eq!(entry(&supplement, "金华").unwrap().weight, 7999);
        let capped: Vec<(&str, i64, &str, i64)> = supplement
            .capped
            .iter()
            .map(|capped| {
                (
                    capped.word.as_str(),
                    capped.weight,
                    capped.competitor.as_str(),
                    capped.competitor_weight,
                )
            })
            .collect();
        assert_eq!(capped, [("金华", 7999, "近华", 8000)]);
        assert!(report(&supplement)
            .iter()
            .any(|line| line.contains("placeholder weights") && line.ends_with(": 9999")));
    }

    #[test]
    fn without_a_common_competitor_the_floor_stands() {
        let supplement = supplement();
        // The fixture's 90th percentile is 90000, so no word of the same key is common.
        assert_eq!(supplement.common, 90000);
        assert!(supplement.capped.is_empty());
        assert_eq!(entry(&supplement, "湖州").unwrap().weight, 50000);
    }

    #[test]
    fn readings_must_be_confirmed_and_unambiguous() {
        let supplement = supplement();
        // 乐陵 is read lao'ling by READINGS and the base dictionary agrees.
        assert!(entry(&supplement, "乐陵市").is_some_and(|entry| entry.key == "lao'ling'shi"));
        // 长子 has chang'zi too, but zhang'zi is its READINGS entry, which the base dictionary ranks already: not written, not excluded either.
        assert!(entry(&supplement, "长子").is_none());
        assert!(!supplement
            .exclusions
            .other_reading
            .iter()
            .any(|(word, _, _)| word == "长子"));
        // 民乐 is an ordinary word read min'yue, and its key comes from the per-character pinyin, not READINGS.
        assert!(entry(&supplement, "民乐").is_none());
        assert!(supplement
            .exclusions
            .other_reading
            .iter()
            .any(|(word, _, _)| word == "民乐"));
        // 区 is polyphonic and 婺城区 appears nowhere in the comparison set, but 城区 confirms 城区's syllables, and 婺 has one reading.
        assert!(entry(&supplement, "婺城区").is_some());
        // Nothing in the comparison set holds 东区 or 金东区: the full name's suffix 区 is confirmed by its fixed reading qu.
        assert!(entry(&supplement, "金东区").is_some_and(|entry| entry.key == "jin'dong'qu"));
        // 六合 has the ordinary reading liu'he too, but lu'he comes from the READINGS entry 六合 and the comparison set has it: raised, liu'he untouched.
        let luhe = entry(&supplement, "六合").unwrap();
        assert_eq!((luhe.key.as_str(), luhe.existing), ("lu'he", Some(2)));
        assert!(!supplement.entries.iter().any(|entry| entry.key == "liu'he"));
        // 称多 is confirmed by the comparison set but has no source for its reading.
        assert!(entry(&supplement, "称多").is_none());
        assert_eq!(supplement.exclusions.unsourced, ["称多", "称多县"]);
        // 中牟: no reading of 中 or 牟 in the single characters, nothing confirms them.
        assert!(supplement
            .exclusions
            .unconfirmed
            .iter()
            .any(|(word, _, _)| word == "中牟县"));
    }

    #[test]
    fn zones_fragments_and_generic_names_are_left_out() {
        let supplement = supplement();
        assert_eq!(
            supplement.exclusions.functional_zones,
            ["金华经济技术开发区"]
        );
        assert!(supplement
            .exclusions
            .fragments
            .contains(&"余杭新区".to_owned()));
        assert!(!supplement
            .exclusions
            .fragments
            .contains(&"清新区".to_owned()));
        assert!(supplement.exclusions.generic.contains(&"城关".to_owned()));
        assert!(supplement.exclusions.generic.contains(&"和平".to_owned()));
        assert!(entry(&supplement, "城关").is_none());
        assert!(entry(&supplement, "和平").is_none());
    }

    #[test]
    fn an_unused_generic_name_fails_the_generator() {
        let error = build(&Inputs {
            provinces: PROVINCES,
            cities: CITIES,
            areas: &(places_readings_names().iter().enumerate().fold(
                String::from("code,name,cityCode,provinceCode\n459007,\"清新区\",4590,45\n459008,\"神农架林区\",4590,45\n"),
                |mut areas, (index, name)| {
                    let _ = writeln!(areas, "{},\"{name}\",4590,45", 459100 + index);
                    areas
                },
            )),
            single_chars: SINGLE_CHARS,
            base: BASE_DICT,
            rime_ice_supplement: "",
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("generic names no place uses"), "{error}");
    }

    #[test]
    fn the_file_is_sorted_with_a_header() {
        let supplement = supplement();
        let text = render(
            &supplement,
            &Provenance {
                upstream_commit: "c49d495b",
                compared: &[(super::BASE, "00")],
                single_chars: (super::SINGLE_CHARS, "11"),
                generator_commit: "0123456789abcdef0123456789abcdef01234567",
            },
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("# 行政区划地名补充表"));
        assert!(
            lines[0].contains("msime 仓库提交 0123456789abcdef0123456789abcdef01234567"),
            "{}",
            lines[0]
        );
        assert!(!lines[0].contains("dictionary-sources.lock.json"));
        let body: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|line| !line.starts_with('#'))
            .collect();
        assert_eq!(body.len(), supplement.entries.len());
        let mut sorted = body.clone();
        sorted.sort_by_key(|line| {
            let mut fields = line.split('\t');
            (
                fields.next().unwrap().to_owned(),
                fields.next().unwrap().to_owned(),
            )
        });
        assert_eq!(body, sorted);
        assert!(text.ends_with('\n') && !text.contains('\r'));
        for line in body {
            assert!(
                crate::msime::parse_custom_word(line).unwrap().is_some(),
                "{line}"
            );
        }
    }
}
