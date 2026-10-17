//! `msime_client_ai_skin_generate` 和 `msime_client_ai_skin_job` 的失败形状：账号一侧的失败带 `retry_after_seconds`，服务端 503 和没拿到响应各有自己的码，AI 皮肤自己的码不变。不联网。

use super::*;
use msime_client_services::account::AccountFailure;
use std::ffi::CStr;

/// 一次失败经这两个入口的响应信封交出去的样子。
fn envelope(failure: AiSkinFailure) -> Value {
    let raw = account_failure_response(|| Err(ai_skin_failure(failure)));
    // SAFETY: 本库刚返回、尚未释放的字符串，只释放一次。
    unsafe {
        let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
        msime_client_string_free(raw);
        serde_json::from_str(&text).unwrap()
    }
}

fn server(status: u16, code: &str, retry_after: Option<&str>) -> AiSkinFailure {
    AiSkinFailure::Account(AccountFailure::from_http_response(
        status,
        json!({ "error": { "code": code, "message": code } })
            .to_string()
            .as_bytes(),
        retry_after,
    ))
}

#[test]
fn account_failures_keep_retry_after_and_tell_503_from_the_network() {
    assert_eq!(
        envelope(server(429, "rate_limit_exceeded", Some("7200"))),
        json!({ "ok": false, "error": "account_rate_limited", "retry_after_seconds": 7200 })
    );
    assert_eq!(
        envelope(server(503, "skin_jobs_busy", Some("5"))),
        json!({ "ok": false, "error": "account_unavailable", "retry_after_seconds": 5 })
    );
    assert_eq!(
        envelope(server(404, "skin_job_not_found", None)),
        json!({ "ok": false, "error": "account_not_found" })
    );
    assert_eq!(
        envelope(AiSkinFailure::Account(AccountFailure::network())),
        json!({ "ok": false, "error": "account_network" })
    );
    assert_eq!(
        envelope(AiSkinFailure::Cancelled),
        json!({ "ok": false, "error": "ai_skin_cancelled" })
    );
    assert_eq!(
        envelope(AiSkinFailure::InvalidResponse),
        json!({ "ok": false, "error": "ai_skin_invalid" })
    );
}
