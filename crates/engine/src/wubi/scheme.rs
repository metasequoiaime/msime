//! 五笔按键处理（`R/schemes/wubi_scheme.*`）：字母 `a..=y`，`z` 只在允许混输拼音时收；最多四个字母，混输的拼音回退正在延长组字时最多 32 个。混输关闭时，组字开头的 `z` 打开临时拼音反查：之后的字母都是拼音，最多 32 个。

use crate::types::{QueryRequest, SchemeKey, SchemeType};

pub const MAX_CODE_LENGTH: usize = 4;
/// Long enough for any pinyin spelling, short enough to stay a composition rather than a paragraph.
pub const MAX_MIXED_CODE_LENGTH: usize = 32;

#[derive(Debug, Clone, Default)]
pub struct WubiScheme {
    raw: String,
    /// No wubi code uses `z`, but mixed input has to accept it: dropping it would not refuse a pinyin spelling, it would silently turn `zhongguo` into `hongguo`. It follows the setting rather than the last query because the letter can open a composition.
    mixed_pinyin_allowed: bool,
    /// A fifth letter is refused unless mixed input found that the table cannot answer the code in hand, so `nihao` can be finished while a fluent wubi typist sees no change.
    extended_length_allowed: bool,
    /// 混输关闭时由会话打开：组字以 `z` 开头就是临时拼音反查。五笔码不用 `z`，混输关闭时这个键原本被拒绝、交回宿主，所以占用它不会挡住任何五笔码。混输打开时 `z` 已经是拼音字母，会话会把这里关掉。
    pinyin_lookup_allowed: bool,
}

/// 临时拼音反查的引导键。
pub const PINYIN_LOOKUP_PREFIX: u8 = b'z';

fn is_wubi_letter(lower: u8, mixed_pinyin_allowed: bool) -> bool {
    (b'a'..=b'y').contains(&lower) || (mixed_pinyin_allowed && lower == b'z')
}

impl WubiScheme {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears the code and the extended-length allowance.
    pub fn reset(&mut self) {
        self.raw.clear();
        self.extended_length_allowed = false;
    }

    /// Keys beyond the length limit or outside the alphabet are ignored, which the session reports as unhandled.
    pub fn handle_key(&mut self, key: SchemeKey) -> bool {
        match key {
            SchemeKey::Backspace => self.raw.pop().is_some(),
            SchemeKey::Letter(letter) if letter.is_ascii_alphabetic() => {
                let lower = letter.to_ascii_lowercase();
                let opens_lookup = self.pinyin_lookup_allowed
                    && self.raw.is_empty()
                    && lower == PINYIN_LOOKUP_PREFIX;
                // 反查里的字母是拼音，`z` 和其余字母一样收下。
                let accepted = opens_lookup
                    || self.in_pinyin_lookup()
                    || is_wubi_letter(lower, self.mixed_pinyin_allowed);
                if accepted && self.raw.len() < self.max_code_length() {
                    self.raw.push(char::from(lower));
                    true
                } else {
                    false
                }
            }
            SchemeKey::Letter(_)
            | SchemeKey::Apostrophe
            | SchemeKey::Semicolon
            | SchemeKey::Minus
            | SchemeKey::Symbol(_)
            | SchemeKey::Requery => false,
        }
    }

    /// Every string field is the raw code; valid when non-empty.
    pub fn build_request(&self) -> QueryRequest {
        let mut request = QueryRequest::default();
        self.build_request_into(&mut request);
        request
    }

    /// 将五笔请求写入已有存储，避免逐键刷新重复克隆同一码字符串。
    pub fn build_request_into(&self, request: &mut QueryRequest) {
        request.scheme = SchemeType::Wubi;
        request.raw_input.clone_from(&self.raw);
        request.raw_input_with_cases.clone_from(&self.raw);
        request.normalized_input.clone_from(&self.raw);
        request.raw_segmentation.clone_from(&self.raw);
        request.normalized_segmentation.clone_from(&self.raw);
        request.segmentation.clone_from(&self.raw);
        request.valid = !self.raw.is_empty();
    }

    pub fn preedit(&self) -> String {
        self.raw.clone()
    }

    /// 宿主编辑：转小写、只留字母表里的字母、按当前上限截断。C++ 在 `raw_input_with_cases` 非空时读它；转小写后两种拼写给出同一个码，传哪个都行。反查允许时，第一个字母是 `z` 的文本整段按反查收：字母全部保留，上限 32。
    pub fn set_raw_input(&mut self, raw: &str) {
        let lookup = self.pinyin_lookup_allowed
            && raw
                .bytes()
                .find(u8::is_ascii_alphabetic)
                .is_some_and(|first| first.to_ascii_lowercase() == PINYIN_LOOKUP_PREFIX);
        // 上限由新文本决定，不能读旧组字：旧组字是反查时 `max_code_length` 会给出 32。
        let limit = if lookup || self.extended_length_allowed {
            MAX_MIXED_CODE_LENGTH
        } else {
            MAX_CODE_LENGTH
        };
        let mixed_pinyin_allowed = self.mixed_pinyin_allowed;
        self.raw.clear();
        self.raw.reserve(limit.min(raw.len()));
        self.raw.extend(
            raw.bytes()
                .map(|byte| byte.to_ascii_lowercase())
                .filter(|&lower| {
                    if lookup {
                        lower.is_ascii_lowercase()
                    } else {
                        is_wubi_letter(lower, mixed_pinyin_allowed)
                    }
                })
                .take(limit)
                .map(char::from),
        );
    }

    /// 正好四个字母。更长的混输拼音是回退查询，不是完整的五笔码；反查里的字母是拼音，四个字母也不是完整五笔码。
    pub fn has_complete_code(&self) -> bool {
        self.raw.len() == MAX_CODE_LENGTH && !self.in_pinyin_lookup()
    }

    pub fn set_mixed_pinyin_allowed(&mut self, allowed: bool) {
        self.mixed_pinyin_allowed = allowed;
    }

    /// 打开或关闭 `z` 临时拼音反查。只决定下一次组字能否以 `z` 开头，已在组字中的文本不变。
    pub fn set_pinyin_lookup_allowed(&mut self, allowed: bool) {
        self.pinyin_lookup_allowed = allowed;
    }

    /// 当前组字是临时拼音反查：反查允许且组字以 `z` 开头。
    pub fn in_pinyin_lookup(&self) -> bool {
        self.pinyin_lookup_allowed && self.raw.as_bytes().first() == Some(&PINYIN_LOOKUP_PREFIX)
    }

    pub fn set_extended_length_allowed(&mut self, allowed: bool) {
        self.extended_length_allowed = allowed;
    }

    pub fn extended_length_allowed(&self) -> bool {
        self.extended_length_allowed
    }

    fn max_code_length(&self) -> usize {
        if self.extended_length_allowed || self.in_pinyin_lookup() {
            MAX_MIXED_CODE_LENGTH
        } else {
            MAX_CODE_LENGTH
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(scheme: &mut WubiScheme, text: &str) {
        for byte in text.bytes() {
            let key = match byte {
                b'\'' => SchemeKey::Apostrophe,
                _ => SchemeKey::Letter(byte),
            };
            scheme.handle_key(key);
        }
    }

    // test_input_session.cpp:775-782: z, a fifth letter and an apostrophe are refused.
    #[test]
    fn refuses_z_a_fifth_letter_and_apostrophes() {
        let mut scheme = WubiScheme::new();
        typed(&mut scheme, "z");
        assert_eq!(scheme.preedit(), "");
        typed(&mut scheme, "abcd");
        typed(&mut scheme, "e");
        assert_eq!(scheme.preedit(), "abcd");
        typed(&mut scheme, "'");
        assert_eq!(scheme.preedit(), "abcd");
        assert!(scheme.has_complete_code());
    }

    #[test]
    fn symbol_keys_are_ignored() {
        let mut scheme = WubiScheme::new();
        typed(&mut scheme, "ab");
        scheme.handle_key(SchemeKey::Symbol(b';'));
        scheme.handle_key(SchemeKey::Symbol(b' '));
        assert_eq!(scheme.preedit(), "ab");
    }

    #[test]
    fn uppercase_letters_are_lowercased() {
        let mut scheme = WubiScheme::new();
        typed(&mut scheme, "WQ");
        assert_eq!(scheme.preedit(), "wq");
    }

    #[test]
    fn mixed_pinyin_accepts_z_and_extended_length_lifts_the_limit() {
        let mut scheme = WubiScheme::new();
        scheme.set_mixed_pinyin_allowed(true);
        typed(&mut scheme, "zhongguo");
        assert_eq!(scheme.preedit(), "zhon");
        assert!(scheme.has_complete_code());

        scheme.set_extended_length_allowed(true);
        typed(&mut scheme, "gguo");
        assert_eq!(scheme.preedit(), "zhongguo");
        assert!(!scheme.has_complete_code());

        // Reset drops the allowance but keeps the setting-driven z.
        scheme.reset();
        assert_eq!(scheme.preedit(), "");
        typed(&mut scheme, "zhongguo");
        assert_eq!(scheme.preedit(), "zhon");
    }

    #[test]
    fn a_leading_z_opens_the_pinyin_lookup() {
        let mut scheme = WubiScheme::new();
        scheme.set_pinyin_lookup_allowed(true);
        typed(&mut scheme, "zzhongguo");
        assert_eq!(scheme.preedit(), "zzhongguo");
        assert!(scheme.in_pinyin_lookup());
        assert!(!scheme.has_complete_code());

        // 反查最多 32 个字母，与混输拼音的上限相同。
        scheme.reset();
        typed(&mut scheme, &"z".repeat(40));
        assert_eq!(scheme.preedit().len(), MAX_MIXED_CODE_LENGTH);

        // 四个字母的反查不是完整五笔码。
        scheme.reset();
        typed(&mut scheme, "zhao");
        assert!(!scheme.has_complete_code());

        // 只有组字开头的 `z` 引导反查；五笔码中间的 `z` 照旧拒绝。
        scheme.reset();
        typed(&mut scheme, "wqz");
        assert_eq!(scheme.preedit(), "wq");
        assert!(!scheme.in_pinyin_lookup());

        // 退格删掉 `z` 就离开反查。
        scheme.reset();
        typed(&mut scheme, "zh");
        scheme.handle_key(SchemeKey::Backspace);
        assert!(scheme.in_pinyin_lookup());
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.preedit(), "");
        assert!(!scheme.in_pinyin_lookup());
        typed(&mut scheme, "abcde");
        assert_eq!(scheme.preedit(), "abcd");
    }

    #[test]
    fn the_pinyin_lookup_stays_closed_unless_allowed() {
        let mut scheme = WubiScheme::new();
        typed(&mut scheme, "zhao");
        assert_eq!(scheme.preedit(), "hao");
        assert!(!scheme.in_pinyin_lookup());

        // 混输里的 `z` 是拼音字母，不是反查。
        let mut mixed = WubiScheme::new();
        mixed.set_mixed_pinyin_allowed(true);
        typed(&mut mixed, "zhao");
        assert_eq!(mixed.preedit(), "zhao");
        assert!(!mixed.in_pinyin_lookup());
        assert!(mixed.has_complete_code());
    }

    #[test]
    fn set_raw_input_keeps_a_lookup_spelling_whole() {
        let mut scheme = WubiScheme::new();
        scheme.set_pinyin_lookup_allowed(true);
        scheme.set_raw_input("Z'zhong-guo");
        assert_eq!(scheme.preedit(), "zzhongguo");
        assert!(scheme.in_pinyin_lookup());

        // 不以 `z` 开头的文本仍是五笔码：`z` 被滤掉，四码截断。
        scheme.set_raw_input("nihaoz");
        assert_eq!(scheme.preedit(), "niha");
        assert!(!scheme.in_pinyin_lookup());
    }

    #[test]
    fn backspace_and_return() {
        let mut scheme = WubiScheme::new();
        typed(&mut scheme, "wq");
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.preedit(), "w");
        scheme.handle_key(SchemeKey::Backspace);
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.preedit(), "");
        typed(&mut scheme, "ab");
        scheme.reset();
        assert_eq!(scheme.preedit(), "");
    }

    #[test]
    fn set_raw_input_normalises_and_clips() {
        let mut scheme = WubiScheme::new();
        scheme.set_raw_input("A'b-Z9cdef");
        assert_eq!(scheme.preedit(), "abcd");

        scheme.set_mixed_pinyin_allowed(true);
        scheme.set_extended_length_allowed(true);
        scheme.set_raw_input("Ni'Hao");
        assert_eq!(scheme.preedit(), "nihao");
    }

    #[test]
    fn set_raw_input_reuses_existing_storage() {
        let mut scheme = WubiScheme::new();
        scheme.set_mixed_pinyin_allowed(true);
        scheme.set_extended_length_allowed(true);
        scheme.set_raw_input("abcdefghijklmnopqrstuvwxyzabcdef");
        let capacity = scheme.raw.capacity();

        scheme.set_raw_input("ab");

        assert_eq!(scheme.preedit(), "ab");
        assert!(scheme.raw.capacity() >= capacity);
    }

    #[test]
    fn request_carries_the_code_in_every_field() {
        let mut scheme = WubiScheme::new();
        assert!(!scheme.build_request().valid);
        typed(&mut scheme, "wq");
        let request = scheme.build_request();
        assert_eq!(request.scheme, SchemeType::Wubi);
        assert!(request.valid);
        for field in [
            &request.raw_input,
            &request.raw_input_with_cases,
            &request.normalized_input,
            &request.raw_segmentation,
            &request.normalized_segmentation,
            &request.segmentation,
        ] {
            assert_eq!(field, "wq");
        }
    }
}
