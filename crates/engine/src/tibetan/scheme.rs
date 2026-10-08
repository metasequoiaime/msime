//! 藏文按键处理：组字是当前音节串的威利原文（区分大小写，`T` `D` `N` `Sh` `A` `I` `U` `M` `H` 都是拼写的一部分），显示是这段原文经 `ewts` crate 转换出的藏文。退格删一个原文按键，而不是一个显示出来的藏文字符。第一次 Esc 把显示锁定为原文（用来直接输入拉丁字母），第二次由会话取消组字。

use std::sync::OnceLock;

use crate::types::{QueryRequest, SchemeKey, SchemeType};
use ewts::EwtsConverter;

/// 空闲时 `handle_key` 接收的非字母键是 `'`（以 achung 起头的音节，如 `'od`）；`/` 不进组字，由会话把它变成垂符，列在这里是为了让宿主无论走字符还是标点路由都把它交给会话。
const SPELLING_SYMBOLS_IDLE: &str = "'/";

/// 组字时额外接收的拼写符号：`+` 叠写、`.` 消歧（`g.yag`）、`-` 反写元音（`-i`）。
const SPELLING_SYMBOLS_COMPOSING: &str = "'+-./";

/// 威利能拼写的大写字母：`A` `I` `U` 长元音、`D` `N` `T` 反写辅音、`Sh` 的 `S`、`H` 送气与 visarga、`M` anusvara、`R` `W` `Y` 下加字的特殊写法、`X` nuqta。其余大写字母（以及小写 `q` `x`）`ewts` 读不了，会原样以拉丁字母留在转换结果里。
const EWTS_UPPERCASE: &[u8] = b"ADHIMNRSTUWXY";

/// 字母能否出现在威利原文里：小写除 `q` `x` 外都可以，大写只有 `EWTS_UPPERCASE` 里的。
fn ewts_letter(letter: u8) -> bool {
    (letter.is_ascii_lowercase() && !matches!(letter, b'q' | b'x'))
        || EWTS_UPPERCASE.contains(&letter)
}

/// 转换表只建一次，所有会话共用。
fn converter() -> &'static EwtsConverter {
    static CONVERTER: OnceLock<EwtsConverter> = OnceLock::new();
    CONVERTER.get_or_init(EwtsConverter::create)
}

#[derive(Debug, Clone, Default)]
pub struct TibetanScheme {
    /// 当前音节串的威利原文，保留大小写。
    raw: String,
    /// Esc 要求显示原文：显示就是 `raw` 本身，之后的按键原样追加。
    raw_locked: bool,
}

impl TibetanScheme {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.raw.clear();
        self.raw_locked = false;
    }

    pub fn is_composing(&self) -> bool {
        !self.raw.is_empty()
    }

    pub fn raw_locked(&self) -> bool {
        self.raw_locked
    }

    /// 威利能拼写的字母（见 `claims_letter`）总是拼写；`'` 随时拼写；`+` `.` `-` 只在组字时拼写；退格删掉最后一个按键。其余按键一律忽略，由会话决定如何结束组字。
    pub fn handle_key(&mut self, key: SchemeKey) -> bool {
        match key {
            SchemeKey::Letter(letter) if self.claims_letter(letter) => {
                self.raw.push(char::from(letter));
                true
            }
            SchemeKey::Symbol(symbol) if self.claims_symbol(symbol) => {
                self.raw.push(char::from(symbol));
                true
            }
            SchemeKey::Backspace => {
                let changed = self.raw.pop().is_some();
                if self.raw.is_empty() {
                    self.raw_locked = false;
                }
                changed
            }
            SchemeKey::Letter(_)
            | SchemeKey::Symbol(_)
            | SchemeKey::Apostrophe
            | SchemeKey::Semicolon
            | SchemeKey::Minus
            | SchemeKey::Requery => false,
        }
    }

    /// Esc：第一次把显示切换为原文并返回 true；没有组字或原文已在显示时返回 false，由会话取消组字。
    pub fn restore_raw(&mut self) -> bool {
        if self.raw.is_empty() || self.raw_locked {
            return false;
        }
        self.raw_locked = true;
        true
    }

    /// 当前状态下宿主应当作字符交给会话的非字母键（`SessionSnapshot::spelling_symbols`）：`handle_key` 接收的拼写符号，加上由会话处理成垂符的 `/`。
    pub fn spelling_symbols(&self) -> &'static str {
        if self.is_composing() {
            SPELLING_SYMBOLS_COMPOSING
        } else {
            SPELLING_SYMBOLS_IDLE
        }
    }

    /// 组字上屏时的文本：转换出的藏文；Esc 锁定后是原文本身。转换结果为空（只剩不产生字符的符号）时退回原文，保证有组字就有显示。
    pub fn preedit(&self) -> String {
        let mut display = String::new();
        self.preedit_into(&mut display);
        display
    }

    pub fn preedit_into(&self, display: &mut String) {
        display.clear();
        if self.raw_locked {
            display.push_str(&self.raw);
            return;
        }
        // `ewts` 不认识梵文的 `kSh`（ཀྵ），只认识显式叠写的 `k+Sh`；只改送去转换的副本，原文按键不变，退格仍删一个按键。
        let converted = if self.raw.contains("kSh") {
            converter().ewts_to_unicode(&self.raw.replace("kSh", "k+Sh"))
        } else {
            converter().ewts_to_unicode(&self.raw)
        };
        if converted.is_empty() {
            display.push_str(&self.raw);
            return;
        }
        display.push_str(&converted);
    }

    /// 原文作为 raw input（大小写保留在 `raw_input_with_cases`，宿主编辑和临时方案据此重建同一串），显示作为切分。仅在组字时有效；没有任何 provider 回答它。
    pub fn build_request(&self) -> QueryRequest {
        let mut request = QueryRequest::default();
        self.build_request_into(&mut request);
        request
    }

    pub fn build_request_into(&self, request: &mut QueryRequest) {
        request.scheme = SchemeType::Tibetan;
        request.raw_input.clear();
        request.raw_input.extend(
            self.raw
                .bytes()
                .map(|byte| char::from(byte.to_ascii_lowercase())),
        );
        request.raw_input_with_cases.clone_from(&self.raw);
        request.normalized_input.clone_from(&request.raw_input);
        request.raw_segmentation.clone_from(&self.raw);
        self.preedit_into(&mut request.normalized_segmentation);
        request
            .segmentation
            .clone_from(&request.normalized_segmentation);
        request.valid = self.is_composing();
    }

    /// 只保留 `handle_key` 会接收的按键，宿主给了带大小写的串时优先用它；按键变了，原文锁定随之解除。
    pub fn set_raw_input(&mut self, raw: &str, raw_with_cases: &str) {
        let source = if raw_with_cases.is_empty() {
            raw
        } else {
            raw_with_cases
        };
        self.reset();
        self.raw.reserve(source.len());
        for key in source.bytes() {
            if self.claims_letter(key) || self.claims_symbol(key) {
                self.raw.push(char::from(key));
            }
        }
    }

    /// 字母是否进入原文：Esc 锁定原文后任何 ASCII 字母都原样追加；否则只接收威利能拼写的字母，其余字母（如大写锁定时的 `B` `O`）不接收，由会话先上屏音节串再原样写出该字母，不让拉丁字母混进藏文转换结果。
    pub fn claims_letter(&self, letter: u8) -> bool {
        if self.raw_locked {
            letter.is_ascii_alphabetic()
        } else {
            ewts_letter(letter)
        }
    }

    /// `/` 不在其中：它结束组字，从不进入原文。
    fn claims_symbol(&self, key: u8) -> bool {
        key == b'\'' || (self.is_composing() && matches!(key, b'+' | b'.' | b'-'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 按会话发送的方式输入 `keys`：字母作为 `Letter`，其余作为 `Symbol`。
    fn typed(keys: &str) -> TibetanScheme {
        let mut scheme = TibetanScheme::new();
        for key in keys.bytes() {
            scheme.handle_key(if key.is_ascii_alphabetic() {
                SchemeKey::Letter(key)
            } else {
                SchemeKey::Symbol(key)
            });
        }
        scheme
    }

    fn wylie(keys: &str) -> String {
        typed(keys).preedit()
    }

    #[test]
    fn syllables_convert_to_tibetan() {
        let cases = [
            ("bkra", "བཀྲ"),
            ("shis", "ཤིས"),
            ("sangs", "སངས"),
            ("rgyas", "རྒྱས"),
            ("bsgrubs", "བསྒྲུབས"),
            ("'od", "འོད"),
            ("'gro", "འགྲོ"),
            ("g.yag", "གཡག"),
            ("gyag", "གྱག"),
            ("pa'i", "པའི"),
        ];
        for (keys, expected) in cases {
            assert_eq!(wylie(keys), expected, "{keys}");
        }
    }

    #[test]
    fn plus_stacks_and_uppercase_spells_sanskrit_letters_and_long_vowels() {
        let cases = [
            ("pad+ma", "པདྨ"),
            ("badz+ra", "བཛྲ"),
            ("Ta", "ཊ"),
            ("Da", "ཌ"),
            ("Na", "ཎ"),
            ("Sha", "ཥ"),
            ("kA", "ཀཱ"),
            ("kI", "ཀཱི"),
            ("kU", "ཀཱུ"),
            ("oM", "ཨོཾ"),
            ("aH", "ཨཿ"),
            ("k-i", "ཀྀ"),
            ("kSha", "ཀྵ"),
            ("k+Sha", "ཀྵ"),
            ("lakSh+mI", "ལཀྵྨཱི"),
        ];
        for (keys, expected) in cases {
            assert_eq!(wylie(keys), expected, "{keys}");
        }
    }

    #[test]
    fn letters_ewts_cannot_spell_are_not_claimed() {
        // 大写锁定打出的 `BOD`：`B` `O` 不进原文，只有威利能读的 `D` 进入，转换结果里没有拉丁字母。
        let scheme = typed("BOD");
        assert_eq!(scheme.raw, "D");
        assert_eq!(scheme.preedit(), "ཌ");
        for letter in b'a'..=b'z' {
            assert_eq!(
                scheme.claims_letter(letter),
                !matches!(letter, b'q' | b'x'),
                "{}",
                char::from(letter)
            );
        }
        for letter in b'A'..=b'Z' {
            let claimed = scheme.claims_letter(letter);
            assert_eq!(
                claimed,
                b"ADHIMNRSTUWXY".contains(&letter),
                "{}",
                char::from(letter)
            );
            // 接收的大写字母跟在 `k` 后面都能被 `ewts` 读成藏文（`S` 要配 `h`）。
            if claimed {
                let keys = if letter == b'S' {
                    "kSha".to_owned()
                } else {
                    format!("k{}", char::from(letter))
                };
                assert!(
                    !wylie(&keys).chars().any(|c| c.is_ascii_alphabetic()),
                    "{keys}"
                );
            }
        }
        assert_eq!(typed("kq").raw, "k");
        assert_eq!(typed("kx").raw, "k");
        // Esc 锁定原文后任何字母都原样追加。
        let mut scheme = typed("ka");
        assert!(scheme.restore_raw());
        scheme.handle_key(SchemeKey::Letter(b'Q'));
        assert_eq!(scheme.preedit(), "kaQ");
    }

    #[test]
    fn backspace_removes_one_wylie_keystroke() {
        let mut scheme = typed("bkra");
        let mut shown = vec![scheme.preedit()];
        while scheme.is_composing() {
            scheme.handle_key(SchemeKey::Backspace);
            shown.push(scheme.preedit());
        }
        // `ewts` 把没有元音的 `bkr` 也读成下加字，所以删掉 `a` 后显示不变，原文却少了一个按键。
        assert_eq!(shown, ["བཀྲ", "བཀྲ", "བཀ", "བ", ""]);
        let mut scheme = typed("bkra");
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.raw, "bkr");
    }

    #[test]
    fn symbols_other_than_the_apostrophe_spell_only_while_composing() {
        let mut scheme = TibetanScheme::new();
        assert_eq!(scheme.spelling_symbols(), "'/");
        for key in *b"+.-/" {
            scheme.handle_key(SchemeKey::Symbol(key));
            assert!(!scheme.is_composing(), "{}", char::from(key));
        }
        scheme.handle_key(SchemeKey::Symbol(b'\''));
        assert_eq!(scheme.raw, "'");
        assert_eq!(scheme.spelling_symbols(), "'+-./");
        // `/` 结束组字，从不进入原文。
        scheme.handle_key(SchemeKey::Symbol(b'/'));
        assert_eq!(scheme.raw, "'");
    }

    #[test]
    fn esc_locks_the_raw_keys_and_a_second_esc_is_left_to_the_session() {
        let mut scheme = typed("bod");
        assert_eq!(scheme.preedit(), "བོད");
        assert!(scheme.restore_raw());
        assert!(scheme.raw_locked());
        assert_eq!(scheme.preedit(), "bod");
        scheme.handle_key(SchemeKey::Letter(b's'));
        assert_eq!(scheme.preedit(), "bods");
        assert!(!scheme.restore_raw());
        for _ in 0..4 {
            scheme.handle_key(SchemeKey::Backspace);
        }
        assert!(!scheme.raw_locked());
        scheme.handle_key(SchemeKey::Letter(b'k'));
        assert_eq!(scheme.preedit(), "ཀ");
        assert!(!TibetanScheme::new().restore_raw());
    }

    #[test]
    fn other_keys_are_ignored() {
        let mut scheme = typed("ka");
        for key in [
            SchemeKey::Apostrophe,
            SchemeKey::Semicolon,
            SchemeKey::Minus,
            SchemeKey::Requery,
            SchemeKey::Letter(b'1'),
            SchemeKey::Symbol(b' '),
            SchemeKey::Symbol(b'1'),
            SchemeKey::Symbol(b';'),
        ] {
            scheme.handle_key(key);
        }
        assert_eq!(scheme.raw, "ka");
    }

    #[test]
    fn set_raw_input_prefers_the_cased_spelling_and_drops_the_lock() {
        let mut scheme = typed("abc");
        assert!(scheme.restore_raw());
        scheme.set_raw_input("ta", "Ta 1/");
        assert!(!scheme.raw_locked());
        assert_eq!(scheme.raw, "Ta");
        assert_eq!(scheme.preedit(), "ཊ");
        scheme.set_raw_input("+.pad+ma", "");
        assert_eq!(scheme.raw, "pad+ma");
        assert_eq!(scheme.preedit(), "པདྨ");
        scheme.set_raw_input("", "BOD");
        assert_eq!(scheme.raw, "D");
    }

    #[test]
    fn set_raw_input_reserves_source_capacity() {
        let source: String = (0..100)
            .map(|index| if index % 5 == 3 { '_' } else { 'a' })
            .collect();
        let mut scheme = TibetanScheme::new();
        scheme.set_raw_input(&source, "");
        assert_eq!(scheme.raw.len(), 80);
        assert_eq!(scheme.raw.capacity(), source.len());
    }

    #[test]
    fn request_carries_the_keys_as_raw_input_and_the_display_as_segmentation() {
        let request = typed("Sha").build_request();
        assert_eq!(request.scheme, SchemeType::Tibetan);
        assert_eq!(request.raw_input, "sha");
        assert_eq!(request.raw_input_with_cases, "Sha");
        assert_eq!(request.normalized_input, "sha");
        assert_eq!(request.raw_segmentation, "Sha");
        assert_eq!(request.segmentation, "ཥ");
        assert_eq!(request.normalized_segmentation, "ཥ");
        assert!(request.valid);
        assert!(!TibetanScheme::default().build_request().valid);
    }
}
