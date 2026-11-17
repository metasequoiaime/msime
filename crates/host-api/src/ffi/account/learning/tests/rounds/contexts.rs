//! 运行时选项的词库身份必须跟随该会话，而非后台新选择；注入存储复用同一数据入口。

use super::*;

#[test]
fn context_uses_retained_session_manifest_and_edition_scheme_set() {
    let harness = Harness::new();
    let baseline = Edition::by_id("vietnamese")
        .unwrap()
        .resource_set()
        .unwrap();
    let mut retained = baseline.clone();
    retained.source_commit = "b".repeat(40);
    let download = "https://github.com/metasequoiaime/msime-dictionary/releases/download/";
    for artifact in &mut retained.artifacts {
        if artifact.url.starts_with(download) {
            artifact.url = format!("{download}dict-v999.0.0/{}", artifact.name);
        }
    }
    let content = retained.generation().unwrap();
    assert_ne!(content, harness.content);
    let root = harness._root.path().join("dictionary-updates");
    let generations = root.join("generations");
    let resources =
        msime_client_providers::dictionary_updates::directory(harness._root.path(), &retained)
            .unwrap();
    std::fs::create_dir_all(&resources).unwrap();
    let selection = msime_client_providers::dictionary_updates::Selection {
        baseline_generation: harness.content.clone(),
        version: "dict-v999.0.0".into(),
        desktop: retained,
        languages: None,
        bundled_resources: harness.paths.resources.clone(),
    };
    std::fs::write(
        generations.join(format!("{content}.json")),
        serde_json::to_vec(&selection).unwrap(),
    )
    .unwrap();
    // 当前选择没有引用这份清单，但已有输入会话仍在使用它。
    std::fs::write(
        root.join("installed.json"),
        br#"{"active":null,"previous":null}"#,
    )
    .unwrap();
    let mut options = harness.options.clone();
    options["resources"] = json!(resources);
    options["dictionaries"] = json!(harness.paths.user_data.join("dictionaries").join(&content));
    options["candidate_skin_catalog"] = json!([]);
    let context = match data::Context::parse(options.clone()) {
        Ok(context) => context,
        Err(_) => panic!("retained context rejected"),
    };
    assert_eq!(context.borrowed().content_id, content);
    assert!(!context.borrowed().main_dictionary);
    // 清单消失后不能退回基线并继续声称是保留的代次。
    std::fs::remove_file(generations.join(format!("{content}.json"))).unwrap();
    assert!(data::Context::parse(options).is_err());
    let mut full = harness.options.clone();
    let content = Edition::full()
        .resource_set()
        .unwrap()
        .generation()
        .unwrap();
    full.as_object_mut().unwrap().remove("edition");
    full["dictionaries"] = json!(harness.paths.user_data.join("dictionaries").join(content));
    let context = match data::Context::parse(full) {
        Ok(context) => context,
        Err(_) => panic!("full context rejected"),
    };
    assert!(context.borrowed().main_dictionary);
}

#[test]
fn data_action_requires_precise_fields_before_creating_keys() {
    let harness = Harness::new();
    for operation in ["learning_sync_once", "learning_merge_pending"] {
        let mut valid =
            json!({"operation": operation, "login": harness.login, "options": harness.options});
        if operation == "learning_sync_once" {
            valid["device_name"] = json!("合成 A");
        }
        let fields: Vec<_> = valid.as_object().unwrap().keys().cloned().collect();
        let mut cases = Vec::new();
        for field in fields {
            let mut invalid = valid.clone();
            invalid.as_object_mut().unwrap().remove(&field);
            cases.push(invalid);
        }
        for field in ["content_id", "main_dictionary", "authorized"] {
            let mut invalid = valid.clone();
            invalid[field] = json!(true);
            cases.push(invalid);
        }
        for invalid in cases {
            assert_eq!(
                call(&harness.source, invalid),
                json!({"ok": false, "error": "account_invalid"})
            );
        }
        let mut read_only = valid;
        read_only["options"]["read_only_state"] = json!(true);
        assert_eq!(
            call(&harness.source, read_only),
            json!({"ok": false, "error": "learning_read_only"})
        );
    }
    assert!(harness.storage.load_learning_keystore().unwrap().is_none());
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}

#[test]
fn injected_offline_merge_uses_independent_learning_slot() {
    let harness = Harness::new();
    harness.ready();
    let store = leaked_store();
    let saved = harness.storage.load().unwrap().unwrap();
    let mut items = store.items.lock().unwrap();
    items.insert(1, serde_json::to_vec(&saved).unwrap());
    items.insert(
        5,
        harness
            .storage
            .load_learning_keystore()
            .unwrap()
            .unwrap()
            .into_bytes(),
    );
    items.insert(4, b"synthetic-clipboard-sentinel".to_vec());
    drop(items);
    let id = registered(store, 2, 0);
    let source = json!({"session_store": id});
    let login = binding(&source);
    assert_eq!(login, harness.login);
    // 离线入口也从该存储读取学习槽，不会读取或替换剪贴板槽。
    let response = call(
        &source,
        json!({"operation": "learning_merge_pending", "login": login,
        "options": harness.options}),
    );
    assert_eq!(response, json!({"ok": true, "value": {"pending": []}}));
    assert_eq!(
        store.items.lock().unwrap().get(&4).unwrap(),
        b"synthetic-clipboard-sentinel"
    );
    assert_eq!(unregister(id)["ok"], true);
}
