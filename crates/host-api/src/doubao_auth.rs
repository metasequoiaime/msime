use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    #[serde(default)]
    auth_mode: String,
    #[serde(default)]
    app_id: String,
    token: String,
    resource_id: String,
}

/// Build sensitive Doubao authentication headers, without making a request.
/// Free the JSON response with `msime_client_string_free`; never log it.
///
/// # Safety
/// `request` must point to `length` readable bytes for this call.
#[no_mangle]
pub unsafe extern "C" fn msime_client_doubao_auth_headers(
    request: *const u8,
    length: usize,
) -> *mut std::ffi::c_char {
    crate::response(|| {
        if request.is_null() || length == 0 || length > 32768 {
            return Err("invalid Doubao authentication request".into());
        }
        let request: Request =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(request, length) })
                .map_err(|_| "invalid Doubao authentication request")?;
        let headers = msime_client_core::credential::doubao_auth::headers(
            &request.auth_mode,
            &request.app_id,
            &request.token,
            &request.resource_id,
        )
        .ok_or("invalid Doubao authentication configuration")?;
        Ok(serde_json::json!({"headers":headers}))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::read;
    use serde_json::json;

    #[test]
    fn abi_uses_shared_auth_policy_and_sanitizes_rejections() {
        for mode in ["api_key", "legacy", ""] {
            let request = json!({"auth_mode":mode,"app_id":"synthetic-app", "token":"synthetic-token","resource_id":"fixture-resource"}).to_string();
            let result = unsafe {
                read(msime_client_doubao_auth_headers(
                    request.as_ptr(),
                    request.len(),
                ))
            };
            assert_eq!(result["ok"], true);
            let headers = result["value"]["headers"].as_array().unwrap();
            // An empty mode means api_key.
            assert_eq!(
                headers.iter().any(|h| h[0] == "x-api-key"),
                mode != "legacy"
            );
            assert_eq!(
                headers.iter().any(|h| h[0] == "x-api-app-key"),
                mode == "legacy"
            );
        }
        for request in [
            b"not-json".as_slice(),
            br#"{"auth_mode":"invalid","token":"synthetic-token","resource_id":"resource"}"#,
        ] {
            let result = unsafe {
                read(msime_client_doubao_auth_headers(
                    request.as_ptr(),
                    request.len(),
                ))
            };
            assert_eq!(result["ok"], false);
            assert!(!result.to_string().contains("synthetic-token"));
        }
        assert_eq!(
            unsafe { read(msime_client_doubao_auth_headers(std::ptr::null(), 0)) }["ok"],
            false
        );
        assert_eq!(
            unsafe { read(msime_client_doubao_auth_headers(b"x".as_ptr(), 32769)) }["ok"],
            false
        );
    }
}
