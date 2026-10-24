//! 文镜式现代韩语罗马字键位，保留原始拉丁按键作为退格来源。
//!
//! 映射参考 rimeinn/rime-mungyeong@34c1b1437824e1e619e7429fb5f461dc7e754939
//! 的 mungyeong_romaja.dict.yaml（public domain）。不移植其 Lua 处理器；
//! 退格删除原始按键，而非拆解上一个韩文字母部件。

use super::dubeolsik::{fold_jamos_with, Jamo};

/// 长码在前，最长匹配；所有来源位置都指向原始 ASCII 输入的字节边界。
const CODES: &[(&[u8], Jamo)] = &[
    (b"wae", Jamo::Vowel(10)),
    (b"yae", Jamo::Vowel(3)),
    (b"yeo", Jamo::Vowel(6)),
    (b"eui", Jamo::Vowel(19)),
    (b"ieo", Jamo::Vowel(6)),
    (b"iae", Jamo::Vowel(3)),
    (b"uae", Jamo::Vowel(10)),
    (b"wi", Jamo::Vowel(16)),
    (b"wa", Jamo::Vowel(9)),
    (b"wo", Jamo::Vowel(14)),
    (b"we", Jamo::Vowel(15)),
    (b"eo", Jamo::Vowel(4)),
    (b"eu", Jamo::Vowel(18)),
    (b"ui", Jamo::Vowel(19)),
    (b"ya", Jamo::Vowel(2)),
    (b"yo", Jamo::Vowel(12)),
    (b"yu", Jamo::Vowel(17)),
    (b"ye", Jamo::Vowel(7)),
    (b"oe", Jamo::Vowel(11)),
    (b"ae", Jamo::Vowel(1)),
    (b"ch", Jamo::Consonant('ㅊ')),
    (b"ng", Jamo::Consonant('ㅇ')),
    (b"ia", Jamo::Vowel(2)),
    (b"io", Jamo::Vowel(12)),
    (b"iu", Jamo::Vowel(17)),
    (b"ie", Jamo::Vowel(7)),
    (b"ua", Jamo::Vowel(9)),
    (b"uo", Jamo::Vowel(14)),
    (b"ue", Jamo::Vowel(15)),
    (b"w", Jamo::Vowel(13)),
    (b"e", Jamo::Vowel(5)),
    (b"r", Jamo::Consonant('ㄹ')),
    (b"t", Jamo::Consonant('ㅌ')),
    (b"y", Jamo::Vowel(20)),
    (b"o", Jamo::Vowel(8)),
    (b"p", Jamo::Consonant('ㅍ')),
    (b"a", Jamo::Vowel(0)),
    (b"s", Jamo::Consonant('ㅅ')),
    (b"S", Jamo::Consonant('ㅆ')),
    (b"d", Jamo::Consonant('ㄷ')),
    (b"D", Jamo::Consonant('ㄸ')),
    (b"g", Jamo::Consonant('ㄱ')),
    (b"G", Jamo::Consonant('ㄲ')),
    (b"h", Jamo::Consonant('ㅎ')),
    (b"j", Jamo::Consonant('ㅈ')),
    (b"J", Jamo::Consonant('ㅉ')),
    (b"k", Jamo::Consonant('ㅋ')),
    (b"x", Jamo::Consonant('ㅇ')),
    (b"c", Jamo::Consonant('ㅊ')),
    (b"b", Jamo::Consonant('ㅂ')),
    (b"B", Jamo::Consonant('ㅃ')),
    (b"n", Jamo::Consonant('ㄴ')),
    (b"m", Jamo::Consonant('ㅁ')),
    (b"u", Jamo::Vowel(13)),
    (b"l", Jamo::Consonant('ㄹ')),
    (b"i", Jamo::Vowel(20)),
    (b"f", Jamo::Consonant('ㅍ')),
];

fn normalized(key: u8) -> u8 {
    match key {
        b'G' | b'D' | b'B' | b'J' | b'S' => key,
        _ => key.to_ascii_lowercase(),
    }
}

fn starts_with(input: &[u8], code: &[u8]) -> bool {
    input.len() >= code.len()
        && input
            .iter()
            .zip(code)
            .all(|(&key, &expected)| normalized(key) == expected)
}

fn vowel_starts(key: u8) -> bool {
    matches!(
        normalized(key),
        b'a' | b'e' | b'i' | b'o' | b'u' | b'w' | b'y'
    )
}

struct Tokens<'a> {
    keys: &'a [u8],
    offset: usize,
}

impl<'a> Tokens<'a> {
    fn new(keys: &'a str) -> Self {
        Self {
            keys: keys.as_bytes(),
            offset: 0,
        }
    }
}

impl Iterator for Tokens<'_> {
    type Item = (usize, Jamo);

    fn next(&mut self) -> Option<Self::Item> {
        let input = self.keys.get(self.offset..)?;
        if input.is_empty() {
            return None;
        }
        let start = self.offset;
        // 末尾 ng 表示 ㅇ，后接元音时按 n+g 重分：hang → 항，hangu → 한구。
        if starts_with(input, b"ng") && input.get(2).is_some_and(|&key| vowel_starts(key)) {
            self.offset += 1;
            return Some((start, Jamo::Consonant('ㄴ')));
        }
        if let Some(&(code, jamo)) = CODES.iter().find(|&&(code, _)| starts_with(input, code)) {
            self.offset += code.len();
            return Some((start, jamo));
        }
        self.offset += 1;
        Some((start, Jamo::Literal(input[0])))
    }
}

/// 后续按键能改变的最早编码。完整 ng 仍可能因下一个元音重分。
fn unstable_start(keys: &str) -> usize {
    let bytes = keys.as_bytes();
    Tokens::new(keys)
        .filter_map(|(start, _)| {
            let suffix = &bytes[start..];
            let extends = CODES.iter().any(|&(code, _)| {
                suffix.len() < code.len()
                    && suffix
                        .iter()
                        .zip(code)
                        .all(|(&key, &expected)| normalized(key) == expected)
            });
            (extends || (suffix.len() == 2 && starts_with(suffix, b"ng"))).then_some(start)
        })
        .min()
        .unwrap_or(keys.len())
}

pub fn compose_into(keys: &str, output: &mut String) {
    output.clear();
    if let Some(last) = fold_jamos_with(Tokens::new(keys), |syllable, _| syllable.render(output)) {
        last.render(output);
    }
}

/// 仅追加稳定的完整音节；返回值借用原始拉丁拼写，不按字母数量倒推来源。
pub(super) fn split_finished_into<'a>(keys: &'a str, output: &mut String) -> &'a str {
    let stable_before = unstable_start(keys);
    let mut keep = keys.len();
    let last = fold_jamos_with(Tokens::new(keys), |syllable, next_start| {
        if keep == keys.len() && next_start <= stable_before {
            syllable.render(output);
        } else {
            keep = keep.min(syllable.start);
        }
    });
    if let Some(last) = last {
        keep = keep.min(last.start);
    }
    &keys[keep..]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn composed(keys: &str) -> String {
        let mut output = String::new();
        compose_into(keys, &mut output);
        output
    }

    #[test]
    fn modern_romaja_spellings_and_aliases() {
        for (keys, expected) in [
            ("ga", "가"),
            ("na", "나"),
            ("xa", "아"),
            ("a", "ㅏ"),
            ("han", "한"),
            ("hang", "항"),
            ("hangug", "한국"),
            ("hangeul", "한글"),
            ("bang", "방"),
            ("bangi", "반기"),
            ("bangxi", "방이"),
            ("geo", "거"),
            ("geu", "그"),
            ("gui", "긔"),
            ("gwa", "과"),
            ("dalg", "닭"),
            ("xeobs", "없"),
            ("Ga", "까"),
            ("Da", "따"),
            ("Ba", "빠"),
            ("Ja", "짜"),
            ("Sa", "싸"),
            ("gyae", "걔"),
            ("gieo", "겨"),
            ("gue", "궤"),
            ("gwi", "귀"),
            ("gya", "갸"),
            ("gye", "계"),
            ("cha", "차"),
            ("ca", "차"),
            ("haN", "한"),
            ("hanq", "한q"),
        ] {
            assert_eq!(composed(keys), expected, "{keys}");
        }
    }

    fn assert_stream(keys: &str) {
        let mut raw = String::new();
        let mut committed = String::new();
        for (index, byte) in keys.bytes().enumerate() {
            raw.push(char::from(byte));
            let keep = split_finished_into(&raw, &mut committed).len();
            raw.drain(..raw.len() - keep);
            assert_eq!(
                format!("{committed}{}", composed(&raw)),
                composed(&keys[..=index]),
                "输入 {keys}，前缀 {}，保留 {raw}",
                &keys[..=index],
            );
        }
    }

    #[test]
    fn streamed_prefixes_equal_whole_spelling() {
        for keys in [
            "hangug",
            "hangeul",
            "bangxi",
            "gwageu",
            "dalggogi",
            "xannyeonghaseyo",
            "hanqga",
            "euiwiyayeo",
        ] {
            assert_stream(keys);
        }
        let alphabet = b"abcdefghijklmnopqrstuvwxyzGDBJS";
        let mut seed = 1_u64;
        for _ in 0..2000 {
            let mut keys = String::new();
            for _ in 0..48 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                keys.push(char::from(alphabet[(seed >> 32) as usize % alphabet.len()]));
            }
            assert_stream(&keys);
        }
    }

    #[test]
    fn all_code_pairs_and_continuations_have_safe_commit_boundaries() {
        for &(first, _) in CODES {
            for &(second, _) in CODES {
                let pair = format!(
                    "{}{}",
                    std::str::from_utf8(first).unwrap(),
                    std::str::from_utf8(second).unwrap()
                );
                for next in b"aeiouywgnchGq" {
                    assert_stream(&format!("{pair}{}", char::from(*next)));
                }
            }
        }
    }

    #[test]
    fn raw_backspace_reparses_the_surviving_latin_keys() {
        for (keys, expected) in [
            ("ban", "반"),
            ("gw", "구"),
            ("ge", "게"),
            ("ha", "하"),
            ("h", "ㅎ"),
        ] {
            assert_eq!(composed(keys), expected);
        }
    }

    #[test]
    fn every_modern_syllable_has_a_canonical_romaja_spelling() {
        let initials = [
            "g", "G", "n", "d", "D", "r", "m", "b", "B", "s", "S", "x", "j", "J", "ch", "k", "t",
            "p", "h",
        ];
        let vowels = [
            "a", "ae", "ya", "yae", "eo", "e", "yeo", "ye", "o", "wa", "wae", "oe", "yo", "u",
            "wo", "we", "wi", "yu", "eu", "ui", "i",
        ];
        let finals = [
            "", "g", "G", "gs", "n", "nj", "nh", "d", "l", "lg", "lm", "lb", "ls", "lt", "lp",
            "lh", "m", "b", "bs", "s", "S", "ng", "j", "ch", "k", "t", "p", "h",
        ];
        for (cho, initial) in initials.iter().enumerate() {
            for (jung, vowel) in vowels.iter().enumerate() {
                for (jong, final_code) in finals.iter().enumerate() {
                    let keys = format!("{initial}{vowel}{final_code}");
                    let expected = char::from_u32(0xac00 + ((cho * 21 + jung) * 28 + jong) as u32)
                        .unwrap()
                        .to_string();
                    assert_eq!(composed(&keys), expected, "{keys}");
                    assert_stream(&keys);
                }
            }
        }
    }
}
