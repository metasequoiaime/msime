//! 流式对话：`msime_client_account_chat_stream` 以 `stream: true` 请求 `/v1/chat/completions`，回复的增量在调用线程上交给宿主的回调，`msime_client_account_chat_cancel` 按宿主给的请求 id 从任何线程取消（#8201 的决定 15）。请求和预检与 `msime_client_account` 的 `chat` 相同，会话也按它的方式选：`{directory, legacy_directory?, client_agent?}` 或 `{session_store}`，登录账号优先、没有登录时用本机匿名账号。SSE 的解析、取消和上限都在 client-services 的 `account::chat_stream`。
//!
//! 取消登记表是进程级的一张，与 AI 皮肤的那张分开。
//!
//! 属于 C ABI 的一部分，这些薄封装保证什么见父模块。

use super::account::{
    absolute_directory, account_failure_response, anonymous_session_with_agent, checked_agent,
    session_with_agent, Failure,
};
use super::session_store::{session_source, Source};
use super::with_bounded_bytes;
use crate::*;
use msime_client_services::account::{AccountChatMessage, AccountError, ChatStreamRequests};
use std::ffi::c_void;

/// 请求的上限：序列化后的对话请求体最多 64 KiB，JSON 转义可能让宿主发来的那份长一些。
const MAX_REQUEST_BYTES: usize = 1024 * 1024;
/// 取消请求只有一个 id。
const MAX_CANCEL_BYTES: usize = 4 * 1024;
/// 请求 id 是不超过 96 字节的 ASCII 标识（字母、数字、`-`、`_`），与 AI 皮肤相同。
const MAX_REQUEST_ID_BYTES: usize = 96;

/// 头文件的 `msime_client_account_chat_delta_callback`。
pub type ChatDeltaCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize);

static REQUESTS: OnceLock<ChatStreamRequests> = OnceLock::new();

fn requests() -> &'static ChatStreamRequests {
    REQUESTS.get_or_init(ChatStreamRequests::default)
}

fn invalid() -> Failure {
    Failure::from(AccountError::Invalid.code().to_owned())
}

fn valid_request_id(id: &str) -> bool {
    msime_client_core::is_bounded_ascii_identifier(id, MAX_REQUEST_ID_BYTES)
}

/// 对话结束时注销请求 id，出错或 panic 时也一样。
struct Registered<'a>(&'a str);

impl Drop for Registered<'_> {
    fn drop(&mut self) {
        requests().finish(self.0);
    }
}

/// 把一段增量交给宿主的回调：UTF-8 字节和长度，不带结尾的 NUL，只在回调期间有效。回调为 NULL 时丢掉。
/// # Safety
/// `on_delta` 和 `context` 在返回的闭包被调用期间有效。
unsafe fn delta_sink(
    on_delta: Option<ChatDeltaCallback>,
    context: *mut c_void,
) -> impl FnMut(&str) {
    move |delta: &str| {
        if let Some(on_delta) = on_delta {
            // SAFETY: 由调用方契约保证；只在调用线程上依次调用，delta 在回调期间有效。
            unsafe { on_delta(context, delta.as_ptr(), delta.len()) }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StreamRequest {
    #[serde(default)]
    directory: Option<String>,
    #[serde(default)]
    legacy_directory: Option<String>,
    #[serde(default)]
    session_store: Option<u64>,
    /// 与 `msime_client_account` 的 `client_agent` 相同：只能与 `directory` 一起给。
    #[serde(default)]
    client_agent: Option<String>,
    request_id: String,
    messages: Vec<AccountChatMessage>,
    model: String,
}

/// 流式对话。请求、回调的约定、返回值和错误码见头文件。
/// # Safety
/// `request` 指向 `length` 个可读的 UTF-8 JSON 字节，NULL 被拒绝。`on_delta` 可为 NULL；否则按头文件的约定在本线程上带着 `context` 调用。
#[no_mangle]
pub unsafe extern "C" fn msime_client_account_chat_stream(
    request: *const u8,
    length: usize,
    on_delta: Option<ChatDeltaCallback>,
    context: *mut c_void,
) -> *mut c_char {
    account_failure_response(|| {
        let code = AccountError::Invalid.code();
        // SAFETY: 由调用方契约保证；长度先经过上限检查。
        let request: StreamRequest = unsafe {
            with_bounded_bytes(request, length, MAX_REQUEST_BYTES, code, |bytes| {
                serde_json::from_slice(bytes).map_err(|_| code.to_owned())
            })
        }?;
        if !valid_request_id(&request.request_id) {
            return Err(invalid());
        }
        let source = session_source(
            request.directory.as_deref(),
            request.legacy_directory.is_some(),
            request.session_store,
        )
        .ok_or_else(invalid)?;
        let agent = checked_agent(request.client_agent.as_deref()).map_err(|_| invalid())?;
        if agent.is_some() && matches!(source, Source::Store(_)) {
            return Err(invalid());
        }
        let cancel = requests().begin(&request.request_id)?;
        let _registered = Registered(&request.request_id);
        // SAFETY: 宿主给的回调和 context 在本次调用期间有效，sink 只在本次调用里用。
        let mut deliver = unsafe { delta_sink(on_delta, context) };
        let account_code = |error: AccountError| Failure::from(error.code().to_owned());
        let result = match source {
            Source::Directory(directory) => {
                let directory = absolute_directory(directory)?;
                let legacy = request
                    .legacy_directory
                    .as_deref()
                    .map(absolute_directory)
                    .transpose()?;
                let session = session_with_agent(directory, legacy, agent).map_err(account_code)?;
                crate::account_ops::chat_stream(
                    &session,
                    || anonymous_session_with_agent(directory, agent),
                    &request.messages,
                    &request.model,
                    &cancel,
                    &mut deliver,
                )
            }
            Source::Store(entry) => crate::account_ops::chat_stream(
                &entry.account,
                || Ok(Arc::clone(&entry.anonymous)),
                &request.messages,
                &request.model,
                &cancel,
                &mut deliver,
            ),
        };
        result.map_err(Failure::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CancelRequest {
    request_id: String,
}

/// 取消一次进行中的流式对话，Value 是当时是否有一个在进行；没有在进行的记下来，同一个 id 随后开始时立即取消（见头文件）。任意线程上调用，立即返回；也可以在增量回调里调用。
/// # Safety
/// `request` 指向 `length` 个可读的 UTF-8 JSON 字节，NULL 被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_account_chat_cancel(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    account_failure_response(|| {
        let code = AccountError::Invalid.code();
        // SAFETY: 由调用方契约保证；长度先经过上限检查。
        let request: CancelRequest = unsafe {
            with_bounded_bytes(request, length, MAX_CANCEL_BYTES, code, |bytes| {
                serde_json::from_slice(bytes).map_err(|_| code.to_owned())
            })
        }?;
        if !valid_request_id(&request.request_id) {
            return Err(invalid());
        }
        Ok(Value::Bool(requests().cancel(&request.request_id)?))
    })
}

#[cfg(test)]
mod tests;
