//! 对话的流式请求：`POST /v1/chat/completions` 带 `stream: true`，服务端回 `text/event-stream`（msime-cloud `internal/server/chat_stream.go`，契约见 `contracts/protocol.json` 的 `chat`）。
//!
//! 服务端的形状：每个事件是一行 `data: <chat.completion.chunk>` 加一个空行，只含 `choices[0].delta` 的 `role`、`content` 和 `finish_reason`，最后一个事件是 `data: [DONE]`。开始输出之前的失败（限流 429、并发满 503 `server_busy`、上游 502 等）仍是 JSON 错误和对应的状态码；开始输出之后上游失败时，最后一个事件是 `data: {"error":{"code":"upstream_error"}}`，随后断开，不发 `[DONE]`。限流只在请求进来时判断，流开始以后不会再有 429。不认识 `stream` 的旧后端回 400 `invalid_chat_request`，这时退回非流式请求，整段回复作为一段增量交出（与 Android 的 `BackendAccount.chatStream` 相同）。
//!
//! 线程：请求和读取在一个后台线程上做，读到的字节经有界的通道交给调用线程，SSE 的解析、校验和 `on_delta` 都在调用线程上。这样取消不必等阻塞中的读取返回：[`ChatStreamCancel::cancel`] 置位以后往同一个通道里塞一条唤醒，调用线程立刻以 `account_cancelled` 返回，丢掉通道的接收端；后台线程下一次拿到数据（或到总时限）时发现接收端已经没了，丢掉响应、断开连接。reqwest 的阻塞客户端没有办法从别的线程打断一次读取，所以连接最晚在总时限（125 秒）时断开，期间只占这一个线程和这一条连接。

use super::client::{chat_body_bytes, chat_reply};
use super::validate::*;
use super::*;
use crate::early_cancels::EarlyCancels;
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};

/// 一次流式回答的总字节数上限。服务端读上游的流最多 1 MiB（`UpstreamResponseBytes`），重新编码后每个事件可能比上游的长一些；回复正文本身另有 16 KiB 的上限。
const MAX_CHAT_STREAM_BYTES: usize = 4 * 1024 * 1024;
/// 一行（以及一个事件拼起来的 data）的字节上限，与服务端读上游时的行缓冲上限相同。
const MAX_CHAT_STREAM_LINE_BYTES: usize = 1024 * 1024;
/// 后台线程每次读多少字节。
const CHAT_STREAM_READ_BYTES: usize = 8 * 1024;
/// 通道里最多攒多少条消息：调用线程的回调慢时后台线程停在发送上，内存最多攒到这么多块。
const CHAT_STREAM_QUEUE: usize = 16;
/// 旧后端不认识 `stream` 时回的错误码。
const STREAM_UNSUPPORTED: &str = "invalid_chat_request";

/// 后台线程交给调用线程的消息。
enum Message {
    /// 响应正文的一块，按到达的样子切，可能在一行、一个事件甚至一个 UTF-8 字符中间断开。
    Bytes(Vec<u8>),
    /// 正文读完了（连接正常关闭）。
    End,
    /// 请求失败：没拿到响应、非 2xx（带状态码、服务端错误码和 `Retry-After`）或读到一半出错。
    Failed(AccountFailure),
    /// 一次拿到的整段回复：服务端没按 SSE 回答，或者退回了非流式请求。
    Reply(Result<String, AccountFailure>),
    /// [`ChatStreamCancel::cancel`] 发来的唤醒，本身不带内容。
    Wake,
}

#[derive(Default)]
struct CancelState {
    cancelled: AtomicBool,
    /// 进行中的那次请求的通道，取消时往里塞一条 [`Message::Wake`]。
    wake: Mutex<Option<SyncSender<Message>>>,
}

/// 一次流式对话的取消把手。克隆的把手指向同一次请求；[`Self::cancel`] 可以在任何线程上调用，也可以在 `on_delta` 里调用，立即返回。
#[derive(Clone, Default)]
pub struct ChatStreamCancel(Arc<CancelState>);

impl ChatStreamCancel {
    /// 取消这次请求：之后不再交出增量，进行中的等待立刻结束，请求以 `account_cancelled` 返回。请求还没开始时，开始以后马上结束。
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
        if let Ok(wake) = self.0.wake.lock() {
            if let Some(sender) = wake.as_ref() {
                // 通道满时不等：调用线程手上已经有消息要处理，处理完一条就会看到取消标记。
                let _ = sender.try_send(Message::Wake);
            }
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }

    /// 已经取消时是 `account_cancelled`。
    pub fn check(&self) -> Result<(), AccountFailure> {
        if self.is_cancelled() {
            return Err(AccountError::Cancelled.into());
        }
        Ok(())
    }

    /// 登记这次请求的通道，返回的守卫在请求结束时注销它。
    fn attach(&self, sender: SyncSender<Message>) -> Attached<'_> {
        if let Ok(mut wake) = self.0.wake.lock() {
            *wake = Some(sender);
        }
        Attached(self)
    }
}

struct Attached<'a>(&'a ChatStreamCancel);

impl Drop for Attached<'_> {
    fn drop(&mut self) {
        if let Ok(mut wake) = self.0 .0.wake.lock() {
            *wake = None;
        }
    }
}

/// 按宿主给的请求 id 登记进行中的流式对话，取消时按 id 找到它。与 AI 皮肤的 `AiSkinRequests` 相同：先于登记到达的取消记在 `EarlyCancels` 里，同一个 id 随后登记时直接按已取消开始。
#[derive(Default)]
pub struct ChatStreamRequests(Mutex<ChatStreamTable>);

#[derive(Default)]
struct ChatStreamTable {
    running: HashMap<String, ChatStreamCancel>,
    early: EarlyCancels,
}

impl ChatStreamRequests {
    /// 登记一次对话，返回它的取消把手；同一个 id 已经有一个在进行时是 `account_busy`，原来那次的把手不动。这个 id 的取消在登记之前就到了时，返回的把手已经取消。
    pub fn begin(&self, id: &str) -> Result<ChatStreamCancel, AccountFailure> {
        let mut requests = self.0.lock().map_err(|_| AccountError::Unavailable)?;
        if requests.running.contains_key(id) {
            return Err(AccountFailure::busy());
        }
        let cancel = ChatStreamCancel::default();
        if requests.early.take(id) {
            cancel.cancel();
        }
        requests.running.insert(id.to_owned(), cancel.clone());
        Ok(cancel)
    }

    /// 取消 `id` 的对话，返回当时是否有一个在进行。没有在进行的记下来，这个 id 随后登记时按已取消开始。只碰这张表和那次请求的通道，立即返回。
    pub fn cancel(&self, id: &str) -> Result<bool, AccountFailure> {
        let mut requests = self.0.lock().map_err(|_| AccountError::Unavailable)?;
        match requests.running.get(id) {
            Some(cancel) => {
                cancel.cancel();
                Ok(true)
            }
            None => {
                requests.early.remember(id);
                Ok(false)
            }
        }
    }

    /// 对话结束（成功、失败或取消）后注销，同一个 id 之后可以再用，但再用时登记之前到的取消不起作用（见 `EarlyCancels`）。之后才到的这个 id 的取消不再当成先到的记下。
    pub fn finish(&self, id: &str) {
        if let Ok(mut requests) = self.0.lock() {
            requests.running.remove(id);
            requests.early.finish(id);
        }
    }
}

/// SSE 的解析：按 LF、CRLF 和单独的 CR 分行（与服务端的 `splitSSELines` 相同），`data:` 行拼成事件，空行结束一个事件；注释行和其他字段忽略，只剥离流开头的一个 BOM。一行是完整的字节才按 UTF-8 解码，所以一个字符被拆在两块里也没关系。
#[derive(Default)]
pub(super) struct ChatStreamDecoder {
    line: Vec<u8>,
    /// 上一行以 CR 结束：紧随的一个 LF 属于同一个换行。
    skip_lf: bool,
    started: bool,
    data: Vec<u8>,
    has_data: bool,
    reply: String,
    received: usize,
    done: bool,
}

impl ChatStreamDecoder {
    /// 喂进一块正文，这块里结束的事件里的非空增量按顺序追加到 `deltas`。收到 `[DONE]` 时返回 `true`，之后的字节不再看。上游报错、正文不合规或超过上限时是失败，出错之前追加到 `deltas` 的留着，由调用方照常交出。
    pub(super) fn feed(
        &mut self,
        bytes: &[u8],
        deltas: &mut Vec<String>,
    ) -> Result<bool, AccountFailure> {
        self.received = self.received.saturating_add(bytes.len());
        if self.received > MAX_CHAT_STREAM_BYTES {
            return Err(AccountError::Unavailable.into());
        }
        for &byte in bytes {
            if self.done {
                break;
            }
            if std::mem::take(&mut self.skip_lf) && byte == b'\n' {
                continue;
            }
            if byte == b'\r' || byte == b'\n' {
                self.skip_lf = byte == b'\r';
                self.end_line(deltas)?;
            } else {
                if self.line.len() >= MAX_CHAT_STREAM_LINE_BYTES {
                    return Err(AccountError::Unavailable.into());
                }
                self.line.push(byte);
            }
        }
        Ok(self.done)
    }

    /// 收到 `[DONE]` 以后的整段回复。
    pub(super) fn into_reply(self) -> String {
        self.reply
    }

    fn end_line(&mut self, deltas: &mut Vec<String>) -> Result<(), AccountFailure> {
        let mut line = std::mem::take(&mut self.line);
        let mut value = line.as_slice();
        if !self.started {
            self.started = true;
            value = value.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(value);
        }
        let result = if value.is_empty() {
            self.dispatch(deltas)
        } else if let Some(data) = value.strip_prefix(b"data:") {
            let data = data.strip_prefix(b" ").unwrap_or(data);
            let separator = usize::from(self.has_data);
            if self.data.len() + separator + data.len() > MAX_CHAT_STREAM_LINE_BYTES {
                Err(AccountError::Unavailable.into())
            } else {
                if self.has_data {
                    self.data.push(b'\n');
                }
                self.data.extend_from_slice(data);
                self.has_data = true;
                Ok(())
            }
        } else {
            // 注释（以冒号开头的心跳）和 event、id、retry 字段不影响内容。
            Ok(())
        };
        line.clear();
        self.line = line;
        result
    }

    /// 处理一个完整事件的 data。
    fn dispatch(&mut self, deltas: &mut Vec<String>) -> Result<(), AccountFailure> {
        if !std::mem::take(&mut self.has_data) {
            return Ok(());
        }
        let data = std::mem::take(&mut self.data);
        let text = std::str::from_utf8(&data).map_err(|_| AccountError::Unavailable)?;
        if text == "[DONE]" {
            if self.reply.trim().is_empty() {
                return Err(AccountError::Unavailable.into());
            }
            self.done = true;
            return Ok(());
        }
        // 空的或不是 JSON 对象的 data（中间层发来的空 data 行或心跳）跳过，与 Android 相同；`[DONE]` 和带 error 的对象照常处理。
        let Ok(serde_json::Value::Object(object)) = serde_json::from_str(text) else {
            return Ok(());
        };
        if object.get("error").is_some_and(|error| !error.is_null()) {
            return Err(AccountError::Unavailable.into());
        }
        #[derive(Deserialize)]
        struct Chunk {
            #[serde(default)]
            choices: Vec<Choice>,
        }
        #[derive(Deserialize)]
        struct Choice {
            #[serde(default)]
            delta: Delta,
        }
        #[derive(Default, Deserialize)]
        struct Delta {
            #[serde(default)]
            content: Option<String>,
        }
        let chunk: Chunk = serde_json::from_value(serde_json::Value::Object(object))
            .map_err(|_| AccountError::Unavailable)?;
        let Some(content) = chunk
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.delta.content)
            .filter(|content| !content.is_empty())
        else {
            return Ok(());
        };
        if crate::text::has_disallowed_control(&content)
            || self.reply.len() + content.len() > MAX_CHAT_RESPONSE_BYTES
        {
            return Err(AccountError::Unavailable.into());
        }
        self.reply.push_str(&content);
        deltas.push(content);
        Ok(())
    }
}

impl BackendAccountClient {
    /// [`AccountApi::chat_stream_detailed`] 的实现，见模块说明。
    pub(super) fn stream_chat(
        &self,
        messages: &[AccountChatMessage],
        model: &str,
        access_token: &str,
        cancel: &ChatStreamCancel,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<String, AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        validate_chat_request(messages, model)?;
        let body = chat_body_bytes(messages, model, true)?;
        if body.len() > MAX_CHAT_REQUEST_BYTES {
            return Err(AccountFailure::too_large());
        }
        cancel.check()?;
        let request = self
            .prepare(
                Method::POST,
                CHAT_PATH,
                Some(access_token),
                Some(body),
                MAX_CHAT_REQUEST_BYTES,
                "text/event-stream",
            )?
            .timeout(CHAT_TIMEOUT);
        match receive(cancel, on_delta, move |sender| {
            stream_response(request, sender)
        }) {
            // 不认识 stream 的旧后端：退回非流式请求，整段回复一次交出去。
            Err(failure)
                if failure.status == Some(400)
                    && failure.server_code.as_deref() == Some(STREAM_UNSUPPORTED) =>
            {
                let client = self.clone();
                let messages = messages.to_vec();
                let model = model.to_owned();
                let access_token = access_token.to_owned();
                receive(cancel, on_delta, move |sender| {
                    let reply = client.chat_detailed(&messages, &model, &access_token);
                    let _ = sender.send(Message::Reply(reply));
                })
            }
            result => result,
        }
    }
}

/// 在后台线程上跑 `work`，在调用线程上接它发来的消息，直到回复完整、失败或取消。
fn receive(
    cancel: &ChatStreamCancel,
    on_delta: &mut dyn FnMut(&str),
    work: impl FnOnce(&SyncSender<Message>) + Send + 'static,
) -> Result<String, AccountFailure> {
    let (sender, receiver) = mpsc::sync_channel(CHAT_STREAM_QUEUE);
    let _attached = cancel.attach(sender.clone());
    // 登记以后再看一次：在登记之前取消的，唤醒没地方塞。
    cancel.check()?;
    std::thread::Builder::new()
        .name("msime-chat-stream".into())
        .spawn(move || {
            // 后台线程无论怎样结束都要发一条消息：取消把手也拿着一个发送端，调用线程等不到「所有发送端都没了」。
            if catch_unwind(AssertUnwindSafe(|| work(&sender))).is_err() {
                let _ = sender.send(Message::Failed(AccountError::Unavailable.into()));
            }
        })
        .map_err(|_| AccountError::Unavailable)?;
    deliver(&receiver, cancel, on_delta)
}

fn deliver(
    receiver: &Receiver<Message>,
    cancel: &ChatStreamCancel,
    on_delta: &mut dyn FnMut(&str),
) -> Result<String, AccountFailure> {
    let mut decoder = ChatStreamDecoder::default();
    let mut deltas = Vec::new();
    loop {
        let message = receiver.recv().map_err(|_| AccountError::Unavailable)?;
        cancel.check()?;
        match message {
            Message::Wake => {}
            Message::Bytes(bytes) => {
                // 先交出出错的事件之前的增量：同一块里先有增量、后有 error 事件时，结果不随 TCP 怎么切块而变。
                let fed = decoder.feed(&bytes, &mut deltas);
                for delta in deltas.drain(..) {
                    cancel.check()?;
                    on_delta(&delta);
                }
                cancel.check()?;
                if fed? {
                    return Ok(decoder.into_reply());
                }
            }
            // 没等到 `[DONE]` 连接就关了：回复不完整。
            Message::End => return Err(AccountFailure::network()),
            Message::Failed(failure) => return Err(failure),
            Message::Reply(reply) => {
                let reply = reply?;
                on_delta(&reply);
                cancel.check()?;
                return Ok(reply);
            }
        }
    }
}

/// 后台线程：发请求，非 2xx 交回失败，SSE 的正文按块交回；接收端没了（调用线程已经返回）就丢掉响应，连接随之断开。
fn stream_response(request: reqwest::blocking::RequestBuilder, sender: &SyncSender<Message>) {
    let message = match request.send() {
        Err(error) => Message::Failed(AccountFailure::from_send_error(&error)),
        Ok(response) if !response.status().is_success() => {
            Message::Failed(failure_from_response(response))
        }
        Ok(response) if !is_event_stream(&response) => {
            // 没按流式回答（例如中间层吞掉了 stream）：按普通 JSON 回复读，整段一次交出去，与 Android 相同。
            Message::Reply(
                read_bounded_response(response, MAX_CHAT_RESPONSE_BYTES)
                    .map_err(AccountFailure::from)
                    .and_then(|bytes| chat_reply(&bytes)),
            )
        }
        Ok(mut response) => {
            let mut buffer = vec![0_u8; CHAT_STREAM_READ_BYTES];
            loop {
                let message = match response.read(&mut buffer) {
                    Ok(0) => Message::End,
                    Ok(read) => Message::Bytes(buffer[..read].to_vec()),
                    // 读到一半连接断开或到了总时限。
                    Err(_) => Message::Failed(AccountFailure::network()),
                };
                let last = !matches!(message, Message::Bytes(_));
                if sender.send(message).is_err() || last {
                    return;
                }
            }
        }
    };
    let _ = sender.send(message);
}

fn is_event_stream(response: &Response) -> bool {
    response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("text/event-stream")
        })
}

#[cfg(test)]
mod tests;
