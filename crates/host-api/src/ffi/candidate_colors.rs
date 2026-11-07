//! 候选颜色更新的协议适配，业务规则由 client-core 持有。

use crate::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateRequest {
    #[serde(default)]
    current: Option<msime_client_core::theme::GlobalTheme>,
    value: Value,
}

/// 返回候选选中文字色的共享更新补丁，不读写偏好文件。
///
/// # Safety
/// `request` 指向 `length` 个可读字节，响应由 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_candidate_color_update(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| unsafe {
        super::with_bounded_bytes(
            request,
            length,
            1024,
            "invalid candidate color request",
            |bytes| {
                let request: UpdateRequest =
                    serde_json::from_slice(bytes).map_err(|_| "invalid candidate color request")?;
                let value = match &request.value {
                    Value::Null => None,
                    Value::String(value) => Some(value.as_str()),
                    _ => return Err("invalid candidate color request".into()),
                };
                let update = msime_client_core::preferences::candidate_color::selected_text_update(
                    request.current.unwrap_or_default(),
                    value,
                )
                .map_err(|error| error.to_string())?;
                serde_json::to_value(update).map_err(|error| error.to_string())
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn invoke(bytes: &[u8]) -> Value {
        let pointer = unsafe { msime_client_candidate_color_update(bytes.as_ptr(), bytes.len()) };
        read(pointer)
    }

    fn read(pointer: *mut c_char) -> Value {
        assert!(!pointer.is_null());
        let bytes = unsafe { CStr::from_ptr(pointer) }.to_bytes().to_vec();
        unsafe { msime_client_string_free(pointer) };
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn candidate_colour_update_exports_the_shared_patch_and_clear_semantics() {
        let picked = invoke(br##"{"current":"native","value":"#7b3fbf"}"##);
        assert_eq!(picked["ok"], true);
        assert_eq!(
            picked["value"],
            json!({
                "selected_text": "#7B3FBF", "global_theme": "custom", "base": "system",
                "reset_skins": true
            })
        );
        let cleared = invoke(br#"{"current":"night","value":null}"#);
        assert_eq!(cleared["ok"], true);
        assert_eq!(
            cleared["value"],
            json!({"selected_text": null, "reset_skins": false})
        );
        let legacy = invoke(br##"{"value":"#123456"}"##);
        assert_eq!(legacy["value"]["base"], "system");
    }

    #[test]
    fn candidate_colour_update_rejects_invalid_json_types_and_boundaries() {
        for request in [
            r#"{}"#,
            r#"{"value":false}"#,
            r#"{"value":3}"#,
            r#"{"current":"unknown","value":null}"#,
            r#"{"current":true,"value":null}"#,
            r##"{"value":"#12345678"}"##,
            r#"{"value":null,"unexpected":true}"#,
        ] {
            assert_eq!(invoke(request.as_bytes())["ok"], false);
        }
        assert_eq!(invoke(&[])["ok"], false);
        assert_eq!(invoke(&[0xff])["ok"], false);
        assert_eq!(invoke(&vec![b' '; 1025])["ok"], false);
        let null = unsafe { msime_client_candidate_color_update(std::ptr::null(), 1) };
        assert_eq!(read(null)["ok"], false);
    }
}
