//! `C` 模式（拆字反查）：用户按部件的拼音找一个不会读的字，`huohuohuo` 得 焱、`nvzi` 得 好、`shouchi` 得 拆。
//!
//! 数据是 `scripts/build_chaizi_table.py` 从 kfcd/chaizi（CC BY 3.0）、Unihan 和 OpenCC 生成的两张表，编进引擎，不读磁盘：
//!
//! - `chaizi.tsv`：`键<TAB>字串`。键是各部件的去调拼音以 `'` 连接，字串按名次排好；按去掉 `'` 后的键排序。
//! - `chaizi-chars.tsv`：`字<TAB>名次<TAB>读音<TAB>繁体字形`，按码位排序。
//!
//! 第一次查询时为两张表各建一份行首偏移表，之后二分查找，不为每个键分配字符串。网页引擎不编入这两张表（它关掉了全部本地模式），查询总是空的。

use std::cmp::Ordering;
use std::sync::OnceLock;

use crate::shuangpin::query::normalize_input;
use crate::shuangpin::ShuangpinProfile;
use crate::types::{CandidateSource, SchemeType, WordItem};

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
const KEYS: &str = include_str!("chaizi.tsv");
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
const CHARS: &str = include_str!("chaizi-chars.tsv");
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
const KEYS: &str = "";
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
const CHARS: &str = "";

/// 一次最多列出的行数。一个键最多 23 个字，所以整串精确命中的字总能列全。
pub const RESULT_LIMIT: usize = 36;
/// 前缀补全要求输入已经打完键的前几个部件：只打一个部件时，以它开头的键动辄几十个（`hen` 34 个、`hang` 26 个），补全出来的字没有意义，还会让 `Chen`、`Chang` 这样的英文在空格时上屏一个生僻字。
const PREFIX_COMPLETE_COMPONENTS: usize = 2;
/// 一个键最多几个部件（实测 10），用来给音节边界开定长数组。
const MAX_COMPONENTS: usize = 16;

/// 一张 TSV 的行首偏移，行不含换行符。
struct Lines {
    text: &'static str,
    starts: Vec<u32>,
}

impl Lines {
    fn new(text: &'static str) -> Self {
        let mut starts = Vec::with_capacity(text.len() / 16);
        let mut start = 0;
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                if index > start {
                    starts.push(start as u32);
                }
                start = index + 1;
            }
        }
        if start < text.len() {
            starts.push(start as u32);
        }
        Self { text, starts }
    }

    fn len(&self) -> usize {
        self.starts.len()
    }

    fn line(&self, index: usize) -> &'static str {
        let text = self.text;
        let rest = &text[self.starts[index] as usize..];
        rest.split_once('\n').map_or(rest, |(line, _)| line)
    }

    fn field(&self, index: usize) -> &'static str {
        let line = self.line(index);
        line.split_once('\t').map_or(line, |(first, _)| first)
    }
}

fn keys() -> &'static Lines {
    static KEY_LINES: OnceLock<Lines> = OnceLock::new();
    KEY_LINES.get_or_init(|| Lines::new(KEYS))
}

fn chars() -> &'static Lines {
    static CHAR_LINES: OnceLock<Lines> = OnceLock::new();
    CHAR_LINES.get_or_init(|| Lines::new(CHARS))
}

/// `chaizi-chars.tsv` 的一行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CharInfo {
    rank: u32,
    reading: &'static str,
    /// OpenCC s2t 对这个字单独转换的结果；和字本身相同时为空。
    traditional: &'static str,
}

fn char_info(character: &str) -> Option<CharInfo> {
    let table = chars();
    let index = binary_search(table.len(), |index| table.field(index).cmp(character))?;
    let mut fields = table.line(index).splitn(4, '\t').skip(1);
    let rank = fields.next()?.parse().ok()?;
    let reading = fields.next().unwrap_or_default();
    let traditional = fields.next().unwrap_or_default();
    Some(CharInfo {
        rank,
        reading,
        traditional,
    })
}

fn binary_search(len: usize, compare: impl Fn(usize) -> Ordering) -> Option<usize> {
    let (mut low, mut high) = (0, len);
    while low < high {
        let middle = low + (high - low) / 2;
        match compare(middle) {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => high = middle,
            Ordering::Equal => return Some(middle),
        }
    }
    None
}

/// 拆字行的注释：这个字的读音（Unihan `kMandarin` 的第一个值，带调）。不在表里或没有读音的字为空。
pub fn reading(word: &str) -> &'static str {
    char_info(word).map_or("", |info| info.reading)
}

/// 去掉 `'` 后的键和查询串比较的结果。表按去掉 `'` 后的键排序，所以这三种结果在表里依次连成三段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Relation {
    Before,
    /// 键以查询串开头（含相等）。
    Extends,
    After,
}

fn relation(key: &str, query: &[u8]) -> Relation {
    let mut letters = key.bytes().filter(|&byte| byte != b'\'');
    for &wanted in query {
        match letters.next() {
            None => return Relation::Before,
            Some(letter) if letter < wanted => return Relation::Before,
            Some(letter) if letter > wanted => return Relation::After,
            Some(_) => {}
        }
    }
    Relation::Extends
}

/// 一个键的音节边界：每个部件读音结束处在去掉 `'` 后的字母串里的位置。
struct Boundaries {
    ends: [usize; MAX_COMPONENTS],
    count: usize,
}

impl Boundaries {
    fn of(key: &str) -> Option<Self> {
        let mut ends = [0; MAX_COMPONENTS];
        let mut count = 0;
        let mut length = 0;
        for syllable in key.split('\'') {
            if count == MAX_COMPONENTS {
                return None;
            }
            length += syllable.len();
            ends[count] = length;
            count += 1;
        }
        Some(Self { ends, count })
    }

    fn ends(&self) -> &[usize] {
        &self.ends[..self.count]
    }
}

/// 查询串：去掉 `'` 的字母，以及用户写下的每个 `'` 落在字母串的什么位置。
struct Query {
    letters: Vec<u8>,
    separators: Vec<usize>,
}

impl Query {
    fn new(code: &str) -> Self {
        let mut letters = Vec::with_capacity(code.len());
        let mut separators = Vec::new();
        for byte in code.bytes() {
            if byte == b'\'' {
                // 打头的 `'` 前面没有部件，不算分隔符：输入时它收不进来，但删掉 `C'huo` 中间的字母会留下它。
                if !letters.is_empty() {
                    separators.push(letters.len());
                }
            } else {
                letters.push(byte);
            }
        }
        Self {
            letters,
            separators,
        }
    }
}

/// 一个命中：精确命中排在前缀补全之前，补全按还差几个部件、再按名次排。
#[derive(Debug, Clone, Copy)]
struct Hit {
    missing: usize,
    rank: u32,
    character: &'static str,
    traditional: &'static str,
    key: &'static str,
}

/// 把 `query` 能查到的字追加到 `hits`。精确命中的 `missing` 为 0。
fn collect(query: &Query, hits: &mut Vec<Hit>) {
    if query.letters.is_empty() {
        return;
    }
    let table = keys();
    let lower = partition_point(table.len(), |index| {
        relation(table.field(index), &query.letters) == Relation::Before
    });
    let upper = partition_point(table.len(), |index| {
        relation(table.field(index), &query.letters) != Relation::After
    });
    let typed = query.letters.len();
    for index in lower..upper {
        let line = table.line(index);
        let Some((key, characters)) = line.split_once('\t') else {
            continue;
        };
        let Some(boundaries) = Boundaries::of(key) else {
            continue;
        };
        let ends = boundaries.ends();
        // 用户写下的 `'` 必须正好落在部件之间，`huo'huo` 不会匹配 `hu'ohuo` 这样的拆法。
        if !query
            .separators
            .iter()
            .all(|separator| ends.contains(separator))
        {
            continue;
        }
        let total = ends.last().copied().unwrap_or_default();
        let missing = if total == typed {
            0
        } else {
            let complete = ends.iter().take_while(|&&end| end <= typed).count();
            if ends.len() < PREFIX_COMPLETE_COMPONENTS || complete < PREFIX_COMPLETE_COMPONENTS {
                continue;
            }
            ends.len() - complete
        };
        for (offset, character) in characters.char_indices() {
            let character = &characters[offset..offset + character.len_utf8()];
            let Some(info) = char_info(character) else {
                continue;
            };
            hits.push(Hit {
                missing,
                rank: info.rank,
                character,
                traditional: info.traditional,
                key,
            });
        }
    }
}

fn partition_point(len: usize, predicate: impl Fn(usize) -> bool) -> usize {
    let (mut low, mut high) = (0, len);
    while low < high {
        let middle = low + (high - low) / 2;
        if predicate(middle) {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    low
}

/// `C` 之后的输入 `code` 拆出的字：先列整串精确命中的字，再列前缀补全（要求已打完前两个部件），同一级按名次。双拼方案另把输入转成全拼再查一次，结果合并；微软双拼的 `;`（ing）只在这次转换里有意义。`code` 里有大写字母时不查表（`CPU`、`CEO` 照原样上屏）。`traditional_output` 为真时宿主会把行转成繁体，这里按转换后的字形去重，保留先出现的一行，免得一屏出现两个「議」。
pub fn query_chaizi(
    code: &str,
    scheme: SchemeType,
    profile: &ShuangpinProfile,
    traditional_output: bool,
) -> Vec<WordItem> {
    let shuangpin = scheme == SchemeType::Shuangpin;
    if code.is_empty()
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'\'' || (shuangpin && byte == b';'))
    {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let query = Query::new(code);
    collect(&query, &mut hits);
    if shuangpin {
        let quanpin = normalize_input(code, profile);
        if !quanpin.is_empty() && quanpin.as_bytes() != query.letters.as_slice() {
            collect(&Query::new(&quanpin), &mut hits);
        }
    }
    hits.sort_by_key(|hit| (hit.missing, hit.rank));
    let mut rows: Vec<Hit> = Vec::new();
    for hit in hits {
        if rows.len() == RESULT_LIMIT {
            break;
        }
        let shown = |hit: &Hit| {
            if traditional_output && !hit.traditional.is_empty() {
                hit.traditional
            } else {
                hit.character
            }
        };
        let duplicate = rows.iter().any(|row| {
            row.character == hit.character || (traditional_output && shown(row) == shown(&hit))
        });
        if !duplicate {
            if rows.is_empty() {
                rows.reserve_exact(RESULT_LIMIT);
            }
            rows.push(hit);
        }
    }
    let count = rows.len();
    rows.into_iter()
        .enumerate()
        .map(|(index, hit)| {
            WordItem::new(
                hit.key,
                hit.character,
                (count - index) as i64,
                CandidateSource::Generated,
                "",
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shuangpin::profile::default_profile;

    fn quanpin(code: &str) -> Vec<String> {
        query_chaizi(code, SchemeType::Quanpin, xiaohe(), false)
            .into_iter()
            .map(|item| item.word.into_string())
            .collect()
    }

    fn xiaohe() -> &'static ShuangpinProfile {
        default_profile()
    }

    #[test]
    fn empty_chaizi_result_does_not_reserve_result_buffer() {
        let profile = xiaohe();
        assert!(!query_chaizi("huohuohuo", SchemeType::Quanpin, profile, false).is_empty());

        let (rows, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            query_chaizi("zzzzzzzz", SchemeType::Quanpin, profile, false)
        });

        assert!(rows.is_empty());
        assert_eq!(
            allocations, 1,
            "无命中拆字查询只应为去分隔符后的输入分配一次：{allocations}"
        );
    }

    #[test]
    fn samples_from_the_spec_come_first() {
        assert_eq!(quanpin("huohuohuo").first().map(String::as_str), Some("焱"));
        assert_eq!(quanpin("nvzi").first().map(String::as_str), Some("好"));
        assert_eq!(quanpin("muzi").first().map(String::as_str), Some("李"));
        assert_eq!(quanpin("shouchi").first().map(String::as_str), Some("拆"));
    }

    #[test]
    fn readings_annotate_the_characters() {
        assert_eq!(reading("焱"), "yàn");
        assert_eq!(reading("好"), "hǎo");
        // 不在表里的字和非汉字没有读音。
        assert_eq!(reading("a"), "");
        assert_eq!(reading(""), "");
    }

    #[test]
    fn separators_must_fall_between_components() {
        assert_eq!(quanpin("huo'huo'huo"), quanpin("huohuohuo"));
        assert_eq!(quanpin("nv'zi").first().map(String::as_str), Some("好"));
        // `'` 切在部件中间时什么也不出。
        assert!(quanpin("hu'ohuohuo").is_empty());
        assert!(quanpin("n'vzi").is_empty());
        // 末尾的 `'` 落在最后一个部件之后，仍是精确命中。
        assert_eq!(
            quanpin("huohuohuo'").first().map(String::as_str),
            Some("焱")
        );
    }

    #[test]
    fn a_leading_separator_is_ignored() {
        // 删掉 `C'huo` 之间的字母会留下打头的 `'`，它前面没有部件，不该让整串查不到。
        assert_eq!(quanpin("'huohuohuo"), quanpin("huohuohuo"));
        assert_eq!(quanpin("'huo'huo'huo"), quanpin("huohuohuo"));
        assert!(quanpin("'").is_empty());
    }

    #[test]
    fn u_umlaut_is_spelled_v_and_nue_lue_both_work() {
        assert!(quanpin("nvzi").contains(&"好".to_owned()));
        // nüe 写作 nve 或 nue 都能查到同一个字（瘧 = 疒 虐）。
        let nve = quanpin("bingnve");
        let nue = quanpin("bingnue");
        assert!(nve.contains(&"瘧".to_owned()), "{nve:?}");
        assert_eq!(nve, nue);
        assert!(quanpin("yannve").contains(&"谑".to_owned()));
    }

    #[test]
    fn radical_aliases_find_the_everyday_names() {
        // 灬 在 Unihan 里读 biāo，日常叫火字底；扌 叫提手旁。
        assert_eq!(quanpin("nenghuo").first().map(String::as_str), Some("熊"));
        assert!(quanpin("rizhaohuo").contains(&"照".to_owned()));
        assert!(quanpin("tichi").contains(&"拆".to_owned()));
        assert!(quanpin("sangong").contains(&"江".to_owned()));
    }

    #[test]
    fn prefix_completion_needs_two_components() {
        // 只打了一个部件：以它开头的键很多，不补全。
        assert!(quanpin("hen").is_empty());
        assert!(quanpin("hang").is_empty());
        assert!(quanpin("pan").is_empty());
        // 打完两个部件后补全第三个部件（焱 = 火 火 火）。
        let rows = quanpin("huohuo");
        assert!(rows.contains(&"焱".to_owned()), "{rows:?}");
        // 精确命中（炎 = 火 火）排在补全之前。
        let exact = rows.iter().position(|word| word == "炎").expect("炎");
        let completion = rows.iter().position(|word| word == "焱").expect("焱");
        assert!(exact < completion, "{rows:?}");
    }

    #[test]
    fn rows_are_ranked_by_level_then_bounded() {
        let rows = query_chaizi("yanyi", SchemeType::Quanpin, xiaohe(), false);
        // 议 是《通用规范汉字表》一级字，排在繁体的 議 之前。
        let words: Vec<_> = rows.iter().map(|item| item.word.as_str()).collect();
        let simplified = words.iter().position(|word| *word == "议").expect("议");
        let traditional = words.iter().position(|word| *word == "議").expect("議");
        assert!(simplified < traditional, "{words:?}");
        assert!(rows.len() <= RESULT_LIMIT);
        // 行的权重递减，来源是 Generated，`pinyin` 记着命中的键。
        assert!(rows.windows(2).all(|pair| pair[0].weight > pair[1].weight));
        assert!(rows
            .iter()
            .all(|item| item.source == CandidateSource::Generated));
        assert_eq!(rows[0].pinyin, "yan'yi");
        // 很短的前缀命中大量的键，结果仍截在上限内。
        assert!(quanpin("kouk").len() <= RESULT_LIMIT);
    }

    #[test]
    fn no_match_and_uppercase_give_nothing() {
        assert!(quanpin("").is_empty());
        assert!(quanpin("xyzzy").is_empty());
        assert!(quanpin("PU").is_empty());
        assert!(quanpin("hUohuohuo").is_empty());
    }

    #[test]
    fn traditional_output_drops_rows_that_convert_alike() {
        let plain = query_chaizi("yanyi", SchemeType::Quanpin, xiaohe(), false);
        let traditional = query_chaizi("yanyi", SchemeType::Quanpin, xiaohe(), true);
        let words = |rows: &[WordItem]| {
            rows.iter()
                .map(|item| item.word.to_string())
                .collect::<Vec<_>>()
        };
        // 议 转成繁体就是 議：繁体输出时只留先出现的 议。
        assert!(words(&plain).contains(&"議".to_owned()));
        assert!(words(&traditional).contains(&"议".to_owned()));
        assert!(!words(&traditional).contains(&"議".to_owned()));
        assert!(traditional.len() < plain.len());
    }

    #[test]
    fn shuangpin_also_queries_the_quanpin_reading() {
        // 小鹤双拼：huo = hu+o → `ho`；焱 = `hohoho`。
        let rows = query_chaizi("hohoho", SchemeType::Shuangpin, xiaohe(), false);
        assert_eq!(rows.first().map(|item| item.word.as_str()), Some("焱"));
        // 全拼方案不做这次转换。
        assert!(query_chaizi("hohoho", SchemeType::Quanpin, xiaohe(), false).is_empty());
        // 微软双拼的 `;` 是 ing：清 = 氵 san（`sj`）+ 青 qing（`q;`）。全拼方案不收 `;`。
        let microsoft =
            crate::shuangpin::profile::profile(crate::types::ShuangpinProfileKind::Microsoft)
                .expect("微软双拼是内置方案");
        let rows = query_chaizi("sjq;", SchemeType::Shuangpin, microsoft, false);
        assert_eq!(rows.first().map(|item| item.word.as_str()), Some("清"));
        assert!(query_chaizi("sjq;", SchemeType::Quanpin, microsoft, false).is_empty());
    }

    #[test]
    fn tables_are_well_formed() {
        let table = keys();
        assert!(table.len() > 10_000);
        for index in 1..table.len() {
            let previous: Vec<u8> = table
                .field(index - 1)
                .bytes()
                .filter(|&b| b != b'\'')
                .collect();
            let current: Vec<u8> = table.field(index).bytes().filter(|&b| b != b'\'').collect();
            assert!(
                previous <= current,
                "{} > {}",
                table.field(index - 1),
                table.field(index)
            );
            assert!(Boundaries::of(table.field(index)).is_some());
        }
        let characters = chars();
        for index in 1..characters.len() {
            assert!(characters.field(index - 1) < characters.field(index));
        }
    }
}
