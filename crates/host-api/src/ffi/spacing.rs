//! 跨次上屏空格的无会话策略入口。

use crate::*;

/// 判断应用、模式及隐私条件，并为新上屏文本补前方的中英文接缝。
/// # Safety
/// `request` 必须指向 `length` 个可读字节；缓冲区不被保留。
#[no_mangle]
pub unsafe extern "C" fn msime_client_cross_commit_spacing(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| unsafe {
        let request =
            super::with_bounded_bytes(request, length, 65536, "invalid spacing buffer", |bytes| {
                serde_json::from_slice(bytes).map_err(|_| "invalid spacing request".into())
            })?;
        crate::chinese_english_spacing::cross_commit_plan(request)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn call(bytes: &[u8]) -> Value {
        unsafe {
            let pointer = msime_client_cross_commit_spacing(bytes.as_ptr(), bytes.len());
            assert!(!pointer.is_null());
            let value = serde_json::from_slice(CStr::from_ptr(pointer).to_bytes()).unwrap();
            msime_client_string_free(pointer);
            value
        }
    }

    #[test]
    fn cross_commit_spacing_abi_is_bounded_and_needs_no_session() {
        let request = json!({"config":{"enabled":true,"excluded_apps":[]},"application":"com.example.Writer","context":{"scheme":0,"local_mode":"temporary_english"},"text":"Rust","preceding":29992});
        let response = call(&serde_json::to_vec(&request).unwrap());
        assert_eq!(response["ok"], true);
        assert_eq!(response["value"]["text"], " Rust");
        assert_eq!(call(b"bad json")["ok"], false);
        assert_eq!(call(&vec![b' '; 65537])["ok"], false);
    }
}
