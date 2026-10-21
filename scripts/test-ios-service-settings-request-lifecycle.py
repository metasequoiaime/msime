#!/usr/bin/env python3
"""服务设置页的异步请求、语音进度与润色结果只写回当前可见页面。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "platforms/ios/App/Sources/settings/FeatureSettingsViews.swift").read_text(encoding="utf-8")
SERVICE = SOURCE.split("struct ServiceSettingsView", 1)[1].split("private struct AICandidatePromptView", 1)[0]
GENERATION = (ROOT / "platforms/ios/App/Sources/app/ViewRequestGeneration.swift").read_text(encoding="utf-8")
TASK = (ROOT / "platforms/ios/App/Sources/app/ViewRequestTask.swift").read_text(encoding="utf-8")


class ServiceSettingsRequestLifecycleTests(unittest.TestCase):
    def test_page_and_operation_generations_are_distinct(self):
        self.assertIn("@State private var request = ViewRequestGeneration()", SERVICE)
        self.assertIn(".onAppear { request.appear() }", SERVICE)
        self.assertIn("request.disappear()", SERVICE)
        self.assertIn("request.invalidate()", SERVICE)
        self.assertEqual(SERVICE.count("let id = request.begin()"), 3)
        self.assertEqual(SERVICE.count("ViewRequestTask.run(request: $request, busy: $busy, pending: $operation,"), 2)
        self.assertIn("@State private var voiceGeneration: UInt64 = 0", SERVICE)
        self.assertIn("func checkCurrent(_ id: UUID) throws", GENERATION)

    def test_async_results_and_partial_voice_text_are_fenced(self):
        self.assertGreaterEqual(SERVICE.count("try request.checkCurrent(id)"), 5)
        self.assertGreaterEqual(SERVICE.count("request.isCurrent(id)"), 14)
        self.assertIn("if request.wrappedValue.isCurrent(id)", TASK)
        self.assertGreaterEqual(SERVICE.count("if request.isCurrent(id) && recognizesLive { liveText = text }"), 2)
        deliver = re.search(r"private func deliver\(.*?\n  \}", SERVICE, re.S)
        self.assertIsNotNone(deliver)
        self.assertRegex(deliver.group(0), re.compile(
            r"let polished = try await VoicePolishService\.polish\(result, settings: polish\).*?try request\.checkCurrent\(id\).*?output = polished.*?status = \"已完成\"",
            re.S,
        ))

    def test_cancelled_recording_preparation_cannot_start_recognition(self):
        self.assertRegex(SERVICE, re.compile(
            r"let recordingID = request\.begin\(\).*?startStreaming\(.*?try request\.checkCurrent\(recordingID\).*?recognizeOnDevice\(pcm\).*?recognizeLive\(pcm\)",
            re.S,
        ))


if __name__ == "__main__":
    unittest.main()
