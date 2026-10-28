//! 持久同意的正常路径适配；故障回调只使用已经固定的本地记录头。

use super::*;

/// 确保持续同意历史。请求 `{directory}` 是偏好目录；读写失败保持错误，不清理已有报告。
/// 成功返回 `{enabled:false}` 或 `{enabled:true,epoch,since_unix_ms}`；不联网。
/// # Safety
/// `request` 指向 `length` 个可读 UTF-8 JSON 字节。
#[no_mangle]
pub unsafe extern "C" fn msime_client_telemetry_consent(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: 调用方遵守文档中的缓冲区约定。
        let request: DirectoryRequest = unsafe { document(request, length, MAX_REQUEST_BYTES)? };
        let store = PreferencesStore::new(absolute_directory(&request.directory)?);
        match store
            .usage_reporting_consent()
            .map_err(|error| error.to_string())?
        {
            Some(consent) => Ok(
                json!({"enabled": true, "epoch": consent.epoch, "since_unix_ms": consent.since_unix_ms}),
            ),
            None => Ok(json!({"enabled": false})),
        }
    })
}
