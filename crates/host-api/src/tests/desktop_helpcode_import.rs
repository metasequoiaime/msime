use super::*;

#[test]
fn desktop_plugin_picker_installs_the_selected_helpcode_table() {
    let fixture = tempfile::tempdir().unwrap();
    let state = fixture.path().join("state");
    let source = fixture.path().join("合成码表.TXT");
    let call = |action| {
        let request = serde_json::to_vec(&json!({
            "state_root": state,
            "script_page": true,
            "action": action,
        }))
        .unwrap();
        read(unsafe { msime_client_plugins(request.as_ptr(), request.len()) })
    };
    let import = || call(json!({"operation": "import", "source": source}));

    std::fs::write(&source, "\u{FEFF}# name: 合成辅助码\r\n你=zz\r\n").unwrap();
    let imported = import();
    assert_eq!(imported["ok"], true, "{imported}");
    assert_eq!(imported["value"]["kind"], "helpcode");
    assert_eq!(imported["value"]["name"], "合成辅助码");
    let id = imported["value"]["id"].as_str().unwrap();

    std::fs::write(&source, "你=ab\n你=cd\n").unwrap();
    let duplicate = import();
    assert_eq!(duplicate["error"], "plugin_invalid");
    assert!(duplicate["detail"]
        .as_str()
        .unwrap()
        .contains("第 1 行和第 2 行"));

    std::fs::write(&source, [0xff]).unwrap();
    let invalid_utf8 = import();
    assert_eq!(invalid_utf8["error"], "plugin_invalid");
    assert!(invalid_utf8["detail"].as_str().unwrap().contains("UTF-8"));

    let too_large = msime_client_services::plugins::helpcode_pack::MAX_TABLE_BYTES + 1;
    std::fs::File::create(&source)
        .unwrap()
        .set_len(too_large)
        .unwrap();
    let oversized = import();
    assert_eq!(oversized["error"], "plugin_invalid");
    assert!(oversized["detail"].as_str().unwrap().contains("1 MiB"));

    let catalog = call(json!({"operation": "catalog"}));
    assert!(catalog["value"]["packages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|pack| pack["id"] == id && pack["kind"] == "helpcode"));

    let mut preferences = chinese_preferences();
    preferences.scheme = msime_client_core::preferences::InputScheme::Shuangpin;
    preferences.shuangpin_helpcode.enabled = true;
    preferences.plugins.helpcode_pack_shuangpin = id.to_owned();
    let handle = plugin_host(fixture.path(), preferences);
    assert_eq!(session_helpcode(handle).as_deref(), Some("zz"));
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
}
