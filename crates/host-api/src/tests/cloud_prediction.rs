use super::*;
use msime_client_core::cloud::candidates::{
    CloudCandidatePreferences, CloudCandidateSource, CloudHttpMethod,
};

fn prediction_host(root: &std::path::Path) -> (u64, Preferences, String) {
    let preferences = Preferences {
        scheme: InputScheme::Quanpin,
        next_word_prediction: true,
        cloud_candidates: true,
        cloud_candidate_provider: CloudCandidatePreferences {
            source: CloudCandidateSource::Custom,
            endpoint: "https://prediction.example.invalid/suffix".into(),
            method: CloudHttpMethod::Post,
            prediction_enabled: true,
            cache_enabled: false,
            ..Default::default()
        },
        ..chinese_preferences()
    };
    let handle = test_host_with_pinyin_fixture(root, preferences.clone());
    // Linux 上验证 Android/macOS 同一 FFI 路径；只注入宿主已选择的引擎选项，不改变生产门禁。
    SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let session = sessions.get_mut(&handle).unwrap();
        let mut options = session.options.clone();
        options.next_word_prediction = true;
        session
            .runtime
            .replace_engine(Session::new(&options).unwrap(), 6)
            .unwrap();
    });
    read(msime_client_focus(handle, true));
    let view = type_letters(handle, b"nihao");
    let candidate = view["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["text"] == "本地")
        .unwrap();
    let picked = read(msime_client_select(
        handle,
        candidate["id"]["generation"].as_u64().unwrap(),
        candidate["id"]["index"].as_u64().unwrap() as usize,
    ));
    assert_eq!(picked["value"]["commit"], "本地");
    assert_eq!(picked["value"]["view"]["prediction"], true);
    let query = read(msime_client_online_query(handle))["value"].to_string();
    (handle, preferences, query)
}

fn reply(handle: u64, query: &str) -> Value {
    let body =
        serde_json::to_vec(&json!({"candidates":[{"text":"后续"},{"text":"测试"}]})).unwrap();
    read(unsafe {
        msime_client_apply_cloud_response(
            handle,
            query.as_ptr(),
            query.len(),
            body.as_ptr(),
            body.len(),
        )
    })
}

#[test]
fn cloud_prediction_ffi_uses_explicit_permission_and_commits_only_the_chosen_suffix() {
    let directory = tempfile::tempdir().unwrap();
    let (handle, _, query) = prediction_host(directory.path());
    let parsed: Value = serde_json::from_str(&query).unwrap();
    assert_eq!(parsed["mode"], "prediction");
    assert_eq!(parsed["query_text"], "本地");
    assert_eq!(parsed["ai_eligible"], false);
    let body: Value =
        serde_json::from_str(parsed["cloud_request"]["body_utf8"].as_str().unwrap()).unwrap();
    assert_eq!(body["mode"], "prediction");
    assert_eq!(body["context"], "本地");
    let applied = reply(handle, &query);
    assert_eq!(applied["value"]["applied"], true, "{applied}");
    assert_eq!(applied["value"]["view"]["prediction"], true);
    let candidate = &applied["value"]["view"]["candidates"][0];
    let committed = read(msime_client_select(
        handle,
        candidate["id"]["generation"].as_u64().unwrap(),
        candidate["id"]["index"].as_u64().unwrap() as usize,
    ));
    assert_eq!(committed["value"]["commit"], "后续");
    assert_eq!(reply(handle, &query)["value"]["applied"], false);
    read(msime_client_destroy(handle));
}

#[test]
fn cloud_prediction_ffi_revocation_and_privacy_reject_in_flight_replies() {
    for private in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let (handle, mut preferences, query) = prediction_host(directory.path());
        if private {
            read(msime_client_set_private_session(handle, true));
        } else {
            preferences.cloud_candidate_provider.prediction_enabled = false;
            assert_eq!(update(handle, 1, &preferences)["ok"], true);
        }
        assert_eq!(reply(handle, &query)["value"]["applied"], false);
        read(msime_client_destroy(handle));
    }
}

#[test]
fn cloud_prediction_baidu_source_uses_the_real_engine_query_and_rejects_error_envelopes() {
    let directory = tempfile::tempdir().unwrap();
    let preferences = Preferences {
        scheme: InputScheme::Quanpin,
        cloud_candidates: true,
        cloud_candidate_provider: CloudCandidatePreferences {
            source: CloudCandidateSource::Baidu,
            cache_enabled: false,
            ..Default::default()
        },
        ..chinese_preferences()
    };
    let handle = test_host_with_pinyin_fixture(directory.path(), preferences);
    read(msime_client_focus(handle, true));
    type_letters(handle, b"nihao");
    let query = read(msime_client_online_query(handle))["value"].clone();
    assert_eq!(query["cloud_eligible"], true);
    let request_url = query["cloud_request"]["url"].as_str().unwrap();
    assert!(request_url.starts_with("https://olimenew.baidu.com/py?input="));
    assert!(request_url.contains("ni%27hao"));
    let text = query.to_string();
    let apply = |body: Value| {
        let bytes = serde_json::to_vec(&body).unwrap();
        read(unsafe {
            msime_client_apply_cloud_response(
                handle,
                text.as_ptr(),
                text.len(),
                bytes.as_ptr(),
                bytes.len(),
            )
        })
    };
    assert_eq!(
        apply(json!({"status":"F","errno":"1","result":[[["拒绝"]]]}))["value"]["applied"],
        false
    );
    let applied =
        apply(json!({"status":"T","errno":"0","result":[[["你好",5,{"pinyin":"ni'hao"}]]]}));
    assert_eq!(applied["value"]["applied"], true, "{applied}");
    assert!(applied["value"]["view"]["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["text"] == "你好" && row["source"] == 2));
    read(msime_client_destroy(handle));
}
