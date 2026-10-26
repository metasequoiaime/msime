//! 设置页插件编辑器经 `msime_client_plugins` 的两项操作：`authoring_save` 新建、生成 id、拒绝覆盖、带 `replace` 替换，`authoring_load` 读回同一份结构，以及内置包和没装的包的拒绝。Windows 的原生设置应用按同一份 JSON 契约调用。

use super::*;

fn plugins_call(state: &std::path::Path) -> impl Fn(Value) -> Value + '_ {
    move |action: Value| {
        let request = serde_json::to_vec(
            &json!({"state_root": state, "sound_packs": null, "action": action}),
        )
        .unwrap();
        read(unsafe { msime_client_plugins(request.as_ptr(), request.len()) })
    }
}

#[test]
fn plugins_boundary_authors_each_kind_and_reads_it_back() {
    let state = tempfile::tempdir().unwrap();
    let call = plugins_call(state.path());
    let content = [
        (
            "phrase_table",
            json!({"phrases": [{"key": "dh", "text": "合成电话"}]}),
        ),
        (
            "command_table",
            json!({"commands": [{"trigger": "sig", "title": "签名", "template": "{date} 合成"}]}),
        ),
        (
            "symbol_set",
            json!({"groups": [{"tab": "symbols", "title": "箭头", "keywords": "jiantou", "items": ["→", "←"]}]}),
        ),
        (
            "wordbook",
            json!({"words": [{"word": "apple", "phonetic": null, "gloss": "n. 苹果"}]}),
        ),
    ];
    for (kind, fields) in content {
        let mut pack = json!({
            "kind": kind,
            "id": null,
            "name": "合成插件",
            "version": "1.0.0",
            "license": "CC0-1.0",
            "author": null,
            "description": "合成的介绍",
        });
        pack.as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        let saved = call(json!({"operation": "authoring_save", "pack": pack, "replace": false}));
        assert_eq!(saved["ok"], true, "{saved}");
        assert_eq!(saved["value"]["kind"], kind);
        let id = saved["value"]["id"].as_str().unwrap().to_owned();
        assert!(id.starts_with("my-") && id.len() == 11, "{id}");
        assert!(state
            .path()
            .join(format!("plugins/{kind}/{id}/plugin.toml"))
            .is_file());

        let loaded = call(json!({"operation": "authoring_load", "kind": kind, "id": id}));
        assert_eq!(loaded["ok"], true, "{loaded}");
        let mut expected = pack.clone();
        expected["id"] = json!(id);
        assert_eq!(loaded["value"], expected);

        // 新建时同 id 已经装着就拒绝，带 replace 才替换。
        let mut renamed = expected.clone();
        renamed["name"] = json!("改过的名字");
        let refused =
            call(json!({"operation": "authoring_save", "pack": renamed, "replace": false}));
        assert_eq!(refused, json!({"ok": false, "error": "plugin_exists"}));
        let replaced =
            call(json!({"operation": "authoring_save", "pack": renamed, "replace": true}));
        assert_eq!(replaced["value"]["name"], "改过的名字", "{replaced}");
    }
    let catalog = call(json!({"operation": "catalog"}));
    assert_eq!(catalog["value"]["packages"].as_array().unwrap().len(), 4);
}

#[test]
fn plugins_boundary_refuses_what_the_editor_cannot_author() {
    let state = tempfile::tempdir().unwrap();
    let call = plugins_call(state.path());
    let pack = |phrases: Value| {
        json!({
            "kind": "phrase_table", "id": null, "name": "合成", "version": "1",
            "license": "CC0-1.0", "author": null, "description": null, "phrases": phrases,
        })
    };

    // 内容不合规时是导入同一条规则的 plugin_invalid。
    let invalid = call(json!({
        "operation": "authoring_save",
        "pack": pack(json!([{"key": "DH", "text": "电话"}])),
        "replace": false,
    }));
    assert_eq!(
        invalid,
        json!({"ok": false, "error": "plugin_invalid", "detail": "短语编码 DH 必须是 1 到 32 个小写字母"})
    );
    let mut reserved = pack(json!([{"key": "dh", "text": "电话"}]));
    reserved["id"] = json!("latex-symbols");
    assert_eq!(
        call(json!({"operation": "authoring_save", "pack": reserved, "replace": true}))["error"],
        "plugin_reserved"
    );
    // 编辑器以外的类型和多出来的字段是页面不会发出的请求。
    let mut sound = pack(json!([]));
    sound["kind"] = json!("sound");
    assert_eq!(
        call(json!({"operation": "authoring_save", "pack": sound, "replace": false}))["error"],
        "invalid"
    );
    let mut extra = pack(json!([{"key": "dh", "text": "电话"}]));
    extra["permissions"] = json!(["read_commit"]);
    assert_eq!(
        call(json!({"operation": "authoring_save", "pack": extra, "replace": false}))["error"],
        "invalid"
    );

    for (kind, id) in [
        ("phrase_table", "latex-symbols"),
        ("sound", "default"),
        ("helpcode", "custom-x"),
        ("phrase_table", "not-installed"),
    ] {
        let refused = call(json!({"operation": "authoring_load", "kind": kind, "id": id}));
        assert_eq!(
            refused["error"], "plugin_not_editable",
            "{kind}/{id}: {refused}"
        );
        assert!(refused["detail"]
            .as_str()
            .is_some_and(|detail| !detail.is_empty()));
    }
    assert!(call(json!({"operation": "catalog"}))["value"]["packages"]
        .as_array()
        .unwrap()
        .is_empty());
}
