//! 账号释义单条的长度上限（#8201 迁移计划 P8）：`msime_client_account_gloss` 的 parse 把规范化后超过 4096 字节的一项置为 null，宿主把非 null 的几项原样交给 `msime_client_apply_translations` 时不会因为那一项整批被拒。

use super::*;
use crate::tests::{read, test_host};

#[test]
fn candidate_translation_runs_share_original_utf8_text_and_identity() {
    let dir = tempfile::tempdir().unwrap();
    let handle = test_host(dir.path());
    read(msime_client_focus(handle, true));
    let mut view = Value::Null;
    for byte in b"U4e2d" {
        view = read(msime_client_character(
            handle,
            *byte,
            byte.is_ascii_uppercase(),
        ))["value"]["view"]
            .clone();
    }
    let before = &view["candidates"][0];
    assert!(before.get("translation_runs").is_none());
    let translation = "n. 合成； v. synthetic";
    let payload =
        serde_json::to_vec(&json!([{ "text": before["text"], "translation": translation }]))
            .unwrap();
    // SAFETY: 合成缓冲区在调用期间有效，返回字符串交由 read 释放。
    let applied = read(unsafe {
        msime_client_apply_translations(
            handle,
            view["generation"].as_u64().unwrap(),
            payload.as_ptr(),
            payload.len(),
        )
    });
    let after = &applied["value"]["view"]["candidates"][0];
    assert_eq!(after["id"], before["id"]);
    assert_eq!(after["text"], before["text"]);
    assert_eq!(after["translation"], translation);
    let expected = json!([
        { "start": 0, "length": 2, "role": "pos" },
        { "start": 9, "length": 3, "role": "separator" },
        { "start": 13, "length": 2, "role": "pos" }
    ]);
    assert_eq!(after["translation_runs"], expected);
    let all = read(msime_client_all_candidates(handle));
    assert_eq!(all["value"]["candidates"][0]["translation_runs"], expected);
    read(msime_client_destroy(handle));
}

fn gloss(request: Value) -> Value {
    let bytes = request.to_string();
    // SAFETY: 缓冲区在调用期间有效，返回的字符串由 read 释放。
    read(unsafe { msime_client_account_gloss(bytes.as_ptr(), bytes.len()) })
}

#[test]
fn an_over_long_gloss_is_null_and_the_rest_of_the_batch_still_applies() {
    let dir = tempfile::tempdir().unwrap();
    let handle = test_host(dir.path());
    read(msime_client_focus(handle, true));
    let mut view = Value::Null;
    for byte in b"U4e2d" {
        view = read(msime_client_character(
            handle,
            *byte,
            byte.is_ascii_uppercase(),
        ))["value"]["view"]
            .clone();
    }
    let generation = view["generation"].as_u64().unwrap();
    let candidate = view["candidates"][0]["text"].as_str().unwrap().to_owned();

    // 同一个候选问了两次，服务端第一项回了 4097 字节。
    let texts = json!([candidate, candidate]);
    let parsed = gloss(json!({
        "operation": "parse",
        "texts": texts,
        "status": 200,
        "body": json!({ "code": 200, "data": ["m".repeat(4097), "合成释义"] }).to_string(),
    }));
    assert_eq!(
        parsed,
        json!({ "ok": true, "value": { "translations": [null, "合成释义"] } })
    );

    let pairs: Vec<Value> = texts
        .as_array()
        .unwrap()
        .iter()
        .zip(parsed["value"]["translations"].as_array().unwrap())
        .filter(|(_, translation)| !translation.is_null())
        .map(|(text, translation)| json!({ "text": text, "translation": translation }))
        .collect();
    let payload = serde_json::to_vec(&pairs).unwrap();
    // SAFETY: 缓冲区在调用期间有效，返回的字符串由 read 释放。
    let applied = read(unsafe {
        msime_client_apply_translations(handle, generation, payload.as_ptr(), payload.len())
    });
    assert_eq!(applied["ok"], true, "{applied}");
    assert_eq!(applied["value"]["applied"], true);
    assert_eq!(
        applied["value"]["view"]["candidates"][0]["translation"],
        "合成释义"
    );

    // 正好 4096 字节的一项照样可用，也照样能交给 apply_translations。
    let longest = "m".repeat(4096);
    let parsed = gloss(json!({
        "operation": "parse",
        "texts": [candidate],
        "status": 200,
        "body": json!({ "code": 200, "data": [longest] }).to_string(),
    }));
    assert_eq!(parsed["value"]["translations"], json!([longest]));
    read(msime_client_destroy(handle));
}
