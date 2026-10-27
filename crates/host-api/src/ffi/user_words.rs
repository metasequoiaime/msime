//! `msime_client_user_words_mirror`：同一台设备上两个各有一份个人词库的进程（HarmonyOS 的设置应用和输入法扩展）之间交换「我的词条」的 C 入口，一个 JSON 请求。请求、结果和错误码见头文件；实现在 crate 的 `user_words_mirror`。
//!
//! 属于 C ABI 的一部分，这些薄封装保证什么见父模块。不联网；会读整份日志、`sync` 还要写本机词库，宿主按头文件说的线程调用。

use super::with_bounded_bytes;
use crate::user_words_mirror::{request_json, INVALID, MAX_REQUEST_BYTES};
use crate::*;

/// 交换「我的词条」的一次调用。格式见头文件。
/// # Safety
/// `request` 指向 `length` 个可读字节；NULL 被拒绝。返回的响应要用 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_user_words_mirror(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: 由上面的调用方契约保证。
        unsafe { with_bounded_bytes(request, length, MAX_REQUEST_BYTES, INVALID, request_json) }
    })
}
