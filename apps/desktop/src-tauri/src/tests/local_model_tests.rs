//! The on-device model commands and the hotword plumbing a `local` voice session starts with.
use crate::voice::local_models::{self, LocalModelInstalls};
use msime_client_core::voice::local_models::LocalModelError;
use serde_json::json;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use serde_json::Value;

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn invoke(command: &str, body: Value) -> Result<Value, Value> {
    let app = tauri::test::mock_builder()
        .manage(LocalModelInstalls::default())
        .invoke_handler(tauri::generate_handler![
            local_models::voice_local_models,
            local_models::voice_local_model_cancel,
            local_models::voice_local_model_remove,
        ])
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("mock application");
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock window");
    tauri::test::get_ipc_response(
        &window,
        tauri::webview::InvokeRequest {
            cmd: command.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: if cfg!(target_os = "windows") {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    )
    .map(|response| response.deserialize::<Value>().expect("JSON response"))
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[test]
fn listing_models_names_the_catalog_under_the_app_voice_model_root() {
    let response = invoke("voice_local_models", json!({})).expect("list resolves");
    let root = response["root"].as_str().unwrap();
    assert!(std::path::Path::new(root).is_absolute());
    assert!(root.ends_with("voice-models"));
    assert_eq!(response["default"], "x-asr-zh-en-streaming");
    let models = response["models"].as_array().unwrap();
    assert!(models
        .iter()
        .any(|model| model["id"] == "x-asr-zh-en-streaming"
            && model["path"]
                .as_str()
                .is_some_and(|path| path.starts_with(root))));
    assert!(models
        .iter()
        .any(|model| model["id"] == "fun-asr-nano" && model["desktop_only"] == true));
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[test]
fn model_commands_refuse_ids_that_are_not_catalog_slugs() {
    for id in [
        "",
        "../x-asr-zh-en-streaming",
        "a/b",
        "x".repeat(129).as_str(),
    ] {
        assert_eq!(
            invoke("voice_local_model_remove", json!({ "id": id })),
            Err(json!({"code": "local_model_unknown"}))
        );
        assert_eq!(
            invoke("voice_local_model_cancel", json!({ "id": id })),
            Err(json!({"code": "local_model_unknown"}))
        );
    }
    assert_eq!(
        invoke(
            "voice_local_model_cancel",
            json!({"id": "x-asr-zh-en-streaming"})
        ),
        Ok(json!(false))
    );
}

#[test]
fn installs_run_once_per_model_and_cancel_only_their_own_flag() {
    let installs = LocalModelInstalls::default();
    let first = installs.begin("x-asr-zh-en-streaming").unwrap();
    let other = installs.begin("sense-voice-small").unwrap();
    assert!(installs.begin("x-asr-zh-en-streaming").is_none());
    assert!(installs.running("x-asr-zh-en-streaming"));

    assert!(installs.cancel("x-asr-zh-en-streaming"));
    assert!(first.load(std::sync::atomic::Ordering::Acquire));
    assert!(!other.load(std::sync::atomic::Ordering::Acquire));

    installs.finish("x-asr-zh-en-streaming");
    assert!(!installs.running("x-asr-zh-en-streaming"));
    assert!(!installs.cancel("x-asr-zh-en-streaming"));
    assert!(installs.begin("x-asr-zh-en-streaming").is_some());
}

#[test]
fn removing_reserves_a_model_slot_against_a_new_install() {
    let installs = LocalModelInstalls::default();
    assert!(installs.begin_remove("x-asr-zh-en-streaming"));
    assert!(installs.begin("x-asr-zh-en-streaming").is_none());
    assert!(installs.running("x-asr-zh-en-streaming"));

    installs.finish("x-asr-zh-en-streaming");
    assert!(!installs.running("x-asr-zh-en-streaming"));
    assert!(installs.begin("x-asr-zh-en-streaming").is_some());
}

#[test]
fn install_failures_map_to_the_codes_the_settings_page_knows() {
    for (error, code) in [
        (LocalModelError::Cancelled, "local_model_cancelled"),
        (
            LocalModelError::Network("fixture".into()),
            "local_model_network",
        ),
        (LocalModelError::HttpStatus(404), "local_model_http_status"),
        (
            LocalModelError::ChecksumMismatch("fixture".into()),
            "local_model_checksum_mismatch",
        ),
        (
            LocalModelError::SizeMismatch("fixture".into()),
            "local_model_checksum_mismatch",
        ),
        (
            LocalModelError::UnsafeArchive("fixture".into()),
            "local_model_invalid_archive",
        ),
        (LocalModelError::InvalidMirror, "local_model_invalid_mirror"),
        (
            LocalModelError::Io(std::io::Error::other("fixture")),
            "local_model_io",
        ),
    ] {
        assert_eq!(local_models::local_model_error_code(&error), code);
    }
    assert_eq!(
        local_models::local_model_root(std::path::Path::new("/fixture/app")),
        std::path::PathBuf::from("/fixture/app/voice-models")
    );
}

#[cfg(unix)]
#[test]
fn session_hotwords_page_the_users_pinyin_words_heaviest_first() {
    let mut requests = Vec::new();
    let hotwords = local_models::dictionary_hotwords_with(200, |action| {
        requests.push(action.clone());
        let offset = action["offset"].as_u64().unwrap();
        Some(if offset == 0 {
            json!({
                "entries": [
                    {"kind": "pinyin", "key": "shui'shan", "value": "水杉", "weight": 10},
                    {"kind": "pinyin", "key": "a", "value": "啊", "weight": 900},
                    {"kind": "pinyin", "key": "abc", "value": "abc", "weight": 900},
                ],
                "has_more": true,
            })
        } else {
            json!({
                "entries": [
                    {"kind": "pinyin", "key": "xiang'mu", "value": "项目", "weight": 50},
                ],
                "has_more": false,
            })
        })
    });
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["operation"], "list");
    assert_eq!(requests[0]["kind"], "pinyin");
    assert_eq!(requests[0]["user_only"], true);
    assert_eq!(requests[1]["offset"], 3);
    let texts: Vec<&str> = hotwords.iter().map(|word| word.text.as_str()).collect();
    assert_eq!(texts, ["项目", "水杉"]);
    assert_eq!(hotwords[1].pinyin, "shui shan");

    // A dictionary that cannot be read gives no hotwords rather than failing the session.
    assert!(local_models::dictionary_hotwords_with(200, |_| None).is_empty());
    assert!(
        local_models::dictionary_hotwords_with(0, |_| panic!("no read for no words")).is_empty()
    );
}

#[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
#[test]
fn desktop_local_sessions_forward_the_model_path_and_hotwords_that_fit() {
    let document = json!({
        "preferences": {"voice_input": {
            "asr_provider": "local",
            "asr_model_path": format!("/models/{}", "m".repeat(700)),
        }}
    });
    let mut options = crate::voice::voice_provider_options(&document).unwrap();
    // Paths are passed whole, never cut at the 512 bytes the names are.
    assert_eq!(
        options["asr_model_path"].as_str().unwrap().len(),
        "/models/".len() + 700
    );

    let hotwords: Vec<_> = (0..2_000)
        .map(|index| msime_client_core::voice::hotwords::Hotword {
            text: format!("词{index}"),
            pinyin: "ci".into(),
        })
        .collect();
    local_models::add_hotwords_within(
        &mut options,
        &hotwords,
        crate::voice::PROVIDER_OPTIONS_BUDGET,
    );
    // The provider accepts only boolean and string options, so the words travel as the `text<TAB>pinyin` lines the Linux hosts send.
    assert!(options.get("hotwords").is_none());
    assert!(options
        .as_object()
        .unwrap()
        .values()
        .all(|value| value.is_string() || value.is_boolean()));
    let packed = options["voice_hotwords"].as_str().unwrap();
    let lines: Vec<&str> = packed.split('\n').collect();
    assert!(!lines.is_empty() && lines.len() < hotwords.len());
    assert_eq!(lines[0], "词0\tci");
    assert_eq!(lines[1], "词1\tci");
    assert!(options.to_string().len() <= crate::voice::PROVIDER_OPTIONS_BUDGET);
    // One more word would not have fitted.
    let next = format!("\\n词{}\\tci", lines.len());
    assert!(options.to_string().len() + next.len() > crate::voice::PROVIDER_OPTIONS_BUDGET);

    let mut full = json!({"asr_provider": "local"});
    local_models::add_hotwords_within(&mut full, &[], 100);
    assert!(full.get("voice_hotwords").is_none());

    // A word that would break the line packing is skipped, not sent torn.
    let mut skipped = json!({"asr_provider": "local"});
    local_models::add_hotwords_within(
        &mut skipped,
        &[
            msime_client_core::voice::hotwords::Hotword {
                text: "水\n杉".into(),
                pinyin: "shui shan".into(),
            },
            msime_client_core::voice::hotwords::Hotword {
                text: "水杉".into(),
                pinyin: "shui\tshan".into(),
            },
            msime_client_core::voice::hotwords::Hotword {
                text: "输入法".into(),
                pinyin: "shu ru fa".into(),
            },
        ],
        crate::voice::PROVIDER_OPTIONS_BUDGET,
    );
    assert_eq!(skipped["voice_hotwords"], "输入法\tshu ru fa");

    for path in ["", "/models/\nx"] {
        let document = json!({"preferences": {"voice_input": {"asr_model_path": path}}});
        let options = crate::voice::voice_provider_options(&document).unwrap();
        assert!(options.get("asr_model_path").is_none());
    }
}
