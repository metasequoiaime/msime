//! `msime_client_dictionary_sync`：「我的词条」经 WebDAV 网盘同步的 C 入口，一个按 `operation` 分发的 JSON 请求。请求、各动作的结果和错误码见头文件；实现在 crate 的 `dictionary_webdav`。
//!
//! 属于 C ABI 的一部分，这些薄封装保证什么见父模块。除 `status` 外每个动作都会联网，宿主在工作线程调用。

use super::with_bounded_bytes;
use crate::dictionary_webdav::{request_json, INVALID, MAX_REQUEST_BYTES};
use crate::*;

/// 词库同步的一个动作。格式见头文件。
/// # Safety
/// `request` 指向 `length` 个可读字节；NULL 被拒绝。返回的响应要用 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_dictionary_sync(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: 由上面的调用方契约保证。
        unsafe { with_bounded_bytes(request, length, MAX_REQUEST_BYTES, INVALID, request_json) }
    })
}

/// 起输入法进程里的后台词库同步。格式见头文件。
/// # Safety
/// `request` 指向 `length` 个可读字节；NULL 被拒绝。返回的响应要用 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_dictionary_sync_start(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: 由上面的调用方契约保证。
        unsafe {
            with_bounded_bytes(
                request,
                length,
                START_REQUEST_BYTES,
                INVALID,
                crate::dictionary_webdav::start_json,
            )
        }
    })
}

/// 叫停后台词库同步，最多等 `timeout_ms` 毫秒（`UINT32_MAX` 一直等）让它退出；返回它是否已经退出。
#[no_mangle]
pub extern "C" fn msime_client_dictionary_sync_stop(timeout_ms: u32) -> bool {
    catch_unwind(|| crate::dictionary_webdav::stop(timeout_ms)).unwrap_or(false)
}

/// 起后台同步的请求只有两个路径。
const START_REQUEST_BYTES: usize = 64 * 1024;
