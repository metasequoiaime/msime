#!/usr/bin/env python3
"""离开翻译服务设置页后，旧的测试请求不能回写新页面状态。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/settings/TranslationProviderSettingsView.swift"


class IOSTranslationProviderViewLifecycleContract(unittest.TestCase):
    def test_view_owns_and_invalidates_test_task(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, r"@State private var operation: Task<Void, Never>\?")
        self.assertRegex(source, r"@State private var testGeneration = OperationGeneration\(\)")
        disappear = re.search(r"\.onDisappear \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(disappear, "translation settings must invalidate work when it disappears")
        body = disappear.group("body")
        self.assertIn("ViewRequestTask.cancel(generation: $testGeneration, task: $operation)", body)

    def test_test_result_is_bound_to_current_generation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("let id = testGeneration.begin()", source)
        self.assertRegex(source, r"operation = Task \{")
        task = re.search(r"operation = Task \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(task, "translation test task must be stored for cancellation")
        body = task.group("body")
        self.assertIn("testGeneration.isCurrent(id)", body)
        self.assertIn("operation = nil", body)


if __name__ == "__main__":
    unittest.main()
