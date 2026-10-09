//! The Dubeolsik (2-beolsik, KS X 5002) layout and the syllable automaton that composes its jamo into precomposed Hangul syllables (U+AC00..U+D7A3).
//!
//! Composition is a fold over the key letters of the text still being composed, so the state is fully described by those letters: Backspace drops the last letter and the fold runs again, which removes exactly one jamo keystroke (a compound vowel or final loses its second half first). Every syllable in the fold records the key index it starts at, so the scheme can hand every syllable but the last to the host and keep composing the last one.

/// One key of the layout: a consonant (as its Hangul Compatibility Jamo, U+3131..U+314E) or a vowel (as its jungseong index 0..=20).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Jamo {
    Consonant(char),
    Vowel(u8),
}

/// The initial consonants in jungseong-table order; the position is the choseong index.
const CHOSEONG: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ',
    'ㅌ', 'ㅍ', 'ㅎ',
];

/// The final consonants; the position plus one is the jongseong index (0 is "no final"). ㄸ, ㅃ and ㅉ never close a syllable.
const JONGSEONG: [char; 27] = [
    'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ', 'ㅀ', 'ㅁ',
    'ㅂ', 'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];

/// Jungseong indices of the vowels the compound rules name.
const O: u8 = 8;
const U: u8 = 13;
const EU: u8 = 18;
const A: u8 = 0;
const AE: u8 = 1;
const EO: u8 = 4;
const E: u8 = 5;
const I: u8 = 20;

/// The first Hangul Compatibility Jamo vowel (ㅏ); the vowels follow in jungseong order.
const COMPATIBILITY_VOWEL_BASE: u32 = 0x314F;
const SYLLABLE_BASE: u32 = 0xAC00;

/// The jamo a key types. Shift gives the five double consonants and ㅒ ㅖ; every other shifted letter types its unshifted jamo, as on the standard layout.
pub fn jamo_for_key(key: u8) -> Option<Jamo> {
    let consonant = |jamo| Some(Jamo::Consonant(jamo));
    let vowel = |index| Some(Jamo::Vowel(index));
    match key {
        b'Q' => consonant('ㅃ'),
        b'W' => consonant('ㅉ'),
        b'E' => consonant('ㄸ'),
        b'R' => consonant('ㄲ'),
        b'T' => consonant('ㅆ'),
        b'O' => vowel(3),
        b'P' => vowel(7),
        b'A'..=b'Z' => jamo_for_key(key.to_ascii_lowercase()),
        b'q' => consonant('ㅂ'),
        b'w' => consonant('ㅈ'),
        b'e' => consonant('ㄷ'),
        b'r' => consonant('ㄱ'),
        b't' => consonant('ㅅ'),
        b'y' => vowel(12),
        b'u' => vowel(6),
        b'i' => vowel(2),
        b'o' => vowel(AE),
        b'p' => vowel(E),
        b'a' => consonant('ㅁ'),
        b's' => consonant('ㄴ'),
        b'd' => consonant('ㅇ'),
        b'f' => consonant('ㄹ'),
        b'g' => consonant('ㅎ'),
        b'h' => vowel(O),
        b'j' => vowel(EO),
        b'k' => vowel(A),
        b'l' => vowel(I),
        b'z' => consonant('ㅋ'),
        b'x' => consonant('ㅌ'),
        b'c' => consonant('ㅊ'),
        b'v' => consonant('ㅍ'),
        b'b' => vowel(17),
        b'n' => vowel(U),
        b'm' => vowel(EU),
        _ => None,
    }
}

/// ㅘ ㅙ ㅚ ㅝ ㅞ ㅟ ㅢ.
fn compound_vowel(first: u8, second: u8) -> Option<u8> {
    Some(match (first, second) {
        (O, A) => 9,
        (O, AE) => 10,
        (O, I) => 11,
        (U, EO) => 14,
        (U, E) => 15,
        (U, I) => 16,
        (EU, I) => 19,
        _ => return None,
    })
}

/// ㄳ ㄵ ㄶ ㄺ ㄻ ㄼ ㄽ ㄾ ㄿ ㅀ ㅄ.
fn compound_final(first: char, second: char) -> Option<char> {
    Some(match (first, second) {
        ('ㄱ', 'ㅅ') => 'ㄳ',
        ('ㄴ', 'ㅈ') => 'ㄵ',
        ('ㄴ', 'ㅎ') => 'ㄶ',
        ('ㄹ', 'ㄱ') => 'ㄺ',
        ('ㄹ', 'ㅁ') => 'ㄻ',
        ('ㄹ', 'ㅂ') => 'ㄼ',
        ('ㄹ', 'ㅅ') => 'ㄽ',
        ('ㄹ', 'ㅌ') => 'ㄾ',
        ('ㄹ', 'ㅍ') => 'ㄿ',
        ('ㄹ', 'ㅎ') => 'ㅀ',
        ('ㅂ', 'ㅅ') => 'ㅄ',
        _ => return None,
    })
}

fn choseong_index(consonant: char) -> Option<u32> {
    CHOSEONG
        .iter()
        .position(|&jamo| jamo == consonant)
        .map(|index| index as u32)
}

fn jongseong_index(consonant: char) -> Option<u32> {
    JONGSEONG
        .iter()
        .position(|&jamo| jamo == consonant)
        .map(|index| index as u32 + 1)
}

/// 合成中的一个音节：元音和尾辅音各最多两个分量，固定槽保留复合字母的拆分信息，尾辅音还记录其按键索引。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Syllable {
    /// Key index of the syllable's first jamo.
    start: usize,
    cho: Option<char>,
    jung: [u8; 2],
    jung_len: usize,
    jong: [(char, usize); 2],
    jong_len: usize,
}

impl Syllable {
    fn starting_at(start: usize) -> Self {
        Self {
            start,
            ..Self::default()
        }
    }

    fn is_empty(&self) -> bool {
        self.cho.is_none() && self.jung_len == 0
    }

    fn jung(&self) -> &[u8] {
        &self.jung[..self.jung_len]
    }

    fn push_jung(&mut self, vowel: u8) {
        assert!(self.jung_len < self.jung.len());
        self.jung[self.jung_len] = vowel;
        self.jung_len += 1;
    }

    fn jong(&self) -> &[(char, usize)] {
        &self.jong[..self.jong_len]
    }

    fn push_jong(&mut self, consonant: (char, usize)) {
        assert!(self.jong_len < self.jong.len());
        self.jong[self.jong_len] = consonant;
        self.jong_len += 1;
    }

    fn pop_jong(&mut self) -> Option<(char, usize)> {
        let last = self.jong_len.checked_sub(1)?;
        self.jong_len = last;
        Some(self.jong[last])
    }

    fn vowel(&self) -> Option<u8> {
        match self.jung() {
            [] => None,
            [single] => Some(*single),
            [first, second] => compound_vowel(*first, *second),
            _ => None,
        }
    }

    fn final_consonant(&self) -> Option<char> {
        match self.jong() {
            [] => None,
            [(single, _)] => Some(*single),
            [(first, _), (second, _)] => compound_final(*first, *second),
            _ => None,
        }
    }

    /// The precomposed syllable, or the standalone compatibility jamo when there is no initial or no vowel.
    fn render(&self, output: &mut String) {
        let vowel = self.vowel();
        match (self.cho, vowel) {
            (Some(cho), Some(vowel)) => {
                let cho = choseong_index(cho).expect("only initials enter the initial slot");
                let jong = self
                    .final_consonant()
                    .and_then(jongseong_index)
                    .unwrap_or(0);
                let code = SYLLABLE_BASE + (cho * 21 + u32::from(vowel)) * 28 + jong;
                output.push(char::from_u32(code).expect("inside the Hangul Syllables block"));
            }
            (Some(cho), None) => output.push(cho),
            (None, Some(vowel)) => output.push(
                char::from_u32(COMPATIBILITY_VOWEL_BASE + u32::from(vowel))
                    .expect("inside the Hangul Compatibility Jamo block"),
            ),
            (None, None) => {}
        }
    }
}

/// 按原状态转移逐个发出已完成音节，返回最后的开放音节；非布局按键跳过。
fn fold_with(keys: &[u8], mut finished: impl FnMut(Syllable)) -> Option<Syllable> {
    let mut current = Syllable::starting_at(0);
    for (index, &key) in keys.iter().enumerate() {
        let Some(jamo) = jamo_for_key(key) else {
            continue;
        };
        match jamo {
            Jamo::Consonant(consonant) => {
                let takes_final = current.cho.is_some()
                    && current.jung_len != 0
                    && match current.jong() {
                        [] => jongseong_index(consonant).is_some(),
                        [(first, _)] => compound_final(*first, consonant).is_some(),
                        _ => false,
                    };
                if takes_final {
                    current.push_jong((consonant, index));
                } else if current.is_empty() {
                    current.start = index;
                    current.cho = Some(consonant);
                } else {
                    finished(std::mem::replace(
                        &mut current,
                        Syllable::starting_at(index),
                    ));
                    current.cho = Some(consonant);
                }
            }
            Jamo::Vowel(vowel) => {
                if let Some((moved, moved_at)) = current.pop_jong() {
                    // 尾辅音后接元音时，将最后一个辅音移作下一音节的首辅音：간+ㅏ → 가나，닭+ㅏ → 달가。
                    finished(std::mem::replace(
                        &mut current,
                        Syllable::starting_at(moved_at),
                    ));
                    current.cho = Some(moved);
                    current.push_jung(vowel);
                } else if current.jung_len == 0 {
                    if current.is_empty() {
                        current.start = index;
                    }
                    current.push_jung(vowel);
                } else if current.jung_len == 1 && compound_vowel(current.jung[0], vowel).is_some()
                {
                    current.push_jung(vowel);
                } else {
                    finished(std::mem::replace(
                        &mut current,
                        Syllable::starting_at(index),
                    ));
                    current.push_jung(vowel);
                }
            }
        }
    }
    (!current.is_empty()).then_some(current)
}

/// The Hangul text `keys` spell.
pub fn compose(keys: &str) -> String {
    let mut output = String::with_capacity(keys.len().saturating_mul(3));
    compose_into(keys, &mut output);
    output
}

/// 将合成的韩文写入已有字符串，保留宿主会话的容量。
pub fn compose_into(keys: &str, output: &mut String) {
    output.clear();
    output.reserve(keys.len().saturating_mul(3));
    if let Some(last) = fold_with(keys.as_bytes(), |syllable| syllable.render(output)) {
        last.render(output);
    }
}

/// 直接追加已完成的音节，保留原提交前缀，并返回仍可编辑的原按键切片。
pub(crate) fn split_finished_into<'a>(keys: &'a str, output: &mut String) -> &'a str {
    let mut reserved = false;
    let last = fold_with(keys.as_bytes(), |syllable| {
        if !reserved {
            output.reserve(keys.len().saturating_mul(3));
            reserved = true;
        }
        syllable.render(output);
    });
    last.map_or(keys, |syllable| &keys[syllable.start..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split_finished(keys: &str) -> (String, &str) {
        let mut finished = String::new();
        let open = split_finished_into(keys, &mut finished);
        (finished, open)
    }

    #[test]
    fn rendered_text_reserves_utf8_bytes() {
        let composed = compose("rkrk");
        assert_eq!(composed, "가가");
        assert_eq!(composed.capacity(), 12);
        let mut finished = String::new();
        let rest = split_finished_into("rkrk", &mut finished);
        assert_eq!(finished, "가");
        assert_eq!(rest, "rk");
        assert!(finished.capacity() >= 12);
    }

    #[test]
    fn compose_into_reserves_the_full_utf8_capacity() {
        let mut output = String::with_capacity(8);
        compose_into("rkrk", &mut output);
        assert_eq!(output, "가가");
        assert!(output.capacity() >= 12);
    }

    #[test]
    fn compose_into_reuses_output_without_temporary_allocations() {
        let long_keys = "dkssudgktpdy".repeat(128);
        let long_text = "안녕하세요".repeat(128);
        for (keys, expected) in [
            ("", ""),
            ("rkrk", "가가"),
            ("rhkd", "광"),
            ("ekfrk", "달가"),
            ("rkqtl", "갑시"),
            ("rr", "ㄱㄱ"),
            ("kk", "ㅏㅏ"),
            ("r1k", "가"),
            (long_keys.as_str(), long_text.as_str()),
        ] {
            let mut output = String::with_capacity(keys.len() * 3 + 16);
            output.push_str("舊值");
            let pointer = output.as_ptr();
            let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
                compose_into(keys, &mut output);
            });
            assert_eq!(output, expected);
            assert_eq!(output.as_ptr(), pointer);
            assert_eq!(allocations, 0, "复用输出不应物化音节向量：{keys}");
        }
    }

    #[test]
    fn compose_only_allocates_the_output_string() {
        for (keys, expected) in [("rkrk", "가가"), ("ekfrk", "달가"), ("rr", "ㄱㄱ")] {
            let (output, allocations) =
                crate::ime::personal_rerank::allocations::count(|| compose(keys));
            assert_eq!(output, expected);
            assert_eq!(allocations, 1, "冷合成只应分配返回字符串");
        }
    }

    #[test]
    fn every_letter_types_its_dubeolsik_jamo() {
        let lower: String = (b'a'..=b'z')
            .map(|key| compose(&char::from(key).to_string()))
            .collect();
        assert_eq!(
            lower,
            "ㅁㅠㅊㅇㄷㄹㅎㅗㅑㅓㅏㅣㅡㅜㅐㅔㅂㄱㄴㅅㅕㅍㅈㅌㅛㅋ"
        );
        let upper: String = (b'A'..=b'Z')
            .map(|key| compose(&char::from(key).to_string()))
            .collect();
        assert_eq!(
            upper,
            "ㅁㅠㅊㅇㄸㄹㅎㅗㅑㅓㅏㅣㅡㅜㅒㅖㅃㄲㄴㅆㅕㅍㅉㅌㅛㅋ"
        );
        assert_eq!(jamo_for_key(b'1'), None);
        assert_eq!(jamo_for_key(b'\''), None);
    }

    #[test]
    fn syllables_compose_from_initial_vowel_and_final() {
        let cases = [
            ("rk", "가"),
            ("rkr", "각"),
            ("dkssud", "안녕"),
            ("gksrmf", "한글"),
            ("dkssudgktpdy", "안녕하세요"),
            ("tkfkd", "사랑"),
            ("Rk", "까"),
            ("Ek", "따"),
            ("Qk", "빠"),
            ("Tk", "싸"),
            ("Wk", "짜"),
            ("rkR", "갂"),
            ("rkT", "갔"),
            ("dO", "얘"),
            ("dP", "예"),
        ];
        for (keys, expected) in cases {
            assert_eq!(compose(keys), expected, "{keys}");
        }
    }

    #[test]
    fn compound_vowels_join_and_others_do_not() {
        let cases = [
            ("rhk", "과"),
            ("rho", "괘"),
            ("rhl", "괴"),
            ("rnj", "궈"),
            ("rnp", "궤"),
            ("rnl", "귀"),
            ("dml", "의"),
            ("hk", "ㅘ"),
            ("ml", "ㅢ"),
            // ㅏ and ㅗ have no compound: the second vowel stands alone.
            ("rkh", "가ㅗ"),
            // A compound vowel takes no third vowel.
            ("rhkl", "과ㅣ"),
        ];
        for (keys, expected) in cases {
            assert_eq!(compose(keys), expected, "{keys}");
        }
    }

    #[test]
    fn compound_finals_join_and_split_before_a_vowel() {
        let cases = [
            ("ekfr", "닭"),
            ("ekfrk", "달가"),
            ("dksw", "앉"),
            ("dkswk", "안자"),
            ("aksg", "많"),
            ("rkqt", "값"),
            ("rkqtdl", "값이"),
            ("rkqtl", "갑시"),
            ("dlfr", "읽"),
            ("tkfa", "삶"),
            ("dufq", "엷"),
            ("rhft", "곬"),
            ("gkfx", "핥"),
            ("dmfv", "읊"),
            ("dlfg", "잃"),
            ("rkt", "갓"),
            ("rkrt", "갃"),
            // No compound for ㄱ+ㄱ: the second one opens a new syllable.
            ("rkrr", "각ㄱ"),
            // A compound final takes no third consonant.
            ("ekfrr", "닭ㄱ"),
        ];
        for (keys, expected) in cases {
            assert_eq!(compose(keys), expected, "{keys}");
        }
    }

    #[test]
    fn a_single_final_moves_to_the_next_syllable() {
        assert_eq!(compose("rksk"), "가나");
        assert_eq!(compose("rksrk"), "간가");
        assert_eq!(compose("dkssud"), "안녕");
        assert_eq!(compose("rkRk"), "가까");
    }

    #[test]
    fn double_initials_that_cannot_close_a_syllable_open_the_next() {
        // ㄸ ㅃ ㅉ are never finals.
        assert_eq!(compose("rkE"), "가ㄸ");
        assert_eq!(compose("rkQ"), "가ㅃ");
        assert_eq!(compose("rkW"), "가ㅉ");
        assert_eq!(compose("rkEk"), "가따");
    }

    #[test]
    fn standalone_jamo_when_no_syllable_forms() {
        assert_eq!(compose("r"), "ㄱ");
        assert_eq!(compose("rr"), "ㄱㄱ");
        // No initial-only clusters: ㄱ then ㅅ are two jamo, not ㄳ.
        assert_eq!(compose("rt"), "ㄱㅅ");
        assert_eq!(compose("k"), "ㅏ");
        assert_eq!(compose("kk"), "ㅏㅏ");
        // A vowel with no initial takes no final.
        assert_eq!(compose("kr"), "ㅏㄱ");
        assert_eq!(compose("krk"), "ㅏ가");
        assert_eq!(compose(""), "");
    }

    #[test]
    fn keys_that_are_not_letters_are_skipped() {
        assert_eq!(compose("r1k"), "가");
    }

    #[test]
    fn split_keeps_only_the_open_syllable() {
        assert_eq!(split_finished("rk"), (String::new(), "rk"));
        assert_eq!(split_finished("rkr"), (String::new(), "rkr"));
        assert_eq!(split_finished("rkrk"), ("가".to_owned(), "rk"));
        assert_eq!(split_finished("ekfrk"), ("달".to_owned(), "rk"));
        assert_eq!(split_finished("rkqtl"), ("갑".to_owned(), "tl"));
        assert_eq!(split_finished("rr"), ("ㄱ".to_owned(), "r"));
        assert_eq!(split_finished("rhkd"), (String::new(), "rhkd"));
        assert_eq!(split_finished("kr"), ("ㅏ".to_owned(), "r"));
        assert_eq!(split_finished("dkssud"), ("안".to_owned(), "sud"));
        assert_eq!(split_finished(""), (String::new(), ""));
    }

    #[test]
    fn split_finished_into_preserves_prefix_and_borrows_open_keys() {
        let long_keys = "dkssudgktpdy".repeat(128);
        let long_finished = format!("{}안녕하세", "안녕하세요".repeat(127));
        for (keys, expected, open) in [
            ("", "", ""),
            ("123🙂'", "", "123🙂'"),
            ("rhkd", "", "rhkd"),
            ("ekfrk", "달", "rk"),
            ("rkqtl", "갑", "tl"),
            ("rr", "ㄱ", "r"),
            ("kk", "ㅏ", "k"),
            ("🙂r1krk!", "가", "rk!"),
            (long_keys.as_str(), long_finished.as_str(), "dy"),
        ] {
            let mut output = String::with_capacity(keys.len() * 3 + 16);
            output.push_str("前綴:");
            let pointer = output.as_ptr();
            let (rest, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                split_finished_into(keys, &mut output)
            });
            assert_eq!(output, format!("前綴:{expected}"));
            assert_eq!(output.as_ptr(), pointer);
            assert_eq!(rest, open);
            assert_eq!(rest.as_ptr(), keys[keys.len() - open.len()..].as_ptr());
            assert_eq!(allocations, 0);
        }

        let mut output = String::new();
        let (rest, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            split_finished_into(&long_keys, &mut output)
        });
        assert_eq!(output, long_finished);
        assert_eq!(rest, "dy");
        assert_eq!(allocations, 1, "长恢复输入的完成文本只预留一次");
    }

    #[test]
    fn every_syllable_round_trips_through_the_fold() {
        // Each of the 11172 syllables is typed through its jamo keys and must come back unchanged.
        let key_for_consonant = |jamo: char| -> &'static str {
            match jamo {
                'ㄱ' => "r",
                'ㄲ' => "R",
                'ㄴ' => "s",
                'ㄷ' => "e",
                'ㄸ' => "E",
                'ㄹ' => "f",
                'ㅁ' => "a",
                'ㅂ' => "q",
                'ㅃ' => "Q",
                'ㅅ' => "t",
                'ㅆ' => "T",
                'ㅇ' => "d",
                'ㅈ' => "w",
                'ㅉ' => "W",
                'ㅊ' => "c",
                'ㅋ' => "z",
                'ㅌ' => "x",
                'ㅍ' => "v",
                'ㅎ' => "g",
                'ㄳ' => "rt",
                'ㄵ' => "sw",
                'ㄶ' => "sg",
                'ㄺ' => "fr",
                'ㄻ' => "fa",
                'ㄼ' => "fq",
                'ㄽ' => "ft",
                'ㄾ' => "fx",
                'ㄿ' => "fv",
                'ㅀ' => "fg",
                'ㅄ' => "qt",
                _ => unreachable!("{jamo}"),
            }
        };
        const VOWEL_KEYS: [&str; 21] = [
            "k", "o", "i", "O", "j", "p", "u", "P", "h", "hk", "ho", "hl", "y", "n", "nj", "np",
            "nl", "b", "m", "ml", "l",
        ];
        for code in 0xAC00u32..=0xD7A3 {
            let offset = code - 0xAC00;
            let cho = CHOSEONG[(offset / (21 * 28)) as usize];
            let jung = ((offset / 28) % 21) as usize;
            let jong = (offset % 28) as usize;
            let mut keys = String::from(key_for_consonant(cho));
            keys.push_str(VOWEL_KEYS[jung]);
            if jong > 0 {
                keys.push_str(key_for_consonant(JONGSEONG[jong - 1]));
            }
            let expected = char::from_u32(code).unwrap().to_string();
            assert_eq!(compose(&keys), expected, "{keys}");
            assert_eq!(split_finished(&keys), (String::new(), keys.as_str()));
            let mut committed = String::from("前綴:");
            assert_eq!(split_finished_into(&keys, &mut committed), keys);
            assert_eq!(committed, "前綴:");
        }
    }
}
