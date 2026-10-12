#!/usr/bin/env python3
"""离开 iOS AI 皮肤生成页后，旧任务不能卡住或覆盖重建页面。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/voice/AISkinGenerationView.swift"


class IOSAISkinGenerationViewLifecycleContract(unittest.TestCase):
    def test_disappear_invalidates_pending_generation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, r"@State private var requestID = UUID\(\)")
        disappear = re.search(r"\.onDisappear \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(disappear, "AI skin view must invalidate work when it disappears")
        body = disappear.group("body")
        self.assertIn("requestID = UUID()", body)
        self.assertIn("request?.cancel()", body)
        self.assertIn("busy = false", body)

    def test_generation_results_and_progress_are_bound_to_the_operation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, re.compile(r"let id = UUID\(\).*?requestID = id", re.S))
        self.assertRegex(source, r"performGeneration\(requestID: id, description:")
        self.assertRegex(source, r"if requestID == id \{ completed = \$0 \}")
        generation = re.search(r"private func performGeneration\(.*?\n  \}", source, re.S)
        self.assertIsNotNone(generation, "could not locate AI skin generation task")
        body = generation.group(0)
        self.assertGreaterEqual(len(re.findall(r"requestID == id", body)), 3)


if __name__ == "__main__":
    unittest.main()
