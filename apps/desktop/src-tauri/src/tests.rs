//! Unit tests for the parent module, in their own file because the module
//! is large enough that mixing them with the implementation obscured both.
//! Same `mod tests` as before, so `use super::*` still names the parent.

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod credential_command_tests;
mod local_model_tests;

#[test]
fn emoji_group_page_capacity_reserves_the_first_page_size() {
    let mut groups = Vec::<super::EmojiCatalogGroup>::new();
    let mut positions = std::collections::HashMap::new();
    super::reserve_emoji_group_page_capacity(&mut groups, &mut positions, 7);
    assert!(groups.capacity() >= 7);
    assert!(positions.capacity() >= 7);
}

/// 插件符号组追加在内置组之后：符号以插件名为上级分类，颜文字排在 All 之后；每组带包 id，组的关键词放在组上，各项的关键词仍是符号本身。
#[test]
fn plugin_symbol_groups_follow_the_built_in_catalog() {
    use msime_client_core::plugins::symbol_set::SymbolTab;
    let group =
        |tab, title: &str, keywords: &str, items: &[&str]| msime_host_api::PluginSymbolGroup {
            pack: "arrows".into(),
            pack_name: "箭头大全".into(),
            tab,
            title: title.into(),
            keywords: keywords.into(),
            items: items.iter().map(|item| (*item).to_owned()).collect(),
        };
    let built_in = |title: &str| super::EmojiCatalogGroup {
        title: title.into(),
        parent: None,
        pack: None,
        keywords: String::new(),
        icon: String::new(),
        items: Vec::new(),
    };
    let mut kaomoji = vec![built_in("All")];
    let mut symbols = vec![built_in("数学")];
    super::append_plugin_symbol_groups(
        vec![
            group(SymbolTab::Symbols, "箭头", "jiantou", &["→", "←"]),
            group(SymbolTab::Kaomoji, "开心", "", &["(^_^)"]),
        ],
        &mut kaomoji,
        &mut symbols,
    );
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[1].title, "箭头");
    assert_eq!(symbols[1].parent.as_deref(), Some("箭头大全"));
    assert_eq!(symbols[1].icon, "→");
    assert_eq!(symbols[1].pack.as_deref(), Some("arrows"));
    // 组的关键词只用于搜索，不覆盖各项自己的关键词。
    assert_eq!(symbols[1].keywords, "jiantou");
    assert_eq!(symbols[1].items[1].keywords, "←");
    assert_eq!(kaomoji[0].title, "All");
    assert_eq!(kaomoji[0].pack, None);
    assert_eq!(kaomoji[1].title, "开心");
    assert_eq!(kaomoji[1].pack.as_deref(), Some("arrows"));
    assert_eq!(kaomoji[1].parent, None);
    assert_eq!(kaomoji[1].items[0].keywords, "(^_^)");
}

#[test]
fn runtime_options_fallback_reserves_all_candidate_slots() {
    let candidates: Vec<std::path::PathBuf> =
        Vec::with_capacity(super::MAX_RUNTIME_OPTIONS_CANDIDATE_CAPACITY);
    assert_eq!(
        candidates.capacity(),
        super::MAX_RUNTIME_OPTIONS_CANDIDATE_CAPACITY
    );
}

#[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
#[test]
fn session_provider_discovery_rejects_symlinked_or_shared_endpoints() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::os::unix::net::UnixListener;

    let runtime = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let target_directory = target.path().join("msime-client");
    std::fs::create_dir(&target_directory).unwrap();
    std::fs::set_permissions(&target_directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let target_socket = target_directory.join("provider.sock");
    let listener = UnixListener::bind(&target_socket).unwrap();

    let linked_directory = runtime.path().join("msime-client");
    symlink(&target_directory, &linked_directory).unwrap();
    assert!(super::discover_session_provider_in(runtime.path(), "provider.sock").is_none());
    drop(listener);

    let directory = runtime.path().join("msime-client");
    std::fs::remove_file(&directory).unwrap();
    std::fs::create_dir(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let socket = directory.join("provider.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    assert_eq!(
        super::discover_session_provider_in(runtime.path(), "provider.sock"),
        Some(socket)
    );
    drop(listener);

    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(super::discover_session_provider_in(runtime.path(), "provider.sock").is_none());
}

#[test]
fn snapshot_restore_preflight_rejects_text_larger_than_native_limit() {
    assert!(crate::platform::account_helpers::snapshot_text_within_limit(512 * 1024 * 1024));
    assert!(!crate::platform::account_helpers::snapshot_text_within_limit(512 * 1024 * 1024 + 1));
}

#[test]
fn external_url_allows_encoded_query_parameters() {
    assert!(super::external_url_is_safe(
        "https://github.com/metasequoiaime/msime/issues/new?title=bug&body=synthetic%20report"
    ));
    assert!(!super::external_url_is_safe("javascript:alert(1)"));
    assert!(!super::external_url_is_safe(
        "https://example.com/path|whoami"
    ));
}

#[cfg(unix)]
#[test]
fn runtime_options_reject_a_symlinked_file() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let outside = root.path().join("outside.json");
    let linked = root.path().join("runtime-options.json");
    std::fs::write(&outside, br#"{"resources":"/synthetic"}"#).unwrap();
    symlink(&outside, &linked).unwrap();

    assert!(super::read_runtime_options_bytes(&linked).is_err());
    assert_eq!(
        std::fs::read(&outside).unwrap(),
        br#"{"resources":"/synthetic"}"#
    );
}

#[cfg(not(target_os = "android"))]
#[test]
fn ai_endpoint_validation_accepts_http_api_urls_and_rejects_unsafe_urls() {
    for endpoint in [
        "https://api.example.test/v1/chat/completions",
        "http://127.0.0.1:8080/v1/chat/completions?tenant=fixture",
        "http://192.168.1.20:1234/v1/chat/completions",
        "http://studio.local:1234/v1",
    ] {
        assert!(crate::ai::validate_ai_endpoint(endpoint).is_ok());
    }
    for endpoint in [
        "file:///tmp/models",
        "http://api.example.test/v1/chat/completions",
        "http://8.8.8.8/v1/chat/completions",
        "https:///v1/chat/completions",
        "https://user:password@example.test/v1/chat/completions",
        "https://example.test/v1/chat/completions#fragment",
        "https://example.test/v1/chat/\ncompletions",
    ] {
        assert!(crate::ai::validate_ai_endpoint(endpoint).is_err());
    }
}

#[cfg(not(target_os = "android"))]
#[test]
fn ai_models_url_reuses_the_api_prefix() {
    let endpoint = crate::ai::validate_ai_endpoint(
        "https://api.example.test/openai/v1/chat/completions?tenant=fixture",
    )
    .unwrap_or_else(|_| panic!("fixture endpoint should be valid"));
    assert_eq!(
        crate::ai::ai_models_url(&endpoint).as_str(),
        "https://api.example.test/openai/v1/models"
    );

    let endpoint = crate::ai::validate_ai_endpoint("https://api.example.test/chat/completions")
        .unwrap_or_else(|_| panic!("fixture endpoint should be valid"));
    assert_eq!(
        crate::ai::ai_models_url(&endpoint).as_str(),
        "https://api.example.test/models"
    );

    for (endpoint, models) in [
        (
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
            "https://generativelanguage.googleapis.com/v1beta/openai/models",
        ),
        (
            "https://open.bigmodel.cn/api/paas/v4/chat/completions",
            "https://open.bigmodel.cn/api/paas/v4/models",
        ),
        (
            "https://host.example.test/v1/chat/completions/",
            "https://host.example.test/v1/models",
        ),
    ] {
        let endpoint = crate::ai::validate_ai_endpoint(endpoint)
            .unwrap_or_else(|_| panic!("fixture endpoint should be valid"));
        assert_eq!(crate::ai::ai_models_url(&endpoint).as_str(), models);
    }
}

#[cfg(not(target_os = "android"))]
#[test]
fn ai_credentials_and_text_reject_empty_or_unsafe_values() {
    assert!(crate::ai::validate_ai_token("fixture-token").is_ok());
    assert!(crate::ai::validate_ai_token("").is_err());
    assert!(crate::ai::validate_ai_token("fixture\n-token").is_err());
    assert!(crate::ai::ai_text_is_valid("多行\nfixture text\t", false));
    assert!(crate::ai::ai_text_is_valid("", true));
    assert!(!crate::ai::ai_text_is_valid("", false));
    assert!(!crate::ai::ai_text_is_valid("fixture\0text", false));
}

#[cfg(not(target_os = "android"))]
#[test]
fn ai_response_body_limit_rejects_oversized_streams() {
    let body = vec![b'x'; crate::ai::MAX_RESPONSE_BYTES + 1];
    let error = crate::ai::read_ai_response_body(std::io::Cursor::new(body)).unwrap_err();
    assert_eq!(error, crate::ai::AiResponseBodyError::TooLarge);
}

#[test]
fn runtime_options_reader_rejects_oversized_documents_without_allocating_them() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("runtime-options.json");
    std::fs::write(
        &path,
        vec![b'x'; super::RUNTIME_OPTIONS_READ_LIMIT as usize + 1],
    )
    .unwrap();
    assert_eq!(
        super::read_runtime_options_bytes(&path).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
}

#[test]
fn linux_runtime_state_directory_treats_a_missing_locator_as_first_run() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("runtime-options.json");
    assert_eq!(super::linux_runtime_state_directory_at(&path), Ok(None));

    let preferences = directory.path().join("preferences");
    std::fs::write(
        &path,
        serde_json::json!({ "preferences_directory": preferences }).to_string(),
    )
    .unwrap();
    assert_eq!(
        super::linux_runtime_state_directory_at(&path),
        Ok(Some(preferences))
    );

    std::fs::write(&path, "{").unwrap();
    assert!(super::linux_runtime_state_directory_at(&path).is_err());
    assert!(
        super::linux_runtime_state_directory_at(std::path::Path::new("runtime-options.json"))
            .is_err()
    );
}

#[test]
fn cantonese_zhuyin_and_stroke_are_offered_only_with_their_installed_dictionary() {
    use msime_client_core::host_surface::{HostCapabilities, HostPlatform};
    use msime_client_core::preferences::InputScheme;
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("language-dictionaries");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("msime-zhuyin.db"), b"sqlite").unwrap();
    // Every host narrows the schemes the same way.
    for platform in [
        HostPlatform::Macos,
        HostPlatform::Windows,
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Ios,
    ] {
        for name in ["msime-cantonese.db", "msime-stroke.db"] {
            let dictionary = directory.join(name);
            if dictionary.exists() {
                std::fs::remove_file(&dictionary).unwrap();
            }
        }
        let offered = |host_options: Option<&serde_json::Value>| {
            let mut capabilities = HostCapabilities::for_platform(platform);
            super::drop_uninstalled_language_schemes(&mut capabilities, host_options, false);
            capabilities.input_schemes
        };
        let without_both = vec![
            InputScheme::Quanpin,
            InputScheme::Shuangpin,
            InputScheme::Wubi,
            InputScheme::Japanese,
            InputScheme::Korean,
            InputScheme::Vietnamese,
            InputScheme::Tibetan,
        ];
        assert_eq!(offered(None), without_both, "{platform:?}");
        assert_eq!(
            offered(Some(&serde_json::json!({}))),
            without_both,
            "{platform:?}"
        );
        // A relative directory is not trusted to mean the installed one.
        assert_eq!(
            offered(Some(
                &serde_json::json!({ "language_dictionaries": "language-dictionaries" })
            )),
            without_both,
            "{platform:?}"
        );
        let named = serde_json::json!({ "language_dictionaries": directory });
        let mut with_zhuyin = without_both.clone();
        with_zhuyin.insert(5, InputScheme::Zhuyin);
        assert_eq!(offered(Some(&named)), with_zhuyin, "{platform:?}");
        std::fs::write(directory.join("msime-cantonese.db"), b"sqlite").unwrap();
        let mut without_stroke = HostCapabilities::for_platform(platform).input_schemes;
        without_stroke.retain(|scheme| *scheme != InputScheme::Stroke);
        assert_eq!(offered(Some(&named)), without_stroke, "{platform:?}");
        std::fs::write(directory.join("msime-stroke.db"), b"sqlite").unwrap();
        assert_eq!(
            offered(Some(&named)),
            HostCapabilities::for_platform(platform).input_schemes,
            "{platform:?}"
        );
    }
    // Without the Windows fallback a document naming only its resources offers neither.
    let mut capabilities = HostCapabilities::for_platform(HostPlatform::Macos);
    let resources_only = serde_json::json!({ "resources": root.path().join("resources") });
    super::drop_uninstalled_language_schemes(&mut capabilities, Some(&resources_only), false);
    assert!(!capabilities.input_schemes.contains(&InputScheme::Cantonese));
    assert!(!capabilities.input_schemes.contains(&InputScheme::Zhuyin));
    assert!(!capabilities.input_schemes.contains(&InputScheme::Stroke));
}

#[test]
fn macos_offers_only_the_language_schemes_its_download_can_install() {
    use msime_client_core::host_surface::{HostCapabilities, HostPlatform};
    use msime_client_core::preferences::InputScheme;
    use msime_client_core::resource_packs::ResourcePack;
    let mut capabilities = HostCapabilities::for_platform(HostPlatform::Macos);
    super::drop_unpinned_language_schemes(&mut capabilities);
    let pinned = ResourcePack::LanguageDictionaries.schemes();
    for (scheme, name) in [
        (InputScheme::Cantonese, "cantonese"),
        (InputScheme::Zhuyin, "zhuyin"),
        (InputScheme::Stroke, "stroke"),
    ] {
        assert_eq!(
            capabilities.input_schemes.contains(&scheme),
            pinned.contains(&name),
            "{name}"
        );
    }
    // 其余方案不需要语言词库，不受影响。
    assert!(capabilities.input_schemes.contains(&InputScheme::Quanpin));
    assert!(capabilities.input_schemes.contains(&InputScheme::Tibetan));
}

#[test]
fn windows_finds_language_dictionaries_beside_resources_its_options_file_does_not_name() {
    use msime_client_core::host_surface::{HostCapabilities, HostPlatform};
    use msime_client_core::preferences::InputScheme;
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("language-dictionaries");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("msime-cantonese.db"), b"sqlite").unwrap();
    let offered = |host_options: &serde_json::Value| {
        let mut capabilities = HostCapabilities::for_platform(HostPlatform::Windows);
        super::drop_uninstalled_language_schemes(&mut capabilities, Some(host_options), true);
        capabilities.input_schemes
    };
    let schemes = offered(&serde_json::json!({ "resources": root.path().join("resources") }));
    assert!(schemes.contains(&InputScheme::Cantonese));
    assert!(!schemes.contains(&InputScheme::Zhuyin));
    assert!(!schemes.contains(&InputScheme::Stroke));
    assert!(schemes.contains(&InputScheme::Vietnamese));
    assert!(schemes.contains(&InputScheme::Tibetan));
    std::fs::write(directory.join("msime-stroke.db"), b"sqlite").unwrap();
    let schemes = offered(&serde_json::json!({ "resources": root.path().join("resources") }));
    assert!(schemes.contains(&InputScheme::Stroke));
    // A relative resources directory is not trusted to locate the installed dictionaries.
    let relative = offered(&serde_json::json!({ "resources": "resources" }));
    assert!(!relative.contains(&InputScheme::Cantonese));
    // A document that names the directory is taken at its word.
    let elsewhere = root.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let named = offered(&serde_json::json!({
        "resources": root.path().join("resources"),
        "language_dictionaries": elsewhere,
    }));
    assert!(!named.contains(&InputScheme::Cantonese));
    assert!(!named.contains(&InputScheme::Stroke));
}

#[test]
fn helpcode_catalog_reads_only_the_host_resource_directory() {
    let directory = tempfile::tempdir().unwrap();
    let resources = directory.path().join("resources");
    let custom = resources.join("helpcodes/custom");
    std::fs::create_dir_all(&custom).unwrap();
    std::fs::write(
        custom.join("synthetic.txt"),
        "# name: Synthetic helper\n# name_en: Synthetic\nword=ab\n",
    )
    .unwrap();
    let document = serde_json::json!({"resources": resources});

    #[cfg(any(target_os = "linux", target_os = "android"))]
    let options = {
        let path = directory.path().join("runtime-options.json");
        std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        super::DictionaryHostOptions { path }
    };
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let options = super::DictionaryHostOptions {
        document: std::sync::Arc::new(document),
    };

    let schemas = super::list_helpcode_schemas_at(&options).unwrap();
    assert_eq!(schemas.len(), 1);
    assert_eq!(schemas[0].schema, "custom/synthetic");
    assert_eq!(schemas[0].name, "Synthetic helper");
    assert_eq!(schemas[0].name_en, "Synthetic");
}

#[test]
fn candidate_panel_status_reader_rejects_oversized_documents() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("candidate-panel.json");
    std::fs::write(
        &path,
        vec![b'x'; super::CANDIDATE_PANEL_STATUS_READ_LIMIT as usize + 1],
    )
    .unwrap();
    assert!(super::read_candidate_panel_status(&path).is_none());
}

#[test]
fn clipboard_text_validation_enforces_nonempty_nul_free_byte_limit() {
    assert!(!crate::clipboard_history::clipboard_text_is_valid(""));
    assert!(!crate::clipboard_history::clipboard_text_is_valid("a\0b"));

    let at_limit = "x".repeat(msime_client_core::clipboard::MAX_TEXT_BYTES);
    assert!(crate::clipboard_history::clipboard_text_is_valid(&at_limit));

    let over_limit = format!("{at_limit}x");
    assert!(!crate::clipboard_history::clipboard_text_is_valid(
        &over_limit
    ));
    assert!(crate::clipboard_history::clipboard_text_is_valid(
        "第一行\nsecond line\n第三行"
    ));
}

#[test]
fn windows_restart_payload_is_exact_utf16_without_terminator() {
    let payload = super::windows_restart_payload();
    let expected: Vec<u8> = "RestartServer"
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    assert_eq!(payload, expected);
    assert_eq!(payload.len(), "RestartServer".encode_utf16().count() * 2);
}

#[test]
fn linux_restart_targets_the_running_input_method_framework() {
    let command = |fcitx5_running, addon| {
        let (program, arguments) = super::linux_input_method_restart_command(fcitx5_running, addon);
        (program, arguments.join(" "))
    };
    assert_eq!(
        command(true, "msime"),
        (
            "gdbus",
            "call --session --dest org.fcitx.Fcitx5 --object-path /controller --method org.fcitx.Fcitx.Controller1.ReloadAddonConfig 'msime'".to_owned()
        )
    );
    // 五笔版只重置自己的插件。
    assert!(command(true, "msime-wubi")
        .1
        .ends_with("ReloadAddonConfig 'msime-wubi'"));
    assert_eq!(command(false, "msime"), ("ibus", "restart".to_owned()));
}

#[test]
fn external_links_require_clean_https_urls() {
    for url in [
        "https://example.com/help",
        "https://updates.example.com/v1?channel=stable",
        "https://example.com/a&b",
    ] {
        assert!(super::external_url_is_safe(url));
    }
    for url in [
        "https://",
        "https:///path",
        "http://example.com",
        "https://example.com/help path",
        "https://user:secret@example.com/help",
        "https://example.com:bad/help",
        "https://example.com/\"quoted\"",
        "https://example.com/\\escape",
    ] {
        assert!(!super::external_url_is_safe(url));
    }
    assert!(!super::external_url_is_safe(&format!(
        "https://example.com/{}",
        "x".repeat(4096)
    )));
}

#[test]
fn ios_clipboard_history_is_permission_gated_not_preference_gated() {
    assert!(!super::clipboard_history_uses_preference(
        msime_client_core::host_surface::HostPlatform::Ios
    ));
    for platform in [
        msime_client_core::host_surface::HostPlatform::Windows,
        msime_client_core::host_surface::HostPlatform::Macos,
        msime_client_core::host_surface::HostPlatform::Linux,
        msime_client_core::host_surface::HostPlatform::Android,
    ] {
        assert!(super::clipboard_history_uses_preference(platform));
    }
}

#[test]
fn ios_routes_only_app_group_dictionary_operations() {
    for operation in [
        "list",
        "edit",
        "import",
        "import_personal",
        "export",
        "retry",
        "dismiss_failure",
    ] {
        assert!(super::ios_personal_dictionary_action(
            &serde_json::json!({ "operation": operation })
        ));
    }
    for action in [
        serde_json::json!({ "operation": "reset" }),
        serde_json::json!({ "operation": "unknown" }),
        serde_json::json!({}),
        serde_json::Value::Null,
    ] {
        assert!(!super::ios_personal_dictionary_action(&action));
    }
}

#[test]
fn ios_first_run_host_options_use_packaged_resources_and_shared_state() {
    let document = super::ios_host_options_document(
        None,
        std::path::Path::new("/fixture/resources"),
        std::path::Path::new("/fixture/shared-state"),
    )
    .expect("first-run options");
    assert_eq!(document["resources"], "/fixture/resources");
    assert_eq!(document["state_root"], "/fixture/shared-state");
    assert!(document.get("language_dictionaries").is_none());
}

#[test]
fn ios_first_run_host_options_name_the_bundled_language_dictionaries() {
    let bundle = tempfile::tempdir().expect("bundle");
    let resources = bundle.path().join("EngineResources");
    let dictionaries = bundle.path().join("language-dictionaries");
    std::fs::create_dir_all(&resources).expect("resources");
    std::fs::create_dir_all(&dictionaries).expect("dictionaries");
    let empty = super::ios_host_options_document(None, &resources, bundle.path())
        .expect("first-run options");
    assert!(empty.get("language_dictionaries").is_none());
    std::fs::write(dictionaries.join("msime-zhuyin.db"), b"fixture").expect("msime-zhuyin.db");
    let document = super::ios_host_options_document(None, &resources, bundle.path())
        .expect("first-run options");
    assert_eq!(
        document["language_dictionaries"],
        dictionaries.to_str().expect("utf-8 path")
    );
}

#[test]
fn ios_named_skin_library_shares_the_apple_app_group_root() {
    let root =
        super::ios_custom_skin_library_root(std::path::Path::new("/fixture/app-group/MSIME"));
    assert_eq!(root, std::path::Path::new("/fixture/app-group"));
    assert_eq!(
        msime_client_core::skin::custom_library::CustomSkinLibraryStore::new(root).path(),
        std::path::Path::new("/fixture/app-group/CustomSkins/library.json")
    );
}

#[test]
fn ios_community_reply_library_shares_the_keyboard_app_group_file() {
    assert_eq!(
        super::ios_community_resource_library_path(std::path::Path::new(
            "/fixture/app-group/MSIME"
        )),
        std::path::Path::new("/fixture/app-group/CommunityLibrary.json")
    );
}

#[test]
fn ios_prepared_host_options_are_preserved_and_malformed_json_is_rejected() {
    let prepared = r#"{"resources":"/prepared","state_root":"/state","api_version":1}"#;
    let document = super::ios_host_options_document(
        Some(prepared),
        std::path::Path::new("/unused/resources"),
        std::path::Path::new("/unused/state"),
    )
    .expect("prepared options");
    assert_eq!(document["resources"], "/prepared");
    assert_eq!(document["state_root"], "/state");
    assert!(super::ios_host_options_document(
        Some("{"),
        std::path::Path::new("/unused/resources"),
        std::path::Path::new("/unused/state"),
    )
    .is_err());
}

#[test]
fn ios_voice_batch_configuration_uses_current_preferences_and_safe_defaults() {
    let mut preferences = msime_client_core::preferences::Preferences::default();
    preferences.voice_input.asr_provider = "openai".into();
    preferences.voice_input.asr_endpoint.clear();
    preferences.voice_input.asr_model.clear();
    preferences.voice_input.asr_token = "synthetic-current".into();
    preferences
        .voice_input
        .asr_tokens
        .insert("openai".into(), "synthetic-stale".into());
    let configuration = crate::voice::mobile_voice_provider_configuration(&preferences).unwrap();
    assert_eq!(configuration.provider, "openai");
    assert_eq!(
        configuration.endpoint,
        "https://api.openai.com/v1/audio/transcriptions"
    );
    assert_eq!(configuration.model, "whisper-1");
    assert_eq!(configuration.token, "synthetic-current");
    assert!(configuration.headers.is_empty());

    preferences.voice_input.asr_provider = "groq".into();
    preferences.voice_input.asr_endpoint = "https://fixture.invalid/transcribe".into();
    preferences.voice_input.asr_model = "fixture-model".into();
    preferences.voice_input.asr_token.clear();
    preferences
        .voice_input
        .asr_tokens
        .insert("groq".into(), "synthetic-slot".into());
    let configuration = crate::voice::mobile_voice_provider_configuration(&preferences).unwrap();
    assert_eq!(configuration.endpoint, "https://fixture.invalid/transcribe");
    assert_eq!(configuration.model, "fixture-model");
    assert_eq!(configuration.token, "synthetic-slot");
    assert!(configuration.headers.is_empty());
}

// EveryAPI and Mistral are the two transcription services the Apple client offers that the
// shared client did not carry. They use the same multipart upload as the providers above, so
// what has to be right is the endpoint and model each one resolves to on its own.
#[test]
fn ios_voice_batch_configuration_covers_everyapi_and_mistral() {
    for (provider, endpoint, model) in [
        (
            "everyapi",
            "https://api.everyapi.ai/v1/audio/transcriptions",
            "openai/whisper-large-v3-turbo",
        ),
        (
            "mistral",
            "https://api.mistral.ai/v1/audio/transcriptions",
            "voxtral-mini-latest",
        ),
        (
            "bailian",
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
            "qwen3-asr-flash",
        ),
    ] {
        let mut preferences = msime_client_core::preferences::Preferences::default();
        preferences.voice_input.asr_provider = provider.into();
        preferences.voice_input.asr_endpoint.clear();
        preferences.voice_input.asr_model.clear();
        preferences
            .voice_input
            .asr_tokens
            .insert(provider.into(), "synthetic-slot".into());
        assert!(preferences.validate().is_ok());
        let configuration =
            crate::voice::mobile_voice_provider_configuration(&preferences).unwrap();
        assert_eq!(configuration.provider, provider);
        assert_eq!(configuration.endpoint, endpoint);
        assert_eq!(configuration.model, model);
        assert_eq!(configuration.token, "synthetic-slot");
        assert_eq!(
            configuration.request_format,
            if provider == "bailian" {
                "chat_audio"
            } else {
                "multipart"
            }
        );
        // Only Doubao carries request headers; a batch provider that grew any would be
        // sending something the multipart transport never validated.
        assert!(configuration.headers.is_empty());
    }
}

#[test]
fn ios_keyboard_ai_preferences_resolve_origin_tokens_and_disable_incomplete_drafts() {
    let mut preferences = msime_client_core::preferences::Preferences::default();
    preferences.ai_assistant.enabled = true;
    preferences.ai_assistant.provider = "deepseek".into();
    preferences.ai_assistant.endpoint = "https://API.Example.invalid/v1/chat/completions".into();
    preferences.ai_assistant.model = "fixture-model".into();
    preferences.ai_assistant.tokens.insert(
        "https://api.example.invalid:443".into(),
        "fixture-origin-token".into(),
    );
    let native = super::ios_keyboard_ai_preferences(&preferences.ai_assistant);
    assert!(native.enabled);
    assert_eq!(native.provider, "deepSeek");
    assert_eq!(native.token, "fixture-origin-token");
    preferences
        .ai_assistant
        .tokens
        .insert("deepseek".into(), "fixture-provider-legacy".into());
    assert_eq!(
        super::ios_keyboard_ai_preferences(&preferences.ai_assistant).token,
        "fixture-origin-token"
    );
    assert_eq!(
        native.prompt,
        "请润色以下文字，保持原意，只返回修改后的文字。"
    );

    preferences.ai_assistant.prompt_id = "custom_2".into();
    preferences.ai_assistant.prompt_custom_1 = "first slot".into();
    preferences.ai_assistant.prompt_custom_2 = "second slot".into();
    assert_eq!(
        super::ios_keyboard_ai_preferences(&preferences.ai_assistant).prompt,
        "second slot"
    );
    preferences.ai_assistant.prompt_custom_2 = "  ".into();
    assert_eq!(
        super::ios_keyboard_ai_preferences(&preferences.ai_assistant).prompt,
        "请润色以下文字，保持原意，只返回修改后的文字。"
    );

    preferences
        .ai_assistant
        .tokens
        .remove("https://api.example.invalid:443");
    assert!(!super::ios_keyboard_ai_preferences(&preferences.ai_assistant).enabled);

    preferences.ai_assistant.tokens.clear();
    assert!(!super::ios_keyboard_ai_preferences(&preferences.ai_assistant).enabled);
}

#[test]
fn ios_voice_doubao_configuration_uses_shared_auth_and_current_preferences() {
    let mut preferences = msime_client_core::preferences::Preferences::default();
    preferences.voice_input.asr_token = "synthetic-key".into();
    preferences.voice_input.asr_app_key = "stale-app".into();
    preferences.voice_input.doubao_auth_mode = "api_key".into();
    preferences.voice_input.doubao_boosting_table_id = "fixture-table".into();
    // The first-run value differs between the macOS test host and the mobile hosts, so state it.
    preferences.voice_input.doubao_enable_ddc = false;
    let configuration = crate::voice::mobile_voice_provider_configuration(&preferences).unwrap();
    assert_eq!(configuration.provider, "doubao");
    assert_eq!(
        configuration.endpoint,
        "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async"
    );
    assert!(configuration.model.is_empty());
    assert!(configuration.token.is_empty());
    assert!(configuration.enable_itn);
    assert!(configuration.enable_punctuation);
    assert!(!configuration.enable_ddc);
    assert_eq!(configuration.boosting_table_id, "fixture-table");
    assert!(configuration
        .headers
        .iter()
        .any(|header| header.name == "x-api-key" && header.value == "synthetic-key"));
    assert!(!configuration
        .headers
        .iter()
        .any(|header| header.name == "x-api-app-key"));

    preferences.voice_input.doubao_auth_mode = "legacy".into();
    preferences.voice_input.asr_app_key = "synthetic-app".into();
    let configuration = crate::voice::mobile_voice_provider_configuration(&preferences).unwrap();
    assert!(configuration
        .headers
        .iter()
        .any(|header| header.name == "x-api-app-key" && header.value == "synthetic-app"));
    assert!(configuration
        .headers
        .iter()
        .any(|header| header.name == "x-api-access-key"));
}

#[cfg(unix)]
#[test]
fn voice_provider_options_only_forwards_known_doubao_auth_modes() {
    let document = serde_json::json!({
        "preferences": {"voice_input": {
            "doubao_auth_mode": "legacy",
            "asr_app_key": "private-app-id",
            "asr_token": "private-token"
        }}
    });
    let result = crate::voice::voice_provider_options(&document);
    assert!(result.is_ok());
    let Ok(options) = result else {
        panic!("voice options should be valid");
    };
    assert_eq!(
        options.get("doubao_auth_mode").and_then(|v| v.as_str()),
        Some("legacy")
    );
    assert!(options.get("asr_app_key").is_none());
    assert!(options.get("asr_token").is_none());

    let document = serde_json::json!({
        "preferences": {"voice_input": {"doubao_auth_mode": "unknown"}}
    });
    let result = crate::voice::voice_provider_options(&document);
    assert!(result.is_ok());
    let Ok(options) = result else {
        panic!("voice options should be valid");
    };
    assert!(options.get("doubao_auth_mode").is_none());
}

#[cfg(unix)]
#[test]
fn voice_provider_options_bound_strings_by_utf8_bytes() {
    let multibyte = "界".repeat(200);
    let document = serde_json::json!({
        "preferences": {"voice_input": {
            "asr_model": multibyte,
            "capture_device": "x".repeat(600)
        }}
    });
    let options = crate::voice::voice_provider_options(&document).unwrap();
    let model = options
        .get("asr_model")
        .and_then(|value| value.as_str())
        .unwrap();
    let device = options
        .get("capture_device")
        .and_then(|value| value.as_str())
        .unwrap();

    assert_eq!(model.len(), 510);
    assert_eq!(model.chars().count(), 170);
    assert_eq!(device.len(), 512);
}

#[cfg(unix)]
#[test]
fn voice_preferences_refresh_keeps_transport_and_reads_latest_store_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let store = super::PreferencesStore::new(root.path());
    let initial = store.load().unwrap();
    let mut preferences = initial.preferences;
    preferences.voice_input.asr_provider = "openai".into();
    let saved = store.save(initial.revision, preferences).unwrap();
    let document = serde_json::json!({
        "voice_provider_socket": "/fixture/voice.sock",
        "preferences": {"voice_input": {"asr_provider": "stale"}}
    });
    let refreshed = crate::voice::refresh_voice_preferences(document, &store).unwrap();
    assert_eq!(
        refreshed["preferences"]["voice_input"]["asr_provider"],
        "openai"
    );
    assert_eq!(refreshed["voice_provider_socket"], "/fixture/voice.sock");
    assert_eq!(saved.revision, store.load().unwrap().revision);
}

#[cfg(unix)]
#[test]
fn credential_tests_route_to_the_configured_provider_without_credentials() {
    let document = serde_json::json!({
        "online_provider_socket": "/fixture/online.sock",
        "translation_provider_socket": "/fixture/translation.sock",
        "voice_provider_socket": "/fixture/voice.sock",
    });
    assert_eq!(
        super::credential_provider_socket(&document, "ai.assistant"),
        Some(std::path::PathBuf::from("/fixture/online.sock"))
    );
    assert_eq!(
        super::credential_provider_socket(&document, "translation.niutrans"),
        Some(std::path::PathBuf::from("/fixture/translation.sock"))
    );
    assert_eq!(
        super::credential_provider_socket(&document, "voice.polish"),
        Some(std::path::PathBuf::from("/fixture/voice.sock"))
    );
    assert!(super::credential_provider_socket(&document, "unknown").is_none());
}

#[test]
fn second_launch_routes_are_taken_from_explicit_arguments() {
    use msime_client_core::host_surface::{SettingsCategory, SurfaceRoute};

    assert_eq!(
        super::launch_route_from_args(&["--route=emoji".into()]),
        Some(SurfaceRoute::Emoji)
    );
    assert_eq!(
        super::launch_route_from_args(&["--route=settings:about".into()])
            .and_then(|route| route.settings_category()),
        Some(SettingsCategory::About)
    );
    assert_eq!(
        super::launch_route_from_args(&["--route=../private".into()]),
        None
    );
    assert_eq!(super::launch_route_from_args(&["--other".into()]), None);
}

/// On macOS only settings launches share the running instance; every panel keeps its own per-session process.
#[cfg(target_os = "macos")]
#[test]
fn macos_single_instance_admits_only_settings_launches() {
    use msime_client_core::host_surface::{SettingsCategory, SurfaceRoute};

    for route in [
        None,
        Some(SurfaceRoute::Settings(None)),
        Some(SurfaceRoute::Settings(Some(SettingsCategory::About))),
        Some(SurfaceRoute::Settings(Some(SettingsCategory::Skin))),
    ] {
        assert!(super::macos_settings_launch(route), "{route:?}");
    }
    for route in [
        SurfaceRoute::Keyboard,
        SurfaceRoute::Emoji,
        SurfaceRoute::Handwriting,
        SurfaceRoute::Voice,
        SurfaceRoute::Clipboard,
        SurfaceRoute::CloudClipboard,
        SurfaceRoute::CloudDictionary,
    ] {
        assert!(!super::macos_settings_launch(Some(route)), "{route:?}");
    }
}

/// A second launch that names nothing still has to raise the window.
///
/// Every route this product asks for itself is explicit, so no route means a person started the
/// application. Answering `None` there is indistinguishable from the launch being ignored: the
/// running instance never comes forward.
#[test]
fn second_launch_without_a_route_activates_the_settings_window() {
    use msime_client_core::host_surface::SurfaceRoute;

    assert_eq!(
        super::second_launch_route(&[]),
        SurfaceRoute::Settings(None)
    );
    assert_eq!(
        super::second_launch_route(&["/opt/msime/msime-desktop".into()]),
        SurfaceRoute::Settings(None)
    );
    // A route that fails to parse is not a request for a different window, so it falls back the
    // same way rather than leaving the launch with nothing to do.
    assert_eq!(
        super::second_launch_route(&["--route=../private".into()]),
        SurfaceRoute::Settings(None)
    );
    // An explicit route still wins - this is a fallback, not an override.
    assert_eq!(
        super::second_launch_route(&["--route=emoji".into()]),
        SurfaceRoute::Emoji
    );
}

/// The pre-paint window colour is the page's own, and stays that way.
///
/// A window background cannot read CSS, so the two values live in Rust as well. Duplicated
/// constants drift silently and the symptom - a one-frame flash of the wrong colour when a window
/// opens - is the kind of thing nobody files a bug about. This reads the stylesheet and compares.
#[test]
fn chrome_background_matches_the_shared_stylesheet() {
    let stylesheet = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/ui/src/styles.css");
    let text = std::fs::read_to_string(&stylesheet)
        .unwrap_or_else(|error| panic!("{}: {error}", stylesheet.display()));
    let declared: Vec<&str> = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("--chrome-bg:"))
        .map(|value| value.trim().trim_end_matches(';'))
        .collect();
    // Dark first, light second, in the order the stylesheet declares its two schemes.
    assert_eq!(
        declared,
        vec!["#202020", "#f3f3f3"],
        "the stylesheet's --chrome-bg values moved; update the constants beside this test"
    );

    for (color, expected) in [
        (super::CHROME_BACKGROUND_DARK, "#202020"),
        (super::CHROME_BACKGROUND_LIGHT, "#f3f3f3"),
    ] {
        assert_eq!(
            format!("#{:02x}{:02x}{:02x}", color.0, color.1, color.2),
            expected
        );
        assert_eq!(color.3, 0xff, "an opaque window, not a translucent one");
    }

    assert_eq!(
        super::chrome_background(Some(tauri::Theme::Dark)),
        super::CHROME_BACKGROUND_DARK
    );
    assert_eq!(
        super::chrome_background(Some(tauri::Theme::Light)),
        super::CHROME_BACKGROUND_LIGHT
    );
    // No theme is the case this exists to improve on, so it takes the platform's own default.
    assert_eq!(
        super::chrome_background(None),
        super::CHROME_BACKGROUND_LIGHT
    );
}

#[test]
fn an_invalid_dictionary_entry_keeps_its_own_code() {
    for reason in [
        "invalid dictionary entry",
        "invalid dictionary entry: code is empty or too long",
        "invalid dictionary entry: code contains characters this dictionary does not accept",
        "invalid dictionary entry: Use complete pinyin syllables separated by apostrophes or spaces",
        "invalid dictionary entry: Each character must have one pinyin syllable (maximum 64)",
        "invalid dictionary entry: Wubi codes contain one to four letters",
    ] {
        assert_eq!(
            super::dictionary_error_code(reason),
            "dictionary_invalid_entry",
            "{reason}"
        );
    }
    // A valid code with a bad word or weight must not be reported as a code problem, or the page tells the user to fix the wrong field.
    for reason in [
        "invalid dictionary entry: word is empty or too long",
        "invalid dictionary entry: word contains a control character",
        "invalid dictionary entry: weight is outside 1 to 100000000",
        "invalid dictionary entry: Weight must be between 1 and 100000000",
        "invalid dictionary entry: The word contains an unsupported control character",
    ] {
        assert_eq!(
            super::dictionary_error_code(reason),
            "dictionary_invalid_word",
            "{reason}"
        );
    }
    // A different failure that merely shares the words is not an entry refusal.
    assert_eq!(
        super::dictionary_error_code("invalid dictionary entryway"),
        "storage"
    );
    assert_eq!(
        super::dictionary_error_code("dictionary edit rejected"),
        "storage"
    );
    // A bundled word refused a new code or text is told it can only be re-weighted or deleted.
    assert_eq!(
        super::dictionary_error_code("bundled dictionary entry is read-only"),
        "dictionary_bundled_readonly"
    );
}

#[test]
fn dictionary_mutations_quiesce_but_reads_do_not() {
    assert!(super::dictionary_action_requires_quiesce(
        &serde_json::json!({"operation": "edit"})
    ));
    assert!(super::dictionary_action_requires_quiesce(
        &serde_json::json!({"operation": "import"})
    ));
    assert!(super::dictionary_action_requires_quiesce(
        &serde_json::json!({"operation": "reset"})
    ));
    assert!(!super::dictionary_action_requires_quiesce(
        &serde_json::json!({"operation": "list"})
    ));
    assert!(!super::dictionary_action_requires_quiesce(
        &serde_json::json!({"operation": "export"})
    ));
}

// The bundle id comes from the edition's macOS identity, which only the macOS build compiles.
#[cfg(target_os = "macos")]
#[test]
fn macos_restart_targets_the_input_method_bundle() {
    assert_eq!(
        super::macos_input_source_restart_args(),
        [
            "-n",
            "-b",
            "app.msime.inputmethod.MetasequoiaIME",
            "--args",
            "--reregister-input-source",
        ]
    );
}

#[test]
fn on_device_translation_downloadable_keeps_only_choosable_targets() {
    // `defaults read` prints a string value followed by a newline.
    assert_eq!(
        super::parse_on_device_translation_downloadable("en,ja\n"),
        ["en", "ja"]
    );
    assert_eq!(
        super::parse_on_device_translation_downloadable(" en , en ,zh-Hant,,xx\n"),
        ["en"]
    );
    assert!(super::parse_on_device_translation_downloadable("").is_empty());
}

#[test]
fn settings_routes_select_a_page_the_shared_ui_accepts() {
    use msime_client_core::host_surface::{SettingsCategory, SurfaceRoute};
    for category in SettingsCategory::ALL {
        assert_eq!(
            super::settings_page_from_route(Some(SurfaceRoute::Settings(Some(category)))),
            Some(category.as_str().to_owned())
        );
    }
    assert_eq!(
        super::settings_page_from_route(Some(SurfaceRoute::Settings(None))),
        None
    );
    assert_eq!(
        super::settings_page_from_route(Some(SurfaceRoute::Emoji)),
        None
    );
}

#[test]
fn packaged_handwriting_model_only_accepts_an_existing_absolute_file() {
    let directory = tempfile::tempdir().unwrap();
    let model = directory.path().join("handwriting-zh_CN.model");
    std::fs::write(&model, b"synthetic").unwrap();
    let options = |value: String| serde_json::json!({ "handwriting_model": value }).to_string();

    // The host options win when they name a model that is actually there.
    assert_eq!(
        super::packaged_handwriting_model(&options(model.to_string_lossy().into_owned())),
        Some(model.clone())
    );

    // A relative or missing path is refused rather than handed to the
    // recognizer, so a stale setting cannot send strokes at something else.
    assert_eq!(
        super::packaged_handwriting_model(&options("model".into())),
        None
    );
    assert_eq!(
        super::packaged_handwriting_model(&options(
            directory
                .path()
                .join("absent.model")
                .to_string_lossy()
                .into_owned()
        )),
        None
    );
}

#[test]
fn ink_handwriting_answer_reports_no_result_when_no_model_can_follow() {
    let candidates = vec!["中".to_string()];
    // Ink 认出了内容：不论有没有模型都直接用。
    assert_eq!(
        super::ink_handwriting_answer(Some(candidates.clone()), true),
        Some(candidates.clone())
    );
    assert_eq!(
        super::ink_handwriting_answer(Some(candidates.clone()), false),
        Some(candidates)
    );
    // Ink 正常运行但没认出内容：有模型就再问模型，没有模型就返回空结果（面板显示未识别到内容），而不是报识别失败。
    assert_eq!(super::ink_handwriting_answer(Some(Vec::new()), true), None);
    assert_eq!(
        super::ink_handwriting_answer(Some(Vec::new()), false),
        Some(Vec::new())
    );
    // Ink 出错或没有中文识别器：交给模型；没有模型时调用方照旧报不可用。
    assert_eq!(super::ink_handwriting_answer(None, true), None);
    assert_eq!(super::ink_handwriting_answer(None, false), None);
}

/// 在 `state_root` 下伪造一个已完整安装的手写资源包，返回其中的模型路径。
fn publish_fake_handwriting_pack(state_root: &std::path::Path) -> std::path::PathBuf {
    use msime_client_core::resource_packs::{self, ResourcePack};
    let pack = resource_packs::root(state_root).join(ResourcePack::Handwriting.id());
    std::fs::create_dir_all(&pack).unwrap();
    let model = pack.join("handwriting-zh_CN.model");
    std::fs::write(&model, b"synthetic").unwrap();
    std::fs::write(
        pack.join(msime_client_core::voice::local_models::MANIFEST_FILE),
        serde_json::to_vec(&ResourcePack::Handwriting.manifest()).unwrap(),
    )
    .unwrap();
    model
}

#[test]
fn packaged_handwriting_model_prefers_the_option_over_a_downloaded_pack() {
    let state = tempfile::tempdir().unwrap();
    let downloaded = publish_fake_handwriting_pack(state.path());
    let configured = state.path().join("configured.model");
    std::fs::write(&configured, b"synthetic").unwrap();
    let document = serde_json::json!({
        "handwriting_model": configured.to_string_lossy(),
        "preferences_directory": state.path().to_string_lossy(),
    });
    assert_eq!(
        super::packaged_handwriting_model(&document.to_string()),
        Some(configured)
    );
    assert_eq!(
        super::downloaded_handwriting_model(Some(&document)),
        Some(downloaded.clone())
    );
    // 只有选项和环境变量都没给、也没有随包模型时，三个桌面平台才用已下载的资源包（macOS 上已下载的还排在旧版本随包的模型之前）。
    if std::env::var_os("MSIME_HANDWRITING_MODEL").is_none_or(|value| value.is_empty())
        && (cfg!(target_os = "macos") || super::bundled_handwriting_model().is_none())
    {
        let document = serde_json::json!({
            "preferences_directory": state.path().to_string_lossy(),
        });
        assert_eq!(
            super::packaged_handwriting_model(&document.to_string()),
            Some(downloaded)
        );
    }
}

/// 指定了手写模型（哪怕文件不在）时识别只用它，下载的资源包用不上，所以 Windows 和 Linux 不提供下载。
#[test]
fn a_configured_handwriting_model_needs_no_download() {
    let document = serde_json::json!({ "handwriting_model": "/synthetic/handwriting-zh_CN.model" });
    assert!(super::handwriting_model_without_pack(Some(&document)));
}

#[test]
fn packaged_handwriting_model_ignores_a_relative_preferences_directory() {
    let state = tempfile::tempdir().unwrap();
    let downloaded = publish_fake_handwriting_pack(state.path());
    let relative =
        std::path::Path::new(".").join(state.path().strip_prefix("/").unwrap_or(state.path()));
    let document = serde_json::json!({ "preferences_directory": relative.to_string_lossy() });
    assert_eq!(super::downloaded_handwriting_model(Some(&document)), None);
    assert_ne!(
        super::packaged_handwriting_model(&document.to_string()),
        Some(downloaded)
    );
    assert_eq!(super::downloaded_handwriting_model(None), None);
    assert_eq!(
        super::downloaded_handwriting_model(Some(&serde_json::json!({}))),
        None
    );
}

#[test]
fn typing_statistics_status_reports_file_availability_without_content() {
    let directory = tempfile::tempdir().unwrap();
    let store = msime_client_core::typing_statistics::TypingStatisticsStore::new(directory.path());
    let missing = super::typing_statistics_status(&store, store.load().unwrap())
        .ok()
        .unwrap();
    let missing_json = serde_json::to_value(missing).unwrap();
    assert_eq!(missing_json["availability"], "neverWritten");
    assert!(missing_json["lastWrittenMs"].is_null());
    // Off is what a fresh profile has, following the reference, which also ships recording off.
    assert_eq!(missing_json["statistics"]["enabled"], false);

    // Turning it on writes the file, which is what moves availability off `neverWritten` - the
    // flag and the availability are reported from the same document but are not the same fact.
    let enabled = store.set_enabled(true).unwrap();
    let ready = super::typing_statistics_status(&store, enabled)
        .ok()
        .unwrap();
    let ready_json = serde_json::to_value(ready).unwrap();
    assert_eq!(ready_json["availability"], "ready");
    assert!(ready_json["lastWrittenMs"].is_number());
    assert_eq!(ready_json["statistics"]["enabled"], true);
}

#[cfg(target_os = "linux")]
#[test]
fn panel_input_targets_are_isolated_by_surface() {
    let state = super::PanelInputState::default();
    let mut targets = state.0.lock().unwrap();
    targets.insert(
        "emoji-panel".into(),
        super::PanelInputTarget::X11("11".into()),
    );
    targets.insert(
        "keyboard-panel".into(),
        super::PanelInputTarget::X11("22".into()),
    );
    targets.remove("emoji-panel");
    assert!(targets.get("emoji-panel").is_none());
    assert!(matches!(
        targets.get("keyboard-panel"),
        Some(super::PanelInputTarget::X11(window)) if window == "22"
    ));
}

#[cfg(not(target_os = "windows"))]
#[test]
fn keyboard_does_not_accept_focus_but_editable_panels_do() {
    assert!(!crate::panel_window::panel_accepts_focus("keyboard-panel"));
    for label in [
        "handwriting-panel",
        "voice-panel",
        "emoji-panel",
        "cloud-clipboard-panel",
        "cloud-dictionary-panel",
    ] {
        assert!(crate::panel_window::panel_accepts_focus(label));
    }
}

#[test]
fn csp_lets_the_skin_editor_decode_a_picked_photo() {
    // The editor reads a picked photo as a data: URL, so the CSP can stay without
    // blob:. jsdom does not enforce CSP, so only this check sees the source go missing.
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let csp = config["app"]["security"]["csp"].as_str().unwrap();
    let img_src = csp
        .split(';')
        .map(str::trim)
        .find(|directive| directive.starts_with("img-src "))
        .unwrap();
    let sources: Vec<&str> = img_src.split_whitespace().collect();
    assert!(sources.contains(&"data:"));
    assert!(!sources.contains(&"blob:"));
}

#[test]
fn every_opened_panel_label_can_be_closed() {
    use msime_client_core::host_surface::{HostPlatform, SurfaceRoute};
    let closable = crate::panel_window::CLOSABLE_PANELS;
    for label in [
        "keyboard-panel",
        "handwriting-panel",
        "emoji-panel",
        "clipboard-panel",
    ] {
        assert!(closable.contains(&label), "{label}");
    }
    // Every open_*_panel command opens a route's panel or the vocabulary panel, on whichever host it runs.
    for platform in [
        HostPlatform::Linux,
        HostPlatform::Macos,
        HostPlatform::Windows,
    ] {
        for surface in SurfaceRoute::ALL
            .into_iter()
            .filter_map(|route| route.panel_for(platform))
        {
            assert!(closable.contains(&surface.label), "{}", surface.label);
        }
    }
    let vocabulary = crate::panel_window::VOCABULARY_PANEL.label;
    assert!(closable.contains(&vocabulary), "{vocabulary}");
}
#[test]
fn toolbar_stylesheet_command_errors_do_not_expose_paths() {
    let state = tempfile::tempdir().unwrap();
    let result = super::read_skin_toolbar_stylesheet_at(state.path().join("skins"), "../sample");
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("expected error"),
    };
    assert_eq!(
        serde_json::to_value(error).unwrap(),
        serde_json::json!({ "code": "storage" })
    );
}
#[test]
fn skin_image_command_contract_filters_non_images_and_paths() {
    let state = tempfile::tempdir().unwrap();
    let root = state.path().join("skins");
    let folder = root.join("sample");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("skin.toml"), "schema_version = 1\nid = 'sample'\nname = 'Sample'\nversion = '1'\nbase = 'system'\n[supports]\nlayouts = ['vertical']\nthemes = ['light']\n[candidate_window]\n[candidate_window.decoration]\n").unwrap();
    std::fs::write(folder.join("preview.png"), [0, 1, 255]).unwrap();
    std::fs::write(folder.join("font.woff2"), [0, 1, 255]).unwrap();
    std::fs::write(folder.join("toolbar.css"), b".sample {}").unwrap();
    std::fs::create_dir_all(folder.join("styles")).unwrap();
    std::fs::write(
        folder.join("styles/imported.css"),
        b"\xEF\xBB\xBF.imported {}",
    )
    .unwrap();
    assert!(matches!(
        super::read_skin_toolbar_stylesheet_at(root.clone(), "sample"),
        Ok(None)
    ));
    let manifest = std::fs::read_to_string(folder.join("skin.toml")).unwrap();
    std::fs::write(
        folder.join("skin.toml"),
        format!("toolbar_stylesheet = 'toolbar.css'\n{manifest}"),
    )
    .unwrap();
    assert!(
        matches!(super::read_skin_toolbar_stylesheet_at(root.clone(), "sample"), Ok(Some(css)) if css == ".sample {}")
    );
    assert!(matches!(
        super::read_skin_stylesheet_at(root.clone(), "sample", "styles/imported.css"),
        Ok(css) if css == ".imported {}"
    ));
    assert!(super::read_skin_stylesheet_at(root.clone(), "sample", "preview.png").is_err());
    assert!(super::read_skin_stylesheet_at(root.clone(), "sample", "../toolbar.css").is_err());
    std::fs::write(folder.join("styles/broken.css"), [0xff, 0xfe]).unwrap();
    assert!(super::read_skin_stylesheet_at(root.clone(), "sample", "styles/broken.css").is_err());
    let result = super::read_skin_image_at(root.clone(), "sample", "preview.png")
        .ok()
        .unwrap();
    let json = serde_json::to_value(result).unwrap();
    assert_eq!(json["contentType"], "image/png");
    assert_eq!(json["bytes"], serde_json::json!([0, 1, 255]));
    let font = super::read_skin_font_at(root.clone(), "sample", "font.woff2")
        .ok()
        .unwrap();
    let font_json = serde_json::to_value(font).unwrap();
    assert_eq!(font_json["contentType"], "font/woff2");
    assert_eq!(font_json["bytes"], serde_json::json!([0, 1, 255]));
    assert!(super::read_skin_font_at(root.clone(), "sample", "preview.png").is_err());
    assert!(super::read_skin_font_at(root.clone(), "sample", "../font.woff2").is_err());
    assert!(super::read_skin_font_at(root.clone(), "../sample", "font.woff2").is_err());
    assert!(super::read_skin_image_at(root.clone(), "sample", "toolbar.css").is_err());
    assert!(super::read_skin_image_at(root.clone(), "sample", "../preview.png").is_err());
    assert!(super::read_skin_image_at(root, "../sample", "preview.png").is_err());
}

#[test]
fn skin_catalog_response_uses_host_root_and_preserves_scan_results() {
    let state = tempfile::tempdir().unwrap();
    let root = state.path().join("skins");
    let folder = root.join("sample");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("skin.toml"),
        r#"schema_version = 1
id = 'sample'
name = 'Sample'
version = '1'
base = 'system'
[supports]
layouts = ['vertical']
themes = ['light']
[candidate_window]
[candidate_window.decoration]
"#,
    )
    .unwrap();
    std::fs::create_dir(root.join("Bad")).unwrap();
    let result = serde_json::to_value(super::read_skin_catalog(root.clone())).unwrap();
    assert_eq!(result["directory"], root.to_string_lossy().as_ref());
    assert_eq!(result["packages"][0]["id"], "sample");
    assert_eq!(result["packages"].as_array().unwrap().len(), 1);
    assert_eq!(result["issues"].as_array().unwrap().len(), 1);
    assert_eq!(
        result["packages"][0]["layouts"],
        serde_json::json!(["vertical"])
    );
    assert_eq!(result["issues"][0]["folder"], "Bad");
    assert!(result.get("catalog").is_none());
}

#[test]
fn scanning_missing_skin_directory_does_not_create_it() {
    let state = tempfile::tempdir().unwrap();
    let root = state.path().join("skins");
    let result = super::read_skin_catalog(root.clone());
    assert!(result.catalog.packages.is_empty());
    assert!(result.catalog.issues.is_empty());
    assert!(!root.exists());
}

// The diagnostic-log action takes no path from the webview: it resolves from the host's preferences directory alone, selects the file in Finder once the input method has written it, and falls back to the directory before that.
#[test]
fn diagnostic_log_target_resolves_from_the_host_directory() {
    let state = tempfile::tempdir().unwrap();
    let directory = state.path().join("MSIME");
    std::fs::create_dir_all(&directory).unwrap();
    assert_eq!(
        super::diagnostic_log_target(&directory),
        super::DiagnosticLogTarget::Directory(directory.clone())
    );
    std::fs::write(directory.join("diagnostic.log"), "focus_in\n").unwrap();
    #[cfg(target_os = "macos")]
    assert_eq!(
        super::diagnostic_log_target(&directory),
        super::DiagnosticLogTarget::File(directory.join("diagnostic.log"))
    );
    #[cfg(not(target_os = "macos"))]
    assert_eq!(
        super::diagnostic_log_target(&directory),
        super::DiagnosticLogTarget::Directory(directory.clone())
    );
    // A directory named like the log is not mistaken for it.
    let other = state.path().join("other");
    std::fs::create_dir_all(other.join("diagnostic.log")).unwrap();
    assert_eq!(
        super::diagnostic_log_target(&other),
        super::DiagnosticLogTarget::Directory(other.clone())
    );
}

#[cfg(target_os = "linux")]
use super::*;
// The panel helpers these cover live in `panel_input` since the delivery code
// moved out of the crate root; `use super::*` no longer reaches them.
#[cfg(target_os = "linux")]
use super::panel_input::{
    focused_sway_container, ime_key_request, panel_text_requires_clipboard, parse_ime_reply,
    parse_xdotool_geometry, sway_rect_for_container, sway_workspace_for_container,
    x11_window_is_owned_by_process, ImeReply,
};

#[cfg(target_os = "linux")]
#[test]
fn linux_input_method_reply_separates_delivery_from_refusal() {
    assert_eq!(parse_ime_reply("{\"ok\":true}\n"), Some(ImeReply::Ok(None)));
    assert_eq!(
        parse_ime_reply("{\"generation\":9,\"ok\":true}\n"),
        Some(ImeReply::Ok(Some(9)))
    );
    assert_eq!(
        parse_ime_reply("{\"error\":\"no_focus\",\"ok\":false}\n"),
        Some(ImeReply::Declined)
    );
    // A host refusal must not be treated as permission to replay through a tool.
    assert_eq!(
        parse_ime_reply("{\"error\":\"restricted\",\"ok\":false}\n"),
        Some(ImeReply::Rejected)
    );
    assert_eq!(
        parse_ime_reply("{\"error\":\"invalid\",\"ok\":false}\n"),
        Some(ImeReply::Rejected)
    );
    assert_eq!(parse_ime_reply("{\"ok\":false}"), None);
    // A missing or garbled answer is not a refusal: the host may already have typed the text.
    assert_eq!(parse_ime_reply(""), None);
    assert_eq!(parse_ime_reply("{\"ok\":\"yes\"}"), None);
    assert_eq!(parse_ime_reply("{}"), None);
}

#[cfg(target_os = "linux")]
#[test]
fn linux_input_method_key_request_names_the_keysym_and_evdev_code() {
    let request: msime_client_core::panels::KeyboardInputRequest =
        serde_json::from_value(serde_json::json!({
            "virtual_key": 0x41,
            "shift": true,
            "modifiers": { "ctrl": true, "alt": false, "win": true },
            "include_sticky_modifiers": true,
        }))
        .unwrap();
    assert_eq!(
        ime_key_request(&request),
        Some(serde_json::json!({
            "op": "key", "key": "a", "keycode": 30, "shift": true,
            "control": true, "alt": false, "super": true,
        }))
    );
    // Commit and navigation keys leave the sticky modifiers behind.
    let backspace: msime_client_core::panels::KeyboardInputRequest =
        serde_json::from_value(serde_json::json!({
            "virtual_key": 0x08,
            "shift": false,
            "modifiers": { "ctrl": true, "alt": true, "win": true },
            "include_sticky_modifiers": false,
        }))
        .unwrap();
    assert_eq!(
        ime_key_request(&backspace),
        Some(serde_json::json!({
            "op": "key", "key": "BackSpace", "keycode": 14, "shift": false,
            "control": false, "alt": false, "super": false,
        }))
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_xdotool_geometry_requires_complete_numeric_shell_fields() {
    let geometry = "WINDOW=4194305\nX=120\nY=48\nWIDTH=1280\nHEIGHT=720\nSCREEN=1\n";
    assert_eq!(
        parse_xdotool_geometry(geometry),
        Some((120.0, 48.0, 1280.0, 720.0))
    );

    for malformed in [
        "X=120\nY=48\nWIDTH=1280\n",
        "X=120\nY=48\nWIDTH=1280\nHEIGHT=oops\n",
        "X=120\nY=48\nWIDTH=1280\nHEIGHT=720\nBROKEN",
    ] {
        assert_eq!(parse_xdotool_geometry(malformed), None);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linux_x11_panel_target_pid_matching_rejects_our_own_window() {
    assert!(x11_window_is_owned_by_process("4242\n", 4242));
    assert!(!x11_window_is_owned_by_process("4243\n", 4242));
    assert!(!x11_window_is_owned_by_process("not-a-pid\n", 4242));
    assert!(!x11_window_is_owned_by_process("", 4242));
}

#[cfg(target_os = "linux")]
#[test]
fn linux_panel_text_uses_clipboard_for_non_ascii_on_keymap_backends() {
    assert!(panel_text_requires_clipboard(
        &PanelInputTarget::X11("11".into()),
        "你好😀"
    ));
    assert!(panel_text_requires_clipboard(
        &PanelInputTarget::Ydotool,
        "你好"
    ));
    assert!(!panel_text_requires_clipboard(
        &PanelInputTarget::Wayland,
        "你好😀"
    ));
    assert!(panel_text_requires_clipboard(
        &PanelInputTarget::Wayland,
        "line\nnext"
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn linux_sway_target_and_geometry_walk_nested_and_floating_nodes() {
    let tree = serde_json::json!({
        "type": "root",
        "nodes": [{
            "type": "workspace",
            "id": 7,
            "rect": {"x": 10, "y": 20, "width": 1600, "height": 900},
            "nodes": [{
                "type": "con",
                "id": 42,
                "focused": true,
                "rect": {"x": 110, "y": 220, "width": 900, "height": 600}
            }],
            "floating_nodes": [{
                "type": "floating_con",
                "id": 99,
                "rect": {"x": 300, "y": 400, "width": 300, "height": 200}
            }]
        }]
    });

    assert_eq!(focused_sway_container(&tree), Some(42));
    assert_eq!(
        sway_rect_for_container(&tree, 42),
        Some((110.0, 220.0, 900.0, 600.0))
    );
    assert_eq!(
        sway_rect_for_container(&tree, 99),
        Some((300.0, 400.0, 300.0, 200.0))
    );
    assert_eq!(
        sway_workspace_for_container(&tree, 42, None),
        Some((10.0, 20.0, 1600.0, 900.0))
    );
    assert_eq!(sway_rect_for_container(&tree, 404), None);
    assert_eq!(sway_workspace_for_container(&tree, 404, None), None);
}

#[cfg(target_os = "linux")]
#[test]
fn linux_sway_workspace_does_not_leak_across_sibling_workspaces() {
    let tree = serde_json::json!({
        "type": "root",
        "nodes": [
            {"type": "workspace", "id": 1,
             "rect": {"x": 0, "y": 0, "width": 800, "height": 600},
             "nodes": [{"id": 11, "rect": {"x": 0, "y": 0, "width": 800, "height": 600}}]},
            {"type": "workspace", "id": 2,
             "rect": {"x": 800, "y": 0, "width": 800, "height": 600},
             "nodes": [{"id": 22, "rect": {"x": 800, "y": 0, "width": 800, "height": 600}}]}
        ]
    });

    assert_eq!(
        sway_workspace_for_container(&tree, 22, None),
        Some((800.0, 0.0, 800.0, 600.0))
    );
    assert_eq!(sway_workspace_for_container(&tree, 33, None), None);
}

#[cfg(target_os = "linux")]
#[test]
fn runtime_options_sync_replaces_preferences_atomically() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("runtime-options.json");
    let document = serde_json::json!({
        "api_version": 1,
        "resources": "/resources",
        "preferences": {"candidate_page_size": 5}
    });
    std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    let state = RuntimeOptionsState {
        path: Some(path.clone()),
        document: Arc::new(Mutex::new(document)),
        skins: None,
    };
    let preferences = Preferences {
        candidate_page_size: 9,
        ..Preferences::default()
    };
    sync_runtime_options(&state, &preferences).unwrap();
    let updated: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(updated["preferences"]["candidate_page_size"], 9);
}

#[cfg(target_os = "linux")]
fn write_candidate_skin(skins: &std::path::Path, id: &str, name: &str) {
    let package = skins.join(id);
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(
        package.join("skin.toml"),
        format!("schema_version = 1\nid = '{id}'\nname = '{name}'\nversion = '1.0'\nbase = 'paper'\n[supports]\nlayouts = ['vertical', 'horizontal']\nthemes = ['light', 'dark']\n[candidate_window]\nmin_width_dip = 10\n[candidate_window.decoration]\ntop_inset_dip = 0\nwidth_dip = 0\n[candidate.light]\nsurface = '#fff0f5'\nselected = '#ff69b4'\ntext = '#301020'\n[candidate.dark]\nsurface = '#301020'\ntext = '#ffe4e1'\nborder = '#ff000080'\n"),
    )
    .unwrap();
}

#[test]
fn resolve_theme_reads_the_custom_package_from_the_host_skin_root() {
    use super::{resolve_theme_at, ResolveThemeRequest};
    use msime_client_core::skin::theme::{GlobalTheme, ThemeSource};
    let directory = tempfile::tempdir().expect("temporary directory");
    let package = directory.path().join("sakura");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(
        package.join("skin.toml"),
        "schema_version = 1\nid = 'sakura'\nname = '樱花'\nversion = '1.0'\nbase = 'paper'\n[supports]\nlayouts = ['vertical', 'horizontal']\nthemes = ['light', 'dark']\n[candidate_window]\nmin_width_dip = 10\n[candidate_window.decoration]\ntop_inset_dip = 0\nwidth_dip = 0\n[candidate.light]\nsurface = '#fff0f5'\n[candidate.dark]\nsurface = '#301020'\n",
    )
    .unwrap();
    let midnight = directory.path().join("midnight");
    std::fs::create_dir_all(&midnight).unwrap();
    std::fs::write(
        midnight.join("skin.toml"),
        "schema_version = 1\nid = 'midnight'\nname = '午夜'\nversion = '1.0'\nbase = 'night'\n[supports]\nlayouts = ['vertical', 'horizontal']\nthemes = ['dark']\n[candidate_window]\nmin_width_dip = 10\n[candidate_window.decoration]\ntop_inset_dip = 0\nwidth_dip = 0\n[candidate.dark]\nsurface = '#101828'\n",
    )
    .unwrap();
    let request =
        |value: serde_json::Value| serde_json::from_value::<ResolveThemeRequest>(value).unwrap();
    let resolved = resolve_theme_at(
        directory.path(),
        request(serde_json::json!({
            "global_theme": "custom",
            "custom_theme": { "candidate_skin": "sakura", "candidate_colors": { "text": "#123456" } },
            "dark": false,
            "layout": "vertical",
        })),
    )
    .unwrap();
    assert_eq!(resolved.source, ThemeSource::Custom);
    assert_eq!(resolved.candidate_skin.as_deref(), Some("sakura"));
    let candidate = resolved.candidate.expect("custom candidate palette");
    // 皮肤包的 paper 底固定浅色，画的是它的浅色配色。
    assert_eq!(
        resolved.appearance,
        Some(msime_client_core::skin::theme::ThemeAppearance::Light)
    );
    assert_eq!(candidate.surface.as_deref(), Some("#FFF0F5"));
    assert_eq!(candidate.text.as_deref(), Some("#123456"));

    // 深色模式取 `candidate_skin_dark` 槽位的包；没设深色槽位时，浅色皮肤在深色模式下不画。
    let dark = resolve_theme_at(
        directory.path(),
        request(serde_json::json!({
            "global_theme": "custom",
            "custom_theme": { "candidate_skin": "sakura", "candidate_skin_dark": "midnight" },
            "dark": true,
            "layout": "vertical",
        })),
    )
    .unwrap();
    assert_eq!(dark.candidate_skin.as_deref(), Some("midnight"));
    assert_eq!(
        dark.candidate
            .expect("dark slot palette")
            .surface
            .as_deref(),
        Some("#101828")
    );
    let light_only = resolve_theme_at(
        directory.path(),
        request(serde_json::json!({
            "global_theme": "custom",
            "custom_theme": { "candidate_skin": "sakura" },
            "dark": true,
            "layout": "vertical",
        })),
    )
    .unwrap();
    assert_eq!(light_only.candidate_skin, None);

    // The layout is required: without it the page could not ask for the surface it previews.
    assert!(
        serde_json::from_value::<ResolveThemeRequest>(serde_json::json!({
            "global_theme": "custom",
            "dark": false,
        }))
        .is_err()
    );

    // A package that is not installed resolves without it, and a built-in theme ignores the custom one.
    let missing = resolve_theme_at(
        directory.path(),
        request(serde_json::json!({
            "global_theme": "custom",
            "custom_theme": { "candidate_skin": "gone" },
            "dark": false,
            "layout": "vertical",
        })),
    )
    .unwrap();
    assert_eq!(missing.candidate_skin, None);
    let builtin = resolve_theme_at(
        directory.path(),
        request(serde_json::json!({
            "global_theme": "night",
            "custom_theme": { "candidate_skin": "sakura" },
            "dark": false,
            "layout": "vertical",
        })),
    )
    .unwrap();
    assert_eq!(builtin.id, GlobalTheme::Night);
    assert_eq!(builtin.candidate_skin, None);

    // A custom theme over a built-in base keeps that base's fixed mode when no package applies.
    let based = resolve_theme_at(
        directory.path(),
        request(serde_json::json!({
            "global_theme": "custom",
            "custom_theme": { "base": "night" },
            "dark": false,
            "layout": "vertical",
        })),
    )
    .unwrap();
    assert_eq!(
        based.appearance,
        Some(msime_client_core::skin::theme::ThemeAppearance::Dark)
    );
    assert!(based.keyboard.is_some());
    // Retired and misspelt ids are refused, not read as system.
    for id in ["fluent", "forest", "Night"] {
        assert!(
            serde_json::from_value::<ResolveThemeRequest>(serde_json::json!({
                "global_theme": id,
                "dark": false,
                "layout": "vertical",
            }))
            .is_err(),
            "{id}"
        );
    }

    // The custom theme is validated like a saved preference; unknown fields are refused.
    assert!(resolve_theme_at(
        directory.path(),
        request(serde_json::json!({
            "global_theme": "custom",
            "custom_theme": { "candidate_colors": { "text": "red" } },
            "dark": false,
            "layout": "vertical",
        })),
    )
    .is_err());
    assert!(
        serde_json::from_value::<ResolveThemeRequest>(serde_json::json!({
            "global_theme": "custom",
            "dark": false,
            "layout": "vertical",
            "skins_directory": "/etc",
        }))
        .is_err()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn runtime_options_sync_publishes_the_installed_skin_catalog() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("runtime-options.json");
    let skins = directory.path().join("skins");
    write_candidate_skin(&skins, "sakura", "樱花");
    let document =
        serde_json::json!({"api_version": 1, "resources": "/resources", "preferences": {}});
    std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    let state = RuntimeOptionsState {
        path: Some(path.clone()),
        document: Arc::new(Mutex::new(document)),
        skins: Some(skins.clone()),
    };
    sync_runtime_options(&state, &Preferences::default()).unwrap();
    let updated: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    // The shape CandidateSkinCatalog.h and CandidateColors.h read: the manifest name under the key `title`, and the colours per theme.
    assert_eq!(
        updated["candidate_skin_catalog"],
        serde_json::json!({"packages": [{
            "id": "sakura",
            "title": "樱花",
            "base": "paper",
            "layouts": ["vertical", "horizontal"],
            "candidate": {
                "light": {"surface": "#FFF0F5", "selected": "#FF69B4", "text": "#301020"},
                "dark": {"surface": "#301020", "text": "#FFE4E1", "border": "#FF000080"},
            },
        }]})
    );
    assert_eq!(updated["resources"], "/resources");

    // A rescan after the user removes the package publishes the smaller list without a save.
    std::fs::remove_dir_all(skins.join("sakura")).unwrap();
    publish_candidate_skin_catalog(&state, &msime_client_core::skin::catalog::scan(&skins))
        .unwrap();
    let rescanned: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        rescanned["candidate_skin_catalog"],
        serde_json::json!({"packages": []})
    );
    assert_eq!(rescanned["preferences"], updated["preferences"]);

    // Before setup there is no document to publish into, and that is not a failure.
    let missing = RuntimeOptionsState {
        path: Some(directory.path().join("absent.json")),
        document: Arc::new(Mutex::new(Value::Null)),
        skins: Some(skins),
    };
    publish_candidate_skin_catalog(
        &missing,
        &msime_client_core::skin::catalog::SkinCatalog::default(),
    )
    .unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn runtime_options_skin_catalog_stays_within_what_the_hosts_read() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let skins = directory.path().join("skins");
    for index in 0..40 {
        write_candidate_skin(
            &skins,
            &format!("skin{index:02}"),
            &format!("皮肤 {index:02}"),
        );
    }
    let catalog = msime_client_core::skin::catalog::scan(&skins);
    let mut preferences = serde_json::to_value(Preferences::default()).unwrap();
    // The last package by name: beyond the package cap and the first to go when trimming, were the selection not protected in both.
    preferences["custom_theme"]["candidate_skin"] = "skin39".into();
    // 深色槽位的皮肤同样在上限之外，同样要留下。
    preferences["custom_theme"]["candidate_skin_dark"] = "skin38".into();
    let mut document = serde_json::json!({"api_version": 1, "preferences": preferences});
    let bytes = runtime_options_with_skin_catalog(&mut document, &skins, &catalog).unwrap();
    assert!(
        bytes.len() <= LINUX_RUNTIME_OPTIONS_CATALOG_BUDGET,
        "{}",
        bytes.len()
    );
    let packages = document["candidate_skin_catalog"]["packages"]
        .as_array()
        .unwrap()
        .clone();
    assert!(
        !packages.is_empty() && packages.len() < 32,
        "{}",
        packages.len()
    );
    assert!(packages.iter().any(|package| package["id"] == "skin39"));
    assert!(packages.iter().any(|package| package["id"] == "skin38"));
    assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), document);

    // A document already too large for the hosts is not made larger by the catalog.
    let mut crowded = serde_json::json!({"preferences": {}, "padding": "x".repeat(LINUX_RUNTIME_OPTIONS_CATALOG_BUDGET)});
    runtime_options_with_skin_catalog(&mut crowded, &skins, &catalog).unwrap();
    assert!(crowded.get("candidate_skin_catalog").is_none());
}

/// About 440 KiB once decoded: a PNG signature followed by zeros, which the preference validation accepts as a screen-keyboard photo.
#[cfg(target_os = "linux")]
fn screen_keyboard_photo() -> String {
    let photo = format!("iVBORw0KGgoA{}", "AAAA".repeat(149_997));
    assert_eq!(photo.len(), 600_000);
    photo
}

#[cfg(target_os = "linux")]
fn runtime_options_fixture(directory: &std::path::Path) -> (PathBuf, PathBuf) {
    let path = directory.join("runtime-options.json");
    let skins = directory.join("skins");
    for (id, name) in [("sakura", "樱花"), ("bamboo", "竹"), ("sumi", "墨")] {
        write_candidate_skin(&skins, id, name);
    }
    let document = serde_json::json!({
        "api_version": 1,
        "resources": "/resources",
        "preferences": Preferences::default(),
    });
    std::fs::write(&path, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
    (path, skins)
}

#[cfg(target_os = "linux")]
#[test]
fn runtime_options_sync_keeps_the_screen_keyboard_photo_out_of_the_host_copy() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (path, skins) = runtime_options_fixture(directory.path());
    let state = RuntimeOptionsState {
        path: Some(path.clone()),
        document: Arc::new(Mutex::new(Value::Null)),
        skins: Some(skins),
    };
    let mut preferences = Preferences {
        candidate_page_size: 9,
        ..Preferences::default()
    };
    preferences.custom_theme.keyboard =
        Some(msime_client_core::preferences::TouchKeyboardSkinDesign {
            photo: Some(screen_keyboard_photo()),
            photo_shade: Some(0.5),
            ..Default::default()
        });
    sync_runtime_options(&state, &preferences).unwrap();

    let bytes = std::fs::read(&path).unwrap();
    assert!(
        bytes.len() <= LINUX_RUNTIME_OPTIONS_LIMIT,
        "{}",
        bytes.len()
    );
    let updated: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(updated["resources"], "/resources");
    assert_eq!(updated["preferences"]["candidate_page_size"], 9);
    let design = updated["preferences"]["custom_theme"]["keyboard"]
        .as_object()
        .unwrap();
    assert!(!design.contains_key("photo"));
    assert_eq!(design["photoShade"], 0.5);
    assert_eq!(
        updated["candidate_skin_catalog"]["packages"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    // The host copy is still a whole Preferences document to the Host API, which reads the missing photo as none.
    let host: Preferences = serde_json::from_value(updated["preferences"].clone()).unwrap();
    assert_eq!(
        host.custom_theme.keyboard.and_then(|design| design.photo),
        None
    );
    assert_eq!(host.candidate_page_size, 9);
}

#[cfg(target_os = "linux")]
#[test]
fn runtime_options_the_hosts_could_not_read_are_refused_and_the_old_file_kept() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (path, skins) = runtime_options_fixture(directory.path());
    let original = std::fs::read(&path).unwrap();
    let mut preferences = Preferences::default();
    // Nothing strips a prompt from the host copy, so a long one is what still outgrows the hosts' read.
    preferences.voice_input.polish_prompt_custom_1 = "润色".repeat(4000);
    // Both the path that publishes a skin catalog and the one without a skins directory are held to the same limit.
    for skins in [Some(skins.clone()), None] {
        let state = RuntimeOptionsState {
            path: Some(path.clone()),
            document: Arc::new(Mutex::new(Value::Null)),
            skins,
        };
        assert!(matches!(
            sync_runtime_options(&state, &preferences),
            Err(RuntimeOptionsError::TooLarge)
        ));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(*state.document.lock().unwrap(), Value::Null);
    }

    // A rescan cannot republish a file that is already past the limit either: the catalog is dropped, and what remains is still refused rather than rewritten.
    let mut oversized: Value = serde_json::from_slice(&original).unwrap();
    oversized["preferences"]["voice_input"]["polish_prompt_custom_1"] = "润色".repeat(4000).into();
    let oversized = serde_json::to_vec_pretty(&oversized).unwrap();
    std::fs::write(&path, &oversized).unwrap();
    let state = RuntimeOptionsState {
        path: Some(path.clone()),
        document: Arc::new(Mutex::new(Value::Null)),
        skins: Some(skins.clone()),
    };
    assert!(matches!(
        publish_candidate_skin_catalog(&state, &msime_client_core::skin::catalog::scan(&skins)),
        Err(RuntimeOptionsError::TooLarge)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), oversized);
}

#[cfg(target_os = "linux")]
#[test]
fn a_save_the_hosts_could_not_read_is_refused_and_the_store_keeps_its_preferences() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (path, skins) = runtime_options_fixture(directory.path());
    let store = Arc::new(PreferencesStore::new(directory.path().join("state")));
    let state = RuntimeOptionsState {
        path: Some(path.clone()),
        document: Arc::new(Mutex::new(Value::Null)),
        skins: Some(skins),
    };
    let save = |revision: u64, preferences: Preferences| {
        tauri::async_runtime::block_on(save_preferences_impl(
            store.clone(),
            state.clone(),
            revision,
            preferences,
        ))
    };

    // Picking a photo for the screen keyboard saves: the store keeps it, and the hosts get a copy they can still read.
    let mut photographed = store.load().unwrap().preferences;
    photographed.custom_theme.keyboard =
        Some(msime_client_core::preferences::TouchKeyboardSkinDesign {
            photo: Some(screen_keyboard_photo()),
            ..Default::default()
        });
    let saved = save(store.load().unwrap().revision, photographed.clone()).unwrap();
    assert_eq!(
        store.load().unwrap().preferences.custom_theme.keyboard,
        photographed.custom_theme.keyboard
    );
    let published = std::fs::read(&path).unwrap();
    assert!(
        published.len() <= LINUX_RUNTIME_OPTIONS_LIMIT,
        "{}",
        published.len()
    );

    // A save the hosts could not read is refused whole: the runtime options stay as they were, and so does the store.
    let mut oversized = saved.preferences.clone();
    oversized.voice_input.polish_prompt_custom_1 = "润色".repeat(4000);
    let refused = save(saved.revision, oversized).unwrap_err();
    assert_eq!(refused.code, "runtime_options_too_large");
    assert_eq!(std::fs::read(&path).unwrap(), published);
    let current = store.load().unwrap();
    assert_eq!(current.preferences, saved.preferences);
}

#[cfg(target_os = "linux")]
#[test]
fn dictionary_requests_do_not_see_the_published_skin_catalog() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("runtime-options.json");
    let data = directory.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    let data = data.to_string_lossy().into_owned();
    let document = serde_json::json!({
        "api_version": 1,
        "resources": data,
        "user_data": data,
        "cache": data,
        "dictionaries": data,
        "preferences": Preferences::default(),
        "candidate_skin_catalog": {"packages": []},
    });
    std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    let action = serde_json::json!({"operation": "list", "offset": 0, "limit": 10});
    let request = |options: &Value| {
        serde_json::to_vec(&serde_json::json!({"options": options, "action": action})).unwrap()
    };
    // The Host API rejects the whole request when the catalog reaches it, which is what every dictionary page hit after the first save.
    assert_eq!(
        msime_host_api::dictionary_request_json(&request(&document)).unwrap_err(),
        "invalid dictionary request"
    );

    let options = DictionaryHostOptions { path }.snapshot().unwrap();
    assert!(options.get("candidate_skin_catalog").is_none());
    assert_eq!(options["user_data"], document["user_data"]);
    assert_ne!(
        msime_host_api::dictionary_request_json(&request(&options))
            .err()
            .as_deref(),
        Some("invalid dictionary request")
    );
}

#[cfg(target_os = "linux")]
#[test]
fn skin_rescan_keeps_its_list_when_the_catalog_cannot_be_published() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("runtime-options.json");
    let skins = directory.path().join("skins");
    write_candidate_skin(&skins, "sakura", "樱花");
    std::fs::write(&path, b"{ not json").unwrap();
    let state = RuntimeOptionsState {
        path: Some(path.clone()),
        document: Arc::new(Mutex::new(Value::Null)),
        skins: Some(skins.clone()),
    };
    let response = rescan_skin_catalog(skins, &state);
    assert_eq!(response.catalog.packages.len(), 1);
    assert_eq!(std::fs::read(&path).unwrap(), b"{ not json");
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn desktop_account_storage_round_trips_an_owner_only_session() {
    use msime_client_core::account::{
        AccountSessionFileLayout, AccountSessionStorage, AccountTokens, AccountUser,
        FileAccountSessionStorage, SavedAccountSession,
    };
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().expect("temporary directory");
    let storage =
        FileAccountSessionStorage::new(directory.path(), AccountSessionFileLayout::Native);
    // Nothing saved yet is an empty store, not a broken one: a first run must
    // report "signed out" rather than "secure storage is unavailable".
    assert!(storage.load().expect("empty store").is_none());

    let session = SavedAccountSession {
        tokens: AccountTokens {
            access_token: "synthetic-access".into(),
            refresh_token: "synthetic-refresh".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
            user: AccountUser {
                id: "synthetic-id".into(),
                display_name: "合成用户".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                email: None,
                avatar_url: None,
            },
        },
        expires_at_unix_ms: 1_700_000_000_000,
        session_id: None,
    };
    storage.save(&session).expect("save");
    let path = directory.path().join("account-session.json");
    // The tokens must never be readable by the rest of the machine, and the
    // temporary file used to publish them must not be left behind.
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(!directory.path().join("account-session.json.new").exists());
    let loaded = storage.load().expect("load").expect("a saved session");
    assert_eq!(loaded.tokens.access_token, "synthetic-access");
    assert_eq!(loaded.expires_at_unix_ms, 1_700_000_000_000);

    // A store another user can read is not one this host wrote. Reporting it as
    // a storage failure keeps the session out of use; answering "signed out"
    // would quietly start a new login against a file someone else can read.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(storage.load().is_err());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(storage.load().expect("load").is_some());

    // A leftover world-readable temporary from an interrupted save must not
    // pass its mode on to the published tokens.
    let temporary = directory.path().join("account-session.json.new");
    std::fs::write(&temporary, b"stale").unwrap();
    std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o644)).unwrap();
    storage.save(&session).expect("save over a stale temporary");
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(storage.load().expect("load").is_some());

    storage.clear().expect("clear");
    assert!(!path.exists());
    // Clearing an already-cleared store is the normal path after a failed
    // refresh, so it is not an error.
    storage.clear().expect("clear again");
    assert!(storage.load().expect("cleared store").is_none());
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn desktop_account_storage_refuses_a_symlinked_or_oversized_store() {
    use msime_client_core::account::{
        AccountSessionFileLayout, AccountSessionStorage, FileAccountSessionStorage,
    };

    let directory = tempfile::tempdir().expect("temporary directory");
    let elsewhere = directory.path().join("elsewhere.json");
    std::fs::write(&elsewhere, b"{}").unwrap();
    let storage =
        FileAccountSessionStorage::new(directory.path(), AccountSessionFileLayout::Native);
    let path = directory.path().join("account-session.json");
    std::os::unix::fs::symlink(&elsewhere, &path).unwrap();
    // Following the link would read through a path this host did not choose.
    assert!(storage.load().is_err());
    std::fs::remove_file(&path).unwrap();

    std::fs::write(&path, vec![b'x'; 64 * 1024 + 1]).unwrap();
    std::fs::set_permissions(
        &path,
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o600),
    )
    .unwrap();
    // A session document is two JWTs and an expiry; this is not one, and it is
    // rejected on its size before any of it is parsed.
    assert!(storage.load().is_err());
}

#[cfg(not(target_os = "android"))]
#[test]
fn the_macos_release_is_read_or_left_out() {
    let plist = concat!(
        "<plist><dict><key>ProductName</key><string>macOS</string>",
        "<key>ProductVersion</key><string>27.0</string>",
        "<key>ProductBuildVersion</key><string>27A1234</string></dict></plist>"
    );
    assert_eq!(
        crate::product_version_from_plist(plist).as_deref(),
        Some("27.0")
    );
    // Anything that is not a release number is left out rather than guessed at: this string ends
    // up in a report the user files, and the build version right beside it is not it.
    for rejected in [
        "<plist><dict><key>ProductBuildVersion</key><string>27A1234</string></dict></plist>",
        "<key>ProductVersion</key><string>27A1234</string>",
        "<key>ProductVersion</key><string></string>",
        "<key>ProductVersion</key><string>27.0",
    ] {
        assert_eq!(crate::product_version_from_plist(rejected), None);
    }
}

/// The settings page reads `backupPath` and `salvaged` from a repair; only the struct's camelCase rename produces those names, and the page's own tests mock the reply, so nothing else would notice a rename dropping it.
#[cfg(not(target_os = "ios"))]
#[test]
fn preferences_recovery_serializes_the_fields_the_settings_page_reads() {
    use msime_client_core::preferences::PreferencesSnapshot;

    let recovered = serde_json::to_value(super::PreferencesRecovery {
        snapshot: PreferencesSnapshot::default(),
        backup_path: Some("/synthetic/preferences.json.corrupt".into()),
        salvaged: true,
    })
    .unwrap();
    assert_eq!(
        recovered["backupPath"],
        "/synthetic/preferences.json.corrupt"
    );
    assert_eq!(recovered["salvaged"], true);
    assert!(recovered["snapshot"]["preferences"].is_object());
    assert!(recovered.get("backup_path").is_none());

    let untouched = serde_json::to_value(super::PreferencesRecovery {
        snapshot: PreferencesSnapshot::default(),
        backup_path: None,
        salvaged: false,
    })
    .unwrap();
    assert!(untouched["backupPath"].is_null());
}

/// A re-check from the settings page reads the input source list and the system-wide input method directory again and reports the rest of the start-time result unchanged, without running the install a second time.
#[cfg(target_os = "macos")]
#[test]
fn input_source_status_rereads_only_what_the_user_can_change() {
    let state = super::InputSourceStartupState::default();
    state.finish(Some(super::InputSourceStartupStatus {
        action: "installed",
        enabled: Some(false),
        bundled_version: Some("0.50.0 (1)".into()),
        installed_version: Some("0.50.0 (1)".into()),
        system_bundles: Vec::new(),
    }));
    let timeout = std::time::Duration::ZERO;
    let system_copy = || {
        vec![std::path::PathBuf::from(
            "/Library/Input Methods/MetasequoiaIME.app",
        )]
    };

    let before = super::input_source_status_now(
        &state,
        timeout,
        || Some(false),
        system_copy,
        || panic!("an install made by this launch is not looked up in the registry"),
    )
    .unwrap();
    assert_eq!(before.enabled, Some(false));
    assert_eq!(
        before.system_bundles,
        vec!["/Library/Input Methods/MetasequoiaIME.app".to_string()]
    );
    let after =
        super::input_source_status_now(&state, timeout, || Some(true), Vec::new, || None).unwrap();
    assert_eq!(after.action, "installed");
    assert_eq!(after.enabled, Some(true));
    assert!(after.system_bundles.is_empty());
    assert_eq!(after.installed_version.as_deref(), Some("0.50.0 (1)"));
    let unreadable =
        super::input_source_status_now(&state, timeout, || None, Vec::new, || None).unwrap();
    assert_eq!(unreadable.enabled, None);
}

/// A launch that ran no start-time check (a development run, a panel launch) has nothing to re-check, and reads neither the input source list nor the system directory for it.
#[cfg(target_os = "macos")]
#[test]
fn input_source_status_is_absent_when_no_start_time_check_ran() {
    let state = super::InputSourceStartupState::default();
    state.finish(None);
    let status = super::input_source_status_now(
        &state,
        std::time::Duration::ZERO,
        || panic!("the input source list is not read without a start-time result"),
        || panic!("the system directory is not read without a start-time result"),
        || panic!("the registry is not read without a start-time result"),
    );
    assert!(status.is_none());
}

/// 首次安装后同一次登录里再打开设置应用：启动检查只能报 `up_to_date`，输入法不在输入法列表里、也不在本登录会话的注册表里时改报 `login_required`，而不是叫用户去添加一个这次登录列不出来的输入法。
#[cfg(target_os = "macos")]
#[test]
fn an_up_to_date_input_method_missing_from_the_registry_needs_a_login() {
    let up_to_date = || super::InputSourceStartupStatus {
        action: "up_to_date",
        enabled: Some(false),
        bundled_version: Some("0.50.0 (1)".into()),
        installed_version: Some("0.50.0 (1)".into()),
        system_bundles: Vec::new(),
    };
    let timeout = std::time::Duration::ZERO;
    let state = super::InputSourceStartupState::default();
    state.finish(Some(up_to_date()));

    let missing =
        super::input_source_status_now(&state, timeout, || Some(false), Vec::new, || Some(false))
            .unwrap();
    assert_eq!(missing.action, "login_required");
    // 查明的结果只查一次，设置页反复询问时不再拉起输入法。
    let again = super::input_source_status_now(
        &state,
        timeout,
        || Some(false),
        Vec::new,
        || panic!("the registry is looked up once per start-time result"),
    )
    .unwrap();
    assert_eq!(again.action, "login_required");

    // 在注册表里：照常是 up_to_date，设置页给出添加步骤。
    let state = super::InputSourceStartupState::default();
    state.finish(Some(up_to_date()));
    let listed =
        super::input_source_status_now(&state, timeout, || Some(false), Vec::new, || Some(true))
            .unwrap();
    assert_eq!(listed.action, "up_to_date");

    // 无从判断时不改结论，下次询问再查。
    let state = super::InputSourceStartupState::default();
    state.finish(Some(up_to_date()));
    let unknown =
        super::input_source_status_now(&state, timeout, || Some(false), Vec::new, || None).unwrap();
    assert_eq!(unknown.action, "up_to_date");
    let retried =
        super::input_source_status_now(&state, timeout, || Some(false), Vec::new, || Some(false))
            .unwrap();
    assert_eq!(retried.action, "login_required");

    // 已经在输入法列表里，或列表读不到时不查注册表。
    for enabled in [Some(true), None] {
        let status = super::input_source_status_now(
            &state,
            timeout,
            move || enabled,
            Vec::new,
            || panic!("the registry is only looked up while the input method is not in the list"),
        )
        .unwrap();
        assert_eq!(status.action, "up_to_date");
    }

    // 启动结果换了（安装窗口重新跑了一次）就重新查。
    state.finish(Some(up_to_date()));
    let fresh =
        super::input_source_status_now(&state, timeout, || Some(false), Vec::new, || Some(true))
            .unwrap();
    assert_eq!(fresh.action, "up_to_date");
}

/// 这次启动刚装好或刚更新时不查注册表：登记后有一段分钟级的空窗，查不到不说明什么。
#[cfg(target_os = "macos")]
#[test]
fn a_fresh_install_or_update_is_not_looked_up_in_the_registry() {
    for action in ["installed", "updated"] {
        let state = super::InputSourceStartupState::default();
        state.finish(Some(super::InputSourceStartupStatus {
            action,
            enabled: Some(false),
            bundled_version: Some("0.50.0 (1)".into()),
            installed_version: Some("0.50.0 (1)".into()),
            system_bundles: Vec::new(),
        }));
        let status = super::input_source_status_now(
            &state,
            std::time::Duration::ZERO,
            || Some(false),
            Vec::new,
            || panic!("a bundle this launch replaced is not looked up in the registry"),
        )
        .unwrap();
        assert_eq!(status.action, action);
    }
}

/// 设置页挂载时和窗口获得焦点时几乎同时问状态：后到的请求等前一个查完，用它存下的结论，不再拉起一次输入法。
#[cfg(target_os = "macos")]
#[test]
fn concurrent_status_requests_look_up_the_registry_once() {
    let state = std::sync::Arc::new(super::InputSourceStartupState::default());
    state.finish(Some(super::InputSourceStartupStatus {
        action: "up_to_date",
        enabled: Some(false),
        bundled_version: Some("0.50.0 (1)".into()),
        installed_version: Some("0.50.0 (1)".into()),
        system_bundles: Vec::new(),
    }));
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    let first = {
        let state = std::sync::Arc::clone(&state);
        std::thread::spawn(move || {
            super::input_source_status_now(
                &state,
                std::time::Duration::ZERO,
                || Some(false),
                Vec::new,
                move || {
                    started_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    Some(false)
                },
            )
            .unwrap()
        })
    };
    started_rx.recv().unwrap();
    let second = {
        let state = std::sync::Arc::clone(&state);
        std::thread::spawn(move || {
            super::input_source_status_now(
                &state,
                std::time::Duration::ZERO,
                || Some(false),
                Vec::new,
                || panic!("a request arriving during a lookup waits for its answer"),
            )
            .unwrap()
        })
    };
    release_tx.send(()).unwrap();
    assert_eq!(first.join().unwrap().action, "login_required");
    assert_eq!(second.join().unwrap().action, "login_required");
}

/// 查询期间安装窗口换了启动结果：旧查询的结论不写进新结果的缓存，下次询问重新查。
#[cfg(target_os = "macos")]
#[test]
fn a_lookup_overtaken_by_a_new_start_time_result_is_not_kept() {
    let up_to_date = || super::InputSourceStartupStatus {
        action: "up_to_date",
        enabled: Some(false),
        bundled_version: Some("0.50.0 (1)".into()),
        installed_version: Some("0.50.0 (1)".into()),
        system_bundles: Vec::new(),
    };
    let timeout = std::time::Duration::ZERO;
    let state = super::InputSourceStartupState::default();
    state.finish(Some(up_to_date()));
    let stale = super::input_source_status_now(
        &state,
        timeout,
        || Some(false),
        Vec::new,
        || {
            state.finish(Some(up_to_date()));
            Some(false)
        },
    )
    .unwrap();
    assert_eq!(stale.action, "login_required");
    let fresh =
        super::input_source_status_now(&state, timeout, || Some(false), Vec::new, || Some(true))
            .unwrap();
    assert_eq!(fresh.action, "up_to_date");
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[test]
fn cloud_clipboard_route_keeps_providers_ahead_of_the_account() {
    use super::{cloud_clipboard_route, CloudClipboardRoute};
    use std::path::PathBuf;

    let never_native = || -> Option<&'static str> { panic!("native session consulted") };
    let never_discover = || -> Option<PathBuf> { panic!("discovery consulted") };

    // A configured socket wins over the native session and discovery.
    assert_eq!(
        cloud_clipboard_route(
            Some("/run/synthetic/cloud-clipboard.sock".into()),
            never_native,
            never_discover,
        )
        .unwrap(),
        CloudClipboardRoute::Provider(PathBuf::from("/run/synthetic/cloud-clipboard.sock"))
    );
    // A configured socket that is not absolute is refused rather than quietly answered by the account.
    assert_eq!(
        cloud_clipboard_route(Some("relative.sock".into()), never_native, never_discover)
            .unwrap_err()
            .code,
        "unavailable"
    );
    // The input method's own session comes before a discovered socket.
    assert_eq!(
        cloud_clipboard_route(None, || Some("native"), never_discover).unwrap(),
        CloudClipboardRoute::Native("native")
    );
    assert_eq!(
        cloud_clipboard_route(
            None,
            || None::<&str>,
            || Some(PathBuf::from("/run/synthetic/discovered.sock")),
        )
        .unwrap(),
        CloudClipboardRoute::Provider(PathBuf::from("/run/synthetic/discovered.sock"))
    );
    // With no provider at all, and for a discovered path that is not absolute, the signed-in account serves the request.
    assert_eq!(
        cloud_clipboard_route(None, || None::<&str>, || None).unwrap(),
        CloudClipboardRoute::Account
    );
    assert_eq!(
        cloud_clipboard_route(
            None,
            || None::<&str>,
            || Some(PathBuf::from("relative.sock")),
        )
        .unwrap(),
        CloudClipboardRoute::Account
    );
}

#[test]
fn cloud_clipboard_target_is_only_valid_for_the_open_it_was_captured_for() {
    use super::FreshInputTarget;

    let mut slot = FreshInputTarget::<&str>::default();
    assert_eq!(slot.target(), None);

    // A target captured while the panel opens is the one it may type into.
    let first = slot.begin_open();
    slot.record(first, Some("synthetic-editor"));
    assert_eq!(slot.target(), Some("synthetic-editor"));

    // Opening again forgets it before anything is captured, and a failed capture leaves the panel copy-only rather than falling back to the earlier editor.
    let second = slot.begin_open();
    assert_eq!(slot.target(), None);
    slot.record(second, None);
    assert_eq!(slot.target(), None);

    // A capture that finishes after a newer open began belongs to the old open and is discarded.
    let third = slot.begin_open();
    slot.record(second, Some("synthetic-stale"));
    assert_eq!(slot.target(), None);
    slot.record(third, Some("synthetic-current"));
    assert_eq!(slot.target(), Some("synthetic-current"));

    // Closing the panel drops the target, and a capture for the closed open cannot bring it back.
    slot.close();
    assert_eq!(slot.target(), None);
    slot.record(third, Some("synthetic-current"));
    assert_eq!(slot.target(), None);
}

#[cfg(target_os = "linux")]
#[test]
fn sway_container_owner_is_read_from_the_matching_view() {
    let tree = serde_json::json!({
        "id": 1,
        "nodes": [{
            "id": 2,
            "nodes": [{ "id": 3, "pid": 4242, "focused": true }],
            "floating_nodes": [{ "id": 4, "pid": 4343 }],
        }],
    });
    assert_eq!(
        crate::panel_input::sway_pid_for_container(&tree, 3),
        Some(4242)
    );
    assert_eq!(
        crate::panel_input::sway_pid_for_container(&tree, 4),
        Some(4343)
    );
    // A container without a pid, or one that is not in the tree, has no owner to compare against.
    assert_eq!(crate::panel_input::sway_pid_for_container(&tree, 2), None);
    assert_eq!(crate::panel_input::sway_pid_for_container(&tree, 9), None);
}

/// 不提供手写的版本（日文、越南文和藏文版）不打开手写面板和手写设置页，别的界面照常；提供中文方案的版本什么都不少。
#[test]
fn editions_without_handwriting_open_no_handwriting_surface() {
    use msime_client_core::edition::Edition;
    use msime_client_core::host_surface::{SettingsCategory, SurfaceRoute};

    let handwriting = [
        SurfaceRoute::Handwriting,
        SurfaceRoute::Settings(Some(SettingsCategory::Handwriting)),
    ];
    let others = [
        SurfaceRoute::Keyboard,
        SurfaceRoute::Emoji,
        SurfaceRoute::Voice,
        SurfaceRoute::Settings(None),
        SurfaceRoute::Settings(Some(SettingsCategory::Input)),
    ];
    for edition in Edition::all() {
        for route in handwriting {
            assert_eq!(
                super::edition_offers_route(edition, route),
                edition.features.handwriting,
                "{} {route:?}",
                edition.id
            );
        }
        for route in others {
            assert!(
                super::edition_offers_route(edition, route),
                "{} {route:?}",
                edition.id
            );
        }
    }
    for id in ["japanese", "vietnamese", "tibetan"] {
        let edition = Edition::by_id(id).unwrap();
        assert!(
            !super::edition_offers_route(edition, SurfaceRoute::Handwriting),
            "{id}"
        );
    }
    for id in ["full", "pinyin", "wubi"] {
        let edition = Edition::by_id(id).unwrap();
        assert!(
            super::edition_offers_route(edition, SurfaceRoute::Handwriting),
            "{id}"
        );
    }
}

#[cfg(unix)]
#[test]
fn candidate_panel_status_reader_rejects_a_symlinked_file() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let outside = root.path().join("outside-status.json");
    let linked = root.path().join("candidate-panel.json");
    std::fs::write(&outside, b"synthetic-status").unwrap();
    symlink(&outside, &linked).unwrap();

    assert!(super::read_candidate_panel_status(&linked).is_none());
}
