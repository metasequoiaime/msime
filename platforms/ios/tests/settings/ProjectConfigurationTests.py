import re
import plistlib
import subprocess
import tempfile
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
    def test_keyboard_settings_sources_include_request_generation_dependency(self):
        project = (IOS_ROOT / "project.yml").read_text()
        keyboard = dict(target_blocks(project))["MSIMEKeyboardTests"]
        self.assertIn("- path: App/Sources/settings/HelpcodeSettingsView.swift", keyboard)
        self.assertIn("- path: App/Sources/app/ViewRequestGeneration.swift", keyboard)
        self.assertIn("- path: App/Sources/app/ViewRequestTask.swift", keyboard)

    def test_service_tests_include_community_item_filter(self):
        project = (IOS_ROOT / "project.yml").read_text()
        service = dict(target_blocks(project))["MSIMEServiceTests"]
        self.assertIn("- path: App/Sources/community/CommunityUnseenItems.swift", service)

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

    def test_app_ships_the_licences_of_the_language_dictionary_data(self):
        # The engine linked into the app and its keyboard extension has Cantonese, Zhuyin and Stroke schemes whose data derives from rime-cantonese (CC BY 4.0), libchewing-data (LGPL-2.1-or-later) and rime-stroke (LGPL-3.0, with the CNS11643 attribution).
        project = (IOS_ROOT / "project.yml").read_text()
        app = dict(target_blocks(project))["MSIMEApp"]
        for licence, holder in (("rime-cantonese-CC-BY-4.0.txt", "CanCLID"), ("libchewing-data-LGPL-2.1.txt", "libchewing Core Team"), ("rime-stroke-LGPL-3.0.txt", "CNS11643中文標準交換碼全字庫網站")):
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

    def test_app_ships_the_licences_of_the_dictionary_data(self):
        # 随 App 分发的词库里有 MIT 与 Apache-2.0 的数据：msime-pinyin.db 的单字读音（pinyin-data）、msime-english.db 的中英释义（ECDICT）、msime-others.db 的颜文字（rime_kaomoji_dict）与关键词整词读音（python-pinyin）、msime-wubi.db 的 86 码表（rime-wubi86-jidian）；两种许可都要求随副本附上全文。msime-pinyin.db 与 msime-stroke.db 还有改编自 rime-wanxiang 的权重，CC BY 4.0 要求署名、改动说明与许可证随副本分发。关于页逐个列出。
        project = (IOS_ROOT / "project.yml").read_text()
        app = dict(target_blocks(project))["MSIMEApp"]
        generated = (IOS_ROOT / "MSIMEClient.xcodeproj/project.pbxproj").read_text()
        about = (IOS_ROOT / "App/Sources/settings/AboutAndDownloadViews.swift").read_text()
        for licence, phrase in (
            ("pinyin-data-MIT.txt", "Copyright (c) 2016 mozillazg"),
            ("ECDICT-MIT.txt", "Copyright (c) 2025 Linwei"),
            ("rime_kaomoji_dict-MIT.txt", "Copyright (c) 2023 aoguai"),
            ("python-pinyin-MIT.txt", "Copyright (c) 2016 mozillazg, 闲耘"),
            ("rime-wubi86-jidian-Apache-2.0.txt", "Version 2.0, January 2004"),
            ("rime-wanxiang-CC-BY-4.0.txt", "7ab73ee0c7f80190004233e70f954833df64c80e"),
        ):
            path = f"../../resources/licenses/{licence}"
            blocks = source_path_blocks(app, path)
            self.assertEqual(len(blocks), 1, licence)
            self.assertIn("buildPhase: resources", blocks[0])
            self.assertIn(phrase, (IOS_ROOT / path).read_text())
            self.assertEqual(generated.count(f"{licence} in Resources"), 2, licence)
            self.assertIn(f'LicenseNotice(file: "{licence.removesuffix(".txt")}"', about)

    def test_app_ships_the_dictionary_sources_notice(self):
        # 键盘扩展的 EngineResources 带着 msime-bigram.bin、msime-trigram.bin 和 msime-wubi.db 等桌面词库：n-gram 表统计自中文维基百科，CC BY-SA 4.0 要求再分发时署名，五笔码表混着几份不同许可的来源，这些都只写在词库来源说明里。说明是 Markdown，关于页按 md 扩展名找它。
        project = (IOS_ROOT / "project.yml").read_text()
        app = dict(target_blocks(project))["MSIMEApp"]
        generated = (IOS_ROOT / "MSIMEClient.xcodeproj/project.pbxproj").read_text()
        about = (IOS_ROOT / "App/Sources/settings/AboutAndDownloadViews.swift").read_text()
        notice = "msime-engine-dictionary-NOTICE.md"
        path = f"../../resources/licenses/{notice}"
        blocks = source_path_blocks(app, path)
        self.assertEqual(len(blocks), 1)
        self.assertIn("buildPhase: resources", blocks[0])
        text = (IOS_ROOT / path).read_text()
        for phrase in ("中文维基百科", "CC-BY-SA 4.0", "### 拼音权重与万象对齐"):
            self.assertIn(phrase, text)
        self.assertEqual(generated.count(f"{notice} in Resources"), 2)
        self.assertIn('LicenseNotice(file: "msime-engine-dictionary-NOTICE"', about)
        self.assertIn('fileExtension: "md")', about)

    def test_app_ships_the_licence_of_the_component_lookup_table(self):
        # 链接进 App 和键盘扩展的 Engine 编入了拆字反查的部件拼音表，它改编自開放詞典的漢語拆字字典（CC BY 3.0），署名、固定提交和改动说明都在这份许可证文件里。
        project = (IOS_ROOT / "project.yml").read_text()
        app = dict(target_blocks(project))["MSIMEApp"]
        path = "../../resources/licenses/kfcd-chaizi-CC-BY-3.0.txt"
        blocks = source_path_blocks(app, path)
        self.assertEqual(len(blocks), 1)
        self.assertIn("buildPhase: resources", blocks[0])
        text = (IOS_ROOT / path).read_text()
        self.assertIn("開放詞典", text)
        self.assertIn("e177ab54ce4edf255315a60593ffecbd8167e96b", text)
        generated = (IOS_ROOT / "MSIMEClient.xcodeproj/project.pbxproj").read_text()
        self.assertEqual(generated.count("kfcd-chaizi-CC-BY-3.0.txt in Resources"), 2)
        about = (IOS_ROOT / "App/Sources/settings/AboutAndDownloadViews.swift").read_text()
        self.assertIn('LicenseNotice(file: "kfcd-chaizi-CC-BY-3.0"', about)
        # 同一张表里每个字的读音、名次和繁体字形取自 Unihan，符号模式的分组表按 Unicode 字符数据库挑选组员，Unicode License V3 要求随数据附上它的版权与许可声明。
        unicode = "../../resources/licenses/Unicode-License-V3.txt"
        blocks = source_path_blocks(app, unicode)
        self.assertEqual(len(blocks), 1)
        self.assertIn("buildPhase: resources", blocks[0])
        self.assertIn("UNICODE LICENSE V3", (IOS_ROOT / unicode).read_text())
        self.assertEqual(generated.count("Unicode-License-V3.txt in Resources"), 2)
        self.assertIn('LicenseNotice(file: "Unicode-License-V3"', about)

    def test_app_ships_the_licences_of_the_engine_code(self):
        # 链接进 App 和键盘扩展的引擎本身（crates/engine、crates/path-trust）和它链接的 rink-core 是 MPL-2.0，cxx_sort.rs 逐步复现了 libc++ 的排序算法（Apache-2.0 WITH LLVM-exception），octagram.rs 移植自 librime-octagram 并按 darts-clone 的布局读双数组，符号模式的 LaTeX 表是本项目自写的 MIT 数据；二进制分发都要附上许可证全文，关于页逐个列出。
        project = (IOS_ROOT / "project.yml").read_text()
        app = dict(target_blocks(project))["MSIMEApp"]
        generated = (IOS_ROOT / "MSIMEClient.xcodeproj/project.pbxproj").read_text()
        about = (IOS_ROOT / "App/Sources/settings/AboutAndDownloadViews.swift").read_text()
        for licence, phrase in (
            ("MPL-2.0.txt", "Mozilla Public License Version 2.0"),
            ("libcxx-Apache-2.0-WITH-LLVM-exception.txt", "LLVM Exceptions to the Apache 2.0 License"),
            ("librime-octagram-BSD-3-Clause.txt", "Copyright (c) 2008-2014, Susumu Yata"),
            ("latex-symbols-MIT.txt", "Copyright (c) 2026 Metasequoia IME contributors"),
        ):
            path = f"../../resources/licenses/{licence}"
            blocks = source_path_blocks(app, path)
            self.assertEqual(len(blocks), 1, licence)
            self.assertIn("buildPhase: resources", blocks[0])
            self.assertIn(phrase, (IOS_ROOT / path).read_text())
            self.assertEqual(generated.count(f"{licence} in Resources"), 2, licence)
            self.assertIn(f'LicenseNotice(file: "{licence.removesuffix(".txt")}"', about)

    def test_app_ships_the_gpl_text_of_the_app_and_the_pinyin_dictionary(self):
        # 水杉输入法本身是 GPL-3.0-only，键盘扩展 EngineResources 里的 msime-pinyin.db 含 rime-ice 的 GPL-3.0 数据，GPL-3.0 要求随目标代码附上许可证全文。键盘扩展只随 App 一起安装，所以全文作为 App 的资源分发：仓库根目录的 LICENSE 原样进 bundle，关于页按没有扩展名的 LICENSE 找它，排在第一项。
        project = (IOS_ROOT / "project.yml").read_text()
        targets = dict(target_blocks(project))
        generated = (IOS_ROOT / "MSIMEClient.xcodeproj/project.pbxproj").read_text()
        about = (IOS_ROOT / "App/Sources/settings/AboutAndDownloadViews.swift").read_text()
        path = "../../LICENSE"
        blocks = source_path_blocks(targets["MSIMEApp"], path)
        self.assertEqual(len(blocks), 1)
        self.assertIn("buildPhase: resources", blocks[0])
        text = (IOS_ROOT / path).read_text()
        for phrase in ("GNU GENERAL PUBLIC LICENSE", "Version 3, 29 June 2007", "END OF TERMS AND CONDITIONS"):
            self.assertIn(phrase, text)
        self.assertEqual(generated.count("/* LICENSE in Resources */"), 2)
        self.assertIn("path = ../..;", generated)
        notices = about[about.index("private static let all = ["):]
        first = re.search(r"LicenseNotice\(file: \"([^\"]+)\"", notices)
        self.assertEqual(first.group(1), "LICENSE")
        self.assertIn('LicenseNotice(file: "LICENSE", title: "水杉输入法", detail: "本应用与拼音词库 · GPL-3.0", fileExtension: "")', about)
        # 键盘扩展不带第二份：它没有许可页，和 App 一起安装。
        self.assertEqual(source_path_blocks(targets["MSIMEKeyboardExtension"], path), [])

    def test_app_and_keyboard_share_the_declared_app_group(self):
        expected = "group.app.msime.ios"
        app = (IOS_ROOT / "App/Resources/MSIMEApp.entitlements").read_text()
        keyboard = (IOS_ROOT / "KeyboardExtension/Resources/MSIMEKeyboardExtension.entitlements").read_text()
        self.assertIn(expected, app)
        self.assertIn(expected, keyboard)
        project = (IOS_ROOT / "project.yml").read_text()
        self.assertIn("CODE_SIGN_ENTITLEMENTS: App/Resources/MSIMEApp.entitlements", project)
        self.assertIn("CODE_SIGN_ENTITLEMENTS: KeyboardExtension/Resources/MSIMEKeyboardExtension.entitlements", project)

    def test_app_group_identifier_is_written_once(self):
        # App Group 标识只写在 MSIMEAppEdition 一处，其他 Swift 代码都引用它；漏掉的那一处会因为 `?? .standard` 悄悄读写另一份数据，其他版本也会读到 full 的数据。测试和 Tauri 公共组件（只有 full）不在检查范围内。
        literal = '"group.app.msime.ios"'
        repo = IOS_ROOT.parents[1]
        owner = repo / "shared/backend/account/BackendAccountClient.swift"
        self.assertIn(f"static let fullAppGroupIdentifier = {literal}", owner.read_text())
        roots = [IOS_ROOT / "App", IOS_ROOT / "KeyboardExtension", IOS_ROOT / "SharedUI", repo / "shared/backend"]
        offenders = [
            str(path.relative_to(repo))
            for root in roots
            for path in sorted(root.rglob("*.swift"))
            if path != owner and "Tests" not in path.relative_to(repo).parts and literal in path.read_text()
        ]
        self.assertEqual(offenders, [])

    def test_url_scheme_is_derived_from_the_edition(self):
        # 键盘拉起 App 的 URL scheme 只写在 MSIMEAppEdition 一处，按版本推出；多个版本装在同一台设备上时，写死的 msime 会让系统任选一个 App 打开，语音交接和设置入口就落到另一个版本。full 的 project.yml 注册的仍是 msime。
        repo = IOS_ROOT.parents[1]
        owner = repo / "shared/backend/account/BackendAccountClient.swift"
        self.assertIn('static let fullURLScheme = "msime"', owner.read_text())
        project = (IOS_ROOT / "project.yml").read_text()
        self.assertIn("CFBundleURLSchemes: [msime]", project)
        roots = [IOS_ROOT / "App", IOS_ROOT / "KeyboardExtension", IOS_ROOT / "SharedUI", repo / "shared/backend"]
        offenders = [
            str(path.relative_to(repo))
            for root in roots
            for path in sorted(root.rglob("*.swift"))
            if path != owner and "Tests" not in path.relative_to(repo).parts
            and re.search(r'"msime://|scheme == "msime"', path.read_text())
        ]
        self.assertEqual(offenders, [])
        launcher = (IOS_ROOT / "KeyboardExtension/Sources/keyboard/KeyboardAppLauncher.swift").read_text()
        self.assertIn('URL(string: "\\(MSIMEAppEdition.urlScheme)://settings")', launcher)
        self.assertIn('URL(string: "\\(MSIMEAppEdition.urlScheme)://voice")', launcher)
        app = (IOS_ROOT / "App/Sources/MetasequoiaImeApp.swift").read_text()
        self.assertIn("url.scheme == MSIMEAppEdition.urlScheme", app)

    def test_scheme_choices_are_narrowed_by_edition(self):
        # 首次引导和方案页都只列本版本的入口；写共享文档的 schemeMapping 也丢掉本版本没有的入口，任何调用方都写不进 host-api 会回退掉的方案。
        welcome = (IOS_ROOT / "App/Sources/app/WelcomeFlowView.swift").read_text()
        self.assertIn("].filter { $0.scheme.isOfferedByEdition }", welcome)
        onboarding = (IOS_ROOT / "App/Sources/app/OnboardingView.swift").read_text()
        self.assertIn("ChineseInputScheme.allCases.filter(\\.isOfferedByEdition)", onboarding)
        bridge = (IOS_ROOT / "SharedUI/core/MetasequoiaInputSessionBridge.swift").read_text()
        self.assertIn("enabledSchemes.contains($0) && $0.isOfferedByEdition", bridge)

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

    def test_mlkit_stats_collection_is_switched_off_before_the_first_mlkit_call(self):
        # ML Kit logs usage statistics unless the process's own defaults say no (MLKitStatsOptOut.swift). Only the keyboard extension links it and only HandwritingRecognizer calls it, so the recognizer's initializer writes the opt-out before anything else.
        podfile = (IOS_ROOT / "Podfile").read_text()
        app = podfile.split("target 'MSIMEKeyboardExtension' do", 1)[0]
        self.assertNotIn("pod 'MLKit", app)
        roots = [IOS_ROOT / name for name in ("App", "KeyboardExtension", "KeyboardTestHost", "SharedUI")]
        importers = sorted(
            path.relative_to(IOS_ROOT).as_posix()
            for root in roots for path in root.rglob("*")
            if path.suffix in (".swift", ".m", ".mm") and re.search(r"import\s+<?MLKit", path.read_text())
        )
        self.assertEqual(importers, ["KeyboardExtension/Sources/handwriting/HandwritingInputView.swift"])
        source = (IOS_ROOT / importers[0]).read_text()
        self.assertRegex(source, r"final class HandwritingRecognizer \{\n  init\(\) \{\n    MLKitStatsOptOut\.apply\(\)\n")

    def test_google_sign_in_is_linked_into_the_app_only(self):
        podfile = (IOS_ROOT / "Podfile").read_text()
        app, extension = podfile.split("target 'MSIMEKeyboardExtension' do", 1)
        self.assertIn("pod 'GoogleSignIn', '10.0.0'", app)
        # 键盘扩展不登录。它要是嵌套在 MSIMEApp 里就会继承 GoogleSignIn，所以必须是顶层 target，MSIMEApp 的块在它之前结束。
        self.assertRegex(app, r"target 'MSIMEApp' do\n(?:  [^\n]*\n|\n)*end\n")
        self.assertNotIn("GoogleSignIn", extension)
        self.assertIn("pod 'MLKitDigitalInkRecognition', '8.0.0'", extension)
        # 回调 scheme 必须是 GIDClientID 倒过来写，否则 Google 登录页回不到 App。
        project = (IOS_ROOT / "project.yml").read_text()
        client = re.search(r"GIDClientID: (\S+)\.apps\.googleusercontent\.com\n", project).group(1)
        self.assertIn(f"CFBundleURLSchemes: [com.googleusercontent.apps.{client}]", project)
        self.assertIn("GIDServerClientID: 237074057301-q5av9h30kspkqnjat4i9hn9oer9ogfpp.apps.googleusercontent.com", project)
        android = (IOS_ROOT / "../android/res/values/strings.xml").read_text()
        self.assertIn("237074057301-q5av9h30kspkqnjat4i9hn9oer9ogfpp.apps.googleusercontent.com", android)

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

    def test_keyboard_ships_the_onnx_rerank_backend_and_both_models(self):
        # 整句联想「增强」的逐键重排：host 库打开 onnx-rerank、链接按锁下载的精简 ONNX Runtime，两档模型暂存在 EngineResources 旁边（资源目录本身要与词库锁逐项相符），键盘扩展把它们作为文件夹打包，build-app.sh 核对它们真的进了扩展。
        native = (IOS_ROOT / "build-native.sh").read_text()
        self.assertIn('scripts/fetch_onnxruntime.py" --target "$rust_target"', native)
        self.assertIn('ORT_LIB_LOCATION="$ort_location"', native)
        self.assertIn("cargo build -p msime-host-api --features onnx-rerank", native)
        staging = (IOS_ROOT / "stage-resources.sh").read_text()
        self.assertIn('scripts/fetch_rerank_model.py', staging)
        project = (IOS_ROOT / "project.yml").read_text()
        keyboard = dict(target_blocks(project))["MSIMEKeyboardExtension"]
        app = (IOS_ROOT / "build-app.sh").read_text()
        for directory in ("rerank-model", "rerank-model-small"):
            blocks = source_path_blocks(keyboard, f"../../target/ios/{directory}")
            self.assertEqual(len(blocks), 1, directory)
            self.assertIn("buildPhase: resources", blocks[0])
            self.assertIn("type: folder", blocks[0])
            self.assertIn(f'"$repo_root/target/ios/{directory}"', staging.replace("$directory", directory))
        self.assertIn("for directory in rerank-model rerank-model-small; do", app)
        self.assertIn('[ -s "$keyboard_bundle/$directory/$name" ]', app)

    def test_host_library_links_before_the_system_sqlite(self):
        # The host library carries rusqlite's bundled SQLite. Linked after $(inherited) (MLKit's -l"sqlite3") or -lsqlite3, the Xcode 27 SDK binds every sqlite3_* to /usr/lib/libsqlite3.dylib, and sqlite3_set_errmsg does not exist there before iOS 27.
        project = (IOS_ROOT / "project.yml").read_text()
        pbxproj = (IOS_ROOT / "MSIMEClient.xcodeproj" / "project.pbxproj").read_text()
        linked = [name for name, body in target_blocks(project) if '"-lmsime_host_api"' in body]
        self.assertIn("MSIMEApp", linked)
        self.assertIn("MSIMEKeyboardExtension", linked)
        for name, body in target_blocks(project):
            if name not in linked:
                continue
            flags = re.findall(r'^ +- "?([^"\n]+)"?$', body.split("OTHER_LDFLAGS:", 1)[1].split("\n        L", 1)[0], re.M)
            self.assertEqual(flags[0], "-lmsime_host_api", name)
        for block in re.findall(r"OTHER_LDFLAGS = \(\n(.*?)\);", pbxproj, re.S):
            if '"-lmsime_host_api"' in block:
                self.assertEqual(block.split("\n", 1)[0].strip(), '"-lmsime_host_api",')

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

    def test_release_date_is_stamped_into_the_app_info_plist_only(self):
        project = (IOS_ROOT / "project.yml").read_text()
        blocks = dict(target_blocks(project))
        # 只有 App 的「关于」页显示发布日期，键盘扩展和测试 target 不跑这个脚本。
        self.assertEqual([name for name, body in blocks.items() if "stamp-release-date.sh" in body], ["MSIMEApp"])
        app = blocks["MSIMEApp"]
        self.assertIn("ENABLE_USER_SCRIPT_SANDBOXING: NO", app)
        self.assertRegex(app, r"inputFiles:\n\s+- \$\(TARGET_BUILD_DIR\)/\$\(INFOPLIST_PATH\)\n\s+basedOnDependencyAnalysis: false")
        # 提交的工程要与 project.yml 同步，本地用 Xcode 打开时也会写入日期。
        generated = (IOS_ROOT / "MSIMEClient.xcodeproj/project.pbxproj").read_text()
        self.assertIn('shellScript = "bash \\"$SRCROOT/scripts/stamp-release-date.sh\\"";', generated)
        self.assertIn('"$(TARGET_BUILD_DIR)/$(INFOPLIST_PATH)",', generated)
        self.assertEqual(generated.count("ENABLE_USER_SCRIPT_SANDBOXING = NO;"), 2)

    def test_release_date_sources_follow_the_shared_priority(self):
        script = IOS_ROOT / "scripts/stamp-release-date.sh"
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "source"
            source.mkdir()
            plist = root / "Info.plist"
            plist.write_bytes(plistlib.dumps({"CFBundleShortVersionString": "0.52.0"}, fmt=plistlib.FMT_BINARY))

            def stamp(**extra):
                env = {"PATH": "/usr/bin:/bin", "TARGET_BUILD_DIR": temp, "INFOPLIST_PATH": "Info.plist", "SRCROOT": str(source), **extra}
                return subprocess.run(["bash", str(script)], env=env, capture_output=True, text=True, timeout=30)

            def stamped():
                return plistlib.loads(plist.read_bytes()).get("MSIMEReleaseDate")

            # 源码目录不在 git 仓库里、也没有 release-date.txt：没有日期，不算错误。
            self.assertEqual(stamp().returncode, 0)
            self.assertIsNone(stamped())
            (source / "release-date.txt").write_text("2026-10-01\n")
            self.assertEqual(stamp().returncode, 0)
            self.assertEqual(stamped(), "2026-10-01")
            # 环境变量优先于文件；写入后 Info.plist 仍是 Xcode 生成时的二进制格式。
            self.assertEqual(stamp(MSIME_RELEASE_DATE="2026-10-19").returncode, 0)
            self.assertEqual(stamped(), "2026-10-19")
            self.assertTrue(plist.read_bytes().startswith(b"bplist00"))
            self.assertNotEqual(stamp(MSIME_RELEASE_DATE="2026/10/19").returncode, 0)
            # 增量构建时这次取不到日期，就要清掉上次写入的值。
            (source / "release-date.txt").unlink()
            self.assertEqual(stamp().returncode, 0)
            self.assertIsNone(stamped())


if __name__ == "__main__":
    unittest.main()
