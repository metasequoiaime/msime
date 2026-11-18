//! 自定义双拼编辑器的无状态接口，统一导入限制、校验和偏好事务。
use super::*;
use msime_client_core::preferences::{Preferences, ShuangpinCustomProfile, ShuangpinProfile};
use msime_client_core::{custom_shuangpin_settings, edition::Edition};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportTable {
    #[serde(default)]
    initials: BTreeMap<String, String>,
    #[serde(default)]
    finals: BTreeMap<String, String>,
    #[serde(default)]
    zero_initials: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Operation {
    Read,
    Check,
    Import,
    Template,
    Use,
    Delete,
    Hints,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    preferences: Preferences,
    edition: String,
    operation: Operation,
    table: Option<ShuangpinCustomProfile>,
    text: Option<String>,
    profile: Option<ShuangpinProfile>,
    hints: Option<bool>,
}

fn plan(mut request: Request) -> Result<Value, String> {
    let edition = Edition::by_id(&request.edition).ok_or("custom_shuangpin_unavailable")?;
    if !edition.offers_touch_scheme(msime_client_core::preferences::TouchKeyboardScheme::Xiaohe) {
        return Err("custom_shuangpin_unavailable".into());
    }
    let stored = &request.preferences.shuangpin_custom_profile;
    let mut draft = if stored.is_empty() {
        crate::shuangpin_custom_layout(request.preferences.shuangpin_profile, stored).table
    } else {
        stored.clone()
    };
    match request.operation {
        Operation::Read => {}
        Operation::Check | Operation::Use => {
            draft = request.table.ok_or("custom_shuangpin_table_required")?;
        }
        Operation::Import => {
            let text = request.text.ok_or("custom_shuangpin_import_invalid")?;
            if text.len() > 16_384 {
                return Err("custom_shuangpin_import_too_large".into());
            }
            let table: ImportTable =
                serde_json::from_str(&text).map_err(|_| "custom_shuangpin_import_invalid")?;
            draft = ShuangpinCustomProfile {
                initials: table.initials,
                finals: table.finals,
                zero_initials: table.zero_initials,
            };
        }
        Operation::Template => {
            let profile = request.profile.ok_or("custom_shuangpin_template_invalid")?;
            if profile == ShuangpinProfile::Custom {
                return Err("custom_shuangpin_template_invalid".into());
            }
            draft =
                crate::shuangpin_custom_layout(profile, &ShuangpinCustomProfile::default()).table;
        }
        Operation::Delete => {
            custom_shuangpin_settings::remove(&mut request.preferences);
            draft = crate::shuangpin_custom_layout(
                request.preferences.shuangpin_profile,
                &request.preferences.shuangpin_custom_profile,
            )
            .table;
        }
        Operation::Hints => {
            request.preferences.touch_shuangpin_key_hints =
                request.hints.ok_or("custom_shuangpin_hints_required")?;
        }
    }
    let layout = crate::shuangpin_custom_layout(ShuangpinProfile::Custom, &draft);
    if matches!(request.operation, Operation::Use) {
        if layout.problem.is_some() {
            return Err("custom_shuangpin_table_invalid".into());
        }
        custom_shuangpin_settings::select(&mut request.preferences, draft.clone(), edition)?;
    }
    Ok(serde_json::json!({"preferences":request.preferences,"draft":draft,"layout":layout}))
}

/// 返回编辑草稿、引擎校验结果和待保存偏好，不访问磁盘。
/// # Safety
/// `request` 指向 `length` 个可读字节；返回值用 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_custom_shuangpin_settings(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| unsafe {
        with_bounded_bytes(
            request,
            length,
            128 * 1024,
            "custom_shuangpin_request_invalid",
            |bytes| {
                let request: Request = serde_json::from_slice(bytes)
                    .map_err(|_| "custom_shuangpin_request_invalid")?;
                plan(request)
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn call(operation: &str, fields: Value) -> Value {
        let mut value = serde_json::json!({"preferences":Preferences::default(),"edition":"full","operation":operation});
        value
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        plan(serde_json::from_value(value).unwrap()).unwrap()
    }
    #[test]
    fn custom_shuangpin_editor_preserves_invalid_drafts_and_uses_valid_tables_atomically() {
        let read = call("read", serde_json::json!({}));
        assert!(read["layout"]["problem"].is_null());
        let table = read["draft"].clone();
        let used = call("use", serde_json::json!({"table":table}));
        assert_eq!(used["preferences"]["shuangpin_profile"], "custom");
        assert_eq!(
            used["preferences"]["touch_keyboard_schemes"]["selected"],
            "xiaohe"
        );
        let mut invalid = table.clone();
        invalid["finals"].as_object_mut().unwrap().remove("ang");
        let checked = call("check", serde_json::json!({"table":invalid}));
        assert_eq!(checked["draft"], invalid);
        assert_eq!(checked["layout"]["problem"]["kind"], "missing_unit");
        assert_eq!(checked["layout"]["problem"]["unit"], "ang");
        assert_eq!(
            call("import", serde_json::json!({"text":table.to_string()}))["draft"],
            table
        );
    }
    #[test]
    fn custom_shuangpin_ffi_checks_buffers_and_rejects_invalid_tables() {
        let request=serde_json::json!({"preferences":Preferences::default(),"edition":"full","operation":"use",
            "table":{"initials":{},"finals":{},"zero_initials":{}}}).to_string();
        for (pointer, length) in [
            (std::ptr::null(), 0),
            (request.as_ptr(), request.len()),
            (request.as_ptr(), 128 * 1024 + 1),
        ] {
            let response = unsafe { msime_client_custom_shuangpin_settings(pointer, length) };
            let document: Value = serde_json::from_str(
                unsafe { std::ffi::CStr::from_ptr(response) }
                    .to_str()
                    .unwrap(),
            )
            .unwrap();
            unsafe {
                msime_client_string_free(response);
            }
            assert_eq!(document["ok"], false);
        }
    }
    #[test]
    fn custom_shuangpin_import_rejects_extra_groups_and_limits_utf8_bytes() {
        for text in ["{\"extra\":{}}".to_owned(), " ".repeat(16_385)] {
            let value = serde_json::json!({"preferences":Preferences::default(),"edition":"full","operation":"import","text":text});
            assert!(plan(serde_json::from_value(value).unwrap()).is_err());
        }
    }
}
