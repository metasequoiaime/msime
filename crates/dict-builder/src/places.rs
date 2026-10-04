//! `places`: the table of Chinese administrative divisions `@` mode offers after the user's own mention list, generated from modood/Administrative-divisions-of-China (WTFPL) at the commit `resources/dictionary-sources.lock.json` pins under `places/`.
//!
//! The output is `crates/engine/src/local/places.tsv`, one `name<TAB>key<TAB>parent` line per division, which the engine embeds with `include_str!`. Provinces come first, then cities, then counties and districts, each in the source's code order, so a short key lists the larger divisions before the smaller ones. A name that recurs (朝阳区 is a district of both 北京市 and 长春市) is kept once, at its first and largest occurrence.
//!
//! The key is the toneless pinyin of every character joined by `'`, the shape a mention key has. The `pinyin` crate reads each character on its own, so the place names whose reading differs from the per-character one are listed in `READINGS`.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::pinyin::per_character;

pub const PROVINCES: &str = "places/provinces.csv";
pub const CITIES: &str = "places/cities.csv";
pub const AREAS: &str = "places/areas.csv";

/// Rows of `cities.csv` that group a province's directly administered districts or counties rather than name a city. Their districts take the province as their parent.
pub(crate) const GROUPING_CITIES: [&str; 4] = [
    "市辖区",
    "县",
    "省直辖县级行政区划",
    "自治区直辖县级行政区划",
];

/// Readings the per-character pinyin gets wrong in place names, as `(fragment, syllables)`. At each position of a name the longest fragment that starts there wins, so `长子` (zhang'zi) overrides `长` (chang). Every entry must be used by the pinned data; an unused one fails the build so the list does not rot when the source moves.
const READINGS: &[(&str, &str)] = &[
    ("长", "chang"),
    ("长子", "zhang'zi"),
    ("都", "du"),
    ("什", "shi"),
    ("佛", "fo"),
    ("藏", "zang"),
    ("勒", "le"),
    ("蚌", "beng"),
    ("圩", "xu"),
    ("重庆", "chong'qing"),
    ("六安", "lu'an"),
    ("六合", "lu'he"),
    ("厦门", "xia'men"),
    ("番禺", "pan'yu"),
    ("乐亭", "lao'ting"),
    ("乐清", "yue'qing"),
    // 新华字典 (as zdic.net quotes it): 乐 lào, 地名用字：河北省乐亭、山东省乐陵.
    ("乐陵", "lao'ling"),
    ("东阿", "dong'e"),
    ("牟平", "mu'ping"),
    ("穆棱", "mu'ling"),
    ("大埔", "da'bu"),
    ("吴堡", "wu'bu"),
    ("寺堡", "si'bu"),
    ("泊头", "bo'tou"),
    ("茄子", "qie'zi"),
    ("珲春", "hun'chun"),
    ("伽师", "jia'shi"),
    ("宝坻", "bao'di"),
    ("曾都", "zeng'du"),
    ("黄陂", "huang'pi"),
    ("加查", "jia'cha"),
    ("尉犁", "yu'li"),
    ("蔚县", "yu'xian"),
    ("洪洞", "hong'tong"),
    // 民政部、教育部、国家语委 2014 年联合批复宕昌县县名读音定为 tàn chāng (cited by zh.wikipedia 宕昌县 from 宕昌县人民政府, 2014-03-31, 国家三部委联合为甘肃省宕昌县正名定音促发展).
    ("宕昌", "tan'chang"),
    // 新华字典 (as zdic.net quotes it): 峒 tóng 〔崆峒〕; the district is named after 崆峒山 (Kōngtóng).
    ("崆峒", "kong'tong"),
    ("单县", "shan'xian"),
    ("繁峙", "fan'shi"),
    ("铅山", "yan'shan"),
    ("筠连", "jun'lian"),
    ("犍为", "qian'wei"),
    ("浚县", "xun'xian"),
    ("泌阳", "bi'yang"),
    ("涡阳", "guo'yang"),
    ("枞阳", "zong'yang"),
    ("荥经", "ying'jing"),
    ("闵行", "min'hang"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub name: String,
    pub key: String,
    pub parent: String,
}

pub(crate) fn records(source: &str, path: &str, columns: usize) -> Result<Vec<Vec<String>>> {
    let mut reader = csv::Reader::from_reader(source.as_bytes());
    let mut rows = Vec::new();
    for (index, record) in reader.records().enumerate() {
        let record = record.with_context(|| format!("{path}: row {}", index + 2))?;
        if record.len() != columns {
            bail!(
                "{path}: row {} has {} columns, expected {columns}",
                index + 2,
                record.len()
            );
        }
        rows.push(record.iter().map(|field| field.trim().to_owned()).collect());
    }
    Ok(rows)
}

/// The `READINGS` entry that applies at the start of `rest`: the longest fragment it starts with.
fn reading_at(rest: &str) -> Option<(usize, &'static (&'static str, &'static str))> {
    READINGS
        .iter()
        .enumerate()
        .filter(|(_, (fragment, _))| rest.starts_with(fragment))
        .max_by_key(|(_, (fragment, _))| fragment.chars().count())
}

/// Which characters of a place name take their reading from a `READINGS` entry of two or more characters: a reading checked for that place, unlike a single-character default such as `长` chang.
pub(crate) fn reviewed_positions(name: &str) -> Vec<bool> {
    let characters: Vec<char> = name.chars().collect();
    let mut reviewed = vec![false; characters.len()];
    let mut position = 0;
    while position < characters.len() {
        let rest: String = characters[position..].iter().collect();
        let length = reading_at(&rest).map_or(1, |(_, (fragment, _))| fragment.chars().count());
        if length >= 2 {
            reviewed[position..position + length].fill(true);
        }
        position += length;
    }
    reviewed
}

/// The pinyin key of a place name, applying `READINGS` and recording which entries were used.
fn key(name: &str, used: &mut [bool]) -> Result<String> {
    let characters: Vec<char> = name.chars().collect();
    let mut syllables: Vec<String> = Vec::new();
    let mut position = 0;
    while position < characters.len() {
        let rest: String = characters[position..].iter().collect();
        if let Some((index, (fragment, reading))) = reading_at(&rest) {
            used[index] = true;
            syllables.extend(reading.split('\'').map(str::to_owned));
            position += fragment.chars().count();
            continue;
        }
        let character = characters[position].to_string();
        let items = per_character(&character);
        match items.as_slice() {
            [item] if !item.is_empty() && item.bytes().all(|byte| byte.is_ascii_lowercase()) => {
                syllables.push(item.clone());
            }
            _ => bail!("{name}: no pinyin for {character:?}"),
        }
        position += 1;
    }
    Ok(syllables.join("'"))
}

/// The places of the three source tables, provinces first, each name once.
pub fn build(provinces: &str, cities: &str, areas: &str) -> Result<Vec<Place>> {
    let provinces = records(provinces, PROVINCES, 2)?;
    let cities = records(cities, CITIES, 3)?;
    let areas = records(areas, AREAS, 4)?;
    let province_name = |code: &str| -> Result<String> {
        provinces
            .iter()
            .find(|row| row[0] == code)
            .map(|row| row[1].clone())
            .with_context(|| format!("unknown province code {code}"))
    };
    let mut entries: Vec<(String, String)> = Vec::new();
    for row in &provinces {
        entries.push((row[1].clone(), String::new()));
    }
    for row in &cities {
        let parent = province_name(&row[2])?;
        if !GROUPING_CITIES.contains(&row[1].as_str()) {
            entries.push((row[1].clone(), parent));
        }
    }
    for row in &areas {
        let city = cities
            .iter()
            .find(|city| city[0] == row[2])
            .with_context(|| format!("{}: unknown city code {}", row[1], row[2]))?;
        let parent = if GROUPING_CITIES.contains(&city[1].as_str()) {
            province_name(&row[3])?
        } else {
            city[1].clone()
        };
        entries.push((row[1].clone(), parent));
    }

    let mut used = vec![false; READINGS.len()];
    let mut seen = HashSet::new();
    let mut places = Vec::new();
    for (name, parent) in entries {
        if name.is_empty() || name.contains(['\t', '\n', '\r']) {
            bail!("unusable place name {name:?}");
        }
        if !seen.insert(name.clone()) {
            continue;
        }
        let key = key(&name, &mut used)?;
        places.push(Place { name, key, parent });
    }
    let unused: Vec<&str> = READINGS
        .iter()
        .zip(&used)
        .filter(|(_, used)| !**used)
        .map(|((fragment, _), _)| *fragment)
        .collect();
    if !unused.is_empty() {
        bail!("readings no place name uses: {}", unused.join(", "));
    }
    Ok(places)
}

pub fn tsv(places: &[Place]) -> String {
    let mut text = String::new();
    for place in places {
        let _ = writeln!(text, "{}\t{}\t{}", place.name, place.key, place.parent);
    }
    text
}

pub fn write(places: &[Place], out: &Path) -> Result<()> {
    std::fs::write(out, tsv(places)).with_context(|| format!("writing {}", out.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROVINCES_CSV: &str = "code,name\n11,\"北京市\"\n50,\"重庆市\"\n34,\"安徽省\"\n";
    const CITIES_CSV: &str = "code,name,provinceCode\n1101,\"市辖区\",11\n5001,\"市辖区\",50\n5002,\"县\",50\n3415,\"六安市\",34\n3403,\"蚌埠市\",34\n";
    const AREAS_CSV: &str = "code,name,cityCode,provinceCode\n110105,\"朝阳区\",1101,11\n500101,\"万州区\",5001,50\n500229,\"城口县\",5002,50\n341502,\"金安区\",3415,34\n340302,\"龙子湖区\",3403,34\n340303,\"朝阳区\",3403,34\n";

    fn fixture() -> Vec<Place> {
        let mut readings_used = AREAS_CSV.to_owned();
        // Every reading must be used, so the fixture names one place per remaining entry.
        for (index, (fragment, _)) in READINGS.iter().enumerate() {
            let _ = writeln!(readings_used, "9{index:05},\"{fragment}区\",3415,34");
        }
        build(PROVINCES_CSV, CITIES_CSV, &readings_used).unwrap()
    }

    fn place<'a>(places: &'a [Place], name: &str) -> &'a Place {
        places.iter().find(|place| place.name == name).unwrap()
    }

    #[test]
    fn provinces_come_first_and_grouping_rows_are_not_places() {
        let places = fixture();
        let names: Vec<&str> = places
            .iter()
            .take(5)
            .map(|place| place.name.as_str())
            .collect();
        assert_eq!(names, ["北京市", "重庆市", "安徽省", "六安市", "蚌埠市"]);
        assert!(places
            .iter()
            .all(|place| !GROUPING_CITIES.contains(&place.name.as_str())));
        assert_eq!(place(&places, "北京市").parent, "");
        assert_eq!(place(&places, "六安市").parent, "安徽省");
    }

    #[test]
    fn a_district_under_a_grouping_row_takes_the_province_as_parent() {
        let places = fixture();
        assert_eq!(place(&places, "万州区").parent, "重庆市");
        assert_eq!(place(&places, "城口县").parent, "重庆市");
        assert_eq!(place(&places, "金安区").parent, "六安市");
    }

    #[test]
    fn a_recurring_name_keeps_its_first_occurrence() {
        let places = fixture();
        let chaoyang: Vec<&Place> = places
            .iter()
            .filter(|place| place.name == "朝阳区")
            .collect();
        assert_eq!(chaoyang.len(), 1);
        assert_eq!(chaoyang[0].parent, "北京市");
    }

    #[test]
    fn keys_use_the_place_readings() {
        let places = fixture();
        assert_eq!(place(&places, "重庆市").key, "chong'qing'shi");
        assert_eq!(place(&places, "六安市").key, "lu'an'shi");
        assert_eq!(place(&places, "蚌埠市").key, "beng'bu'shi");
        assert_eq!(place(&places, "北京市").key, "bei'jing'shi");
        assert_eq!(place(&places, "长子区").key, "zhang'zi'qu");
        assert_eq!(place(&places, "长区").key, "chang'qu");
        assert_eq!(place(&places, "朝阳区").key, "chao'yang'qu");
        assert_eq!(place(&places, "乐陵区").key, "lao'ling'qu");
        assert_eq!(place(&places, "崆峒区").key, "kong'tong'qu");
        assert_eq!(place(&places, "宕昌区").key, "tan'chang'qu");
    }

    #[test]
    fn reviewed_positions_cover_multi_character_readings_only() {
        assert_eq!(reviewed_positions("六合区"), [true, true, false]);
        assert_eq!(reviewed_positions("长沙"), [false, false]);
        assert_eq!(reviewed_positions("长子县"), [true, true, false]);
    }

    #[test]
    fn an_unused_reading_fails_the_build() {
        let error = build(PROVINCES_CSV, CITIES_CSV, AREAS_CSV)
            .unwrap_err()
            .to_string();
        assert!(error.contains("readings no place name uses"), "{error}");
    }

    #[test]
    fn the_table_is_one_line_per_place() {
        let places = vec![Place {
            name: "六安市".into(),
            key: "lu'an'shi".into(),
            parent: "安徽省".into(),
        }];
        assert_eq!(tsv(&places), "六安市\tlu'an'shi\t安徽省\n");
    }
}
