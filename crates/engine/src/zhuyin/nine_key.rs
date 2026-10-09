//! 注音九键：十个数字键各承载几个注音符号（分组取自美国专利 US 6,009,444 的 FIG.1），用户按数字串打出一个音节的符号位置，再按声调键结束这个音节。一串数字对应所有长度相同、第 i 个符号落在第 i 个数字键上的合法带调音节，由词库自己的音节表决定哪些合法。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::layout::TONE_MARKS;

/// 每个数字键上的注音符号，37 个符号各在且只在一个键上。分组固定照专利 FIG.1，不要调整。
pub const KEYPAD: [(u8, &str); 10] = [
    (b'1', "ㄅㄆㄇㄈ"),
    (b'2', "ㄉㄊㄋㄌ"),
    (b'3', "ㄍㄎㄏ"),
    (b'4', "ㄐㄑㄒ"),
    (b'5', "ㄓㄔㄕㄖ"),
    (b'6', "ㄗㄘㄙ"),
    (b'7', "ㄚㄛㄜㄝ"),
    (b'8', "ㄧㄨㄩㄦ"),
    (b'9', "ㄞㄟㄠㄡ"),
    (b'0', "ㄢㄣㄤㄥ"),
];

/// 一到五声（ˉ ˊ ˇ ˋ ˙）的声调键，下标与 `layout::TONE_MARKS` 一致。空格同样是一声。
pub const TONE_KEYS: [u8; 5] = *b"zxcvb";

/// 一个音节最多的符号数（声母、介音、韵母各一），也就是一串数字的最大长度。
pub const MAX_DIGITS: usize = 3;

/// 空闲时认领的非字母键：数字开始一个音节；空格不在其中，空闲时由宿主自己打出空格。
pub const IDLE_SYMBOLS: &str = "1234567890";

/// 组字中、列表关闭时认领的非字母键：数字和作为一声的空格。
pub const SYMBOLS: &str = "1234567890 ";

/// 列表打开时认领的非字母键：数字关闭列表继续拼写，空格留给运行时选行。
pub const LIST_OPEN_SYMBOLS: &str = "1234567890";

/// 承载 `symbol` 的数字键；不是注音符号时为 `None`。
pub fn digit(symbol: char) -> Option<u8> {
    KEYPAD
        .iter()
        .find(|(_, symbols)| symbols.contains(symbol))
        .map(|(key, _)| *key)
}

/// 声调键追加的调号：空格和 `z` 是一声（不加符号），`x c v b` 依次是 ˊ ˇ ˋ ˙；其他键为 `None`。
pub fn tone_mark(key: u8) -> Option<&'static str> {
    if key == b' ' {
        return Some(TONE_MARKS[0]);
    }
    TONE_KEYS
        .iter()
        .position(|tone_key| *tone_key == key)
        .map(|index| TONE_MARKS[index])
}

/// 词库音节表按「数字串 + 调号」分组后的索引。
#[derive(Debug)]
pub struct NineKeyIndex {
    /// 键是数字串加调号（一声没有调号），值是这一组的带调音节，按字典序排好，保证结果确定。
    readings: HashMap<String, Arc<[String]>>,
    /// 任何一个音节数字串的全部前缀（含它自己），用来拒绝打不出任何音节的数字。
    prefixes: HashSet<String>,
}

impl NineKeyIndex {
    /// 从 `LanguageDictionary::syllables()` 建索引。末字符是 ˊ ˇ ˋ ˙ 之一时它是调号，否则是一声；其余字符逐个映射到数字键，有符号不在键盘上的音节被跳过。
    pub fn new(syllables: Vec<String>) -> Self {
        let mut groups: HashMap<String, Vec<String>> = HashMap::new();
        let mut prefixes = HashSet::new();
        for syllable in syllables {
            let (symbols, mark) = split_tone(&syllable);
            let Some(digits) = symbols.chars().map(digit).collect::<Option<Vec<u8>>>() else {
                continue;
            };
            if digits.is_empty() || digits.len() > MAX_DIGITS {
                continue;
            }
            let digits = String::from_utf8(digits).expect("keypad keys are ASCII digits");
            for end in 1..=digits.len() {
                prefixes.insert(digits[..end].to_owned());
            }
            groups
                .entry(format!("{digits}{mark}"))
                .or_default()
                .push(syllable);
        }
        let readings = groups
            .into_iter()
            .map(|(code, mut readings)| {
                readings.sort();
                readings.dedup();
                (code, Arc::from(readings))
            })
            .collect();
        Self { readings, prefixes }
    }

    /// 数字串 `digits` 以调号 `mark` 结束时的全部合法带调音节；没有这样的音节时为 `None`。
    pub fn readings(&self, digits: &[u8], mark: &str) -> Option<Arc<[String]>> {
        let digits = std::str::from_utf8(digits).ok()?;
        // 索引键由不超过 `MAX_DIGITS` 的数字和至多一个 Unicode 调号组成，超长拼接键必定未命中。
        let mut buffer = [0; MAX_DIGITS + char::MAX.len_utf8()];
        let length = digits.len().checked_add(mark.len())?;
        let code = buffer.get_mut(..length)?;
        code[..digits.len()].copy_from_slice(digits.as_bytes());
        code[digits.len()..].copy_from_slice(mark.as_bytes());
        self.readings.get(std::str::from_utf8(code).ok()?).cloned()
    }

    /// `digits` 是否还能拼成某个音节，也就是它是否为某个音节数字串的前缀。
    pub fn accepts(&self, digits: &[u8]) -> bool {
        std::str::from_utf8(digits).is_ok_and(|digits| self.prefixes.contains(digits))
    }

    /// 分组的数量，供测试核对索引规模。
    #[cfg(test)]
    fn group_count(&self) -> usize {
        self.readings.len()
    }
}

/// 把带调音节拆成符号和调号；一声没有调号。
fn split_tone(syllable: &str) -> (&str, &str) {
    match syllable.chars().next_back() {
        Some(last) if TONE_MARKS[1..].iter().any(|mark| mark.starts_with(last)) => {
            syllable.split_at(syllable.len() - last.len_utf8())
        }
        _ => (syllable, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zhuyin::layout::DACHEN;

    fn owned(syllables: &[&str]) -> Vec<String> {
        syllables
            .iter()
            .map(|syllable| (*syllable).to_owned())
            .collect()
    }

    #[test]
    fn every_bopomofo_is_on_exactly_the_patent_key() {
        // 专利 FIG.1 的分组逐字核对。
        let expected = [
            ('1', "ㄅㄆㄇㄈ"),
            ('2', "ㄉㄊㄋㄌ"),
            ('3', "ㄍㄎㄏ"),
            ('4', "ㄐㄑㄒ"),
            ('5', "ㄓㄔㄕㄖ"),
            ('6', "ㄗㄘㄙ"),
            ('7', "ㄚㄛㄜㄝ"),
            ('8', "ㄧㄨㄩㄦ"),
            ('9', "ㄞㄟㄠㄡ"),
            ('0', "ㄢㄣㄤㄥ"),
        ];
        for ((key, symbols), (expected_key, expected_symbols)) in KEYPAD.iter().zip(expected) {
            assert_eq!(char::from(*key), expected_key);
            assert_eq!(*symbols, expected_symbols);
        }
        // 37 个符号（大千键盘的全部符号）各落在唯一一个键上。
        let mut seen = HashSet::new();
        for (_, symbol, _) in DACHEN {
            let key = digit(symbol).unwrap_or_else(|| panic!("{symbol} has no key"));
            let holders = KEYPAD
                .iter()
                .filter(|(_, symbols)| symbols.contains(symbol))
                .count();
            assert_eq!(holders, 1, "{symbol}");
            assert!(KEYPAD.iter().any(|(k, s)| *k == key && s.contains(symbol)));
            seen.insert(symbol);
        }
        assert_eq!(seen.len(), 37);
        let total: usize = KEYPAD
            .iter()
            .map(|(_, symbols)| symbols.chars().count())
            .sum();
        assert_eq!(total, 37);
        assert_eq!(digit('a'), None);
        assert_eq!(digit('ˇ'), None);
    }

    #[test]
    fn tone_keys_map_to_marks_and_space_is_the_first_tone() {
        assert_eq!(tone_mark(b' '), Some(""));
        assert_eq!(tone_mark(b'z'), Some(""));
        assert_eq!(tone_mark(b'x'), Some("ˊ"));
        assert_eq!(tone_mark(b'c'), Some("ˇ"));
        assert_eq!(tone_mark(b'v'), Some("ˋ"));
        assert_eq!(tone_mark(b'b'), Some("˙"));
        for key in *b"1234567890a6347" {
            assert_eq!(tone_mark(key), None, "{}", char::from(key));
        }
    }

    #[test]
    fn symbol_strings_hold_digits_and_space_only_while_composing() {
        assert_eq!(IDLE_SYMBOLS, "1234567890");
        assert_eq!(SYMBOLS, "1234567890 ");
        assert_eq!(LIST_OPEN_SYMBOLS, "1234567890");
        for symbols in [IDLE_SYMBOLS, SYMBOLS, LIST_OPEN_SYMBOLS] {
            assert!(!symbols.contains([',', '.', '/', ';', '-']));
        }
    }

    #[test]
    fn index_groups_syllables_by_digits_and_tone() {
        let index = NineKeyIndex::new(owned(&[
            "ㄋㄧˇ",
            "ㄌㄧˇ",
            "ㄋㄧ",
            "ㄇㄚ˙",
            "ㄇㄚ",
            "ㄅㄚ",
            "ㄏㄠˇ",
            "ㄓㄨㄤ",
            "ㄢ",
        ]));
        assert_eq!(
            index.readings(b"28", "ˇ").as_deref(),
            Some(&owned(&["ㄋㄧˇ", "ㄌㄧˇ"])[..])
        );
        // 一声不带调号，与带调的分在不同组。
        assert_eq!(
            index.readings(b"28", "").as_deref(),
            Some(&owned(&["ㄋㄧ"])[..])
        );
        assert_eq!(
            index.readings(b"17", "˙").as_deref(),
            Some(&owned(&["ㄇㄚ˙"])[..])
        );
        assert_eq!(
            index.readings(b"17", "").as_deref(),
            Some(&owned(&["ㄅㄚ", "ㄇㄚ"])[..])
        );
        assert_eq!(
            index.readings(b"580", "").as_deref(),
            Some(&owned(&["ㄓㄨㄤ"])[..])
        );
        assert_eq!(
            index.readings(b"0", "").as_deref(),
            Some(&owned(&["ㄢ"])[..])
        );
        assert_eq!(index.readings(b"28", "ˋ"), None);
        assert_eq!(index.readings(b"2", "ˇ"), None);
        assert_eq!(index.group_count(), 7);
    }

    #[test]
    fn index_reading_lookups_do_not_allocate_temporary_keys() {
        let syllables = TONE_MARKS
            .iter()
            .map(|mark| format!("ㄓㄨㄤ{mark}"))
            .collect::<Vec<_>>();
        let index = NineKeyIndex::new(syllables.clone());
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            for (mark, expected) in TONE_MARKS.iter().zip(&syllables) {
                let readings = index.readings(b"580", mark).unwrap();
                assert_eq!(readings.as_ref(), std::slice::from_ref(expected));
            }
            assert!(index.readings(b"999", "ˇ").is_none());
        });
        assert_eq!(allocations, 0);
    }

    #[test]
    fn index_reading_lookups_preserve_key_part_and_invalid_input_behavior() {
        let index = NineKeyIndex::new(owned(&["ㄋㄧˇ"]));
        let expected = index.readings(b"28", "ˇ").unwrap();
        for (digits, mark) in [
            (b"2".as_slice(), "8ˇ"),
            ("28ˇ".as_bytes(), ""),
            (b"".as_slice(), "28ˇ"),
        ] {
            let readings = index.readings(digits, mark).unwrap();
            assert!(Arc::ptr_eq(&expected, &readings));
        }
        for (digits, mark) in [
            (b"\xff".as_slice(), "ˇ"),
            (b"28".as_slice(), "測試長調號"),
            (b"2899999999".as_slice(), "ˇ"),
            (b"".as_slice(), ""),
        ] {
            assert!(index.readings(digits, mark).is_none());
        }
    }

    #[test]
    fn index_accepts_only_prefixes_of_some_syllable() {
        let index = NineKeyIndex::new(owned(&["ㄋㄧˇ", "ㄓㄨㄤ", "ㄢ"]));
        for accepted in ["2", "28", "5", "58", "580", "0"] {
            assert!(index.accepts(accepted.as_bytes()), "{accepted}");
        }
        for rejected in ["", "1", "29", "282", "5800", "00"] {
            assert!(!index.accepts(rejected.as_bytes()), "{rejected}");
        }
    }

    #[test]
    fn index_skips_symbols_off_the_keypad() {
        let index = NineKeyIndex::new(owned(&["nei", "ㄋㄧˇ", "ㄋㄧㄠㄢ"]));
        assert_eq!(index.group_count(), 1);
        assert!(!index.accepts(b"2890"));
    }
}
