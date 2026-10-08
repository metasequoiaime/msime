//! UTF-8 and Han character helpers several modules share. They reproduce the C++ helpers' exact counting rules, including the ones whose names mislead.

/// Han code points as the helpcode tables and candidate edges see them: U+3007, the CJK blocks and extensions up to U+323AF.
pub fn is_han_code_point(code_point: u32) -> bool {
    matches!(code_point, 0x3007 | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F | 0x30000..=0x323AF)
}

pub fn is_han(character: char) -> bool {
    is_han_code_point(character as u32)
}

/// The first Han character of `text`, empty when there is none.
pub fn first_han_char(text: &str) -> &str {
    text.char_indices()
        .find(|(_, c)| is_han(*c))
        .map_or("", |(at, c)| &text[at..at + c.len_utf8()])
}

/// The last Han character of `text`, empty when there is none.
pub fn last_han_char(text: &str) -> &str {
    text.char_indices()
        .rev()
        .find(|(_, c)| is_han(*c))
        .map_or("", |(at, c)| &text[at..at + c.len_utf8()])
}

/// Character count by UTF-8 lead byte, whatever the characters are. The C++ `count_han_chars` counts this way despite its name, and callers use it as a character count (shuangpin doubles it into a key count), so the port keeps the rule rather than a Han filter.
pub fn count_han_chars(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count = 0;
    let mut at = 0;
    while at < bytes.len() {
        let lead = bytes[at];
        at += if lead >= 0xF0 {
            4
        } else if lead >= 0xE0 {
            3
        } else if lead >= 0xC0 {
            2
        } else {
            1
        };
        count += 1;
    }
    count
}

pub fn count_utf8_chars(text: &str) -> usize {
    text.chars().count()
}

/// Non-empty and every character Han.
pub fn is_all_han(text: &str) -> bool {
    !text.is_empty() && text.chars().all(is_han)
}

/// 含汉字且不止一个字符：「只出单字」要去掉的词和整句（`你好`、`T恤`）。单个汉字、英文、表情和颜文字都不算。
pub fn is_han_phrase(text: &str) -> bool {
    text.chars().nth(1).is_some() && text.chars().any(is_han)
}

/// The last `count` Unicode scalars of `text`; empty for zero.
pub fn last_characters(text: &str, count: usize) -> &str {
    if count == 0 {
        return "";
    }
    match text.char_indices().rev().nth(count - 1) {
        Some((at, _)) => &text[at..],
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_match_the_reference_rules() {
        assert_eq!(first_han_char("a你好"), "你");
        assert_eq!(last_han_char("你好b"), "好");
        assert_eq!(first_han_char("abc"), "");
        assert_eq!(count_han_chars("a你𠀀"), 3);
        assert_eq!(last_characters("a你好", 2), "你好");
        assert_eq!(last_characters("a你好", 0), "");
        assert_eq!(last_characters("好", 5), "好");
        assert!(is_han_phrase("你好"));
        assert!(is_han_phrase("T恤"));
        assert!(!is_han_phrase("你"));
        assert!(!is_han_phrase("𠀀"));
        assert!(!is_han_phrase("GitHub"));
        assert!(!is_han_phrase("(^_^)"));
        assert!(is_all_han("你好"));
        assert!(!is_all_han("你a"));
    }
}
