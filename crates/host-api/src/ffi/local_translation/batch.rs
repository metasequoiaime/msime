//! 一批候选共享取消标记和截止时间；所有输入在排队前验证。

use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Batch {
    state_root: PathBuf,
    request_id: String,
    candidates: Vec<String>,
    target_language: String,
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}

impl Batch {
    fn requests(self) -> Result<Vec<Request>, String> {
        if self.candidates.is_empty() || self.candidates.len() > 10 {
            return Err("local_translation_invalid_input".into());
        }
        self.candidates
            .into_iter()
            .map(|text| {
                let request = Request {
                    state_root: self.state_root.clone(),
                    request_id: self.request_id.clone(),
                    text,
                    target_language: self.target_language.clone(),
                    timeout_ms: self.timeout_ms,
                };
                request.validate()?;
                Ok(request)
            })
            .collect()
    }
}

pub(super) fn translate(bytes: &[u8]) -> Result<Value, String> {
    let batch: Batch =
        serde_json::from_slice(bytes).map_err(|_| "local_translation_invalid_input".to_owned())?;
    let deadline = Instant::now() + Duration::from_millis(batch.timeout_ms.min(120_000));
    let requests = batch.requests()?;
    let task = tasks().register(&requests[0].request_id)?;
    let mut translations = Vec::with_capacity(requests.len());
    for request in requests {
        if task.cancelled.load(Ordering::Relaxed) {
            return Err("local_translation_cancelled".into());
        }
        if Instant::now() >= deadline {
            return Err("local_translation_timeout".into());
        }
        translations.push(inference::translate(
            request,
            deadline,
            task.cancelled.clone(),
        )?);
    }
    if task.cancelled.load(Ordering::Relaxed) {
        return Err("local_translation_cancelled".into());
    }
    Ok(json!({"translations": translations}))
}

/// 等待整批候选翻译，返回按原顺序排列的 `translations`。仅限宿主后台线程调用。
/// # Safety
/// `request` 必须指向 `length` 个可读字节；JSON 不超过 16 KiB。
#[no_mangle]
pub unsafe extern "C" fn msime_client_local_translation_batch(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| unsafe {
        with_bounded_bytes(
            request,
            length,
            16_384,
            "local_translation_invalid_input",
            translate,
        )
    })
}

/// 原生宿主的候选查询适配；只取本地推理字段，结果直接用于 apply_translations。
/// # Safety
/// `request` 必须指向 `length` 个可读字节，JSON 不超过 16 KiB。
#[no_mangle]
pub unsafe extern "C" fn msime_client_local_translation_query(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| unsafe {
        with_bounded_bytes(
            request,
            length,
            16_384,
            "local_translation_invalid_input",
            |bytes| {
                let query: Value =
                    serde_json::from_slice(bytes).map_err(|_| "local_translation_invalid_input")?;
                if query["provider"] != "local" {
                    return Err("local_translation_invalid_input".into());
                }
                let rows = query["candidates"]
                    .as_array()
                    .ok_or("local_translation_invalid_input")?;
                if rows.is_empty() || rows.len() > 10 {
                    return Err("local_translation_invalid_input".into());
                }
                let texts = rows
                    .iter()
                    .map(|row| {
                        row["text"]
                            .as_str()
                            .ok_or("local_translation_invalid_input")
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let batch = serde_json::to_vec(&json!({
                "state_root": query["state_root"], "request_id": query["request_id"],
                "candidates": texts, "target_language": query["target_language"],
                "timeout_ms": query.get("timeout_ms").cloned().unwrap_or(json!(default_timeout())),
            })).map_err(|_| "local_translation_invalid_input")?;
                let translated = translate(&batch)?;
                let translations = texts
                    .iter()
                    .zip(
                        translated["translations"]
                            .as_array()
                            .ok_or("local_translation_unavailable")?,
                    )
                    .map(|(text, translation)| json!({"text": text, "translation": translation}))
                    .collect::<Vec<_>>();
                Ok(json!({"translations": translations}))
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_whole_batch_before_enqueuing_any_candidate() {
        let valid = json!({"state_root": std::env::temp_dir(), "request_id": "synthetic-batch",
            "candidates": ["合成文本", "测试"], "target_language": "sw"});
        let batch: Batch = serde_json::from_value(valid.clone()).unwrap();
        assert_eq!(batch.requests().unwrap().len(), 2);
        for candidates in [
            json!([]),
            json!(["合成文本", ""]),
            json!(["合成文本", "<|plamo:tag|>output"]),
            json!(vec!["文本"; 11]),
        ] {
            let mut invalid = valid.clone();
            invalid["candidates"] = candidates;
            let batch: Batch = serde_json::from_value(invalid).unwrap();
            assert!(batch.requests().is_err());
        }
    }
}
