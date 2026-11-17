//! 同步宿主桥只接受本机清除，不能借此调用会联网的通用账号操作。

use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsResetRequest {
    directory: String,
    options: Value,
}

/// 清除本机学习并保存当前账号屏障；不联网、不创建密钥。
///
/// # Safety
/// `request` 必须指向 `length` 个可读字节；空指针与超长请求被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_learning_reset_settings(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    account_failure_response(|| {
        // SAFETY: 缓冲区有效性由调用方保证，读取前校验空指针与长度。
        let request: SettingsResetRequest = unsafe {
            with_bounded_bytes(
                request,
                length,
                MAX_REQUEST_BYTES,
                "account_invalid",
                |bytes| serde_json::from_slice(bytes).map_err(|_| "account_invalid".to_owned()),
            )?
        };
        let directory = absolute_directory(&request.directory)?;
        let session =
            session_for(directory, None).map_err(|error| Failure::from(error.code().to_owned()))?;
        crate::settings_dictionary::request_with_account(
            session.as_ref(),
            &request.options,
            &json!({ "operation": "reset" }),
            || {},
        )
        .map_err(|code| code.to_owned().into())
    })
}
