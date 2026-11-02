#!/usr/bin/env python3
"""iOS 语音标点（#10595）在 SwiftUI 页面里的接线：设置搜索能找到「语音标点」，App 语音页（`ServiceSettingsView.deliver`）在润色之后、每条路径只处理一次。

这两处都在主 App 的 SwiftUI 视图里，不在任何 XCTest target 中，所以在这里钉源码；处理规则本身由 `MSIMEKeyboardTests/VoicePunctuationTests` 对着真实 FFI 测，键盘听写由 `SpaceVoicePanelTests/testVoicePunctuationAppliesOnceToTheFinalText` 走完真实会话。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
IOS = ROOT / "platforms/ios"
SETTINGS_DESIGN = (IOS / "App/Sources/app/SettingsDesign.swift").read_text(encoding="utf-8")
VOICE_PAGE = (IOS / "App/Sources/settings/VoiceSettingsView.swift").read_text(encoding="utf-8")
FEATURE = (IOS / "App/Sources/settings/FeatureSettingsViews.swift").read_text(encoding="utf-8")
SERVICE = FEATURE.split("struct ServiceSettingsView", 1)[1].split("private struct AICandidatePromptView", 1)[0]
KEYBOARD = (IOS / "KeyboardExtension/Sources/keyboard/KeyboardViewController.swift").read_text(encoding="utf-8")
ANDROID_PAGES = (ROOT / "platforms/android/java/app/msime/android/home/PageId.java").read_text(encoding="utf-8")


def swift_keywords(page: str) -> list[str]:
    line = next(line for line in SETTINGS_DESIGN.splitlines() if f"case .{page}: return [" in line)
    return re.findall(r'"([^"]*)"', line)


def deliver() -> str:
    match = re.search(r"@MainActor private func deliver\(.*?\n  \}\n", SERVICE, re.S)
    if match is None:
        raise AssertionError("ServiceSettingsView.deliver not found")
    return match.group(0)


def member(source: str, signature: str) -> str:
    """从 `signature` 起到同一类型里下一个两格缩进的成员为止的源码。"""
    start = source.index(signature)
    following = re.search(r"\n  (?:private |@objc|@MainActor|///|var |func |override )", source[start + len(signature):])
    return source[start:start + len(signature) + following.start()] if following else source[start:]


class SettingsSearchTests(unittest.TestCase):
    def test_search_for_voice_punctuation_finds_the_voice_page(self):
        # `SettingsPage.matching` 按「标题或某个关键词包含查询」找页面；「语音输入」和原有的「语音」「识别」都不包含「语音标点」。
        self.assertIn("page.keywords.contains { $0.localizedCaseInsensitiveContains(query) }", SETTINGS_DESIGN)
        query = "语音标点"
        self.assertTrue(any(query in keyword for keyword in swift_keywords("voice")), swift_keywords("voice"))
        self.assertIn(f'DesignSelectRow(title: "{query}"', VOICE_PAGE, "the keyword names a row the page really has")

    def test_android_lists_the_same_row_for_search(self):
        voice = ANDROID_PAGES[ANDROID_PAGES.index("VOICE("):ANDROID_PAGES.index("HANDWRITING(")]
        self.assertIn('"语音标点"', voice)


class ServiceSettingsDeliverTests(unittest.TestCase):
    """3c02a5ba28 之后的 `deliver`：润色拿识别原文，语音标点在润色完成后处理；三条路径（识别原文、润色结果、不润色的结果）各处理一次。"""

    def setUp(self):
        self.body = deliver()
        self.polish_branch, self.plain_branch = self.body.split("\n    } else {\n", 1)

    def test_only_the_voice_page_reads_the_preference(self):
        # AI 服务页的「测试」也经过 deliver，它的回复不是语音结果。
        self.assertRegex(self.body, re.compile(
            r"let punctuation = kind == \.voice\s*\? VoicePunctuation\.mode\(MetasequoiaInputSessionBridge\.loadSharedPreferences\(\)\) : VoicePunctuation\.auto"))

    def test_each_path_applies_punctuation_exactly_once(self):
        self.assertEqual(self.body.count("VoicePunctuation.finalText("), 3)
        self.assertEqual(self.body.count("VoicePunctuation.apply("), 0, "deliver goes through finalText, which tells punctuation-only results apart")
        self.assertIn("transcript = VoicePunctuation.finalText(result, mode: punctuation)", self.polish_branch)
        self.assertIn("VoicePunctuation.finalText(polished, mode: punctuation)", self.polish_branch)
        self.assertIn("VoicePunctuation.finalText(result, mode: punctuation)", self.plain_branch)

    def test_polish_gets_the_recognized_text_and_punctuation_runs_after_it(self):
        polish = self.polish_branch.index("let polished = try await VoicePolishService.polish(result, settings: polish)")
        # 润色前不因只剩标点提前返回：模型拿到的是带标点的识别原文，是否「没有识别到文字」看润色结果。
        self.assertNotIn("没有识别到文字", self.polish_branch[:polish])
        self.assertNotIn("return", self.polish_branch[:polish])
        after = self.polish_branch[polish:]
        self.assertRegex(after, re.compile(
            r"try request\.checkCurrent\(id\)\s*"
            r"guard let text = VoicePunctuation\.finalText\(polished, mode: punctuation\) else \{\s*"
            r"output = \"\"\s*status = \"没有识别到文字\"\s*return\s*\}\s*"
            r"output = text\s*status = \"已完成\"",
        ))

    def test_shown_transcript_is_the_processed_one(self):
        # 「改用识别原文」和润色失败时保留的都是 transcript，它在润色之前就已处理过，交给键盘的不会再带回标点。
        self.assertRegex(self.polish_branch, re.compile(
            r"transcript = VoicePunctuation\.finalText\(result, mode: punctuation\) \?\? \"\"\s*output = transcript\s*status = \"识别完成，正在润色…\""))
        failure = self.polish_branch[self.polish_branch.index("} catch {"):]
        self.assertNotIn("output =", failure, "a failed polish keeps the processed transcript already in output")
        self.assertIn('Button("改用识别原文") { output = transcript; status = "已改用识别原文" }', SERVICE)

    def test_failed_polish_of_a_punctuation_only_result_says_nothing_was_recognized(self):
        # 识别结果只剩标点时 transcript 是空的，润色再失败也没有原文可保留，状态不能说「已保留识别原文」，与不润色的路径一样提示没有识别到文字。
        failure = self.polish_branch[self.polish_branch.index("} catch {"):]
        self.assertRegex(failure, re.compile(
            r"guard request\.isCurrent\(id\), !Task\.isCancelled else \{ return \}\n"
            r"(?: *//[^\n]*\n)? *if transcript\.isEmpty \{\s*status = \"没有识别到文字\"\s*\} else \{\s*"
            r"status = \"润色失败，已保留识别原文：\\\(error\.localizedDescription\)\"\s*\}"))

    def test_unpolished_result_is_processed_once(self):
        self.assertRegex(self.plain_branch, re.compile(
            r"guard let text = VoicePunctuation\.finalText\(result, mode: punctuation\) else \{\s*"
            r"output = \"\"\s*status = \"没有识别到文字\"\s*return\s*\}\s*"
            r"output = text\s*status = \"已完成\""))

    def test_handoff_sends_what_the_page_shows(self):
        self.assertIn("voiceTransfer = try VoiceTextHandoffStore().save(output)", SERVICE)

    def test_keyboard_does_not_process_a_handed_off_result_again(self):
        claim = member(KEYBOARD, "  private func showKeyboardVoice() {")
        self.assertIn("let text = try store.consume(entry.id)", claim)
        self.assertNotIn("VoicePunctuation", claim)


if __name__ == "__main__":
    unittest.main()
