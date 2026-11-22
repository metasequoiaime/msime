//! 所有宿主共用的 AI 候选消息与回包协议；只做内存校验，不执行网络或会话变更。

use crate::*;
use msime_client_providers::ai::{self, AiInputMode, AiSuggestionRequest};
use msime_input_runtime::OnlineQuery;

pub(super) fn suggestion_request(query: &OnlineQuery, limit: u8) -> AiSuggestionRequest {
    let english = query.mode == msime_engine::OnlineInputMode::English;
    let input = if english {
        query.query_text.clone()
    } else if query.scheme == msime_engine::SchemeType::Quanpin as u8 {
        // 身份保留大小写和用户单引号；双拼原始按键不冒充全拼。
        query
            .identity
            .strip_prefix("0:")
            .unwrap_or_default()
            .to_owned()
    } else {
        String::new()
    };
    let sentence_punctuation = cfg!(target_os = "macos") && !english && !input.is_empty();
    AiSuggestionRequest {
        mode: if english {
            AiInputMode::English
        } else {
            AiInputMode::Pinyin
        },
        input,
        segmented_pinyin: query.pinyin_segments.clone(),
        context: query.ai_context.clone(),
        candidate_limit: limit,
        sentence_punctuation,
    }
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Message {
        query: OnlineQuery,
        limit: u8,
    },
    Parse {
        query: OnlineQuery,
        body: String,
        limit: u8,
    },
}

/// 消息包含原始输入，不可写入日志；解析结果是通过读音与输入覆盖校验的字符串数组或 null。
/// # Safety
/// `request` 指向 `length` 个可读字节；调用期间保持有效，函数不保留缓冲区。
#[no_mangle]
pub unsafe extern "C" fn msime_client_ai_candidate_protocol(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| unsafe {
        with_bounded_bytes(
            request,
            length,
            8 * 1024 * 1024,
            "invalid AI candidate document",
            |bytes| {
                let request: Request =
                    serde_json::from_slice(bytes).map_err(|_| "invalid AI candidate document")?;
                let (query, limit) = match &request {
                    Request::Message { query, limit } | Request::Parse { query, limit, .. } => {
                        (query, *limit)
                    }
                };
                if !query.ai_eligible || !(1..=10).contains(&limit) {
                    return Ok(Value::Null);
                }
                let suggestion = suggestion_request(query, limit);
                match request {
                    Request::Message { .. } => {
                        let message = ai::alignment::user_message(&suggestion)
                            .map_err(|_| "invalid AI candidate input")?;
                        let prompt = if suggestion.mode == AiInputMode::English {
                            ai::DEFAULT_ENGLISH_CANDIDATE_PROMPT
                        } else {
                            ai::DEFAULT_CANDIDATE_PROMPT
                        };
                        Ok(json!({"message":message,"default_prompt":prompt}))
                    }
                    Request::Parse { body, .. } => Ok(ai::alignment::parse_candidate_response(
                        body.as_bytes(),
                        &suggestion,
                    )
                    .map(|response| {
                        json!(response
                            .candidates
                            .into_iter()
                            .map(|row| row.text)
                            .collect::<Vec<_>>())
                    })
                    .unwrap_or(Value::Null)),
                }
            },
        )
    })
}
