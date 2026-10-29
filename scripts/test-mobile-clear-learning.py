#!/usr/bin/env python3
"""「词库 › 学习」的「清除学习记录」在 Android 和 iOS 上用同一套文案和同一条路。

标题、说明、按钮、确认框第一句、网盘同步那一句和失败提示逐字相同，确认框中间各说各的：Android 的账号同步含个人词库，说账号和同账号的其他设备会跟着换；iOS 不含，也没有命名词库，改说导入的词一并删除。两端都先确认、在主线程之外发 `msime_client_dictionary` 的 `reset`。鸿蒙不提供这一项，原因见 `.agents/notes/implemented/feature/2026-10-29-mobile-clear-learning.md`：键盘的学习记录在它自己的沙箱里，设置应用清不到。
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
ANDROID_COPY = ROOT / "platforms/android/java/app/msime/android/dictionary/ClearLearningCopy.java"
ANDROID_PAGE = ROOT / "platforms/android/java/app/msime/android/home/LexiconPage.java"
IOS_COPY = ROOT / "platforms/ios/SharedUI/dictionary/ClearLearningCopy.swift"
IOS_PAGE = ROOT / "platforms/ios/App/Sources/settings/DictionarySettingsView.swift"
IOS_BRIDGE = ROOT / "platforms/ios/SharedUI/core/MetasequoiaInputSessionBridge.swift"
HARMONY_CLIENT = ROOT / "apps/harmony/src/main.tsx"

SHARED = {"TITLE": "title", "SUBTITLE": "subtitle", "ACTION": "action", "CONFIRM_TITLE": "confirmTitle", "REMOVED": "removed", "WEBDAV": "webdav", "BUSY": "busy", "FAILED": "failed"}


def java_constant(source: str, name: str) -> str:
    match = re.search(rf'static final String {name} =\s*"([^"]*)";', source)
    if match is None:
        raise AssertionError(f"Android ClearLearningCopy.{name} is not a single string literal")
    return match.group(1)


def swift_constant(source: str, name: str) -> str:
    match = re.search(rf'static let {name} =\s*"([^"]*)"\n', source)
    if match is None:
        raise AssertionError(f"iOS ClearLearningCopy.{name} is not a single string literal")
    return match.group(1)


class ClearLearningContract(unittest.TestCase):
    def test_both_platforms_use_the_same_wording(self):
        android = ANDROID_COPY.read_text(encoding="utf-8")
        ios = IOS_COPY.read_text(encoding="utf-8")
        for java_name, swift_name in SHARED.items():
            self.assertEqual(java_constant(android, java_name), swift_constant(ios, swift_name), java_name)
        self.assertIn("开着云同步时，账号里的个人词库也会换成清除后的这一份，登录同一账号的其他设备同步时会跟着换掉，或者提示你选择。", android)
        self.assertIn("已添加的词库保留", android)
        self.assertIn("+ WEBDAV;", android)
        self.assertIn("导入和从社区添加的词条也一并删除。", ios)
        ios_message = ios.split("static let confirmMessage", 1)[1].split("\n", 1)[0]
        self.assertNotIn("账号", ios_message)
        self.assertTrue(ios_message.endswith("+ webdav"), ios_message)

    def test_android_confirms_before_resetting_off_the_main_thread(self):
        page = ANDROID_PAGE.read_text(encoding="utf-8")
        self.assertIn("learning.button(ClearLearningCopy.TITLE, ClearLearningCopy.SUBTITLE, ClearLearningCopy.ACTION, this::confirmClearLearning);", page)
        self.assertIn("MessageDialog.confirm(requireContext(), ClearLearningCopy.CONFIRM_TITLE, ClearLearningCopy.CONFIRM_MESSAGE,", page)
        self.assertIn("HostTask.run(this, DictionaryCollectionsStore::resetLearning, result -> {", page)

    def test_ios_confirms_before_resetting_off_the_main_thread(self):
        page = IOS_PAGE.read_text(encoding="utf-8")
        learning = page.split("@ViewBuilder private var learningRows", 1)[1].split("private static let countOptions", 1)[0]
        self.assertIn("Button { confirmsClearLearning = true } label: {", learning)
        self.assertIn("DesignNavRowLabel(title: ClearLearningCopy.title, subtitle: ClearLearningCopy.subtitle)", learning)
        self.assertIn(".confirmationDialog(ClearLearningCopy.confirmTitle, isPresented: $confirmsClearLearning, titleVisibility: .visible) {", page)
        self.assertIn("Button(ClearLearningCopy.action, role: .destructive) { Task { await clearLearning() } }", page)
        self.assertIn("} message: { Text(ClearLearningCopy.confirmMessage) }", page)
        clear = page.split("@MainActor private func clearLearning() async {", 1)[1].split("\n  }\n", 1)[0]
        self.assertIn("await Task.detached(priority: .userInitiated) {", clear)
        self.assertIn("MetasequoiaInputSessionBridge.resetLearnedData()", clear)
        bridge = IOS_BRIDGE.read_text(encoding="utf-8")
        reset = bridge.split("static func resetLearnedData(", 1)[1].split("\n  }\n", 1)[0]
        self.assertIn('callOptions(msimeClientDictionary, ["options": options, "action": ["operation": "reset"]])', reset)
        self.assertIn('catch InputBridgeFailure.response("dictionary maintenance busy")', reset)

    def test_harmony_does_not_offer_a_reset_it_cannot_complete(self):
        self.assertNotIn("resetLearnedData", HARMONY_CLIENT.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
