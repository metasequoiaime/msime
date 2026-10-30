//! Native management roundtrip with synthetic entries and isolated writable data.

// Integration tests and examples are their own crates, so the exemption the
// library root carries does not reach them. Same boundary, same reason: this
// target drives the C ABI directly.
#![allow(unsafe_code)]
use msime_host_api::*;
use serde_json::{json, Value};
use std::ffi::{c_char, CStr};

fn read(pointer: *mut c_char) -> Value {
    assert!(!pointer.is_null());
    // SAFETY: every pointer here is an owned host-api response.
    let value = unsafe { serde_json::from_slice(CStr::from_ptr(pointer).to_bytes()).unwrap() };
    unsafe { msime_client_string_free(pointer) };
    value
}
fn request(options: &Value, action: Value) -> Value {
    let bytes = serde_json::to_vec(&json!({ "options": options, "action": action })).unwrap();
    read(unsafe { msime_client_dictionary(bytes.as_ptr(), bytes.len()) })
}
fn create(options: &Value) -> u64 {
    let bytes = serde_json::to_vec(options).unwrap();
    let result = read(unsafe { msime_client_create(bytes.as_ptr(), bytes.len()) });
    assert_eq!(result["ok"], true);
    result["value"]["session"].as_u64().unwrap()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resources = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("usage: dictionary_requests <verified-resources>")?,
    );
    let state = tempfile::tempdir()?;
    let options: Value =
        serde_json::from_str(&prepare_host_configuration(&resources, state.path())?)?;
    let entry =
        json!({ "kind": "quick_phrase", "key": "fixture", "value": "测试短语", "weight": 12345 });
    let add = json!({ "operation": "edit", "previous": null, "replacement": entry, "request_id": "native-add" });
    let list = json!({ "operation": "list", "offset": 0, "limit": 100 });
    let session = create(&options);
    assert_eq!(request(&options, list.clone())["ok"], true);
    assert_eq!(
        request(&options, add.clone())["error"],
        "dictionary maintenance busy"
    );
    read(msime_client_destroy(session));
    assert_eq!(request(&options, add.clone())["value"]["applied"], true);
    assert_eq!(request(&options, add)["value"]["applied"], true);
    assert_eq!(
        request(&options, list.clone())["value"]["entries"],
        json!([entry])
    );
    let session = create(&options);
    read(msime_client_focus(session, true));
    read(msime_client_character(session, b'K', true));
    for byte in b"fixture" {
        read(msime_client_character(session, *byte, false));
    }
    let result = read(msime_client_command(session, 1));
    assert_eq!(result["value"]["commit"], "测试短语");
    read(msime_client_destroy(session));
    let remove = json!({ "operation": "edit", "previous": entry, "replacement": null, "request_id": "native-remove" });
    assert_eq!(request(&options, remove.clone())["ok"], true);
    assert_eq!(request(&options, remove)["ok"], true);
    assert_eq!(
        request(&options, list.clone())["value"]["entries"],
        json!([])
    );

    // A code typed in upper case is the same code, and a weight of zero is a weight the Engine
    // will not store. Both are folded here rather than refused: the Engine lowercases the key
    // itself and the reference's settings page does the same at its own boundary, while a row
    // lost over a rank difference of one is a word the user does not get back.
    let shouted = json!({ "kind": "quick_phrase", "key": "QQ", "value": "企鹅", "weight": 0 });
    let add_shouted = json!({ "operation": "edit", "previous": null, "replacement": shouted.clone(), "request_id": "native-fold" });
    assert_eq!(request(&options, add_shouted)["value"]["applied"], true);
    let stored = request(&options, list.clone());
    assert_eq!(
        stored["value"]["entries"],
        json!([{ "kind": "quick_phrase", "key": "qq", "value": "企鹅", "weight": 1 }]),
        "the stored entry is the folded one"
    );

    // And it is reachable by typing the code, which is the point of folding it rather than
    // storing what was typed into the box.
    let session = create(&options);
    read(msime_client_focus(session, true));
    read(msime_client_character(session, b'K', true));
    for byte in b"qq" {
        read(msime_client_character(session, *byte, false));
    }
    let typed = read(msime_client_command(session, 1));
    assert_eq!(typed["value"]["commit"], "企鹅");
    read(msime_client_destroy(session));

    // Removing it by its code as typed works, because both sides fold the code the same way. The weight is matched as stored: a stored row can carry 0 (a derived wubi word does), so the previous row's weight is not lifted the way a new entry's is.
    let stored_shouted =
        json!({ "kind": "quick_phrase", "key": "QQ", "value": "企鹅", "weight": 1 });
    let remove_shouted = json!({ "operation": "edit", "previous": stored_shouted, "replacement": null, "request_id": "native-fold-remove" });
    assert_eq!(request(&options, remove_shouted)["ok"], true);
    assert_eq!(
        request(&options, list.clone())["value"]["entries"],
        json!([])
    );

    // An English code and the word it types out are two different texts. `dont` types out `don't`, which is the case the reference's own importer exists to accept: an English entry's word does not have to be its code again letter for letter, because the table keeps the code and the displayed word in separate columns.
    let contraction = json!({ "kind": "english", "key": "dont", "value": "don't", "weight": 4096 });
    let add_contraction = json!({ "operation": "edit", "previous": null, "replacement": contraction.clone(), "request_id": "native-english-display" });
    assert_eq!(
        request(&options, add_contraction)["value"]["applied"],
        true,
        "an English word may differ from the code that types it"
    );
    assert_eq!(
        request(&options, list.clone())["value"]["entries"],
        json!([contraction]),
        "and it is stored as both texts rather than folded into one"
    );

    // A code carrying an apostrophe of its own is the other half of the reference's rule: it asks
    // only that the code be letters, hyphens and apostrophes.
    let hyphenated =
        json!({ "kind": "english", "key": "e-mail", "value": "e-mail", "weight": 4096 });
    let add_hyphenated = json!({ "operation": "edit", "previous": null, "replacement": hyphenated.clone(), "request_id": "native-english-hyphen" });
    assert_eq!(
        request(&options, add_hyphenated)["value"]["applied"],
        true,
        "a hyphen belongs to the code as much as to the word"
    );

    // The point of storing two texts: typing the code offers the word. Temporary English mode is
    // Shift+Y, the same key the reference documents for it.
    let session = create(&options);
    read(msime_client_focus(session, true));
    read(msime_client_character(session, b'Y', true));
    for byte in b"dont" {
        read(msime_client_character(session, *byte, false));
    }
    let view = read(msime_client_view(session));
    let offered: Vec<String> = view["value"]["candidates"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|candidate| candidate["text"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        offered.iter().any(|text| text == "don't"),
        "typing the code offers the word it types out; got {offered:?}"
    );
    read(msime_client_destroy(session));

    let remove_contraction = json!({ "operation": "edit", "previous": contraction, "replacement": null, "request_id": "native-english-remove" });
    assert_eq!(request(&options, remove_contraction)["ok"], true);
    let remove_hyphenated = json!({ "operation": "edit", "previous": hyphenated, "replacement": null, "request_id": "native-english-hyphen-remove" });
    assert_eq!(request(&options, remove_hyphenated)["ok"], true);
    assert_eq!(
        request(&options, list.clone())["value"]["entries"],
        json!([])
    );

    // Full pinyin typed the way people type it. The add form hands over `nihao`; it is cut into the syllables the Engine stores, as an imported row is, and a search without separators - or one that stops inside a syllable - finds it.
    let typed = json!({ "kind": "pinyin", "key": "nihao", "value": "你好", "weight": 10000 });
    let stored = json!({ "kind": "pinyin", "key": "ni'hao", "value": "你好", "weight": 10000 });
    let add_typed = json!({ "operation": "edit", "previous": null, "replacement": typed, "request_id": "native-pinyin-unseparated" });
    assert_eq!(
        request(&options, add_typed)["value"]["applied"],
        true,
        "full pinyin without separators is saved"
    );
    for query in ["nihao", "nih", "ni hao", "ni'hao"] {
        let search = json!({ "operation": "list", "offset": 0, "limit": 100, "kind": "pinyin", "query": query });
        assert_eq!(
            request(&options, search)["value"]["entries"],
            json!([stored]),
            "{query:?} finds the separated key"
        );
    }
    // Jianpin is refused, and the reason survives to the caller instead of a generic rejection.
    let jianpin = json!({ "kind": "pinyin", "key": "nhao", "value": "你好", "weight": 10000 });
    let add_jianpin = json!({ "operation": "edit", "previous": null, "replacement": jianpin, "request_id": "native-pinyin-jianpin" });
    let refused = request(&options, add_jianpin);
    assert_eq!(refused["ok"], false);
    assert!(
        refused["error"]
            .as_str()
            .is_some_and(|reason| reason.starts_with("invalid dictionary entry: ")),
        "the refusal names the rule; got {refused}"
    );
    let remove_stored = json!({ "operation": "edit", "previous": stored, "replacement": null, "request_id": "native-pinyin-remove" });
    assert_eq!(request(&options, remove_stored)["ok"], true);
    assert_eq!(request(&options, list)["value"]["entries"], json!([]));

    println!(
        "native list/busy/add/retry/recreate/commit/remove/fold/english-display/pinyin-search roundtrip passed"
    );
    Ok(())
}
