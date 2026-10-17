//! `msime_client_client_agent`（#8201 迁移计划 P11）：经 C 接口逐条跑 `shared/contracts/client-agent/cases.json`，拼出的字符串能直接当 `msime_client_account` 的 `client_agent` 用；请求本身不合法时是 `account_invalid`。拼装规则本身由 client-services 的测试核对。

use super::*;
use std::ffi::CStr;

fn call(bytes: &[u8]) -> Value {
    // SAFETY: 缓冲区在调用期间有效，返回的字符串读完释放。
    unsafe {
        let raw = msime_client_client_agent(bytes.as_ptr(), bytes.len());
        let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
        msime_client_string_free(raw);
        serde_json::from_str(&text).unwrap()
    }
}

#[test]
fn the_shared_cases_pass_through_the_c_entry() {
    let contract: Value = serde_json::from_str(include_str!(
        "../../../../../shared/contracts/client-agent/cases.json"
    ))
    .unwrap();
    let cases = contract["cases"].as_array().unwrap();
    assert!(cases.len() >= 20);
    for case in cases {
        let mut request = case.clone();
        let object = request.as_object_mut().unwrap();
        let agent = object.remove("agent");
        let error = object.remove("error");
        object.remove("note");
        let expected = match (agent, error) {
            (Some(agent), None) => {
                assert!(valid_client_agent(agent.as_str().unwrap()), "{agent}");
                json!({ "ok": true, "value": agent })
            }
            (None, Some(error)) => json!({ "ok": false, "error": error }),
            _ => panic!("{case}"),
        };
        assert_eq!(call(request.to_string().as_bytes()), expected, "{case}");
    }
}

#[test]
fn a_malformed_request_is_account_invalid() {
    let invalid = json!({ "ok": false, "error": "account_invalid" });
    for request in [
        "".to_owned(),
        "[]".to_owned(),
        json!({ "platform": "ios", "version": "1.0" }).to_string(),
        json!({ "platform": "ios", "version": "1.0", "edition": null }).to_string(),
        json!({ "platform": "ios", "version": 1, "edition": "full" }).to_string(),
        json!({ "platform": "ios", "version": "1.0", "edition": "full", "model": "iPhone" })
            .to_string(),
        json!({ "platform": "ios", "version": "1.0", "edition": "full", "device": "x".repeat(16 * 1024) })
            .to_string(),
    ] {
        assert_eq!(call(request.as_bytes()), invalid, "{request:.80}");
    }
    // SAFETY: 空指针按头文件是请求不合法，返回的字符串读完释放。
    let null = unsafe {
        let raw = msime_client_client_agent(std::ptr::null(), 4);
        let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
        msime_client_string_free(raw);
        serde_json::from_str::<Value>(&text).unwrap()
    };
    assert_eq!(null, invalid);
    // 上限以内的长段照样清洗，不算不合法。
    let long = json!({ "platform": "ios", "version": "1.0", "edition": "full", "device": "x".repeat(15 * 1024) });
    assert_eq!(call(long.to_string().as_bytes())["ok"], true);
}
