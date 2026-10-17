//! 释义单条的长度上限（#8201 迁移计划 P8）：规范化后超过 4096 字节的一项置为 `null`，同一批里其余的照常返回；服务端 65536 字节的上限仍按整个响应不可用处理（见 `tests` 里的 `a_malformed_envelope_fails_the_whole_response`）。

use super::*;

fn parse(texts: &[&str], data: &[String]) -> Vec<Option<String>> {
    let texts: Vec<String> = texts.iter().map(|text| (*text).to_owned()).collect();
    let body = serde_json::json!({ "code": 200, "data": data });
    parse_translate_response(&texts, body.to_string().as_bytes())
        .unwrap()
        .translations
}

#[test]
fn a_translation_longer_than_the_host_limit_only_nulls_that_item() {
    let longest = "a".repeat(MAX_USABLE_TRANSLATION_BYTES);
    let too_long = "a".repeat(MAX_USABLE_TRANSLATION_BYTES + 1);
    assert_eq!(
        parse(
            &["苹果", "香蕉", "梨"],
            &["apple".to_owned(), too_long, longest.clone()]
        ),
        [Some("apple".to_owned()), None, Some(longest)]
    );
}

#[test]
fn the_limit_applies_to_the_normalized_translation() {
    // 首尾空白去掉以后正好 4096 字节，照样可用；多字节字符按字节算，4095 字节加一个三字节的字就超了。
    let padded = format!("\n {} \t", "b".repeat(MAX_USABLE_TRANSLATION_BYTES));
    let multibyte = format!("{}词", "c".repeat(MAX_USABLE_TRANSLATION_BYTES - 1));
    assert_eq!(
        parse(&["梨", "桃"], &[padded, multibyte]),
        [Some("b".repeat(MAX_USABLE_TRANSLATION_BYTES)), None]
    );
}

#[test]
fn a_host_received_response_uses_the_same_limit() {
    let texts = vec!["苹果".to_owned(), "香蕉".to_owned()];
    let body = serde_json::json!({
        "code": 200,
        "data": ["apple", "x".repeat(MAX_USABLE_TRANSLATION_BYTES + 1)],
    });
    assert_eq!(
        translate_response(&texts, 200, body.to_string().as_bytes(), None)
            .unwrap()
            .translations,
        [Some("apple".to_owned()), None]
    );
}
