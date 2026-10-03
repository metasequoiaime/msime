import re
import plistlib
import unittest
from pathlib import Path


IOS_ROOT = Path(__file__).resolve().parents[2]


def target_blocks(project):
    """Yield (name, body) for each entry under `targets:`, split on its indentation."""
    lines = project.splitlines(keepends=True)
    start = next(i for i, line in enumerate(lines) if line.rstrip() == "targets:")
    blocks, name, body = [], None, []
    for line in lines[start + 1:]:
        if line.strip() and not line.startswith("   ") and not line.startswith("  -"):
            if re.fullmatch(r"  [A-Za-z0-9_]+:\n", line):
                if name:
                    blocks.append((name, "".join(body)))
                name, body = line.strip().rstrip(":"), []
                continue
            if not line.startswith(" "):
                break
        body.append(line)
    if name:
        blocks.append((name, "".join(body)))
    return blocks


def source_path_blocks(body, wanted):
    """Yield the body of every `- path: <wanted>` entry in a sources list."""
    lines = body.splitlines(keepends=True)
    out = []
    for i, line in enumerate(lines):
        if line.strip() != f"- path: {wanted}":
            continue
        indent = len(line) - len(line.lstrip())
        collected = []
        for following in lines[i + 1:]:
            if not following.strip():
                collected.append(following)
                continue
            if len(following) - len(following.lstrip()) <= indent:
                break
            collected.append(following)
        out.append("".join(collected))
    return out


class ProjectConfigurationTests(unittest.TestCase):
    def test_app_explains_microphone_access_for_voice_input(self):
        explanation = "仅在你开始语音输入时录音：本地模型在本机识别，系统语音识别交由 iOS 处理，其他识别服务会把录音发送到你配置的服务。"
        project = (IOS_ROOT / "project.yml").read_text()
        generated = (IOS_ROOT / "MSIMEClient.xcodeproj/project.pbxproj").read_text()
        self.assertIn(f'INFOPLIST_KEY_NSMicrophoneUsageDescription: "{explanation}"', project)
        self.assertEqual(
            generated.count(f'INFOPLIST_KEY_NSMicrophoneUsageDescription = "{explanation}";'),
            2,
        )

    def test_privacy_manifest_is_valid_and_packaged_by_app_and_keyboard(self):
        privacy_path = IOS_ROOT / "SharedResources/PrivacyInfo.xcprivacy"
        with privacy_path.open("rb") as file:
            privacy = plistlib.load(file)
        accessed = {
            item["NSPrivacyAccessedAPIType"]: item["NSPrivacyAccessedAPITypeReasons"]
            for item in privacy["NSPrivacyAccessedAPITypes"]
        }
        self.assertEqual(
            accessed["NSPrivacyAccessedAPICategoryUserDefaults"],
            ["CA92.1", "1C8F.1"],
        )
        self.assertEqual(
            accessed["NSPrivacyAccessedAPICategorySystemBootTime"], ["35F9.1"]
        )
        self.assertFalse(privacy["NSPrivacyTracking"])
        # Anonymous usage reporting: crash reports, keyboard sessions and daily activity, and the random install id, none linked to the user or used for tracking.
        self.assertEqual(privacy["NSPrivacyCollectedDataTypes"], [
            {"NSPrivacyCollectedDataType": "NSPrivacyCollectedDataTypeCrashData", "NSPrivacyCollectedDataTypeLinked": False, "NSPrivacyCollectedDataTypeTracking": False, "NSPrivacyCollectedDataTypePurposes": ["NSPrivacyCollectedDataTypePurposeAnalytics", "NSPrivacyCollectedDataTypePurposeAppFunctionality"]},
            {"NSPrivacyCollectedDataType": "NSPrivacyCollectedDataTypeProductInteraction", "NSPrivacyCollectedDataTypeLinked": False, "NSPrivacyCollectedDataTypeTracking": False, "NSPrivacyCollectedDataTypePurposes": ["NSPrivacyCollectedDataTypePurposeAnalytics"]},
            {"NSPrivacyCollectedDataType": "NSPrivacyCollectedDataTypeDeviceID", "NSPrivacyCollectedDataTypeLinked": False, "NSPrivacyCollectedDataTypeTracking": False, "NSPrivacyCollectedDataTypePurposes": ["NSPrivacyCollectedDataTypePurposeAnalytics"]},
        ])
        project = (IOS_ROOT / "project.yml").read_text()
        self.assertEqual(project.count("path: SharedResources/PrivacyInfo.xcprivacy"), 2)
        generated = (IOS_ROOT / "MSIMEClient.xcodeproj/project.pbxproj").read_text()
        self.assertIn("path = PrivacyInfo.xcprivacy", generated)
        self.assertEqual(generated.count("PrivacyInfo.xcprivacy in Resources"), 4)

    def test_app_ships_the_licences_of_the_embedded_speech_runtime(self):
        project = (IOS_ROOT / "project.yml").read_text()
        generated = (IOS_ROOT / "MSIMEClient.xcodeproj/project.pbxproj").read_text()
        self.assertIn("SherpaOnnxC.xcframework", project)
        notices = (IOS_ROOT / "SharedResources/VoiceRuntime-NOTICES.txt").read_text()
        self.assertIn("Apache License", notices)
        self.assertIn("MIT License", notices)
        for resource in ("VoiceRuntime-NOTICES.txt", "onnxruntime-ThirdPartyNotices.txt"):
            self.assertTrue(any((IOS_ROOT / path).is_file() for path in (f"SharedResources/{resource}", f"../linux/data/licenses/{resource}")))
            self.assertEqual(generated.count(f"{resource} in Resources"), 2)

    def test_app_ships_the_licence_of_the_embedded_hanja_table(self):
        # The engine linked into the app and its keyboard extension embeds libhangul's Hanja table, which is BSD-3-Clause.
        project = (IOS_ROOT / "project.yml").read_text()
        app = dict(target_blocks(project))["MSIMEApp"]
        blocks = source_path_blocks(app, "../../resources/licenses/libhangul-hanja-BSD-3-Clause.txt")
        self.assertEqual(len(blocks), 1)
        self.assertIn("buildPhase: resources", blocks[0])
        self.assertIn("Choe Hwanjin", (IOS_ROOT / "../../resources/licenses/libhangul-hanja-BSD-3-Clause.txt").read_text())

    def test_app_ships_the_licences_of_the_cantonese_and_zhuyin_data(self):
        # The engine linked into the app and its keyboard extension has Cantonese and Zhuyin schemes whose data derives from rime-cantonese (CC BY 4.0) and libchewing-data (LGPL-2.1-or-later).
        project = (IOS_ROOT / "project.yml").read_text()
        app = dict(target_blocks(project))["MSIMEApp"]
        for licence, holder in (("rime-cantonese-CC-BY-4.0.txt", "CanCLID"), ("libchewing-data-LGPL-2.1.txt", "libchewing Core Team")):
            path = f"../../resources/licenses/{licence}"
            blocks = source_path_blocks(app, path)
            self.assertEqual(len(blocks), 1)
            self.assertIn("buildPhase: resources", blocks[0])
            self.assertIn(holder, (IOS_ROOT / path).read_text())

    def test_app_ships_the_licences_of_the_vietnamese_and_tibetan_crates(self):
        # 链接进 App 和键盘扩展的 Engine 编入了越南文方案的 vi crate 和藏文方案的 ewts crate，二者都按 MIT 使用。
        project = (IOS_ROOT / "project.yml").read_text()
        app = dict(target_blocks(project))["MSIMEApp"]
        for licence, holder in (("vi-MIT.txt", "Hung Nguyen"), ("ewts-MIT.txt", "Maxim Zommer")):
            path = f"../../resources/licenses/{licence}"
            blocks = source_path_blocks(app, path)
            self.assertEqual(len(blocks), 1)
            self.assertIn("buildPhase: resources", blocks[0])
            self.assertIn(holder, (IOS_ROOT / path).read_text())

    def test_app_and_keyboard_share_the_declared_app_group(self):
        expected = "group.app.msime.ios"
        app = (IOS_ROOT / "App/Resources/MSIMEApp.entitlements").read_text()
        keyboard = (IOS_ROOT / "KeyboardExtension/Resources/MSIMEKeyboardExtension.entitlements").read_text()
        self.assertIn(expected, app)
        self.assertIn(expected, keyboard)
        project = (IOS_ROOT / "project.yml").read_text()
        self.assertIn("CODE_SIGN_ENTITLEMENTS: App/Resources/MSIMEApp.entitlements", project)
        self.assertIn("CODE_SIGN_ENTITLEMENTS: KeyboardExtension/Resources/MSIMEKeyboardExtension.entitlements", project)

    def test_app_icon_assets_and_alternate_names_are_configured(self):
        project = (IOS_ROOT / "project.yml").read_text()
        self.assertIn("- path: App/Resources/Assets.xcassets", project)
        self.assertIn("ASSETCATALOG_COMPILER_APPICON_NAME: AppIcon", project)
        self.assertIn(
            "ASSETCATALOG_COMPILER_ALTERNATE_APPICON_NAMES: AppIconForest AppIconSky AppIconDusk AppIconVermilion",
            project,
        )
        assets = IOS_ROOT / "App/Resources/Assets.xcassets"
        for name in ["AppIcon", "AppIconForest", "AppIconSky", "AppIconDusk", "AppIconVermilion"]:
            self.assertTrue((assets / f"{name}.appiconset/Contents.json").is_file(), name)
        for name in ["Classic", "Forest", "Sky", "Dusk", "Vermilion"]:
            self.assertTrue((assets / f"AppIconPreview{name}.imageset/Contents.json").is_file(), name)

    # A `swift build` under shared/backend leaves 2000+ files in .build, and every target that takes
    # that directory as a source path would otherwise compile them into the app: the archive fails
    # with dozens of "Multiple commands produce" errors naming MSIMEBackend.o and precompiled
    # modules.
    def test_every_shared_backend_source_path_excludes_swiftpm_output(self):
        project = (IOS_ROOT / "project.yml").read_text()
        blocks = [
            block
            for _, body in target_blocks(project)
            for block in source_path_blocks(body, "../../shared/backend")
        ]
        self.assertTrue(blocks, "no target takes shared/backend as a source path any more")
        for block in blocks:
            self.assertIn("- .build/**", block)

    # XcodeGen's Info.plist detection ignores source excludes, so targets that rely entirely on a
    # generated plist must pin an empty INFOPLIST_FILE. The application target is different: it has
    # an explicit plist path for its URL scheme and must keep that file while generating the rest
    # of its keys from settings.
    def test_targets_that_generate_their_plist_pin_the_file_setting(self):
        project = (IOS_ROOT / "project.yml").read_text()
        generated = [
            name
            for name, body in target_blocks(project)
            if "GENERATE_INFOPLIST_FILE: YES" in body
        ]
        self.assertTrue(generated, "no target asks Xcode to generate its Info.plist any more")
        for name, body in target_blocks(project):
            if "GENERATE_INFOPLIST_FILE: YES" in body and "info:\n      path:" not in body:
                self.assertIn('INFOPLIST_FILE: ""', body, name)

    # The developer account carries app.msime.ios and app.msime.ios.keyboard with the App Group the
    # entitlements declare. A bundle identifier outside that prefix has no profile that satisfies
    # the App Groups entitlement, and signing fails before anything reaches a device.
    def test_bundle_identifiers_match_the_provisioned_app_ids(self):
        project = (IOS_ROOT / "project.yml").read_text()
        identifiers = re.findall(r"PRODUCT_BUNDLE_IDENTIFIER: (\S+)", project)
        self.assertTrue(identifiers)
        for identifier in identifiers:
            self.assertTrue(identifier == "app.msime.ios" or identifier.startswith("app.msime.ios."),
                            identifier)
        self.assertIn("PRODUCT_BUNDLE_IDENTIFIER: app.msime.ios\n", project)
        self.assertIn("PRODUCT_BUNDLE_IDENTIFIER: app.msime.ios.keyboard\n", project)
        self.assertIn("bundleIdPrefix: app.msime.ios", project)
        self.assertIn("DEVELOPMENT_TEAM: LXCL4Z68GU", project)

    def test_device_uses_real_handwriting_and_simulator_keeps_buildable_fallback(self):
        project = (IOS_ROOT / "project.yml").read_text()
        self.assertIn("EXCLUDED_SOURCE_FILE_NAMES[sdk=iphoneos*]: HandwritingInputViewFallback.swift", project)
        self.assertIn("EXCLUDED_SOURCE_FILE_NAMES[sdk=iphonesimulator*]: HandwritingInputView.swift HandwritingDownloadSession.m", project)
        self.assertIn("SWIFT_OBJC_BRIDGING_HEADER", project)
        podfile = (IOS_ROOT / "Podfile").read_text()
        self.assertIn("pod 'MLKitDigitalInkRecognition', '8.0.0'", podfile)
        self.assertIn("target 'MSIMEKeyboardExtension'", podfile)
        fallback = (IOS_ROOT / "SharedUI/input/HandwritingInputViewFallback.swift").read_text()
        self.assertIn("var onResults: (([String]) -> Void)?", fallback)
        self.assertIn("func use(at index: Int) -> Bool { false }", fallback)
        self.assertIn("func commitFirst() -> Bool { false }", fallback)

    def test_every_keyboard_scroll_view_turns_off_the_ios26_edge_effect(self):
        roots = [IOS_ROOT / "SharedUI", IOS_ROOT / "KeyboardExtension/Sources"]
        sources = sorted(path for root in roots for path in root.rglob("*.swift"))
        self.assertTrue(sources)
        uikit, swiftui = [], []
        for path in sources:
            if path.name == "ScrollEdgeEffects.swift":
                continue
            text = path.read_text()
            if re.search(r"= UIScrollView\(\)|: UIScrollView \{", text) and "disableEdgeEffects()" not in text:
                uikit.append(path.name)
            if re.search(r"^\s*ScrollView \{", text, re.M) and "disablingScrollEdgeEffects()" not in text:
                swiftui.append(path.name)
        self.assertEqual(uikit, [], "UIKit scroll views must call disableEdgeEffects()")
        self.assertEqual(swiftui, [], "SwiftUI scroll views must call disablingScrollEdgeEffects()")

    def test_about_and_download_links_use_the_shared_client_repository(self):
        source = (IOS_ROOT / "App/Sources/settings/AboutAndDownloadViews.swift").read_text()
        feedback = (IOS_ROOT / "App/Sources/settings/HelpAndFeedbackViews.swift").read_text()
        self.assertEqual(source.count('"msime"'), 1)
        self.assertNotIn("MSIME-Apple", source)
        self.assertNotIn("MSIME-Windows", source)
        self.assertNotIn("MSIME-Linux", source)
        self.assertEqual(
            source.count("https://github.com/metasequoiaime/msime")
            + feedback.count("https://github.com/metasequoiaime/msime"),
            2,
        )
        project = (IOS_ROOT / "MSIMEClient.xcodeproj/project.pbxproj").read_text()
        self.assertIn("path = HelpAndFeedbackViews.swift", project)
        self.assertEqual(project.count("HelpAndFeedbackViews.swift in Sources"), 2)

    def test_shipping_build_uses_the_native_ios_host(self):
        script = (IOS_ROOT / "build-app.sh").read_text()
        self.assertIn('tauri_target=aarch64-sim', script)
        self.assertIn('tauri_target=aarch64', script)
        guard = 'if [ "${MSIME_IOS_TAURI_COMPONENT:-0}" = 1 ]; then'
        self.assertIn(guard, script)
        component, shipping = script.split(guard, 1)[1].split("\nfi\n", 1)
        # The Tauri iOS build is the shared component, reachable only behind the explicit opt-in.
        self.assertIn('pnpm --filter @msime/desktop tauri ios build \\', component)
        self.assertIn('--target "$tauri_target" --no-sign --ci', component)
        self.assertNotIn('tauri ios build', shipping)
        # The default product is the native host under platforms/ios.
        self.assertIn('xcodegen generate', shipping)
        self.assertIn('-scheme MSIMEApp', shipping)
        self.assertNotIn('MSIME_IOS_LEGACY_APP', script)

    def test_on_device_speech_runtime_is_fetched_and_embedded_only_in_the_app(self):
        project = (IOS_ROOT / "project.yml").read_text()
        blocks = dict(target_blocks(project))
        framework = "- framework: ../../target/voice-runtime/ios/SherpaOnnxC.xcframework"
        # The keyboard extension's memory limit cannot hold a model, so only the app links the runtime.
        self.assertEqual([name for name, body in blocks.items() if "SherpaOnnxC" in body], ["MSIMEApp"])
        app = blocks["MSIMEApp"]
        self.assertIn(framework, app)
        self.assertRegex(app.split(framework, 1)[1], r"^\n\s+embed: true\n\s+codeSign: true\n")
        self.assertIn("INFOPLIST_KEY_NSSpeechRecognitionUsageDescription:", app)
        # Only the Swift file that runs the recognizer imports the runtime; everything the unit tests compile stays free of it.
        importers = sorted(path.name for path in (IOS_ROOT / "App").rglob("*.swift") if "import SherpaOnnxC" in path.read_text())
        self.assertEqual(importers, ["LocalSpeechRecognizer.swift"])
        generated = (IOS_ROOT / "MSIMEClient.xcodeproj/project.pbxproj").read_text()
        self.assertIn("SherpaOnnxC.xcframework in Embed Frameworks", generated)
        self.assertEqual(generated.count("INFOPLIST_KEY_NSSpeechRecognitionUsageDescription"), 2)
        # Fetched and verified at build time, never committed.
        script = (IOS_ROOT / "build-app.sh").read_text()
        fetch = 'python3 "$repo_root/scripts/fetch_voice_runtime.py" --platform ios'
        self.assertIn(fetch, script)
        self.assertLess(script.index(fetch), script.rindex("xcodegen generate"))


if __name__ == "__main__":
    unittest.main()
