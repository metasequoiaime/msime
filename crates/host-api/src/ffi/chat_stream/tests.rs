//! C 入口的测试：请求校验、请求 id 的登记和取消、没有会话时的错误码。不联网；增量、取消和换号的流程在 `account_ops` 的测试里用假的后端驱动，SSE 的传输在 client-services 的测试里。

use super::*;
use std::ffi::CStr;

unsafe fn text(raw: *mut c_char) -> Value {
    // SAFETY: 调用方给的是本库返回、尚未释放的字符串。
    let text = unsafe { CStr::from_ptr(raw) }.to_str().unwrap().to_owned();
    // SAFETY: 同上，只释放一次。
    unsafe { msime_client_string_free(raw) };
    serde_json::from_str(&text).unwrap()
}

/// 增量回调：把收到的每一段记进 `context` 指向的列表。
unsafe extern "C" fn record(context: *mut c_void, delta: *const u8, length: usize) {
    // SAFETY: 测试给的 context 是一个活着的 `Vec<String>`，delta 在回调期间有效。
    let deltas = unsafe { &mut *context.cast::<Vec<String>>() };
    let bytes = unsafe { std::slice::from_raw_parts(delta, length) };
    deltas.push(String::from_utf8(bytes.to_vec()).unwrap());
}

fn stream(request: Value) -> (Value, Vec<String>) {
    let bytes = request.to_string();
    let mut deltas = Vec::<String>::new();
    // SAFETY: 缓冲区和 deltas 在调用期间有效，返回的字符串在 `text` 里释放。
    let response = unsafe {
        text(msime_client_account_chat_stream(
            bytes.as_ptr(),
            bytes.len(),
            Some(record),
            (&mut deltas as *mut Vec<String>).cast(),
        ))
    };
    (response, deltas)
}

fn cancel(request: Value) -> Value {
    let bytes = request.to_string();
    // SAFETY: 同上。
    unsafe {
        text(msime_client_account_chat_cancel(
            bytes.as_ptr(),
            bytes.len(),
        ))
    }
}

fn chat(directory: &Path, request_id: &str) -> Value {
    json!({ "directory": directory, "request_id": request_id, "messages": [{ "role": "user", "content": "合成的问题" }], "model": "synthetic-model" })
}

#[test]
fn bad_requests_fail_before_the_network() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("account");
    let invalid = json!({ "ok": false, "error": "account_invalid" });
    let mut extra = chat(&directory, "synthetic");
    extra["extra"] = json!(1);
    let mut both = chat(&directory, "synthetic");
    both["session_store"] = json!(u64::MAX);
    let mut empty = chat(&directory, "synthetic");
    empty["messages"] = json!([]);
    let mut blank = chat(&directory, "synthetic");
    blank["model"] = json!("");
    for request in [
        chat(&directory, "bad id"),
        chat(&directory, &"a".repeat(97)),
        chat(Path::new("relative"), "synthetic"),
        json!({ "request_id": "synthetic", "messages": [{ "role": "user", "content": "问题" }], "model": "synthetic-model" }),
        extra,
        both,
        empty,
        blank,
    ] {
        let (response, deltas) = stream(request.clone());
        assert_eq!(response, invalid, "{request}");
        assert!(deltas.is_empty());
    }
    // 序列化后超过 64 KiB 在取令牌之前就结束。
    let mut large = chat(&directory, "synthetic");
    large["messages"] = json!((0..5)
        .map(|_| json!({ "role": "user", "content": "字".repeat(5000) }))
        .collect::<Vec<_>>());
    assert_eq!(
        stream(large).0,
        json!({ "ok": false, "error": "account_too_large" })
    );
    // SAFETY: NULL 被拒绝，不会读它。
    let null = unsafe {
        text(msime_client_account_chat_stream(
            std::ptr::null(),
            0,
            None,
            std::ptr::null_mut(),
        ))
    };
    assert_eq!(null, invalid);
    assert_eq!(cancel(json!({ "request_id": "bad id" })), invalid);
    assert_eq!(
        cancel(json!({ "request_id": "synthetic", "extra": 1 })),
        invalid
    );
    assert!(!directory.exists());
}

#[test]
fn signed_out_requests_are_unauthorized_and_release_their_id() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("account");
    let unauthorized = json!({ "ok": false, "error": "account_unauthorized" });
    let request = chat(&directory, "synthetic-signed-out");
    let (response, deltas) = stream(request.clone());
    assert_eq!(response, unauthorized);
    assert!(deltas.is_empty());
    // 第一次结束时 id 已经注销：同一个 id 可以再用，也没有在进行的可以取消。
    assert_eq!(stream(request).0, unauthorized);
    assert_eq!(
        cancel(json!({ "request_id": "synthetic-signed-out" })),
        json!({ "ok": true, "value": false })
    );
}

#[test]
fn a_running_request_id_is_busy_and_cancellable() {
    let running = requests().begin("synthetic-running").unwrap();
    let _registered = Registered("synthetic-running");
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        stream(chat(root.path(), "synthetic-running")).0,
        json!({ "ok": false, "error": "account_busy" })
    );
    // 被拒的那次没有动原来那次的登记。
    assert_eq!(
        cancel(json!({ "request_id": "synthetic-running" })),
        json!({ "ok": true, "value": true })
    );
    assert!(running.is_cancelled());
}

#[test]
fn deltas_reach_the_host_callback_as_utf8_bytes() {
    let mut deltas = Vec::<String>::new();
    // SAFETY: record 和 deltas 在 sink 被调用期间有效。
    let mut sink = unsafe { delta_sink(Some(record), (&mut deltas as *mut Vec<String>).cast()) };
    sink("你好");
    sink("🌲\n");
    drop(sink);
    assert_eq!(deltas, ["你好", "🌲\n"]);
    // 回调为 NULL 时什么也不做。
    // SAFETY: 不会调用任何回调。
    let mut silent = unsafe { delta_sink(None, std::ptr::null_mut()) };
    silent("丢掉");
}
