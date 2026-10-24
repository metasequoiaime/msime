//! Windows 原生手写面板的有界请求；识别与模型回退不依赖 WebView。

use crate::*;
use msime_client_core::panels::{HandwritingRecognitionRequest, HandwritingRecognitionResult};
use msime_host_windows::ink::{self, InkError};

const REQUEST_LIMIT: usize = 262_144;

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Status {},
    Recognize {
        request: HandwritingRecognitionRequest,
        #[serde(default)]
        model: Option<String>,
    },
}

fn recognize(
    request: HandwritingRecognitionRequest,
    model: Option<String>,
) -> Result<Value, String> {
    request.validate().map_err(|_| "invalid_stroke")?;
    if let Some(path) = &model {
        if path.len() > 4096 || path.contains('\0') || !Path::new(path).is_absolute() {
            return Err("invalid_handwriting_model".into());
        }
    }
    let query = HandwritingQuery {
        language: request.language,
        strokes: request
            .strokes
            .into_iter()
            .map(|stroke| {
                stroke
                    .points
                    .into_iter()
                    .map(|point| msime_input_runtime::HandwritingPoint {
                        x: point.x,
                        y: point.y,
                    })
                    .collect()
            })
            .collect(),
    };
    let strokes = query
        .strokes
        .iter()
        .map(|stroke| stroke.iter().map(|point| (point.x, point.y)).collect())
        .collect::<Vec<ink::Stroke>>();
    let recognized = ink::recognize(&strokes);
    let candidates = match recognized {
        Ok(candidates) if !candidates.is_empty() || model.is_none() => candidates,
        _ if model.is_some() => handwriting_local_candidates(model.as_deref().unwrap(), &query)
            .map_err(|_| "handwriting_unavailable")?,
        Err(InkError::NoChineseRecognizer) => return Err("no_chinese_recognizer".into()),
        Err(InkError::EmptyInput) => return Err("invalid_stroke".into()),
        _ => return Err("handwriting_unavailable".into()),
    };
    let result = HandwritingRecognitionResult { candidates };
    result
        .validate()
        .map_err(|_| "invalid_handwriting_result")?;
    serde_json::to_value(result).map_err(|_| "handwriting_unavailable".into())
}

/// 原生 Windows 手写请求，返回标准 JSON 封装。仅工作线程调用；不访问输入会话、不联网。
///
/// # Safety
/// `request` 在调用期间必须指向 `length` 字节的可读内存；不保留该缓冲区。
#[no_mangle]
pub unsafe extern "C" fn msime_client_windows_handwriting(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        let request = unsafe {
            super::with_bounded_bytes(
                request,
                length,
                REQUEST_LIMIT,
                "invalid_handwriting_buffer",
                |bytes| {
                    serde_json::from_slice::<Request>(bytes)
                        .map_err(|_| "invalid_handwriting_request".into())
                },
            )
        }?;
        if !Edition::of_windows_package()?.features.handwriting {
            return Err("handwriting_not_offered".into());
        }
        match request {
            Request::Status {} => Ok(json!({"system_recognizer": ink::has_chinese_recognizer()})),
            Request::Recognize { request, model } => recognize(request, model),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn call(document: Value) -> Value {
        let bytes = serde_json::to_vec(&document).unwrap();
        let answer = unsafe { msime_client_windows_handwriting(bytes.as_ptr(), bytes.len()) };
        assert!(!answer.is_null());
        let result = serde_json::from_slice(unsafe { CStr::from_ptr(answer) }.to_bytes()).unwrap();
        unsafe { msime_client_string_free(answer) };
        result
    }

    #[test]
    fn bounded_requests_reject_invalid_buffers_and_documents() {
        for (pointer, length) in [(std::ptr::null(), 0), (b"{}".as_ptr(), REQUEST_LIMIT + 1)] {
            let answer = unsafe { msime_client_windows_handwriting(pointer, length) };
            let result: Value =
                serde_json::from_slice(unsafe { CStr::from_ptr(answer) }.to_bytes()).unwrap();
            unsafe { msime_client_string_free(answer) };
            assert_eq!(result["error"], "invalid_handwriting_buffer");
        }
        assert_eq!(
            call(json!({"operation":"status", "unexpected":true}))["error"],
            "invalid_handwriting_request"
        );
        assert_eq!(
            call(json!({"operation":"unknown"}))["error"],
            "invalid_handwriting_request"
        );
    }

    #[test]
    fn invalid_strokes_and_model_paths_never_reach_the_recognizer() {
        if !Edition::of_windows_package().unwrap().features.handwriting {
            assert_eq!(
                call(json!({"operation":"status"}))["error"],
                "handwriting_not_offered"
            );
            return;
        }
        for strokes in [
            json!([]),
            json!([{"points":[]}]),
            json!([{"points":[{"x":4097,"y":0}]}]),
        ] {
            assert_eq!(
                call(json!({"operation":"recognize", "request":{"strokes":strokes}}))["error"],
                "invalid_stroke"
            );
        }
        assert_eq!(
            call(
                json!({"operation":"recognize", "request":{"strokes":[{"points":[{"x":0,"y":0},{"x":1,"y":1}]}]}, "model":"relative.model"})
            )["error"],
            "invalid_handwriting_model"
        );
    }

    #[test]
    fn public_abi_recognizes_a_synthetic_cross_when_system_chinese_is_installed() {
        if !Edition::of_windows_package().unwrap().features.handwriting {
            return;
        }
        let status = call(json!({"operation":"status"}));
        assert_eq!(status["ok"], true);
        if status["value"]["system_recognizer"] != true {
            eprintln!("未安装系统中文手写识别器：尚未验证 C ABI 的实际中文识别");
            return;
        }
        let answer = call(json!({
            "operation":"recognize", "request":{"strokes":[
                {"points":[{"x":80,"y":160},{"x":240,"y":160}]},
                {"points":[{"x":160,"y":80},{"x":160,"y":240}]}
            ]}
        }));
        assert_eq!(answer["ok"], true);
        let candidates = answer["value"]["candidates"].as_array().unwrap();
        assert!(candidates.contains(&json!("十")), "{answer}");
    }
}
