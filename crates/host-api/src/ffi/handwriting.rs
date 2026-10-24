//! 原生手写板的离线笔画识别，不访问会话或网络。

use crate::*;
use msime_client_core::panels::{HandwritingRecognitionRequest, HandwritingRecognitionResult};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    model: String,
    request: HandwritingRecognitionRequest,
}

fn recognize(request: Request) -> Result<Value, String> {
    if request.model.len() > 4096
        || request.model.contains('\0')
        || !Path::new(&request.model).is_absolute()
    {
        return Err("invalid_handwriting_model".into());
    }
    request.request.validate().map_err(|_| "invalid_stroke")?;
    if request.request.language != "zh-CN" {
        return Err("unsupported_handwriting_language".into());
    }
    let strokes: Vec<Vec<(f32, f32)>> = request
        .request
        .strokes
        .into_iter()
        .map(|stroke| {
            stroke
                .points
                .into_iter()
                .map(|point| (point.x, point.y))
                .collect()
        })
        .collect();
    let candidates = msime_engine::handwriting_recognize(&request.model, &strokes, 420.0, 420.0)
        .map_err(|_| "handwriting_unavailable")?;
    serde_json::to_value(HandwritingRecognitionResult { candidates })
        .map_err(|_| "handwriting_unavailable".into())
}

/// 接收有界 JSON 请求，在工作线程上识别 420×420 坐标内的一个汉字，返回标准应答封装。
///
/// # Safety
/// `request` 必须在调用期间指向 `length` 字节的可读内存，函数不保留该缓冲区。
#[no_mangle]
pub unsafe extern "C" fn msime_client_handwriting(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        let request = unsafe {
            super::with_bounded_bytes(
                request,
                length,
                262_144,
                "invalid_handwriting_buffer",
                |bytes| {
                    serde_json::from_slice(bytes).map_err(|_| "invalid_handwriting_request".into())
                },
            )
        }?;
        recognize(request)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi::msime_client_string_free;
    use std::ffi::CStr;

    fn request(model: &str, strokes: Value) -> Request {
        serde_json::from_value(json!({"model": model, "request": {"strokes": strokes}})).unwrap()
    }

    #[test]
    fn rejects_untrusted_paths_and_invalid_strokes() {
        assert_eq!(
            recognize(request("relative.model", json!([]))).unwrap_err(),
            "invalid_handwriting_model"
        );
        assert_eq!(
            recognize(request("/missing.model", json!([]))).unwrap_err(),
            "invalid_stroke"
        );
        assert_eq!(
            recognize(request("/missing.model", json!([{"points": []}]))).unwrap_err(),
            "invalid_stroke"
        );
    }

    #[test]
    fn rejects_oversized_buffer_before_reading() {
        let reply = unsafe { msime_client_handwriting(std::ptr::null(), 262_145) };
        let value: Value =
            serde_json::from_str(unsafe { CStr::from_ptr(reply) }.to_str().unwrap()).unwrap();
        unsafe { msime_client_string_free(reply) };
        assert_eq!(value["error"], "invalid_handwriting_buffer");
    }

    #[test]
    fn recognizes_synthetic_ink_through_abi() {
        let Ok(model) = std::env::var("MSIME_HANDWRITING_MODEL") else {
            eprintln!("skipped: set MSIME_HANDWRITING_MODEL to the pinned model");
            return;
        };
        let bytes = serde_json::to_vec(&json!({
            "model": model,
            "request": {"strokes": [
                {"points": [{"x":35,"y":40},{"x":35,"y":105}]},
                {"points": [{"x":35,"y":40},{"x":125,"y":40},{"x":125,"y":105}]},
                {"points": [{"x":35,"y":105},{"x":125,"y":105}]},
                {"points": [{"x":80,"y":15},{"x":80,"y":140}]}
            ]}
        }))
        .unwrap();
        let reply = unsafe { msime_client_handwriting(bytes.as_ptr(), bytes.len()) };
        let value: Value =
            serde_json::from_slice(unsafe { CStr::from_ptr(reply) }.to_bytes()).unwrap();
        unsafe { msime_client_string_free(reply) };
        assert_eq!(value["ok"], true);
        assert!(value["value"]["candidates"]
            .as_array()
            .unwrap()
            .contains(&json!("中")));
    }
}
