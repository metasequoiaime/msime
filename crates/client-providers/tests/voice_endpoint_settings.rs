use msime_client_providers::voice::{
    provider::valid_mobile_voice_endpoint, provider_settings::endpoint_issue,
};

#[test]
fn settings_follow_the_runtime_url_policy_for_both_transports() {
    for (endpoint, websocket, accepted) in [
        ("https://example.invalid/v1", false, true),
        ("https://例子.invalid/合成🍃", false, true),
        ("https://[::1]:443/v1", false, true),
        ("wss://example.invalid/stream", true, true),
        ("https://example.invalid:bad/v1", false, false),
        ("https://[broken/v1", false, false),
        ("https://bad host/v1", false, false),
        ("https:///v1", false, false),
        ("https://user:synthetic@example.invalid/v1", false, false),
        ("https://example.invalid/v1#fragment", false, false),
        ("https://example.invalid/\0", false, false),
        ("http://example.invalid/v1", false, false),
        ("wss://example.invalid/v1", false, false),
        ("https://example.invalid/stream", true, false),
        ("ws://example.invalid/stream", true, false),
    ] {
        assert_eq!(endpoint_issue(endpoint, websocket).is_none(), accepted);
        assert_eq!(valid_mobile_voice_endpoint(endpoint, websocket), accepted);
    }
}

#[test]
fn settings_preserve_the_mobile_hosts_case_and_at_sign_restrictions() {
    for endpoint in [
        "HTTPS://example.invalid/v1",
        "https://example.invalid/path@synthetic",
    ] {
        assert!(valid_mobile_voice_endpoint(endpoint, false));
        assert!(endpoint_issue(endpoint, false).is_some());
    }
}

#[test]
fn settings_allow_defaults_and_trim_like_runtime_configuration_resolution() {
    for websocket in [false, true] {
        assert_eq!(endpoint_issue("", websocket), None);
        assert_eq!(endpoint_issue(" \t\n", websocket), None);
        let endpoint = if websocket {
            "wss://example.invalid/stream"
        } else {
            "https://example.invalid/v1"
        };
        assert_eq!(endpoint_issue(&format!(" \t{endpoint}\n"), websocket), None);
    }
    assert_eq!(
        endpoint_issue("\0", false),
        Some("接口地址要以 https:// 开头，且不能带用户名或密码")
    );
}

#[test]
fn settings_keep_the_runtime_utf8_bound_and_existing_protocol_hints() {
    let prefix = "https://example.invalid/";
    let exact = format!("{prefix}{}", "x".repeat(2_048 - prefix.len()));
    assert_eq!(endpoint_issue(&exact, false), None);
    assert!(endpoint_issue(&(exact + "x"), false).is_some());
    assert!(endpoint_issue(&format!("{prefix}{}", "合".repeat(800)), false).is_some());
    assert_eq!(
        endpoint_issue("invalid", true),
        Some("豆包的接口地址要以 wss:// 开头")
    );
    assert_eq!(
        endpoint_issue("invalid", false),
        Some("接口地址要以 https:// 开头，且不能带用户名或密码")
    );
}
