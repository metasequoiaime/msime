//! 由宿主自己读写回环 socket 的 Google 登录（鸿蒙：ArkTS 的 TCPSocketServer）。宿主负责监听、打开浏览器、经自己的 HTTPS 栈申请 challenge 和提交授权码；这里给出与桌面端 `AccountSession::sign_in_google_with_browser` 相同的判定：回跳地址的形状、授权链接是否可以打开、等多久、一次回环请求是不是回跳，以及写回浏览器的页面。
//!
//! Part of the C ABI; see the parent module for what these shims guarantee.

use super::with_bounded_bytes;
use crate::*;
use msime_client_core::account::{
    google_loopback_plan, google_loopback_reply, google_loopback_target, GoogleCallback,
    GOOGLE_CALLBACK_IO_TIMEOUT, GOOGLE_CALLBACK_MAX_REQUEST_BYTES,
};

/// 请求的上限：一次回跳的请求头最多 8 KiB，加上 state 和 JSON 的转义还有余量。
const MAX_REQUEST_BYTES: usize = 32 * 1024;

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum GoogleLoopbackRequest {
    /// 监听在 `port` 上时，向后端申请 challenge 用的回跳地址。
    Target { port: u16 },
    /// 打开浏览器之前，校验后端给的授权链接。
    Plan {
        authorization_url: String,
        target: String,
        expires_in: u64,
    },
    /// 一次回环连接读到的请求头（没读到完整请求头时为 null）。
    Reply { head: Option<String>, state: String },
}

/// Google 登录的回环判定，供自己读写 socket 的宿主使用。操作：`target`、`plan`、`reply`，各自的请求与返回值见头文件。
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_google_loopback(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: forwarded from the documented caller contract.
        let request: GoogleLoopbackRequest = unsafe {
            with_bounded_bytes(
                request,
                length,
                MAX_REQUEST_BYTES,
                "invalid request buffer",
                |bytes| {
                    serde_json::from_slice(bytes).map_err(|_| "invalid request document".to_owned())
                },
            )?
        };
        match request {
            GoogleLoopbackRequest::Target { port } => {
                if port < 1024 {
                    return Err("account_unavailable".to_owned());
                }
                Ok(json!({ "target": google_loopback_target(port) }))
            }
            GoogleLoopbackRequest::Plan {
                authorization_url,
                target,
                expires_in,
            } => {
                let plan = google_loopback_plan(&authorization_url, &target, expires_in)
                    .map_err(|error| error.code().to_owned())?;
                Ok(json!({
                    "state": plan.state,
                    "wait_ms": u64::try_from(plan.wait.as_millis()).unwrap_or(u64::MAX),
                    "max_request_bytes": GOOGLE_CALLBACK_MAX_REQUEST_BYTES,
                    "io_timeout_ms": u64::try_from(GOOGLE_CALLBACK_IO_TIMEOUT.as_millis()).unwrap_or(u64::MAX),
                }))
            }
            GoogleLoopbackRequest::Reply { head, state } => {
                let reply = google_loopback_reply(head.as_deref(), &state);
                let outcome = match &reply.outcome {
                    GoogleCallback::Ignored => json!({ "outcome": "ignored" }),
                    GoogleCallback::Code(code) => json!({ "outcome": "code", "code": code }),
                    GoogleCallback::Failed(error) => {
                        json!({ "outcome": "failed", "error": error.code() })
                    }
                };
                let mut value = outcome;
                value["response"] = json!(reply.response);
                Ok(value)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn call(request: Value) -> Value {
        let bytes = request.to_string();
        // SAFETY: the buffer is live for the call, and the returned string is freed below.
        unsafe {
            let raw = msime_client_google_loopback(bytes.as_ptr(), bytes.len());
            let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
            msime_client_string_free(raw);
            serde_json::from_str(&text).unwrap()
        }
    }

    /// 只用于 53682 这个监听：redirect_uri 已按查询参数编码好。
    fn authorization_url(state: &str) -> String {
        format!("https://accounts.google.com/o/oauth2/v2/auth?client_id=fixture-client.apps.googleusercontent.com&redirect_uri=http%3A%2F%2F127.0.0.1%3A53682%2Fcallback&response_type=code&state={state}")
    }

    #[test]
    fn a_host_listener_gets_the_desktop_decisions() {
        let target = call(json!({ "operation": "target", "port": 53682 }));
        assert_eq!(
            target,
            json!({ "ok": true, "value": { "target": "http://127.0.0.1:53682/callback" } })
        );
        assert_eq!(
            call(json!({ "operation": "target", "port": 80 })),
            json!({ "ok": false, "error": "account_unavailable" })
        );

        let target = "http://127.0.0.1:53682/callback";
        let plan = call(json!({
            "operation": "plan",
            "authorization_url": authorization_url("fixture-state"),
            "target": target,
            "expires_in": 600,
        }));
        assert_eq!(plan["ok"], true);
        assert_eq!(plan["value"]["state"], "fixture-state");
        assert_eq!(plan["value"]["wait_ms"], 300_000);
        assert_eq!(plan["value"]["max_request_bytes"], 8192);
        assert_eq!(plan["value"]["io_timeout_ms"], 2000);
        assert_eq!(
            call(json!({
                "operation": "plan",
                "authorization_url": "https://evil.test/o/oauth2/v2/auth",
                "target": target,
                "expires_in": 600,
            })),
            json!({ "ok": false, "error": "account_unavailable" })
        );

        let code = call(json!({
            "operation": "reply",
            "head": "GET /callback?state=fixture-state&code=4%2F0Afixture HTTP/1.1\r\n\r\n",
            "state": "fixture-state",
        }));
        assert_eq!(code["value"]["outcome"], "code");
        assert_eq!(code["value"]["code"], "4/0Afixture");
        assert!(code["value"]["response"]
            .as_str()
            .unwrap()
            .starts_with("HTTP/1.1 200 OK\r\n"));
        let denied = call(json!({
            "operation": "reply",
            "head": "GET /callback?state=fixture-state&error=access_denied HTTP/1.1\r\n\r\n",
            "state": "fixture-state",
        }));
        assert_eq!(denied["value"]["outcome"], "failed");
        assert_eq!(denied["value"]["error"], "account_cancelled");
        let other = call(json!({ "operation": "reply", "head": null, "state": "fixture-state" }));
        assert_eq!(other["value"]["outcome"], "ignored");
        assert!(other["value"]["response"]
            .as_str()
            .unwrap()
            .starts_with("HTTP/1.1 404 Not Found\r\n"));
    }
}
