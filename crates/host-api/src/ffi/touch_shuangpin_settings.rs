//! 原生触屏双拼设置的无状态适配；读盘与 CAS 保存沿用偏好接口。
use super::with_bounded_bytes;
use crate::*;
use msime_client_core::{
    edition::Edition, preferences::ShuangpinProfile, touch_shuangpin_settings,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    preferences: Value,
    edition: String,
    profile: Option<ShuangpinProfile>,
}

fn settings(request: Request) -> Result<Value, String> {
    let edition = Edition::by_id(&request.edition).ok_or("touch_shuangpin_unavailable")?;
    let mut preferences =
        Preferences::from_submission(request.preferences).map_err(|e| e.to_string())?;
    if let Some(profile) = request.profile {
        touch_shuangpin_settings::select(&mut preferences, profile, edition)?;
    }
    Ok(
        json!({ "options": touch_shuangpin_settings::catalog(edition),
        "profile": preferences.shuangpin_profile, "preferences": preferences }),
    )
}

/// 返回共享双拼目录，带 profile 时计算选择后的偏好；不读写文件。
/// # Safety
/// request 指向 length 个可读字节；响应由 msime_client_string_free 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_touch_shuangpin_settings(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| unsafe {
        with_bounded_bytes(
            request,
            length,
            PREFERENCES_DOCUMENT_LIMIT,
            "invalid touch shuangpin request",
            |bytes| {
                let request =
                    serde_json::from_slice(bytes).map_err(|_| "invalid touch shuangpin request")?;
                settings(request)
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_and_selection_use_shared_preferences() {
        let before = serde_json::to_value(Preferences::default()).unwrap();
        let loaded = settings(Request {
            preferences: before.clone(),
            edition: "full".into(),
            profile: None,
        })
        .unwrap();
        assert_eq!(loaded["preferences"], before);
        assert_eq!(loaded["options"].as_array().unwrap().len(), 9);
        let selected = settings(Request {
            preferences: before,
            edition: "full".into(),
            profile: Some(ShuangpinProfile::Sogou),
        })
        .unwrap();
        assert_eq!(selected["profile"], "sogou");
        assert_eq!(
            selected["preferences"]["touch_keyboard_schemes"]["selected"],
            "sogou"
        );
        assert!(serde_json::from_value::<Request>(
            json!({"preferences":{},"edition":"full","profile":"unknown"})
        )
        .is_err());
    }
}
