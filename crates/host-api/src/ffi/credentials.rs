//! The settings pages' credential checks: the 「测试配置」 buttons, the translation endpoint hint, and the AI page's endpoint check, model listing and polish test.
//!
//! Part of the C ABI; see the parent module for what these shims guarantee.

use crate::*;

/// The largest `msime_client_test_api_credential` request: a service name and a configuration of a few credentials and an endpoint.
const CREDENTIAL_TEST_REQUEST_LIMIT: usize = 64 * 1024;

/// One 「测试配置」 check, `{service, config}` as the shared settings page's `testApiCredential` sends it, through `credential::test_service`, the same function the Tauri settings shell calls. Value `{ok, message}`; the message is the Chinese line the page shows, and never carries the provider's reply or the credential. Blocks on the network for up to about 15 seconds: call off the UI thread.
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_test_api_credential(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        let document: Value = unsafe {
            with_bounded_bytes(
                request,
                length,
                CREDENTIAL_TEST_REQUEST_LIMIT,
                "invalid credential test request",
                |bytes| {
                    serde_json::from_slice(bytes)
                        .map_err(|_| "invalid credential test request".to_owned())
                },
            )
        }?;
        let service = document
            .get("service")
            .and_then(Value::as_str)
            .ok_or("invalid credential test request")?;
        let config = document
            .get("config")
            .filter(|config| config.is_object())
            .ok_or("invalid credential test request")?;
        let result = msime_client_providers::credential::test_service(service, config);
        Ok(json!({"ok": result.ok, "message": result.message}))
    })
}

/// The voice page's 「测试」 check for a host whose settings page has no copy of the provider defaults (the Android native app): `{preferences_directory, service}` with `service` `voice.asr` or `voice.polish`. The configuration is resolved from the stored preferences by `voice::provider::credential_test_config`, the same resolution the keyboard uses, then checked by `credential::test_service`. Value `{ok, message}` as for `msime_client_test_api_credential`; a provider with nothing to test (the platform recognizer, on-device models, an unknown id) is answered without the network. Blocks on the network for up to about 15 seconds: call off the UI thread.
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_test_voice_credential(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        let document: Value = unsafe {
            with_bounded_bytes(
                request,
                length,
                CREDENTIAL_TEST_REQUEST_LIMIT,
                "invalid credential test request",
                |bytes| {
                    serde_json::from_slice(bytes)
                        .map_err(|_| "invalid credential test request".to_owned())
                },
            )
        }?;
        let service = document
            .get("service")
            .and_then(Value::as_str)
            .filter(|service| matches!(*service, "voice.asr" | "voice.polish"))
            .ok_or("invalid credential test request")?;
        let directory = document
            .get("preferences_directory")
            .and_then(Value::as_str)
            .filter(|directory| std::path::Path::new(directory).is_absolute())
            .ok_or("invalid preferences directory")?;
        let snapshot = msime_client_core::preferences::PreferencesStore::new(directory)
            .load()
            .map_err(|_| "preferences unavailable")?;
        let Some(config) = msime_client_providers::voice::provider::credential_test_config(
            &snapshot.preferences,
            service,
        ) else {
            return Ok(json!({"ok": false, "message": "当前服务不需要测试。"}));
        };
        let result = msime_client_providers::credential::test_service(service, &config);
        Ok(json!({"ok": result.ok, "message": result.message}))
    })
}

/// The hint the settings page shows under the custom translation endpoint, `translation::endpoint_issue` (empty when there is none), and whether the preferences store accepts the endpoint (`accepted`: empty, or `translation::is_supported_endpoint`). Value `{issue, accepted}`. No I/O.
/// # Safety
/// `endpoint` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_translation_endpoint_issue(
    endpoint: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        let endpoint = unsafe {
            with_bounded_bytes(endpoint, length, 64 * 1024, "invalid endpoint", |bytes| {
                std::str::from_utf8(bytes)
                    .map(str::to_owned)
                    .map_err(|_| "invalid endpoint".to_owned())
            })
        }?;
        Ok(json!({
            "issue": msime_client_core::translation::endpoint_issue(&endpoint),
            "accepted": endpoint.is_empty()
                || msime_client_core::translation::is_supported_endpoint(&endpoint),
        }))
    })
}

/// The largest `msime_client_ai_models` or `msime_client_ai_test` request: an endpoint, a model and up to three 16 KiB texts (token, prompt, text to polish), with room for JSON escaping.
const AI_SETTINGS_REQUEST_LIMIT: usize = 256 * 1024;

/// The string fields of an AI settings request; a missing or non-string field fails the whole request with `ai_invalid`.
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
unsafe fn ai_settings_request<const N: usize>(
    request: *const u8,
    length: usize,
    fields: [&str; N],
) -> Result<[String; N], String> {
    let document: Value = unsafe {
        with_bounded_bytes(
            request,
            length,
            AI_SETTINGS_REQUEST_LIMIT,
            "ai_invalid",
            |bytes| serde_json::from_slice(bytes).map_err(|_| "ai_invalid".to_owned()),
        )
    }?;
    let mut values: [String; N] = std::array::from_fn(|_| String::new());
    for (value, field) in values.iter_mut().zip(fields) {
        *value = document
            .get(field)
            .and_then(Value::as_str)
            .ok_or("ai_invalid")?
            .to_owned();
    }
    Ok(values)
}

/// The AI page's 「获取模型」, `{endpoint, token}` as the shared page's `aiAssistant.fetchModels` sends it, through `ai::settings::fetch_models`, the function the Tauri `ai_models` command calls. Value: the model ids. Error: the command's code. Blocks on the network for up to about 15 seconds: call off the UI thread.
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_ai_models(request: *const u8, length: usize) -> *mut c_char {
    response(|| {
        let [endpoint, token] =
            unsafe { ai_settings_request(request, length, ["endpoint", "token"]) }?;
        msime_client_providers::ai::settings::fetch_models(&endpoint, &token)
            .map(|models| json!(models))
            .map_err(|error| error.code().to_owned())
    })
}

/// AI 公开预设目录；不读取偏好或凭据，不访问网络。
#[no_mangle]
pub extern "C" fn msime_client_ai_provider_catalog() -> *mut c_char {
    response(|| {
        use msime_client_providers::ai::{provider_catalog::PROVIDERS, provider_uses_token};
        Ok(Value::Array(
            PROVIDERS
                .iter()
                .map(|preset| {
                    let mut entry = json!(preset);
                    entry["uses_token"] = json!(provider_uses_token(preset.id));
                    entry
                })
                .collect(),
        ))
    })
}

/// 计算服务商切换补丁；请求含启用状态、服务商、端点、模型及可选的地址/模型槽位，不含凭据。
/// # Safety
/// `request` 指向 `length` 个可读 UTF-8 JSON 字节，长度不得超过 16 KiB。
#[no_mangle]
pub unsafe extern "C" fn msime_client_ai_provider_update(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        use msime_client_providers::ai::provider_catalog::{provider_update, AiProviderFields};
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            provider: String,
            current: AiProviderFields,
        }
        let request: Request = unsafe {
            with_bounded_bytes(request, length, 16 * 1024, "ai_invalid", |bytes| {
                serde_json::from_slice(bytes).map_err(|_| "ai_invalid".to_owned())
            })
        }?;
        provider_update(&request.provider, &request.current)
            .map(|patch| json!(patch))
            .ok_or_else(|| "ai_invalid".to_owned())
    })
}

/// The AI page's 「测试」, `{endpoint, model, prompt, token, text}` as the shared page's `aiAssistant.test` sends it, through `ai::settings::test_polish`, the function the Tauri `ai_test` command calls. Value: the reply text. Error: the command's code. Blocks on the network for up to about 15 seconds: call off the UI thread.
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_ai_test(request: *const u8, length: usize) -> *mut c_char {
    response(|| {
        let [endpoint, model, prompt, token, text] = unsafe {
            ai_settings_request(
                request,
                length,
                ["endpoint", "model", "prompt", "token", "text"],
            )
        }?;
        msime_client_providers::ai::settings::test_polish(&endpoint, &model, &prompt, &token, &text)
            .map(|reply| json!(reply))
            .map_err(|error| error.code().to_owned())
    })
}

/// The AI page's view of its endpoint: `origin`, the key `ai_assistant.tokens` stores the token under (`ai::endpoint::credential_origin`, null when the endpoint is not usable), `problem` (null, `invalid` or `cleartext_public`) and `hint`, the line the page shows under the field (`ai::endpoint::hint`, empty when usable). No I/O.
/// # Safety
/// `endpoint` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_ai_endpoint_check(
    endpoint: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        let endpoint = unsafe {
            with_bounded_bytes(endpoint, length, 64 * 1024, "invalid endpoint", |bytes| {
                std::str::from_utf8(bytes)
                    .map(str::to_owned)
                    .map_err(|_| "invalid endpoint".to_owned())
            })
        }?;
        use msime_client_providers::ai::endpoint::{self, AiEndpointError};
        let problem = match endpoint::validate(&endpoint) {
            Ok(_) => Value::Null,
            Err(AiEndpointError::Invalid) => json!("invalid"),
            Err(AiEndpointError::CleartextPublicHost) => json!("cleartext_public"),
        };
        Ok(json!({
            "origin": endpoint::credential_origin(&endpoint),
            "problem": problem,
            "hint": endpoint::hint(&endpoint),
        }))
    })
}
