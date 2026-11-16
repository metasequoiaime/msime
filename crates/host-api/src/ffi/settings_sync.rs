//! 设置和可移植主题的 WebDAV 同步 C 入口，业务由共享 Rust 服务层持有。
use crate::*;

/// 返回有界 JSON 响应；请求与操作见 `msime_client.h`。
/// # Safety
/// `request` 指向 `length` 个可读字节；返回值必须用 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_settings_sync(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| unsafe {
        super::with_bounded_bytes(
            request,
            length,
            crate::settings_webdav::MAX_REQUEST_BYTES,
            crate::settings_webdav::INVALID,
            crate::settings_webdav::request_json,
        )
    })
}
