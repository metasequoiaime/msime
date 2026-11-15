//! 阿里云机器翻译的有界无网络桥接，目录、校验与签名均归共享层。
use crate::*;

/// 执行 `catalog`、`validate`、`request` 或 `parse`，不执行网络请求。
/// # Safety
/// `request` 在本次调用期间必须指向 `length` 个可读字节。
#[no_mangle]
pub unsafe extern "C" fn msime_client_aliyun_translation(
    request: *const u8,
    length: usize,
) -> *mut std::ffi::c_char {
    response(|| unsafe {
        super::with_bounded_bytes(
            request,
            length,
            1048576,
            "invalid Aliyun request buffer",
            |bytes| msime_client_core::translation::aliyun::execute(bytes).map_err(str::to_owned),
        )
    })
}
