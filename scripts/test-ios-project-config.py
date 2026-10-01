import json
import plistlib
import re
import unittest
from pathlib import Path


TAURI_ROOT = Path(__file__).resolve().parents[1] / "apps/desktop/src-tauri"
APPLE_ROOT = TAURI_ROOT / "gen/apple"


class IOSProjectConfigTests(unittest.TestCase):
    def test_tauri_app_declares_export_compliance_without_non_exempt_encryption(self):
        project = (APPLE_ROOT / "project.yml").read_text()
        self.assertIn("ITSAppUsesNonExemptEncryption: false", project)
        with (APPLE_ROOT / "msime-desktop_iOS/Info.plist").open("rb") as file:
            info = plistlib.load(file)
        self.assertIs(info["ITSAppUsesNonExemptEncryption"], False)

    def test_tauri_app_explains_microphone_access_for_voice_input(self):
        explanation = "仅在你开始语音输入时录音，并发送到你配置的语音识别服务。"
        project = (APPLE_ROOT / "project.yml").read_text()
        self.assertIn(f'NSMicrophoneUsageDescription: "{explanation}"', project)
        with (APPLE_ROOT / "msime-desktop_iOS/Info.plist").open("rb") as file:
            info = plistlib.load(file)
        self.assertEqual(info["NSMicrophoneUsageDescription"], explanation)

    def test_tauri_voice_panel_uses_bounded_native_recording_and_all_asr_transports(self):
        project = (APPLE_ROOT / "project.yml").read_text()
        generated = (APPLE_ROOT / "msime-desktop.xcodeproj/project.pbxproj").read_text()
        plugin = TAURI_ROOT / "../../../crates/tauri-mobile-platform"
        plugin_rust = (plugin / "src/lib.rs").read_text()
        swift = (plugin / "ios/Sources/MobilePlatformPlugin.swift").read_text()
        doubao = (plugin / "ios/Sources/IOSVoiceDoubaoTransport.swift").read_text()
        rust_entry = (TAURI_ROOT / "src/lib.rs").read_text()
        rust_voice = (TAURI_ROOT / "src/voice.rs").read_text()

        self.assertIn("sdk: AVFoundation.framework", project)
        self.assertIn("AVFoundation.framework in Frameworks", generated)
        self.assertIn('run_mobile_plugin_async::<MobileVoiceTranscriptionResponse>("recognizeVoice", request)', plugin_rust)
        self.assertIn('"stopVoice"', plugin_rust)
        self.assertIn('"cancelVoice"', plugin_rust)
        self.assertIn("import AVFoundation", swift)
        self.assertIn("AVAudioRecorder(url: file", swift)
        self.assertIn("AVSampleRateKey: 16_000", swift)
        self.assertIn("AVNumberOfChannelsKey: 1", swift)
        self.assertIn("AVLinearPCMBitDepthKey: 16", swift)
        self.assertIn("DispatchQueue.main.asyncAfter(deadline: .now() + 60", swift)
        self.assertIn("UIApplication.didEnterBackgroundNotification", swift)
        self.assertIn("private func cancelForBackground()", swift)
        self.assertIn("NotificationCenter.default.removeObserver", swift)
        self.assertIn('request.setValue("multipart/form-data; boundary=', swift)
        self.assertIn("willPerformHTTPRedirection", swift)
        self.assertIn("private static let maximumResponseBytes = 1024 * 1024", swift)
        self.assertIn(
            '["openai", "siliconflow", "groq", "everyapi", "mistral"].contains(args.provider)', swift
        )
        self.assertIn('args.provider == "doubao"', swift)
        self.assertIn('components.scheme?.lowercased() == "wss"', swift)
        self.assertIn('@_silgen_name("msime_client_doubao_start_frame")', doubao)
        self.assertIn('@_silgen_name("msime_client_doubao_audio_frame")', doubao)
        self.assertIn('@_silgen_name("msime_client_doubao_decode_frame")', doubao)
        self.assertIn("URLSessionWebSocketTask", doubao)
        self.assertIn("task.maximumMessageSize = Self.maximumFrameBytes", doubao)
        self.assertIn("private static let pcmChunkBytes = 6_400", doubao)
        self.assertIn("willPerformHTTPRedirection", doubao)
        self.assertIn("MobileVoiceRequestHeader", rust_voice)
        # The resolution moved into the shared crate so the Android keyboard, which never goes
        # through this shell, reads the same answer. What this pins is unchanged: the Doubao
        # authentication headers come from the shared policy rather than a copy in a host.
        self.assertIn(
            "use msime_client_core::voice::provider::{", rust_voice
        )
        shared_voice = (
            TAURI_ROOT.parents[2] / "crates/client-core/src/voice/provider.rs"
        ).read_text()
        self.assertIn("crate::credential::doubao_auth::headers(", shared_voice)
        self.assertIn('#[cfg(not(target_os = "ios"))]', rust_entry)
        self.assertIn("mobile_voice_provider_configuration(&snapshot.preferences)", rust_voice)
        self.assertIn('phase: Some("recording".into())', rust_voice)
        self.assertIn('phase: Some("recognizing".into())', rust_voice)

    def test_privacy_manifest_is_shared_by_tauri_app_and_keyboard(self):
        privacy_path = TAURI_ROOT / "../../../platforms/ios/SharedResources/PrivacyInfo.xcprivacy"
        with privacy_path.resolve().open("rb") as file:
            privacy = plistlib.load(file)
        self.assertFalse(privacy["NSPrivacyTracking"])
        # Anonymous usage reporting: crash reports, keyboard sessions and daily activity, and the random install id, none linked to the user or used for tracking.
        self.assertEqual(privacy["NSPrivacyCollectedDataTypes"], [
            {"NSPrivacyCollectedDataType": "NSPrivacyCollectedDataTypeCrashData", "NSPrivacyCollectedDataTypeLinked": False, "NSPrivacyCollectedDataTypeTracking": False, "NSPrivacyCollectedDataTypePurposes": ["NSPrivacyCollectedDataTypePurposeAnalytics", "NSPrivacyCollectedDataTypePurposeAppFunctionality"]},
            {"NSPrivacyCollectedDataType": "NSPrivacyCollectedDataTypeProductInteraction", "NSPrivacyCollectedDataTypeLinked": False, "NSPrivacyCollectedDataTypeTracking": False, "NSPrivacyCollectedDataTypePurposes": ["NSPrivacyCollectedDataTypePurposeAnalytics"]},
            {"NSPrivacyCollectedDataType": "NSPrivacyCollectedDataTypeDeviceID", "NSPrivacyCollectedDataTypeLinked": False, "NSPrivacyCollectedDataTypeTracking": False, "NSPrivacyCollectedDataTypePurposes": ["NSPrivacyCollectedDataTypePurposeAnalytics"]},
        ])
        project = (APPLE_ROOT / "project.yml").read_text()
        reference = "path: ../../../../../platforms/ios/SharedResources/PrivacyInfo.xcprivacy"
        self.assertEqual(project.count(reference), 2)
        generated = (APPLE_ROOT / "msime-desktop.xcodeproj/project.pbxproj").read_text()
        self.assertIn("path = PrivacyInfo.xcprivacy", generated)
        self.assertEqual(generated.count("PrivacyInfo.xcprivacy in Resources"), 4)

    def test_platform_config_uses_the_shipping_identity_and_supported_version(self):
        config = json.loads((TAURI_ROOT / "tauri.ios.conf.json").read_text())
        self.assertEqual(config["identifier"], "com.metasequoiaime.client")
        self.assertEqual(config["productName"], "水杉输入法")
        self.assertEqual(config["bundle"]["iOS"]["minimumSystemVersion"], "17.0")

    def test_native_entry_and_entitlement_share_keyboard_state_without_private_data(self):
        entry = (APPLE_ROOT / "Sources/msime-desktop/main.mm").read_text()
        self.assertIn("group.app.msime.ios", entry)
        self.assertIn('setenv("MSIME_CLIENT_STATE_DIR"', entry)
        with (APPLE_ROOT / "msime-desktop_iOS/msime-desktop_iOS.entitlements").open("rb") as file:
            entitlements = plistlib.load(file)
        self.assertEqual(
            entitlements["com.apple.security.application-groups"],
            ["group.app.msime.ios"],
        )

    def test_tauri_ios_onboarding_reuses_the_legacy_app_marker(self):
        plugin = TAURI_ROOT / "../../../crates/tauri-mobile-platform"
        rust = (plugin / "src/lib.rs").read_text()
        swift = (plugin / "ios/Sources/MobilePlatformPlugin.swift").read_text()
        entry = (TAURI_ROOT / "src/lib.rs").read_text()
        desktop = (TAURI_ROOT.parent / "src/main.tsx").read_text()
        onboarding = (TAURI_ROOT.parent / "../../packages/ui/src/account/onboarding-page.tsx").read_text()

        self.assertIn('"onboardingStatus"', rust)
        self.assertIn('"completeOnboarding"', rust)
        self.assertIn('"hasCompletedOnboarding"', swift)
        self.assertIn("ios_onboarding_status", entry)
        self.assertIn("ios_onboarding_complete", entry)
        self.assertIn('invoke<boolean>("ios_onboarding_status")', desktop)
        self.assertIn('invoke("ios_onboarding_complete")', desktop)
        self.assertIn("className={onboarding.skip}", onboarding)

    def test_generated_project_builds_the_shared_rust_mobile_entry(self):
        project = (APPLE_ROOT / "project.yml").read_text()
        self.assertIn("PRODUCT_BUNDLE_IDENTIFIER: com.metasequoiaime.client", project)
        self.assertIn("iOS: 17.0", project)
        self.assertIn("pnpm tauri ios xcode-script", project)
        self.assertIn("framework: libapp.a", project)
        self.assertIn("../../../../../target/ios/EngineResources", project)
        self.assertIn('          - "-lsqlite3"', project)

    def test_keyboard_brand_asset_is_packaged_for_app_and_extension(self):
        project = (APPLE_ROOT / "project.yml").read_text()
        generated = (APPLE_ROOT / "msime-desktop.xcodeproj/project.pbxproj").read_text()
        asset = TAURI_ROOT / "../../../platforms/ios/SharedResources/KeyboardBrand.png"

        self.assertTrue(asset.resolve().is_file())
        self.assertEqual(
            project.count("../../../../../platforms/ios/SharedResources/KeyboardBrand.png"), 2
        )
        self.assertIn("KeyboardBrand.png in Resources", generated)
        self.assertGreaterEqual(generated.count("KeyboardBrand.png in Resources"), 2)

    def test_language_dictionaries_are_bundled_beside_engine_resources_for_app_and_extension(self):
        project = (APPLE_ROOT / "project.yml").read_text()
        generated = (APPLE_ROOT / "msime-desktop.xcodeproj/project.pbxproj").read_text()
        rust_entry = (TAURI_ROOT / "src/lib.rs").read_text()

        entry = (
            "      - path: ../../../../../target/ios/language-dictionaries\n"
            "        buildPhase: resources\n"
            "        type: folder\n"
            "        optional: true\n"
        )
        self.assertEqual(project.count(entry), 2)
        self.assertEqual(generated.count("language-dictionaries in Resources */,"), 2)
        self.assertIn("msime_host_api::installed_language_dictionaries(resources)", rust_entry)

    def test_tauri_app_embeds_the_native_keyboard_extension(self):
        project = (APPLE_ROOT / "project.yml").read_text()
        self.assertIn("  MSIMEKeyboardExtension:\n    type: app-extension", project)
        self.assertIn("PRODUCT_BUNDLE_IDENTIFIER: com.metasequoiaime.client.keyboard", project)
        self.assertIn("CODE_SIGN_ENTITLEMENTS: ../../../../../platforms/ios/KeyboardExtension/Resources/MSIMEKeyboardExtension.entitlements", project)
        self.assertIn("SWIFT_OBJC_BRIDGING_HEADER: $(SRCROOT)/../../../../../platforms/ios/KeyboardExtension/Sources/core/MetasequoiaKeyboard-Bridging-Header.h", project)
        self.assertIn("SWIFT_VERSION: 5.0", project)
        self.assertIn("path: MSIMEKeyboardExtension/Info.plist", project)
        self.assertIn("      - target: MSIMEKeyboardExtension", project)

        info_path = APPLE_ROOT / "MSIMEKeyboardExtension/Info.plist"
        with info_path.resolve().open("rb") as file:
            info = plistlib.load(file)
        extension = info["NSExtension"]
        self.assertEqual(extension["NSExtensionPointIdentifier"], "com.apple.keyboard-service")
        self.assertTrue(extension["NSExtensionAttributes"]["RequestsOpenAccess"])

        entitlement_path = APPLE_ROOT / "../../../../../platforms/ios/KeyboardExtension/Resources/MSIMEKeyboardExtension.entitlements"
        with entitlement_path.resolve().open("rb") as file:
            entitlements = plistlib.load(file)
        self.assertEqual(
            entitlements["com.apple.security.application-groups"],
            ["group.app.msime.ios"],
        )

    def test_tauri_keyboard_extension_registers_all_shipping_swift_dependencies(self):
        project = (APPLE_ROOT / "project.yml").read_text()
        generated = (APPLE_ROOT / "msime-desktop.xcodeproj/project.pbxproj").read_text()

        # The checked-in XcodeGen output is the shipping project used by Tauri. Keep the
        # generated target in lockstep with project.yml so a newly added keyboard dependency
        # cannot silently compile only in the legacy native project.
        self.assertIn("../../../../../platforms/ios/KeyboardExtension/Sources", project)
        self.assertIn("../../../../../platforms/ios/SharedUI", project)
        sources = [
            "KeyboardSymbolPanelView.swift",
            "CandidateTranslationStore.swift",
            "CandidateTranslationPreference.swift",
            "BackendChatClient.swift",
            "BackendAccountSession.swift",
            "BackendAnonymousAccount.swift",
            "BackendLocalStore.swift",
        ]
        for source in sources:
            self.assertIn(f"{source} in Sources", generated)

    def test_tauri_app_packages_and_registers_ios_alternate_icons(self):
        project = (APPLE_ROOT / "project.yml").read_text()
        generated_project = (APPLE_ROOT / "msime-desktop.xcodeproj/project.pbxproj").read_text()
        alternate_names = [
            "AppIconForest",
            "AppIconSky",
            "AppIconDusk",
            "AppIconVermilion",
        ]
        self.assertIn("ASSETCATALOG_COMPILER_APPICON_NAME: AppIcon", project)
        self.assertIn(
            "ASSETCATALOG_COMPILER_ALTERNATE_APPICON_NAMES: " + " ".join(alternate_names),
            project,
        )
        self.assertIn("ASSETCATALOG_COMPILER_INCLUDE_ALL_APPICON_ASSETS: true", project)
        self.assertIn('- "**/libapp.a"', project)
        self.assertIn("ASSETCATALOG_COMPILER_ALTERNATE_APPICON_NAMES", generated_project)
        self.assertNotIn("libapp.a in Resources", generated_project)
        for name in alternate_names:
            icon_set = APPLE_ROOT / f"Assets.xcassets/{name}.appiconset"
            self.assertTrue((icon_set / "Contents.json").is_file())
            self.assertTrue((icon_set / "Icon.png").is_file())

        manifest = (TAURI_ROOT / "Cargo.toml").read_text()
        rust_entry = (TAURI_ROOT / "src/lib.rs").read_text()
        desktop_entry = (TAURI_ROOT.parent / "src/main.tsx").read_text()
        plugin = TAURI_ROOT / "../../../crates/tauri-mobile-platform"
        swift = (plugin / "ios/Sources/MobilePlatformPlugin.swift").read_text()
        self.assertIn("msime-tauri-mobile-platform", manifest)
        self.assertIn("builder.plugin(msime_tauri_mobile_platform::init())", rust_entry)
        self.assertIn("open_system_keyboard_settings", rust_entry)
        self.assertIn("app_icon_info", rust_entry)
        self.assertIn("app_icon_set", rust_entry)
        self.assertIn('invoke("open_system_keyboard_settings")', desktop_entry)
        self.assertIn("UIApplication.openSettingsURLString", swift)
        self.assertIn("openSystemKeyboardSettings", swift)
        self.assertIn("application.supportsAlternateIcons", swift)
        self.assertIn("application.setAlternateIconName(requestedName)", swift)
        self.assertIn("application.alternateIconName != requestedName", swift)
        self.assertIn('invoke.reject("app_icon", code: "app_icon")', swift)

    def test_keyboard_keeps_device_mlkit_and_simulator_fallback_boundaries(self):
        project = (APPLE_ROOT / "project.yml").read_text()
        self.assertIn("EXCLUDED_SOURCE_FILE_NAMES[sdk=iphoneos*]: HandwritingInputViewFallback.swift", project)
        self.assertIn("EXCLUDED_SOURCE_FILE_NAMES[sdk=iphonesimulator*]: HandwritingInputView.swift HandwritingDownloadSession.m", project)
        self.assertIn("../../../../../target/ios/EngineResources", project)
        self.assertIn('          - "-lmsime_host_api"', project)

        podfile = (APPLE_ROOT / "Podfile").read_text()
        self.assertIn("target 'MSIMEKeyboardExtension'", podfile)
        self.assertIn("pod 'MLKitDigitalInkRecognition', '8.0.0'", podfile)

    def test_mobile_platform_keeps_account_sessions_in_the_ios_keychain(self):
        plugin = TAURI_ROOT / "../../../crates/tauri-mobile-platform"
        rust = (plugin / "src/lib.rs").read_text()
        swift = (plugin / "ios/Sources/MobilePlatformPlugin.swift").read_text()

        self.assertIn('run_mobile_plugin::<AccountSessionResponse>("loadSession", ())', rust)
        self.assertIn('run_mobile_plugin("saveSession", AccountSessionRequest { value })', rust)
        self.assertIn('run_mobile_plugin("clearSession", ())', rust)
        self.assertIn('kSecAttrService as String: "app.msime.backend.account"', swift)
        self.assertIn('kSecAttrAccount as String: "https://api.msime.app"', swift)
        self.assertIn("kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly", swift)
        self.assertIn('kSecAttrService as String: "app.msime.ios.community"', swift)
        self.assertIn("static let maximumPayloadBytes = 16 * 1024", swift)
        self.assertIn("@objc public func loadSession", swift)
        self.assertIn("@objc public func saveSession", swift)
        self.assertIn("@objc public func clearSession", swift)

    def test_ios_registers_shared_account_commands_and_ui(self):
        rust_entry = (TAURI_ROOT / "src/lib.rs").read_text()
        account = (TAURI_ROOT / "src/platform/ios/ios_account.rs").read_text()
        shared_account = (TAURI_ROOT / "src/platform/mobile/mobile_account_helpers.rs").read_text()
        desktop_entry = (TAURI_ROOT.parent / "src/main.tsx").read_text()
        mobile_services = (TAURI_ROOT.parent / "src/core/mobile-host-services.ts").read_text()

        self.assertIn('use platform::ios::ios_account;', rust_entry)
        self.assertIn('ios_account::setup(app.handle())?', rust_entry)
        for command in [
            "account_status",
            "account_providers",
            "account_request_code",
            "account_login",
            "account_profile",
            "account_chat_models",
            "account_chat",
            "account_rename",
            "account_logout",
            "account_delete",
            "account_forget",
            "account_preferences_schema",
            "account_preferences_load",
            "account_preferences_upload",
            "account_preferences_apply",
        ]:
            self.assertIn(f"ios_account::{command}", rust_entry)
            self.assertIn(f"pub async fn {command}", account)
        self.assertIn("BackendAccountSession::new", account)
        self.assertIn("IosAccountStorage(platform.clone())", account)
        self.assertIn("createMobileHostServices", desktop_entry)
        self.assertIn("accountSettingsSync", mobile_services)
        self.assertIn("...baseAccount", mobile_services)
        self.assertIn('invoke("account_chat_models")', mobile_services)
        self.assertIn('invoke<{ content: string }>("account_chat", { messages, model })', mobile_services)

    def test_ios_account_settings_sync_bridges_app_group_keyboard_preferences(self):
        plugin = TAURI_ROOT / "../../../crates/tauri-mobile-platform"
        rust = (plugin / "src/lib.rs").read_text()
        swift = (plugin / "ios/Sources/MobilePlatformPlugin.swift").read_text()
        account = (TAURI_ROOT / "src/platform/ios/ios_account.rs").read_text()
        shared_account = (TAURI_ROOT / "src/platform/mobile/mobile_account_helpers.rs").read_text()
        mapping = (TAURI_ROOT / "src/platform/ios/ios_account/account_preferences.rs").read_text()
        desktop_entry = (TAURI_ROOT.parent / "src/main.tsx").read_text()
        mobile_services = (TAURI_ROOT.parent / "src/core/mobile-host-services.ts").read_text()

        self.assertIn('run_mobile_plugin::<IosKeyboardPreferences>("loadKeyboardPreferences", ())', rust)
        self.assertIn('run_mobile_plugin::<IosKeyboardPreferences>("saveKeyboardPreferences", preferences)', rust)
        self.assertIn('UserDefaults(suiteName: "group.app.msime.ios")', swift)
        for key in [
            "chineseInputScheme",
            "chineseOutputUsesTraditional",
            "keyboardSoundEnabled",
            "keyboardHapticsEnabled",
            "keyboardHapticStrength",
            "dictionaryLearningEnabled",
            "globalTheme",
            "customKeyboardSkin.v1",
        ]:
            self.assertIn(key, swift)
        # IosKeyboardPreferences is camelCase serde, so the plugin must read and write the same field names; a leftover retired key would fail every load and save.
        self.assertIn("pub global_theme: String", rust)
        self.assertIn("let globalTheme: String", swift)
        self.assertNotIn('"keyboardSkin"', swift)
        for theme in ["system", "shuishan", "light", "paper", "night", "ink", "custom"]:
            self.assertIn(f'"{theme}"', swift)
        self.assertIn("merge_account_preferences", account)
        self.assertIn("platform.save_keyboard_preferences(&previous_native)", account)
        self.assertIn('"platform.ios.nine_key"', mapping)
        self.assertIn('"platform.ios.custom_keyboard_skin"', mapping)
        self.assertIn('"platform.ios.custom_theme_base"', mapping)
        self.assertIn("createMobileHostServices", desktop_entry)
        self.assertIn("settingsSync: accountSettingsSync", mobile_services)

    def test_ios_cloud_clipboard_uses_the_shared_account_and_native_copy_boundaries(self):
        plugin = TAURI_ROOT / "../../../crates/tauri-mobile-platform"
        plugin_rust = (plugin / "src/lib.rs").read_text()
        swift = (plugin / "ios/Sources/MobilePlatformPlugin.swift").read_text()
        rust_entry = (TAURI_ROOT / "src/lib.rs").read_text()
        account = (TAURI_ROOT / "src/platform/ios/ios_account.rs").read_text()
        clipboard = (TAURI_ROOT / "src/platform/cloud_clipboard.rs").read_text()
        desktop_entry = (TAURI_ROOT.parent / "src/main.tsx").read_text()

        self.assertIn(
            "platform::cloud_clipboard::cloud_clipboard_request(state.session(), action).await",
            rust_entry,
        )
        self.assertIn("pub(crate) fn session(&self) -> &Arc<Session>", account)
        self.assertIn("pub(crate) async fn cloud_clipboard_request<", clipboard)
        self.assertIn("session.clipboard(&search)", clipboard)
        self.assertIn(".set_clipboard_enabled(enabled)", clipboard)
        self.assertIn("session.add_clipboard(&text)", clipboard)
        self.assertIn(".delete_clipboard(Some(&id))", clipboard)
        self.assertIn('run_mobile_plugin("copyText", CopyTextRequest { text })', plugin_rust)
        self.assertIn("@objc public func copyText", swift)
        self.assertIn("UIPasteboard.general.string = args.text", swift)
        self.assertEqual(
            desktop_entry.count(
                'openCloudClipboard: async () => navigateMobilePanel("cloud-clipboard")'
            ),
            2,
        )

    def test_ios_cloud_dictionary_uses_shared_account_and_snapshot_queue(self):
        project = (APPLE_ROOT / "project.yml").read_text()
        generated = (APPLE_ROOT / "msime-desktop.xcodeproj/project.pbxproj").read_text()
        rust_entry = (TAURI_ROOT / "src/lib.rs").read_text()
        account = (TAURI_ROOT / "src/platform/ios/ios_account.rs").read_text()
        shared_account = (TAURI_ROOT / "src/platform/mobile/mobile_account_helpers.rs").read_text()
        desktop_entry = (TAURI_ROOT.parent / "src/main.tsx").read_text()
        bridge = (TAURI_ROOT / "../../../platforms/ios/App/Sources/dictionary/TauriDictionarySnapshotBridge.swift").read_text()

        self.assertIn("ios_account::cloud_dictionary_request(state, request).await", rust_entry)
        self.assertIn("pub async fn cloud_dictionary_request", account)
        for method in [
            "dictionary_catalog(kind, &code, offset, &scheme, &profile)",
            ".add_dictionary(",
            ".update_dictionary(",
            ".personal_candidates(",
            ".rank_candidate(",
            ".remove_candidate(",
            ".import_dictionary(",
            ".export_dictionary(",
        ]:
            self.assertIn(method, shared_account)
        self.assertIn(
            'openCloudDictionary: async () => navigateMobilePanel("cloud-dictionary")',
            desktop_entry,
        )
        for operation in [
            "dictionary_snapshot_preview(state).await",
            "dictionary_snapshot_export(state).await",
            "dictionary_snapshot_restore_preview(state, text).await",
            "dictionary_snapshot_restore(state, text, expected_sha256, revision).await",
            "dictionary_snapshot_enqueue(state, token).await",
            "dictionary_snapshot_status(state).await",
            "dictionary_snapshot_cancel(state).await",
        ]:
            self.assertIn(operation, account)
        for path in [
            "../../../../../platforms/ios/SharedUI/dictionary/DictionarySnapshotQueue.swift",
            "../../../../../shared/backend/clients/BackendSnapshotClient.swift",
            "../../../../../platforms/ios/App/Sources/dictionary/TauriDictionarySnapshotBridge.swift",
        ]:
            self.assertIn(path, project)
        for source in [
            "BackendSnapshotClient.swift in Sources",
            "DictionarySnapshotQueue.swift in Sources",
            "TauriDictionarySnapshotBridge.swift in Sources",
        ]:
            self.assertIn(source, generated)
        self.assertIn("DictionarySnapshotQueue()", bridge)
        self.assertIn("BackendPreparedSnapshot(copying: url)", bridge)
        self.assertIn('@_cdecl("msime_ios_dictionary_snapshot_request")', bridge)
        capabilities = (TAURI_ROOT.parent / "src/input/mobile-host-capabilities.ts").read_text()
        self.assertIn("snapshot: isMobileHost(platform) || platform === \"macos\"", capabilities)
        self.assertIn("snapshotNative: platform === \"macos\"", capabilities)

    def test_ios_community_services_use_shared_backend_and_tauri_ui(self):
        rust_entry = (TAURI_ROOT / "src/lib.rs").read_text()
        account = (TAURI_ROOT / "src/platform/ios/ios_account.rs").read_text()
        community = (TAURI_ROOT / "src/platform/mobile/mobile_community.rs").read_text()
        desktop_entry = (TAURI_ROOT.parent / "src/main.tsx").read_text()
        mobile_services = (TAURI_ROOT.parent / "src/core/mobile-host-services.ts").read_text()

        self.assertIn("use platform::mobile::mobile_community;", rust_entry)
        for symbol in [
            "mobile_community::community_skin_list",
            "mobile_community::community_skin_download",
            "mobile_community::ai_skin_generate",
            "mobile_community::community_resource_list",
            "mobile_community::community_resource_apply",
        ]:
            self.assertIn(symbol, rust_entry)
        self.assertIn("MobileCommunityState::new(client, &session)", account)
        self.assertIn("app.manage(community);", account)
        for symbol in [
            "BackendCommunitySkinService",
            "BackendCommunityResourceService",
            "BackendAiSkinService",
            "pub async fn community_skin_list",
            "pub async fn ai_skin_generate",
            "pub async fn community_resource_list",
            "CommunityResourceLibraryStore",
        ]:
            self.assertIn(symbol, community)
        self.assertIn("createMobileHostServices", desktop_entry)
        self.assertIn("communitySkins:", mobile_services)
        self.assertIn("communityResources:", mobile_services)
        self.assertIn("aiSkins:", mobile_services)

    def test_xcode27_runtime_exports_are_built_before_the_rust_mobile_library(self):
        project = (APPLE_ROOT / "project.yml").read_text()
        self.assertIn("revision: a83e2b2f196e3fa9605cb21c7d3b82652205c279", project)
        self.assertIn("  MSIMESwiftRsRuntimeExports:\n    type: library.static", project)
        self.assertIn("      - target: MSIMESwiftRsRuntimeExports", project)
        build_script = (TAURI_ROOT / "build.rs").read_text()
        self.assertIn("CONFIGURATION_BUILD_DIR", build_script)
        self.assertIn("cargo:rustc-link-lib=static=MSIMESwiftRsRuntimeExports", build_script)

    def test_ios_tauri_dictionary_edits_use_the_app_group_queue(self):
        project = (APPLE_ROOT / "project.yml").read_text()
        generated = (APPLE_ROOT / "msime-desktop.xcodeproj/project.pbxproj").read_text()
        rust_entry = (TAURI_ROOT / "src/lib.rs").read_text()
        store = (TAURI_ROOT / "../../../platforms/ios/SharedUI/dictionary/PersonalDictionaryStore.swift").read_text()
        bridge = (TAURI_ROOT / "../../../platforms/ios/App/Sources/dictionary/TauriPersonalDictionaryBridge.swift").read_text()

        for path in [
            "../../../../../platforms/ios/SharedUI/dictionary/PersonalDictionaryStore.swift",
            "../../../../../platforms/ios/SharedUI/dictionary/PersonalDictionaryImport.swift",
            "../../../../../platforms/ios/SharedUI/dictionary/PersonalWordBridge.swift",
            "../../../../../platforms/ios/App/Sources/dictionary/TauriPersonalDictionaryBridge.swift",
        ]:
            self.assertIn(path, project)
        target = re.search(
            r"/\* msime-desktop_iOS \*/ = \{.*?buildPhases = \((.*?)\);",
            generated,
            re.DOTALL,
        )
        self.assertIsNotNone(target)
        source_phase = re.search(r"([A-F0-9]{24}) /\* Sources \*/", target.group(1))
        self.assertIsNotNone(source_phase)
        phase = re.search(
            rf"{source_phase.group(1)} /\* Sources \*/ = \{{.*?files = \((.*?)\);",
            generated,
            re.DOTALL,
        )
        self.assertIsNotNone(phase)
        for source in [
            "PersonalDictionaryBridge.swift",
            "PersonalDictionaryImport.swift",
            "PersonalDictionaryStore.swift",
            "PersonalWordBridge.swift",
            "TauriPersonalDictionaryBridge.swift",
        ]:
            self.assertIn(f"{source} in Sources", phase.group(1))
        self.assertIn("ios_personal_dictionary_action", rust_entry)
        self.assertIn("msime_ios_native_ffi::personal_dictionary_request", rust_entry)
        self.assertIn('case "import_personal":', bridge)
        self.assertIn('case "edit":', bridge)
        self.assertIn("requestID: requestID", bridge)
        self.assertIn("PersonalDictionaryStore", bridge)
        self.assertIn("var id = UUID().uuidString", store)
        self.assertIn("decoder.dateDecodingStrategy = .custom", store)
        self.assertIn("Date(timeIntervalSinceReferenceDate: seconds)", store)
        self.assertIn("encoder.dateEncodingStrategy = .iso8601", store)

    def test_keyboard_cancels_delayed_gloss_work_when_hidden(self):
        controller = (
            TAURI_ROOT / "../../../platforms/ios/KeyboardExtension/Sources/keyboard/KeyboardViewController.swift"
        ).read_text()
        disappear_start = controller.index("  override func viewWillDisappear")
        disappear = controller[disappear_start : controller.index("\n  private func", disappear_start)]
        self.assertIn("candidateGlossTimer?.invalidate()", disappear)
        self.assertIn("candidateGlossTimer = nil", disappear)

    def test_candidate_translation_cancel_stops_inflight_tasks(self):
        store = (
            TAURI_ROOT / "../../../platforms/ios/KeyboardExtension/Sources/candidate/CandidateTranslationStore.swift"
        ).read_text()
        self.assertIn("private var tasks: [UUID: Task<Void, Never>] = [:]", store)
        self.assertIn("for task in tasks.values { task.cancel() }", store)
        self.assertIn("tasks.removeAll()", store)
        self.assertIn("tasks[id] = task", store)
        self.assertIn("tasks[id] = nil", store)


if __name__ == "__main__":
    unittest.main()
