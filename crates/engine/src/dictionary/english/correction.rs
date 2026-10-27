//! 移动端英文整词纠错与连写分词；只读索引查询，缓存和搜索长度都有上限。

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::BuildHasher;
use std::sync::Arc;

use super::EnglishDictionary;

#[derive(Clone)]
struct Lexeme {
    key: String,
    display: String,
    weight: i64,
}

const MAX_INPUT: usize = 32;
const MAX_EDITS: usize = 2;
const MAX_WORDS: usize = 6;
const PREFIX_ROWS: usize = 128;
const CACHE_PAGES: usize = 128;
/// 整词查询缓存的条目上限（命中与未命中合计），满了整体清空。
const CACHE_WORDS: usize = 4096;
/// 固定词形没有词频，按常用词计。
const CANONICAL_WEIGHT: i64 = 1_000_000_000;
/// 每多一个词的代价。约等于词库总频次的对数，使分词近似一元语言模型：多切一刀必须换来足够常见的词。
const WORD_COST: f64 = 30.0;
/// 邻键、元音替换与相邻对调的代价。
const SUBSTITUTE_COST: f64 = 8.0;
/// 少按一键的代价。
const MISSING_COST: f64 = 8.0;
/// 多按一键的代价更高：连写时多出的字母常属于相邻的词，`haveaniceday` 不能把 `a` 当成 `nice` 的多余键。
const EXTRA_COST: f64 = 12.0;
/// 分词中的每个词至少要有的词频；固定词形不受限。
const SPLIT_MIN_WEIGHT: i64 = 10_000_000;
/// 不超过三个字母的分词片段更容易是碎片（res、er、ate），门槛更高。
const SHORT_SPLIT_MIN_WEIGHT: i64 = 25_000_000;
/// 纠到三字母词时目标必须很常见（the、you、and），避免把缩写和生词改成罕见短词。
const SHORT_TARGET_MIN_WEIGHT: i64 = 100_000_000;
/// 元音互换只用于至少五个字母的目标词（seperate、pleese），短词的元音变化通常是另一个词。
const VOWEL_SWAP_MIN_LENGTH: usize = 5;
/// 至少七个字母的目标词允许两次编辑（tommorow）。
const TWO_EDIT_MIN_LENGTH: usize = 7;

#[derive(Default)]
pub(super) struct CorrectionCache {
    pages: HashMap<String, Arc<Vec<Lexeme>>>,
    order: VecDeque<String>,
    /// 命中的整词以词库键为键，只保存固定词库的条目。
    words: HashMap<String, Lexeme>,
    /// 未命中的片段只保存哈希，不留下用户按键的原文。
    misses: HashSet<u64>,
    hasher: RandomState,
}

#[derive(Clone, Default)]
struct Path {
    text: String,
    edits: usize,
    words: usize,
    cost: f64,
}

impl Path {
    fn better_than(&self, other: &Self) -> bool {
        self.cost < other.cost
            || (self.cost == other.cost
                && ((self.edits, self.words) < (other.edits, other.words)
                    || ((self.edits, self.words) == (other.edits, other.words)
                        && self.text < other.text)))
    }
}

/// 动态规划的一步：从当前位置消耗 `consumed` 字节得到 `display`。
struct Choice {
    consumed: usize,
    display: String,
    edits: usize,
    edit_cost: f64,
    weight: i64,
    canonical: bool,
}

impl EnglishDictionary {
    /// 返回与原文不同的唯一建议；未知词、标识符、超长输入和缺失词库都允许原样输入。
    pub(crate) fn correct_mobile_english(&self, raw: &str) -> Option<String> {
        if raw.is_empty()
            || raw.len() > MAX_INPUT
            || !raw
                .bytes()
                .all(|byte| byte.is_ascii_alphabetic() || byte == b'\'')
        {
            return None;
        }
        // 显式大小写也优先于固定词形恢复，避免把 IM、ENGLISH 等自动改写。
        if raw.bytes().skip(1).any(|byte| byte.is_ascii_uppercase()) {
            return None;
        }
        let input = raw.to_ascii_lowercase();
        if protected(&input) {
            return None;
        }
        let capitalize = |mut text: String| {
            if raw.as_bytes()[0].is_ascii_uppercase() {
                text[..1].make_ascii_uppercase();
            }
            (text != raw).then_some(text)
        };
        if let Some(display) = canonical(&input.replace('\'', "")) {
            if !input.contains('\'') || display.eq_ignore_ascii_case(&input) {
                return capitalize(display.to_owned());
            }
        }
        if !self.ready() || input.contains('\'') {
            return None;
        }
        if let Some(item) = self.exact_correction_word(&input) {
            return capitalize(item.display);
        }
        let mut paths =
            vec![vec![None::<Path>; (MAX_EDITS + 1) * (MAX_WORDS + 1)]; input.len() + 1];
        paths[0][0] = Some(Path::default());
        for start in 0..input.len() {
            if paths[start].iter().all(Option::is_none) {
                continue;
            }
            let remaining = &input[start..];
            // 整串只有一个词时不是分词，罕见词与专名照常可以作为整词纠错结果。
            let eligible = |choice: &Choice| {
                (start == 0 && choice.consumed == input.len()) || split_piece(choice)
            };
            let mut choices = Vec::new();
            for end in 1..=remaining.len() {
                let piece = &remaining[..end];
                if let Some(display) = canonical(piece) {
                    choices.push(Choice {
                        consumed: end,
                        display: display.to_owned(),
                        edits: 0,
                        edit_cost: 0.0,
                        weight: CANONICAL_WEIGHT,
                        canonical: true,
                    });
                } else if let Some(item) = self.exact_correction_word(piece) {
                    choices.push(Choice {
                        consumed: end,
                        display: item.display,
                        edits: 0,
                        edit_cost: 0.0,
                        weight: item.weight,
                        canonical: false,
                    });
                }
            }
            if remaining.len() >= 3
                && paths[start]
                    .iter()
                    .flatten()
                    .any(|path| path.edits < MAX_EDITS && path.words < MAX_WORDS)
            {
                let mut matches = self.correction_matches(remaining, &standard_prefixes(remaining));
                matches.retain(eligible);
                if matches.is_empty() {
                    matches = self
                        .correction_matches(remaining, &alternate_prefixes(remaining, start == 0));
                    matches.retain(eligible);
                }
                choices.extend(matches);
            }
            choices.retain(eligible);
            for path in paths[start]
                .clone()
                .into_iter()
                .flatten()
                .filter(|path| path.words < MAX_WORDS)
            {
                for choice in &choices {
                    let next = Path {
                        text: if path.text.is_empty() {
                            choice.display.clone()
                        } else {
                            format!("{} {}", path.text, choice.display)
                        },
                        edits: path.edits + choice.edits,
                        words: path.words + 1,
                        cost: path.cost + WORD_COST
                            - (choice.weight.max(0) as f64 + 1.0).ln().min(22.0)
                            + choice.edit_cost,
                    };
                    if next.edits > MAX_EDITS {
                        continue;
                    }
                    let slot = next.edits * (MAX_WORDS + 1) + next.words;
                    let destination = &mut paths[start + choice.consumed][slot];
                    if destination.as_ref().is_none_or(|old| next.better_than(old)) {
                        *destination = Some(next);
                    }
                }
            }
        }
        let best =
            paths.pop()?.into_iter().flatten().reduce(
                |a, b| {
                    if a.better_than(&b) {
                        a
                    } else {
                        b
                    }
                },
            )?;
        capitalize(best.text)
    }

    fn exact_correction_word(&self, word: &str) -> Option<Lexeme> {
        if word.len() < 2 {
            return None;
        }
        let connection = self.connection.as_ref()?;
        let hash = {
            let cache = self.correction_cache.borrow();
            if let Some(item) = cache.words.get(word) {
                return Some(item.clone());
            }
            let hash = cache.hasher.hash_one(word);
            if cache.misses.contains(&hash) {
                return None;
            }
            hash
        };
        let found = connection
            .prepare_cached(
                "SELECT display,weight FROM english_words WHERE word=?1 ORDER BY weight DESC,display LIMIT 1",
            )
            .ok()
            .and_then(|mut statement| {
                statement
                    .query_row([word], |row| {
                        Ok(Lexeme {
                            key: word.to_owned(),
                            display: row.get(0)?,
                            weight: row.get(1)?,
                        })
                    })
                    .ok()
            })
            .filter(|item| valid_display(&item.display));
        let mut cache = self.correction_cache.borrow_mut();
        if cache.words.len() + cache.misses.len() >= CACHE_WORDS {
            cache.words.clear();
            cache.misses.clear();
        }
        match &found {
            Some(item) => {
                cache.words.insert(word.to_owned(), item.clone());
            }
            None => {
                cache.misses.insert(hash);
            }
        }
        found
    }

    fn correction_page(&self, prefix: &str) -> Arc<Vec<Lexeme>> {
        if let Some(page) = self.correction_cache.borrow().pages.get(prefix) {
            return Arc::clone(page);
        }
        let page = Arc::new(
            self.query_prefix(prefix, PREFIX_ROWS)
                .into_iter()
                .map(|item| Lexeme {
                    key: item.pinyin.into_string(),
                    display: item.word.into_string(),
                    weight: item.weight,
                })
                .collect::<Vec<_>>(),
        );
        let mut cache = self.correction_cache.borrow_mut();
        if cache.pages.len() == CACHE_PAGES {
            if let Some(key) = cache.order.pop_front() {
                cache.pages.remove(&key);
            }
        }
        cache.order.push_back(prefix.to_owned());
        cache.pages.insert(prefix.to_owned(), Arc::clone(&page));
        page
    }

    fn correction_matches(&self, remaining: &str, prefixes: &[String]) -> Vec<Choice> {
        let mut matches = Vec::new();
        for prefix in prefixes {
            for item in self.correction_page(prefix).iter() {
                if !correction_target(item) {
                    continue;
                }
                for (consumed, edits, edit_cost) in
                    edit_matches(remaining.as_bytes(), item.key.as_bytes())
                {
                    let display = canonical(&item.key).unwrap_or(&item.display).to_owned();
                    matches.push(Choice {
                        consumed,
                        display,
                        edits,
                        edit_cost,
                        weight: item.weight,
                        canonical: false,
                    });
                }
            }
        }
        matches
    }
}

fn valid_display(display: &str) -> bool {
    !display.is_empty()
        && display
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || byte == b'\'')
}

/// 编辑纠错的目标：至少三个字母；三字母目标必须是很常见的小写词（排除 Sep、May 这类缩写和专名）；首字母之后有大写的缩写（TNT、ASAP、iPhone）从不作为编辑目标。
fn correction_target(item: &Lexeme) -> bool {
    item.key.len() >= 3
        && (item.key.len() > 3
            || (item.weight >= SHORT_TARGET_MIN_WEIGHT
                && !item.display.bytes().any(|byte| byte.is_ascii_uppercase())))
        && valid_display(&item.display)
        && !item
            .display
            .bytes()
            .skip(1)
            .any(|byte| byte.is_ascii_uppercase())
}

/// 多词分词中的片段：固定词形照常可用；其余必须是常见的小写词，或至少四个字母、只有首字母大写的专名（Monday、London），排除 Sep、ESE、TNT 这类缩写和 res、er 这类碎片。
fn split_piece(choice: &Choice) -> bool {
    if choice.canonical {
        return true;
    }
    let bytes = choice.display.as_bytes();
    let lowercase = bytes.iter().all(|byte| !byte.is_ascii_uppercase());
    let proper = bytes.len() >= 4 && !bytes[1..].iter().any(u8::is_ascii_uppercase);
    let minimum = if choice.display.len() <= 3 {
        SHORT_SPLIT_MIN_WEIGHT
    } else {
        SPLIT_MIN_WEIGHT
    };
    (lowercase || proper) && choice.weight >= minimum
}

fn protected(word: &str) -> bool {
    matches!(
        word,
        "omg"
            | "thx"
            | "lol"
            | "lmao"
            | "rofl"
            | "brb"
            | "btw"
            | "idk"
            | "imo"
            | "imho"
            | "tbh"
            | "wfh"
            | "fyi"
            | "afk"
            | "gg"
            | "ok"
            | "okay"
            | "pls"
            | "plz"
            | "cya"
            | "ty"
            | "np"
            | "u"
            | "ur"
            | "ttyl"
            | "ikr"
            | "smh"
            | "nvm"
            | "dm"
            | "rn"
            | "xd"
            | "asap"
            | "irl"
            | "tmr"
            | "tmrw"
            | "bc"
            | "cuz"
            | "ppl"
            | "msg"
            | "pic"
            | "pics"
            | "gf"
            | "bf"
            | "bff"
            | "jk"
            | "ya"
            | "yea"
            | "yeah"
            | "nope"
            | "yep"
            | "lmk"
            | "idc"
            | "ngl"
            | "omw"
            | "sry"
    )
}

/// 固定词形。只收去掉撇号后本身不是常用英文词的缩约形式：well、shell、hell、ill、id、its、lets、were 等都是词，不在这里。
fn canonical(word: &str) -> Option<&'static str> {
    Some(match word {
        "a" => "a",
        "i" => "I",
        "im" => "I'm",
        "ive" => "I've",
        "youre" => "you're",
        "youve" => "you've",
        "youll" => "you'll",
        "youd" => "you'd",
        "hes" => "he's",
        "shes" => "she's",
        "theyre" => "they're",
        "theyve" => "they've",
        "theyll" => "they'll",
        "theyd" => "they'd",
        "weve" => "we've",
        "itll" => "it'll",
        "thats" => "that's",
        "thatll" => "that'll",
        "whats" => "what's",
        "whatll" => "what'll",
        "theres" => "there's",
        "therell" => "there'll",
        "heres" => "here's",
        "wheres" => "where's",
        "whered" => "where'd",
        "whos" => "who's",
        "wholl" => "who'll",
        "hows" => "how's",
        "howd" => "how'd",
        "arent" => "aren't",
        "isnt" => "isn't",
        "wasnt" => "wasn't",
        "werent" => "weren't",
        "dont" => "don't",
        "doesnt" => "doesn't",
        "didnt" => "didn't",
        "havent" => "haven't",
        "hasnt" => "hasn't",
        "hadnt" => "hadn't",
        "wont" => "won't",
        "cant" => "can't",
        "couldnt" => "couldn't",
        "shouldnt" => "shouldn't",
        "wouldnt" => "wouldn't",
        "mustnt" => "mustn't",
        "neednt" => "needn't",
        "wouldve" => "would've",
        "couldve" => "could've",
        "shouldve" => "should've",
        "chinese" => "Chinese",
        "english" => "English",
        _ => return None,
    })
}

fn neighbors(a: u8, b: u8) -> bool {
    fn position(byte: u8) -> Option<(i32, i32)> {
        for (row, (letters, offset)) in [
            (b"qwertyuiop".as_slice(), 0),
            (b"asdfghjkl".as_slice(), 1),
            (b"zxcvbnm".as_slice(), 2),
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(column) = letters.iter().position(|&key| key == byte) {
                return Some((column as i32 * 4 + offset, row as i32));
            }
        }
        None
    }
    match (position(a), position(b)) {
        (Some((ax, ay)), Some((bx, by))) => (ax - bx).abs() <= 4 && (ay - by).abs() <= 1,
        _ => false,
    }
}

fn vowel(byte: u8) -> bool {
    matches!(byte, b'a' | b'e' | b'i' | b'o' | b'u')
}

/// `input` 的各个前缀与 `word` 的受限编辑距离：邻键替换（目标够长时也允许元音互换）、多键、少键与相邻对调各算一次，短词最多一次、长词最多两次。返回 `(消耗字节, 编辑次数, 编辑代价)`，只含至少三字节且确有编辑的前缀。
fn edit_matches(input: &[u8], word: &[u8]) -> Vec<(usize, usize, f64)> {
    let limit = if word.len() >= TWO_EDIT_MIN_LENGTH {
        2
    } else {
        1
    };
    let rows = input.len().min(word.len() + limit);
    if rows + limit < word.len() {
        return Vec::new();
    }
    let substitutable = |a: u8, b: u8| {
        neighbors(a, b) || (word.len() >= VOWEL_SWAP_MIN_LENGTH && vowel(a) && vowel(b))
    };
    // 每格保存最小代价及其编辑次数；超过上限的格子为 None。
    let columns = word.len() + 1;
    let mut table = vec![None::<(f64, usize)>; (rows + 1) * columns];
    let index = |i: usize, j: usize| i * columns + j;
    table[0] = Some((0.0, 0));
    // 每次编辑最多让对角线偏移一格，只需计算宽度为 2×上限+1 的斜带。
    for i in 0..=rows {
        let mut reachable = i == 0;
        for j in i.saturating_sub(limit)..=(i + limit).min(word.len()) {
            if i == 0 && j == 0 {
                continue;
            }
            let mut best = None::<(f64, usize)>;
            let mut consider = |from: Option<(f64, usize)>, cost: f64, edits: usize| {
                if let Some((base, count)) = from {
                    let next = (base + cost, count + edits);
                    if next.1 <= limit && best.is_none_or(|old| next.0 < old.0) {
                        best = Some(next);
                    }
                }
            };
            if i > 0 && j > 0 {
                let (a, b) = (input[i - 1], word[j - 1]);
                if a == b {
                    consider(table[index(i - 1, j - 1)], 0.0, 0);
                } else if substitutable(a, b) {
                    consider(table[index(i - 1, j - 1)], SUBSTITUTE_COST, 1);
                }
                if i > 1 && j > 1 && a == word[j - 2] && input[i - 2] == b && a != b {
                    consider(table[index(i - 2, j - 2)], SUBSTITUTE_COST, 1);
                }
            }
            if i > 0 {
                consider(table[index(i - 1, j)], EXTRA_COST, 1);
            }
            if j > 0 {
                consider(table[index(i, j - 1)], MISSING_COST, 1);
            }
            table[index(i, j)] = best;
            reachable |= best.is_some();
        }
        if !reachable {
            break;
        }
    }
    (3..=rows)
        .filter_map(|consumed| {
            table[index(consumed, word.len())]
                .filter(|&(_, edits)| edits > 0)
                .map(|(cost, edits)| (consumed, edits, cost))
        })
        .collect()
}

/// 每个位置都查的前缀页：前两键原样、第二键漏按或与第三键对调、前两键对调、第二键按到邻键。
fn standard_prefixes(input: &str) -> Vec<String> {
    let bytes = input.as_bytes();
    let mut prefixes = vec![
        input[..2].to_owned(),
        String::from_utf8(vec![bytes[0], bytes[2]]).expect("ASCII"),
        String::from_utf8(vec![bytes[1], bytes[0]]).expect("ASCII"),
    ];
    for key in b'a'..=b'z' {
        if key != bytes[1] && neighbors(key, bytes[1]) {
            prefixes.push(String::from_utf8(vec![bytes[0], key]).expect("ASCII"));
        }
    }
    prefixes.sort_unstable();
    prefixes.dedup();
    prefixes
}

/// 标准前缀都没有匹配时再查：首键按到邻键、多按了首键；在整串开头还查首键或第二键漏按，这两种各要 26 页，放到连写中间的每个起点会让一次查询读上百页，而词中间的漏键在连写里也难与前一个词的末尾区分。
fn alternate_prefixes(input: &str, first_word: bool) -> Vec<String> {
    let bytes = input.as_bytes();
    let standard = standard_prefixes(input);
    let mut prefixes = Vec::new();
    for key in b'a'..=b'z' {
        if first_word {
            for prefix in [[key, bytes[0]], [bytes[0], key]] {
                prefixes.push(String::from_utf8(prefix.to_vec()).expect("ASCII"));
            }
        }
        if neighbors(key, bytes[0]) {
            prefixes.push(String::from_utf8(vec![key, bytes[1]]).expect("ASCII"));
        }
    }
    prefixes.push(input[1..3].to_owned());
    prefixes.sort_unstable();
    prefixes.dedup();
    prefixes.retain(|prefix| !standard.contains(prefix));
    prefixes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::english::ensure_english_schema;
    use rusqlite::Connection;

    fn dictionary() -> (tempfile::TempDir, EnglishDictionary) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("english.db");
        ensure_english_schema(&path).unwrap();
        let connection = Connection::open(&path).unwrap();
        for word in [
            "what", "is", "that", "language", "speak", "hello", "world", "english", "chinese",
        ] {
            connection
                .execute(
                    "INSERT INTO english_words VALUES (?1,?1,5000000000)",
                    [word],
                )
                .unwrap();
        }
        let dictionary = EnglishDictionary::open(&path, None, None);
        (root, dictionary)
    }

    /// 按真实词库的量级给出词频，`(键, 展示, 词频)`。
    fn weighted(rows: &[(&str, &str, i64)]) -> (tempfile::TempDir, EnglishDictionary) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("english.db");
        ensure_english_schema(&path).unwrap();
        let connection = Connection::open(&path).unwrap();
        for (word, display, weight) in rows {
            connection
                .execute(
                    "INSERT INTO english_words VALUES (?1,?2,?3)",
                    rusqlite::params![word, display, weight],
                )
                .unwrap();
        }
        let dictionary = EnglishDictionary::open(&path, None, None);
        (root, dictionary)
    }

    const RULE_ROWS: &[(&str, &str, i64)] = &[
        ("the", "the", 23_135_851_162),
        ("and", "and", 12_997_637_966),
        ("to", "to", 12_136_980_858),
        ("you", "you", 2_996_181_025),
        ("are", "are", 2_393_614_870),
        ("how", "how", 571_848_080),
        ("love", "love", 201_063_526),
        ("go", "go", 421_086_358),
        ("go", "Go", 421_086_357),
        ("id", "id", 119_974_321),
        ("id", "ID", 119_974_320),
        ("good", "good", 365_796_396),
        ("hello", "hello", 32_960_381),
        ("hell", "hell", 22_791_884),
        ("address", "address", 261_872_866),
        ("dress", "dress", 27_318_959),
        ("separate", "separate", 36_138_447),
        ("sep", "Sep", 107_145_206),
        ("er", "er", 17_212_593),
        ("ate", "ate", 5_929_336),
        ("res", "res", 19_797_705),
        ("rate", "rate", 207_634_179),
        ("please", "please", 380_046_348),
        ("plea", "plea", 3_445_286),
        ("ese", "ESE", 1_353_074),
        ("tnt", "TNT", 2_604_555),
        ("tomorrow", "tomorrow", 20_976_724),
        ("tom", "tom", 47_775_300),
        ("memo", "memo", 6_710_272),
        ("row", "row", 29_649_598),
        ("morrow", "morrow", 2_789_797),
        ("tech", "tech", 93_401_669),
        ("yogi", "yogi", 829_863),
        ("abcd", "abcd", 643_124),
        ("yugo", "Yugo", 525_806),
        ("weird", "weird", 11_556_427),
        ("wired", "wired", 8_371_548),
        ("bad", "bad", 95_000_000),
        ("zap", "zap", 1_000_000),
        ("plus", "plus", 94_730_251),
        ("cyan", "cyan", 2_111_144),
        ("asap", "ASAP", 3_488_320),
        ("idc", "IDC", 2_290_381),
        ("yeah", "yeah", 25_858_426),
        ("well", "well", 362_082_755),
        ("its", "its", 525_627_757),
        ("lets", "lets", 15_876_825),
        ("were", "were", 570_699_558),
        ("ill", "ill", 16_143_967),
        ("shell", "shell", 20_599_318),
        ("wont", "wont", 3_978_693),
        ("hows", "hows", 540_560),
        ("whose", "whose", 43_634_263),
        ("that", "that", 3_400_031_103),
        ("what", "what", 812_395_582),
        ("haven", "haven", 10_569_212),
        ("your", "your", 2_062_066_547),
        ("have", "have", 1_564_202_750),
        ("nice", "nice", 69_395_609),
        ("ice", "ice", 41_706_145),
        ("day", "day", 446_236_148),
        ("monday", "Monday", 79_697_206),
        ("dont", "don't", 100_000_000),
    ];

    #[test]
    fn mobile_english_restores_unambiguous_contractions() {
        let (_root, dictionary) = weighted(RULE_ROWS);
        for (input, expected) in [
            ("thats", "that's"),
            ("whats", "what's"),
            ("theres", "there's"),
            ("heres", "here's"),
            ("wheres", "where's"),
            ("whos", "who's"),
            ("hows", "how's"),
            ("havent", "haven't"),
            ("hasnt", "hasn't"),
            ("hadnt", "hadn't"),
            ("wont", "won't"),
            ("itll", "it'll"),
            ("youd", "you'd"),
            ("theyd", "they'd"),
            ("theyll", "they'll"),
            ("mustnt", "mustn't"),
            ("neednt", "needn't"),
            ("wouldve", "would've"),
            ("couldve", "could've"),
            ("shouldve", "should've"),
            ("therell", "there'll"),
            ("thatll", "that'll"),
            ("whatll", "what'll"),
            ("wholl", "who'll"),
            ("whered", "where'd"),
            ("howd", "how'd"),
            ("Thats", "That's"),
            ("Wont", "Won't"),
            ("what's", "what's"),
        ] {
            assert_eq!(
                dictionary.correct_mobile_english(input).as_deref(),
                (input != expected).then_some(expected),
                "{input}"
            );
        }
        // 去掉撇号后本身就是常用词的形式不能自动改写；显式大小写同样只保留原文。
        for input in [
            "well", "its", "lets", "were", "ill", "id", "shell", "hell", "THATS", "WONT", "tHats",
        ] {
            assert_eq!(dictionary.correct_mobile_english(input), None, "{input}");
        }
    }

    #[test]
    fn mobile_english_leaves_chat_abbreviations() {
        let (_root, dictionary) = weighted(RULE_ROWS);
        for input in [
            "pls", "plz", "cya", "ty", "np", "u", "ur", "thx", "ttyl", "ikr", "smh", "nvm", "dm",
            "rn", "xd", "asap", "irl", "tmr", "tmrw", "bc", "cuz", "ppl", "msg", "pic", "pics",
            "gf", "bf", "bff", "jk", "ya", "yea", "yeah", "nope", "yep", "ok", "okay", "idc",
            "Pls", "ASAP",
        ] {
            assert_eq!(dictionary.correct_mobile_english(input), None, "{input}");
        }
    }

    #[test]
    fn mobile_english_splits_avoid_acronyms_and_rare_pieces() {
        let (_root, dictionary) = weighted(RULE_ROWS);
        for (input, expected) in [
            ("seperate", Some("separate")),
            ("adress", Some("address")),
            ("helli", Some("hello")),
            ("goid", Some("good")),
            ("pleese", Some("please")),
            ("tommorow", Some("tomorrow")),
            ("iloveyuo", Some("I love you")),
            ("howaeryou", Some("how are you")),
            ("haveaniceday", Some("have a nice day")),
            ("mondaygood", Some("Monday good")),
            // 只能切成全大写缩写、首字母大写的短缩写或罕见碎片时保留原文。
            ("tntese", None),
            ("sepid", None),
            ("reser", None),
            ("memorow", None),
        ] {
            assert_eq!(
                dictionary.correct_mobile_english(input).as_deref(),
                expected,
                "{input}"
            );
        }
        // 单个字母只有 a 与 I 能成为分词片段。
        for input in ["goodx", "xgood", "hellob"] {
            let correction = dictionary.correct_mobile_english(input);
            assert!(
                correction.as_deref().is_none_or(|text| text
                    .split(' ')
                    .all(|word| word.len() > 1 || matches!(word, "a" | "I"))),
                "{input}: {correction:?}"
            );
        }
    }

    #[test]
    fn mobile_english_reaches_three_letter_targets() {
        let (_root, dictionary) = weighted(RULE_ROWS);
        for (input, expected) in [
            ("teh", "the"),
            ("tge", "the"),
            ("yoi", "you"),
            ("abd", "and"),
            ("Teh", "The"),
            ("wierd", "weird"),
        ] {
            assert_eq!(
                dictionary.correct_mobile_english(input).as_deref(),
                Some(expected),
                "{input}"
            );
        }
        // 三字母精确词保持原样；罕见的三字母词不作为纠错目标。
        for input in ["the", "you", "and", "bad", "zao"] {
            assert_eq!(dictionary.correct_mobile_english(input), None, "{input}");
        }
    }

    #[test]
    fn mobile_english_word_cache_keeps_answers_and_bound() {
        let inputs = [
            "teh",
            "seperate",
            "helli",
            "iloveyuo",
            "howaeryou",
            "tommorow",
            "wierd",
            "abd",
            "haveaniceday",
            "tntese",
            "thats",
            "pls",
        ];
        let (_root, cold) = weighted(RULE_ROWS);
        let expected = inputs
            .iter()
            .map(|input| {
                let (_root, fresh) = weighted(RULE_ROWS);
                fresh.correct_mobile_english(input)
            })
            .collect::<Vec<_>>();
        // 逐键查询后缓存已热，答案必须与每次新开词库相同。
        for input in inputs {
            for end in 1..=input.len() {
                cold.correct_mobile_english(&input[..end]);
            }
        }
        let warm = inputs
            .iter()
            .map(|input| cold.correct_mobile_english(input))
            .collect::<Vec<_>>();
        assert_eq!(warm, expected);
        {
            let cache = cold.correction_cache.borrow();
            assert!(!cache.words.is_empty() && !cache.misses.is_empty());
            // 命中只以词库键保存，未命中不留下按键原文。
            assert!(cache
                .words
                .keys()
                .all(|key| RULE_ROWS.iter().any(|row| row.0 == key.as_str())));
        }
        // 填满后整体清空，清空后的答案仍相同。
        for index in 0..CACHE_WORDS * 2 {
            cold.exact_correction_word(&format!("zz{index}"));
            let cache = cold.correction_cache.borrow();
            assert!(cache.words.len() + cache.misses.len() <= CACHE_WORDS);
        }
        let evicted = inputs
            .iter()
            .map(|input| cold.correct_mobile_english(input))
            .collect::<Vec<_>>();
        assert_eq!(evicted, expected);
    }

    #[test]
    fn mobile_english_issue_examples() {
        let (_root, dictionary) = dictionary();
        for (input, expected) in [
            ("whatisthat", "what is that"),
            ("languaage", "language"),
            ("languge", "language"),
            ("langiage", "language"),
            ("i", "I"),
            ("i'm", "I'm"),
            ("chinese", "Chinese"),
            ("english", "English"),
            ("youre", "you're"),
            ("hes", "he's"),
            ("imchinese", "I'm Chinese"),
            ("speskebglish", "speak English"),
            ("Whatisthat", "What is that"),
        ] {
            assert_eq!(
                dictionary.correct_mobile_english(input).as_deref(),
                Some(expected),
                "{input}"
            );
        }
    }

    #[test]
    fn mobile_english_preserves_words_abbreviations_and_identifiers() {
        let (_root, dictionary) = dictionary();
        for input in [
            "language",
            "hello",
            "Hello",
            "omg",
            "OMG",
            "thx",
            "iPhone",
            "xqzv",
            "hello_world",
            "é",
            "abcdefghijklmnopqrstuvwxyzabcdefg",
        ] {
            assert_eq!(dictionary.correct_mobile_english(input), None, "{input}");
        }
    }

    #[test]
    fn mobile_english_preserves_explicit_case_before_canonical_spelling() {
        let (_root, dictionary) = dictionary();
        for input in [
            "IM", "I'M", "I'VE", "ENGLISH", "CHINESE", "eNglish", "yOuRe",
        ] {
            assert_eq!(dictionary.correct_mobile_english(input), None, "{input}");
        }
        for (input, expected) in [("Im", "I'm"), ("Youre", "You're"), ("English", "English")] {
            assert_eq!(
                dictionary.correct_mobile_english(input).as_deref(),
                (input != expected).then_some(expected),
                "{input}"
            );
        }
    }

    #[test]
    fn mobile_english_first_keys_and_transposition_can_be_corrected() {
        let (_root, dictionary) = dictionary();
        for input in ["kanguage", "alnguage", "anguage", "lnaguage", "languagge"] {
            assert_eq!(
                dictionary.correct_mobile_english(input).as_deref(),
                Some("language"),
                "{input}"
            );
        }
    }

    #[test]
    fn mobile_english_missing_dictionary_and_cache_are_bounded() {
        let dictionary = EnglishDictionary::open(std::path::Path::new(""), None, None);
        assert_eq!(dictionary.correct_mobile_english("languaage"), None);
        assert_eq!(dictionary.correct_mobile_english("i").as_deref(), Some("I"));
        for first in b'a'..=b'z' {
            for second in b'a'..=b'z' {
                dictionary.correction_page(std::str::from_utf8(&[first, second]).unwrap());
            }
        }
        let cache = dictionary.correction_cache.borrow();
        assert_eq!(cache.pages.len(), CACHE_PAGES);
        assert_eq!(cache.order.len(), CACHE_PAGES);
    }

    /// 真实词库的期望表；`None` 表示只保留原文。
    const SHIPPED_EXPECTATIONS: &[(&str, Option<&str>)] = &[
        ("thats", Some("that's")),
        ("whats", Some("what's")),
        ("havent", Some("haven't")),
        ("youd", Some("you'd")),
        ("theres", Some("there's")),
        ("heres", Some("here's")),
        ("itll", Some("it'll")),
        ("mustnt", Some("mustn't")),
        ("theyll", Some("they'll")),
        ("whos", Some("who's")),
        ("wont", Some("won't")),
        ("hasnt", Some("hasn't")),
        ("hadnt", Some("hadn't")),
        ("wheres", Some("where's")),
        ("hows", Some("how's")),
        ("seperate", Some("separate")),
        ("tommorow", Some("tomorrow")),
        ("pleese", Some("please")),
        ("adress", Some("address")),
        ("howaeryou", Some("how are you")),
        ("iloveyuo", Some("I love you")),
        ("helli", Some("hello")),
        ("goid", Some("good")),
        ("teh", Some("the")),
        ("tge", Some("the")),
        ("yoi", Some("you")),
        ("abd", Some("and")),
        ("wierd", Some("weird")),
        ("pls", None),
        ("cya", None),
        ("imchinese", Some("I'm Chinese")),
        ("speskebglish", Some("speak English")),
        ("whatisthat", Some("what is that")),
        ("languaage", Some("language")),
        ("languge", Some("language")),
        ("langiage", Some("language")),
        ("Imchinese", Some("I'm Chinese")),
        ("Speskebglish", Some("Speak English")),
        ("youre", Some("you're")),
        ("hes", Some("he's")),
        ("i", Some("I")),
        ("chinese", Some("Chinese")),
        ("english", Some("English")),
        ("dont", Some("don't")),
        ("Dont", Some("Don't")),
        ("howareyou", Some("how are you")),
        ("iloveyou", Some("I love you")),
        ("thankyouverymuch", Some("thank you very much")),
        ("goodmorning", Some("good morning")),
        ("nicetomeetyou", Some("nice to meet you")),
        ("doyouspeakenglish", Some("do you speak English")),
        ("whereareyougoing", Some("where are you going")),
        ("iamastudent", Some("I am a student")),
        ("seeyoutomorrow", Some("see you tomorrow")),
        ("haveaniceday", Some("have a nice day")),
        ("imfine", Some("I'm fine")),
        ("idontknow", Some("I don't know")),
        ("iwanttogohome", Some("I want to go home")),
        ("imsorry", Some("I'm sorry")),
        ("whatareyoudoing", Some("what are you doing")),
        ("recieve", Some("receive")),
        ("becuase", Some("because")),
        ("becasue", Some("because")),
        ("untill", Some("until")),
        ("thier", Some("their")),
        ("beleive", Some("believe")),
        ("freind", Some("friend")),
        ("occured", Some("occurred")),
        ("hapy", Some("happy")),
        ("hrllo", Some("hello")),
        ("wprld", Some("world")),
        ("nifht", Some("night")),
        ("thsnk", Some("thank")),
        ("goodmornig", Some("good morning")),
        ("imchinesee", Some("I'm Chinese")),
        ("monday", Some("Monday")),
        ("january", Some("January")),
        ("london", Some("London")),
        ("hello", None),
        ("language", None),
        ("the", None),
        ("what", None),
        ("speak", None),
        ("omg", None),
        ("thx", None),
        ("lol", None),
        ("btw", None),
        ("idk", None),
        ("IM", None),
        ("I'M", None),
        ("ENGLISH", None),
        ("eNglish", None),
        ("Im", Some("I'm")),
        ("Youre", Some("You're")),
    ];

    #[test]
    #[ignore = "需要用 MSIME_ENGLISH_CORRECTION_TEST_DB 指定固定的真实英文词库"]
    fn mobile_english_shipped_dictionary() {
        let path = std::env::var_os("MSIME_ENGLISH_CORRECTION_TEST_DB").expect("真实词库路径");
        let path = std::path::Path::new(&path);
        assert!(EnglishDictionary::open(path, None, None).ready());
        // 构建机与 CI 共用，计时取多轮中的最小值，排除其他进程的干扰。
        const ROUNDS: usize = 5;
        let mut mismatches = Vec::new();
        let (mut cold_total, mut cold_max) = (0.0_f64, 0.0_f64);
        let (mut typed_total, mut typed_max, mut typed_calls) = (0.0_f64, 0.0_f64, 0_u32);
        for &(input, expected) in SHIPPED_EXPECTATIONS {
            let mut actual = None;
            let mut cold = f64::INFINITY;
            let mut typed = vec![f64::INFINITY; input.len()];
            for _ in 0..ROUNDS {
                // 冷启动：新词库对象，没有任何缓存。
                let dictionary = EnglishDictionary::open(path, None, None);
                let started = std::time::Instant::now();
                actual = dictionary.correct_mobile_english(input);
                cold = cold.min(started.elapsed().as_secs_f64() * 1e3);
                // 逐键输入：同一对象依次查询每个前缀，与手机每按一键查询一次相同。
                let dictionary = EnglishDictionary::open(path, None, None);
                for end in 1..=input.len() {
                    let started = std::time::Instant::now();
                    let result = dictionary.correct_mobile_english(&input[..end]);
                    typed[end - 1] = typed[end - 1].min(started.elapsed().as_secs_f64() * 1e3);
                    if end == input.len() {
                        assert_eq!(result, actual, "{input}: 缓存改变了结果");
                    }
                }
            }
            let (slowest_end, slowest) = typed
                .iter()
                .copied()
                .enumerate()
                .fold((0, 0.0), |a, b| if b.1 > a.1 { b } else { a });
            cold_total += cold;
            cold_max = cold_max.max(cold);
            typed_total += typed.iter().sum::<f64>();
            typed_calls += typed.len() as u32;
            typed_max = typed_max.max(slowest);
            let ok = actual.as_deref() == expected;
            eprintln!(
                "{} {input} -> {:?} (expected {:?}) cold {cold:.2}ms, typed max {slowest:.2}ms at {}",
                if ok { "ok  " } else { "FAIL" },
                actual,
                expected,
                &input[..=slowest_end],
            );
            if !ok {
                mismatches.push(input);
            }
        }
        eprintln!(
            "cold mean {:.2}ms max {cold_max:.2}ms; typed mean {:.2}ms max {typed_max:.2}ms over {typed_calls} calls",
            cold_total / SHIPPED_EXPECTATIONS.len() as f64,
            typed_total / f64::from(typed_calls),
        );
        assert!(mismatches.is_empty(), "{mismatches:?}");
    }
}
