//! 流式对话的测试：解析器直接喂字节；传输经 `BackendAccountClient::loopback` 连本机的假服务器，服务器按脚本一段段写出 SSE（形状照 msime-cloud `origin/main` 的 `chat_stream.go`），中间可以停下来等测试放行。

use super::super::transport_tests::{error_response, json_response, read_request, Received};
use super::*;
use serde_json::json;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;

const SSE_HEAD: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n";
const DONE: &[u8] = b"data: [DONE]\n\n";
const UPSTREAM_ERROR: &[u8] = b"data: {\"error\":{\"code\":\"upstream_error\"}}\n\n";

fn token() -> String {
    "a".repeat(64)
}

fn messages() -> Vec<AccountChatMessage> {
    vec![AccountChatMessage {
        role: "user".into(),
        content: "合成的问题".into(),
    }]
}

/// 服务端发的一个增量事件，形状与 `chatStreamOut` 相同。
fn delta(content: &str) -> Vec<u8> {
    let chunk = json!({
        "id": "synthetic-chunk",
        "object": "chat.completion.chunk",
        "choices": [{ "index": 0, "delta": { "content": content }, "finish_reason": null }],
    });
    format!("data: {chunk}\n\n").into_bytes()
}

fn role() -> Vec<u8> {
    let chunk = json!({
        "object": "chat.completion.chunk",
        "choices": [{ "index": 0, "delta": { "role": "assistant" }, "finish_reason": null }],
    });
    format!("data: {chunk}\n\n").into_bytes()
}

/// 假服务器在一个连接上做的事。
enum Step {
    Write(Vec<u8>),
    Sleep(u64),
    /// 停下来，等测试放行（或测试结束、发送端被丢掉，最多 5 秒）。
    Wait(Receiver<()>),
    /// 看客户端有没有断开连接：5 秒内读到 EOF、连接被重置或连接已经失效到设不了读超时时报 true。
    ReportClosed(Sender<bool>),
}

/// 每个连接执行一份脚本；收到的请求按顺序交回。写失败（客户端已经断开）时跳过这一步。
fn serve(scripts: Vec<Vec<Step>>) -> (String, Receiver<Received>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || {
        for script in scripts {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = sent.send(read_request(&mut stream));
            for step in script {
                match step {
                    Step::Write(bytes) => {
                        let _ = std::io::Write::write_all(&mut stream, &bytes);
                        let _ = std::io::Write::flush(&mut stream);
                    }
                    Step::Sleep(milliseconds) => {
                        std::thread::sleep(Duration::from_millis(milliseconds))
                    }
                    Step::Wait(release) => {
                        // 最多等 5 秒：客户端没按预期返回时测试以断言失败结束，而不是一直挂着。
                        let _ = release.recv_timeout(Duration::from_secs(5));
                    }
                    Step::ReportClosed(report) => {
                        // 客户端断开后前面的写会收到 RST，macOS 上对已被重置的连接设读超时会返回 EINVAL，这本身就说明连接已经断了，不能当成假服务器自己出错。
                        let closed = match stream.set_read_timeout(Some(Duration::from_secs(5))) {
                            Err(_) => true,
                            Ok(()) => {
                                let mut byte = [0_u8; 1];
                                match stream.read(&mut byte) {
                                    Ok(0) => true,
                                    Ok(_) => false,
                                    Err(error) => !matches!(
                                        error.kind(),
                                        std::io::ErrorKind::WouldBlock
                                            | std::io::ErrorKind::TimedOut
                                    ),
                                }
                            }
                        };
                        let _ = report.send(closed);
                    }
                }
            }
        }
    });
    (origin, received)
}

/// 发一次流式对话，返回结果、按顺序收到的增量和每次回调所在的线程。
fn stream(
    origin: &str,
    cancel: &ChatStreamCancel,
    mut during: impl FnMut(&str),
) -> (Result<String, AccountFailure>, Vec<String>) {
    let client = BackendAccountClient::loopback(origin).unwrap();
    let caller = std::thread::current().id();
    let mut deltas = Vec::new();
    let result = client.chat_stream_detailed(
        &messages(),
        "synthetic-model",
        &token(),
        cancel,
        &mut |delta| {
            assert_eq!(std::thread::current().id(), caller);
            deltas.push(delta.to_owned());
            during(delta);
        },
    );
    (result, deltas)
}

fn decode(chunks: &[&[u8]]) -> (Result<bool, AccountFailure>, Vec<String>) {
    let mut decoder = ChatStreamDecoder::default();
    let mut deltas = Vec::new();
    for chunk in chunks {
        match decoder.feed(chunk, &mut deltas) {
            Ok(false) => {}
            other => return (other, deltas),
        }
    }
    (Ok(false), deltas)
}

#[test]
fn bytes_split_anywhere_decode_to_the_same_deltas() {
    // BOM、注释心跳、CRLF 和单独的 CR、只带角色的事件、一个事件里的两行 data（拼起来是 JSON）、空 data 和不是对象的 data。
    let mut whole = b"\xEF\xBB\xBF: keep-alive\r\n\r\n".to_vec();
    whole.extend(role());
    whole.extend(delta("你好"));
    whole.extend(b"event: message\rdata: {\"choices\":[{\"delta\":\r\ndata: {\"content\":\"\xF0\x9F\x8C\xB2 \xE4\xB8\x96\"}}]}\r\n\r\n");
    whole.extend(b"data:\n\ndata: keep-alive\n\n");
    whole.extend(delta("界\n"));
    whole.extend(DONE);
    whole.extend(delta("[DONE] 之后的不再看"));
    let expected = vec!["你好".to_owned(), "🌲 世".to_owned(), "界\n".to_owned()];

    let (result, deltas) = decode(&[&whole]);
    assert!(result.unwrap());
    assert_eq!(deltas, expected);
    // 每个字节单独一块：UTF-8 字符、CRLF 和 `data:` 前缀都被拆开。
    let bytes: Vec<&[u8]> = whole.chunks(1).collect();
    let (result, deltas) = decode(&bytes);
    assert!(result.unwrap());
    assert_eq!(deltas, expected);

    let mut decoder = ChatStreamDecoder::default();
    let mut deltas = Vec::new();
    assert!(decoder.feed(&whole, &mut deltas).unwrap());
    assert_eq!(decoder.into_reply(), "你好🌲 世界\n");
}

#[test]
fn bad_streams_fail_as_unavailable() {
    let unavailable = |chunks: &[&[u8]]| {
        let (result, _) = decode(chunks);
        result.unwrap_err().code()
    };
    // 上游在开始输出以后失败：服务端发 error 事件后断开。
    assert_eq!(
        unavailable(&[&delta("部分"), UPSTREAM_ERROR]),
        "account_unavailable"
    );
    // 只有空白就结束、换行和制表符以外的控制字符、正文不是 UTF-8。
    assert_eq!(unavailable(&[&delta("  "), DONE]), "account_unavailable");
    assert_eq!(unavailable(&[&delta("a\u{7}b")]), "account_unavailable");
    assert_eq!(
        unavailable(&[b"data: {\"choices\":[{\"delta\":{\"content\":\"\xE4\xB8\"}}]}\n\n"]),
        "account_unavailable"
    );
    // 回复超过 16 KiB。
    let large = "字".repeat(MAX_CHAT_RESPONSE_BYTES / 3);
    assert_eq!(
        unavailable(&[&delta(&large), &delta("字")]),
        "account_unavailable"
    );
    // 一行超过 1 MiB，整条流超过 4 MiB。
    let long_line = vec![b'x'; MAX_CHAT_STREAM_LINE_BYTES + 1];
    assert_eq!(unavailable(&[b": ", &long_line]), "account_unavailable");
    let comments = b": keep-alive\n".repeat(MAX_CHAT_STREAM_BYTES / 13 + 1);
    assert_eq!(unavailable(&[&comments]), "account_unavailable");

    // 恰好 16 KiB 还收。
    let (result, deltas) = decode(&[&delta(&large), &delta("a"), DONE]);
    assert!(result.unwrap());
    assert_eq!(deltas.concat().len(), MAX_CHAT_RESPONSE_BYTES);
}

#[test]
fn deltas_arrive_on_the_calling_thread_in_order() {
    let mut first = SSE_HEAD.to_vec();
    first.extend(role());
    let mut split = delta("你好");
    // 「好」的三个字节拆在两次写里。
    let at = split
        .windows(3)
        .position(|window| window == "好".as_bytes())
        .unwrap()
        + 1;
    let tail = split.split_off(at);
    let (origin, received) = serve(vec![vec![
        Step::Write(first),
        Step::Write(split),
        Step::Sleep(30),
        Step::Write(tail),
        Step::Sleep(30),
        Step::Write([delta("，世界"), DONE.to_vec()].concat()),
    ]]);
    let (result, deltas) = stream(&origin, &ChatStreamCancel::default(), |_| {});
    assert_eq!(result.unwrap(), "你好，世界");
    assert_eq!(deltas.concat(), "你好，世界");
    assert!(deltas.len() >= 2, "{deltas:?}");

    let request = received.recv().unwrap();
    assert_eq!(request.line(), "POST /v1/chat/completions HTTP/1.1");
    assert_eq!(request.header("accept"), Some("text/event-stream"));
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {}", token()).as_str())
    );
    assert_eq!(
        request.json(),
        json!({ "messages": [{ "role": "user", "content": "合成的问题" }], "model": "synthetic-model", "max_tokens": 2048, "stream": true })
    );
}

#[test]
fn rate_limits_before_the_stream_keep_retry_after() {
    let (origin, _received) = serve(vec![
        vec![Step::Write(error_response(
            "429 Too Many Requests",
            "rate_limit_exceeded",
            &[("Retry-After", "60")],
        ))],
        vec![Step::Write(error_response(
            "503 Service Unavailable",
            "server_busy",
            &[("Retry-After", "1")],
        ))],
    ]);
    for seconds in [60, 1] {
        let (result, deltas) = stream(&origin, &ChatStreamCancel::default(), |_| {});
        let failure = result.unwrap_err();
        assert_eq!(failure.code(), "account_rate_limited");
        assert_eq!(failure.retry_after_seconds, Some(seconds));
        assert!(deltas.is_empty());
    }
}

#[test]
fn a_failure_mid_stream_keeps_the_deltas_already_delivered() {
    let (origin, _received) = serve(vec![
        // 上游失败：error 事件后断开。
        vec![Step::Write(
            [SSE_HEAD, &delta("部分"), UPSTREAM_ERROR].concat(),
        )],
        // 没有 error 事件也没有 [DONE] 就断开。
        vec![Step::Write([SSE_HEAD, &delta("部分")].concat())],
    ]);
    for code in ["account_unavailable", "account_network"] {
        let (result, deltas) = stream(&origin, &ChatStreamCancel::default(), |_| {});
        assert_eq!(result.unwrap_err().code(), code);
        assert_eq!(deltas, ["部分"]);
    }
}

#[test]
fn cancelling_from_another_thread_returns_while_the_server_is_silent() {
    let (release, wait) = mpsc::channel();
    let (closed, was_closed) = mpsc::channel();
    let (origin, _received) = serve(vec![vec![
        Step::Write([SSE_HEAD, &delta("第一段")].concat()),
        Step::Wait(wait),
        Step::Write(delta("取消以后的")),
        Step::Sleep(50),
        Step::Write(delta("取消以后的")),
        Step::ReportClosed(closed),
    ]]);
    let cancel = ChatStreamCancel::default();
    let remote = cancel.clone();
    let cancelled_at = Arc::new(Mutex::new(None));
    let stamp = Arc::clone(&cancelled_at);
    let (result, deltas) = stream(&origin, &cancel, move |_| {
        // 第一段到了以后，另一个线程在服务器不说话时取消。
        let remote = remote.clone();
        let stamp = Arc::clone(&stamp);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            *stamp.lock().unwrap() = Some(Instant::now());
            remote.cancel();
        });
    });
    let returned = Instant::now();
    assert_eq!(result.unwrap_err().code(), "account_cancelled");
    assert_eq!(deltas, ["第一段"]);
    let cancelled_at = cancelled_at.lock().unwrap().unwrap();
    assert!(
        returned.duration_since(cancelled_at) < Duration::from_secs(2),
        "returned {:?} after the cancel",
        returned.duration_since(cancelled_at)
    );
    // 服务器再发数据时，后台线程发现调用方已经走了，断开连接。
    release.send(()).unwrap();
    assert!(was_closed.recv().unwrap());
}

#[test]
fn cancelling_inside_the_callback_stops_the_next_delta() {
    let (origin, _received) = serve(vec![vec![Step::Write(
        [SSE_HEAD, &delta("一"), &delta("二"), DONE].concat(),
    )]]);
    let cancel = ChatStreamCancel::default();
    let inner = cancel.clone();
    let (result, deltas) = stream(&origin, &cancel, move |_| inner.cancel());
    assert_eq!(result.unwrap_err().code(), "account_cancelled");
    assert_eq!(deltas, ["一"]);
}

#[test]
fn cancelling_before_the_response_headers_returns_at_once() {
    let (release, wait) = mpsc::channel();
    let (origin, received) = serve(vec![vec![
        Step::Wait(wait),
        Step::Write([SSE_HEAD, &delta("太晚了"), DONE].concat()),
    ]]);
    let cancel = ChatStreamCancel::default();
    let remote = cancel.clone();
    let canceller = std::thread::spawn(move || {
        // 等服务器收到请求再取消，取消时它还没回响应头。
        received.recv().unwrap();
        std::thread::sleep(Duration::from_millis(50));
        let at = Instant::now();
        remote.cancel();
        at
    });
    let (result, deltas) = stream(&origin, &cancel, |_| {});
    let returned = Instant::now();
    assert_eq!(result.unwrap_err().code(), "account_cancelled");
    assert!(deltas.is_empty());
    assert!(returned.duration_since(canceller.join().unwrap()) < Duration::from_secs(2));
    release.send(()).unwrap();

    // 已经取消的把手不再发请求。
    let (result, _) = stream("http://127.0.0.1:9", &cancel, |_| {});
    assert_eq!(result.unwrap_err().code(), "account_cancelled");
}

#[test]
fn an_old_backend_without_streaming_answers_in_one_delta() {
    let reply =
        json!({ "choices": [{ "message": { "role": "assistant", "content": "整段回复" } }] });
    let (origin, received) = serve(vec![
        vec![Step::Write(error_response(
            "400 Bad Request",
            "invalid_chat_request",
            &[],
        ))],
        vec![Step::Write(json_response("200 OK", reply.clone()))],
        // 收到流式请求却按 JSON 回答（中间层吞掉了 stream）。
        vec![Step::Write(json_response("200 OK", reply))],
    ]);
    let (result, deltas) = stream(&origin, &ChatStreamCancel::default(), |_| {});
    assert_eq!(result.unwrap(), "整段回复");
    assert_eq!(deltas, ["整段回复"]);
    assert_eq!(received.recv().unwrap().json()["stream"], json!(true));
    let fallback = received.recv().unwrap();
    assert_eq!(fallback.json()["stream"], json!(false));
    assert_eq!(fallback.header("accept"), Some("application/json"));

    let (result, deltas) = stream(&origin, &ChatStreamCancel::default(), |_| {});
    assert_eq!(result.unwrap(), "整段回复");
    assert_eq!(deltas, ["整段回复"]);
}

#[test]
fn other_bad_requests_do_not_fall_back() {
    let (origin, received) = serve(vec![vec![Step::Write(error_response(
        "400 Bad Request",
        "invalid_message",
        &[],
    ))]]);
    let (result, _) = stream(&origin, &ChatStreamCancel::default(), |_| {});
    assert_eq!(result.unwrap_err().code(), "account_invalid");
    received.recv().unwrap();
    assert!(received.recv_timeout(Duration::from_millis(200)).is_err());
    // 连不上是 account_network。
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let closed = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let (result, _) = stream(&closed, &ChatStreamCancel::default(), |_| {});
    assert_eq!(result.unwrap_err().code(), "account_network");
}

#[test]
fn request_ids_are_busy_until_finished_and_cancel_only_their_own() {
    let requests = ChatStreamRequests::default();
    let first = requests.begin("synthetic-first").unwrap();
    let second = requests.begin("synthetic-second").unwrap();
    assert_eq!(
        requests.begin("synthetic-first").err().unwrap().code(),
        "account_busy"
    );
    assert_eq!(requests.cancel("synthetic-missing"), Ok(false));
    assert_eq!(requests.cancel("synthetic-first"), Ok(true));
    assert!(first.is_cancelled());
    assert!(!second.is_cancelled());
    requests.finish("synthetic-first");
    assert!(!requests.begin("synthetic-first").unwrap().is_cancelled());
}
