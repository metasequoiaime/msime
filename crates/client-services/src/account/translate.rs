//! `POST /v1/translate`：账号释义。请求按服务端的规则校验（msime-cloud `internal/server/handlers.go` 的 `translate`：最多 32 条，每条非空白、不超过 8192 字节，语言代码 2–16 个 `[A-Za-z-]`），整包不超过服务端 `decode` 的 65536 字节；响应逐项规范化，坏的一项只把那一项置为 `null`。
//!
//! 本 crate 自己发请求时走 [`AccountApi::translate`]；按键路径上自己发 HTTP 的宿主（Windows 的 curl、Linux）用 [`translate_request_body`] 拿请求体、用 [`parse_translate_response`] 解读响应，C 接口是 `msime_client_account_gloss`。

use super::*;
use serde_json::Value;

pub const TRANSLATE_PATH: &str = "/v1/translate";
/// 一次最多翻译的条数（服务端 `translationBatchLimit`）。
pub const MAX_TRANSLATE_TEXTS: usize = 32;
/// 每条原文的字节上限（服务端 `contract.TranslationInputBytes`）。
pub const MAX_TRANSLATE_TEXT_BYTES: usize = 8192;
/// 序列化后的请求体上限（服务端 `contract.JsonBodyBytes`）；32 × 8192 的最坏情况会超过它。
pub const MAX_TRANSLATE_REQUEST_BYTES: usize = 65536;
/// 每条译文的字节上限（服务端 `contract.OutputTextBytes`）。
const MAX_TRANSLATION_BYTES: usize = 65536;
/// 响应的上限：32 条最长的译文，转义后最多翻倍，再留出信封的余量。
pub(super) const MAX_TRANSLATE_RESPONSE_BYTES: usize =
    2 * MAX_TRANSLATE_TEXTS * MAX_TRANSLATION_BYTES + 4096;
/// `timeout_ms` 不给时的超时。
pub const DEFAULT_TRANSLATE_TIMEOUT_MS: u64 = 10_000;
/// `timeout_ms` 允许的范围。
pub const TRANSLATE_TIMEOUT_MS: std::ops::RangeInclusive<u64> = 1_000..=30_000;

/// 一次释义请求。`source_lang` 由调用方给，发出时与 `target_lang` 一起转成大写。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountTranslateRequest {
    pub texts: Vec<String>,
    pub source_lang: String,
    pub target_lang: String,
}

/// 释义结果，与请求的 `texts` 一一对应；规范化后不可用的一项是 `None`。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountTranslations {
    pub translations: Vec<Option<String>>,
}

/// 语言代码：2–16 个 ASCII 字母或 `-`，与服务端的 `language` 相同。
fn valid_language(value: &str) -> bool {
    (2..=16).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || byte == b'-')
}

/// 校验请求并给出要发的请求体（UTF-8 JSON，只用 `texts` 字段）。不合法是 `account_invalid`；序列化后超过 [`MAX_TRANSLATE_REQUEST_BYTES`] 是 `account_too_large`，不发出去换回服务端的 400 `invalid_json`。
pub fn translate_request_body(
    request: &AccountTranslateRequest,
) -> Result<Vec<u8>, AccountFailure> {
    if request.texts.is_empty()
        || request.texts.len() > MAX_TRANSLATE_TEXTS
        || request
            .texts
            .iter()
            .any(|text| text.trim().is_empty() || text.len() > MAX_TRANSLATE_TEXT_BYTES)
        || !valid_language(&request.source_lang)
        || !valid_language(&request.target_lang)
    {
        return Err(AccountError::Invalid.into());
    }
    #[derive(Serialize)]
    struct Body<'a> {
        texts: &'a [String],
        source_lang: String,
        target_lang: String,
    }
    let body = serde_json::to_vec(&Body {
        texts: &request.texts,
        source_lang: request.source_lang.to_ascii_uppercase(),
        target_lang: request.target_lang.to_ascii_uppercase(),
    })
    .map_err(|_| AccountError::Invalid)?;
    if body.len() > MAX_TRANSLATE_REQUEST_BYTES {
        return Err(AccountFailure::too_large());
    }
    Ok(body)
}

/// 解读一个 2xx 响应 `{"code":200,"data":[...]}`。`code` 必须是整数 200，`data` 必须与 `texts` 等长、每项是不超过 65536 字节的字符串，否则整个响应不可用（`account_unavailable`）。每项再规范化：制表符和换行折成空格后去掉首尾空白，含其他控制字符、为空或与原文相同的项变成 `None`，不让整批作废。
pub fn parse_translate_response(
    texts: &[String],
    body: &[u8],
) -> Result<AccountTranslations, AccountFailure> {
    #[derive(Deserialize)]
    struct Response {
        code: Value,
        data: Vec<String>,
    }
    let response: Response = serde_json::from_slice(body).map_err(|_| AccountError::Unavailable)?;
    // 字符串 "200" 和浮点数 200.0 都不认。
    if response.code.as_u64() != Some(200)
        || response.data.len() != texts.len()
        || response
            .data
            .iter()
            .any(|item| item.len() > MAX_TRANSLATION_BYTES)
    {
        return Err(AccountError::Unavailable.into());
    }
    let translations = response
        .data
        .into_iter()
        .zip(texts)
        .map(|(translation, source)| normalize_translation(&translation, source))
        .collect();
    Ok(AccountTranslations { translations })
}

/// 一项译文的规范化，见 [`parse_translate_response`]。
fn normalize_translation(translation: &str, source: &str) -> Option<String> {
    let folded: String = translation
        .chars()
        .map(|character| match character {
            '\t' | '\n' | '\r' => ' ',
            other => other,
        })
        .collect();
    let trimmed = folded.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_control) || trimmed == source.trim() {
        return None;
    }
    Some(trimmed.to_owned())
}

/// 宿主自己收到的释义响应：2xx 时按 [`parse_translate_response`] 解读，其余按 [`AccountFailure::from_http_response`] 映射。
pub fn translate_response(
    texts: &[String],
    status: u16,
    body: &[u8],
    retry_after: Option<&str>,
) -> Result<AccountTranslations, AccountFailure> {
    if (200..300).contains(&status) {
        parse_translate_response(texts, body)
    } else {
        Err(AccountFailure::from_http_response(
            status,
            body,
            retry_after,
        ))
    }
}

impl BackendAccountClient {
    pub(super) fn translate_texts(
        &self,
        request: &AccountTranslateRequest,
        timeout: Duration,
        access_token: &str,
    ) -> Result<AccountTranslations, AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let body = translate_request_body(request)?;
        let bytes = self.request_detailed(
            Method::POST,
            TRANSLATE_PATH,
            Some(access_token),
            Some(body),
            MAX_TRANSLATE_REQUEST_BYTES,
            MAX_TRANSLATE_RESPONSE_BYTES,
            timeout,
            "application/json",
        )?;
        parse_translate_response(&request.texts, &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(texts: &[&str]) -> AccountTranslateRequest {
        AccountTranslateRequest {
            texts: texts.iter().map(|text| (*text).to_owned()).collect(),
            source_lang: "zh".into(),
            target_lang: "en-us".into(),
        }
    }

    #[test]
    fn the_request_body_follows_the_server_rules() {
        let body = translate_request_body(&request(&["水杉", "输入法"])).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            serde_json::json!({ "texts": ["水杉", "输入法"], "source_lang": "ZH", "target_lang": "EN-US" })
        );
        let invalid =
            |value: AccountTranslateRequest| translate_request_body(&value).unwrap_err().code();
        assert_eq!(invalid(request(&[])), "account_invalid");
        assert_eq!(invalid(request(&["  "])), "account_invalid");
        assert_eq!(invalid(request(&vec!["词"; 33])), "account_invalid");
        let long = "a".repeat(MAX_TRANSLATE_TEXT_BYTES + 1);
        assert_eq!(invalid(request(&[long.as_str()])), "account_invalid");
        let mut bad_language = request(&["词"]);
        bad_language.target_lang = "e".into();
        assert_eq!(invalid(bad_language.clone()), "account_invalid");
        bad_language.target_lang = "en_US".into();
        assert_eq!(invalid(bad_language), "account_invalid");
        // 32 条各 8192 字节合法，但整包超过 64 KiB：本地报 too_large，不发出去。
        let full = "a".repeat(MAX_TRANSLATE_TEXT_BYTES);
        let texts = vec![full.as_str(); MAX_TRANSLATE_TEXTS];
        assert_eq!(invalid(request(&texts)), "account_too_large");
    }

    #[test]
    fn a_bad_item_only_nulls_that_item() {
        let texts: Vec<String> = ["苹果", "香蕉", "梨", "桃", "杏"]
            .map(String::from)
            .to_vec();
        let body = serde_json::json!({
            "code": 200,
            "data": [" apple\n", "香蕉", "bad\u{7}value", "\t", "peach\tblossom"],
        });
        let parsed = parse_translate_response(&texts, body.to_string().as_bytes()).unwrap();
        assert_eq!(
            parsed.translations,
            [
                Some("apple".to_owned()),
                None,
                None,
                None,
                Some("peach blossom".to_owned())
            ]
        );
    }

    #[test]
    fn a_malformed_envelope_fails_the_whole_response() {
        let texts = vec!["苹果".to_owned()];
        for body in [
            serde_json::json!({ "code": "200", "data": ["apple"] }),
            serde_json::json!({ "code": 200.0, "data": ["apple"] }),
            serde_json::json!({ "code": 0, "data": ["apple"] }),
            serde_json::json!({ "code": 200, "data": ["apple", "extra"] }),
            serde_json::json!({ "code": 200, "data": "apple" }),
            serde_json::json!({ "code": 200, "data": ["a".repeat(MAX_TRANSLATION_BYTES + 1)] }),
        ] {
            assert_eq!(
                parse_translate_response(&texts, body.to_string().as_bytes())
                    .unwrap_err()
                    .code(),
                "account_unavailable",
                "{body}"
            );
        }
    }

    #[test]
    fn host_received_failures_keep_their_status_and_retry() {
        let texts = vec!["苹果".to_owned()];
        let limited = translate_response(
            &texts,
            429,
            br#"{"error":{"code":"rate_limit_exceeded"}}"#,
            Some("60"),
        )
        .unwrap_err();
        assert_eq!(limited.code(), "account_rate_limited");
        assert_eq!(limited.retry_after_seconds, Some(60));
        let busy = translate_response(&texts, 503, br#"{"error":{"code":"server_busy"}}"#, None)
            .unwrap_err();
        assert_eq!(busy.code(), "account_rate_limited");
        let date = translate_response(&texts, 503, b"", Some("Wed, 21 Oct 2026 07:28:00 GMT"))
            .unwrap_err();
        assert_eq!(date.code(), "account_unavailable");
        assert_eq!(date.retry_after_seconds, None);
        assert_eq!(
            translate_response(&texts, 401, b"", None)
                .unwrap_err()
                .code(),
            "account_unauthorized"
        );
    }
}
