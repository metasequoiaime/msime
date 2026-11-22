//! AI 拼音候选的原始输入、手动边界与逐字读音校验，不参与本地切分和候选排序。

use pinyin::ToPinyinMulti;
use serde_json::{json, Value};

use super::{AiError, AiInputMode, AiSuggestionRequest, AiSuggestionResponse};

pub fn valid_raw_input(input: &str) -> bool {
    !input.is_empty()
        && input.len() <= 256
        && input.bytes().any(|byte| byte.is_ascii_alphabetic())
        && input
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || byte == b'\'')
}

/// 偏移按去掉单引号后的原始字母计数，0 和末尾边界也保留。
pub fn explicit_boundaries(input: &str) -> Vec<usize> {
    let mut offset = 0;
    let mut boundaries = Vec::new();
    for byte in input.bytes() {
        if byte == b'\'' {
            if boundaries.last() != Some(&offset) {
                boundaries.push(offset);
            }
        } else {
            offset += 1;
        }
    }
    boundaries
}

pub fn user_message(request: &AiSuggestionRequest) -> Result<Value, AiError> {
    request.validate()?;
    let mut message = serde_json::to_value(request).map_err(|_| AiError::InvalidConfiguration)?;
    message["output_format"] = json!("json");
    if request.mode == AiInputMode::Pinyin && !request.input.is_empty() {
        message["explicit_boundaries"] = json!(explicit_boundaries(&request.input));
        if request.sentence_punctuation {
            message["punctuation_instruction"] = json!(super::SENTENCE_PUNCTUATION_PROMPT);
        }
    }
    Ok(message)
}

/// 只过滤当前请求的候选回复；翻译、聊天和英文纠错继续使用原有解析器。
pub fn parse_candidate_response(
    body: &[u8],
    request: &AiSuggestionRequest,
) -> Option<AiSuggestionResponse> {
    request.validate().ok()?;
    if request.mode == AiInputMode::English {
        return super::parse_chat_completion_response(body, request.candidate_limit);
    }
    let raw = if request.input.is_empty() {
        request.segmented_pinyin.join("'")
    } else {
        request.input.clone()
    };
    let letters = raw.replace('\'', "").to_ascii_lowercase();
    let boundaries = explicit_boundaries(&raw);
    let sentence_punctuation = request.sentence_punctuation && !request.input.is_empty();
    super::parse_chat_completion_with(body, request.candidate_limit, |entry, text| {
        let proposed: Vec<String>;
        let segments = if let Some(value) = entry.get("segmentation") {
            let Ok(parts) = serde_json::from_value::<Vec<String>>(value.clone()) else {
                return false;
            };
            proposed = parts;
            &proposed
        } else {
            &request.segmented_pinyin
        };
        covers_input(segments, &letters, &boundaries)
            && aligns_text(text, segments, &letters, sentence_punctuation)
    })
}

fn covers_input(segments: &[String], letters: &str, boundaries: &[usize]) -> bool {
    if segments.is_empty() || segments.len() > 128 {
        return false;
    }
    let mut offset = 0;
    for segment in segments {
        if segment.is_empty()
            || segment.len() > 32
            || !segment.bytes().all(|byte| byte.is_ascii_alphabetic())
        {
            return false;
        }
        let end = offset + segment.len();
        if boundaries
            .iter()
            .any(|boundary| offset < *boundary && *boundary < end)
            || !letters
                .get(offset..end)
                .is_some_and(|part| canonical(part) == canonical(&segment.to_ascii_lowercase()))
        {
            return false;
        }
        offset = end;
    }
    offset == letters.len()
}

/// 与输入法常见的无声调键入形式一致；不把普通 lu/nu 改成带变音的读音。
fn canonical(syllable: &str) -> String {
    let syllable = syllable.replace('ü', "v");
    match syllable.as_str() {
        "lue" => "lve".into(),
        "nue" => "nve".into(),
        "jv" => "ju".into(),
        "jve" => "jue".into(),
        "qv" => "qu".into(),
        "qve" => "que".into(),
        "xv" => "xu".into(),
        "xve" => "xue".into(),
        "yv" => "yu".into(),
        "yve" => "yue".into(),
        _ => syllable,
    }
}

fn aligns_text(text: &str, segments: &[String], letters: &str, sentence_punctuation: bool) -> bool {
    let mut remaining = text.trim();
    let mut index = 0;
    let mut raw_offset = 0;
    let mut previous_ascii = false;
    let mut previous_punctuation = false;
    while !remaining.is_empty() {
        if remaining.starts_with(' ') {
            remaining = remaining.trim_start_matches(' ');
            continue;
        }
        if remaining.as_bytes()[0].is_ascii_alphabetic() {
            let length = remaining
                .bytes()
                .take_while(u8::is_ascii_alphabetic)
                .count();
            let word = &remaining[..length];
            let mut offset = 0;
            while offset < word.len() {
                let Some(segment) = segments.get(index) else {
                    return false;
                };
                let end = offset + segment.len();
                let raw_end = raw_offset + segment.len();
                if !word.get(offset..end).is_some_and(|part| {
                    letters
                        .get(raw_offset..raw_end)
                        .is_some_and(|raw| part.eq_ignore_ascii_case(raw))
                }) {
                    return false;
                }
                offset = end;
                raw_offset = raw_end;
                index += 1;
            }
            remaining = &remaining[length..];
            previous_ascii = true;
        } else {
            let ch = remaining.chars().next().expect("非空候选片段");
            remaining = &remaining[ch.len_utf8()..];
            if matches!(ch, '，' | '。' | '？' | '！' | '；' | '：' | '、') {
                if !sentence_punctuation
                    || index == 0
                    || previous_punctuation
                    || (previous_ascii
                        && remaining
                            .trim_start_matches(' ')
                            .as_bytes()
                            .first()
                            .is_some_and(u8::is_ascii_alphabetic))
                {
                    return false;
                }
                previous_punctuation = true;
                continue;
            }
            let Some(segment) = segments.get(index) else {
                return false;
            };
            let expected = canonical(&segment.to_ascii_lowercase());
            if !ch.to_pinyin_multi().is_some_and(|readings| {
                readings
                    .into_iter()
                    .any(|reading| canonical(reading.plain()) == expected)
            }) {
                return false;
            }
            raw_offset += segment.len();
            index += 1;
            previous_ascii = false;
        }
        previous_punctuation = false;
    }
    index == segments.len()
}

#[cfg(test)]
mod tests;
