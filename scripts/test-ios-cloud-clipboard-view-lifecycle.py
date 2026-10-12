#!/usr/bin/env python3
"""离开 iOS 云剪贴板页后，旧任务不能卡住或覆盖重建页面。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/keyboard/CloudClipboardView.swift"


class IOSCloudClipboardViewLifecycleContract(unittest.TestCase):
    def test_disappear_invalidates_pending_operation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, r"@State private var requestID = UUID\(\)")
        disappear = re.search(r"\.onDisappear \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(disappear, "cloud clipboard view must invalidate work when it disappears")
        body = disappear.group("body")
        self.assertIn("requestID = UUID()", body)
        self.assertIn("busy = false", body)
        self.assertIn("pending?.cancel()", body)

    def test_results_are_bound_to_the_operation_generation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, re.compile(r"let id = UUID\(\).*?requestID = id", re.S))
        self.assertRegex(source, r"execute\(action, requestID: id\)")
        execute = re.search(r"private func execute\(.*?\n  \}", source, re.S)
        self.assertIsNotNone(execute, "could not locate CloudClipboardView.execute")
        body = execute.group(0)
        self.assertGreaterEqual(len(re.findall(r"requestID == id", body)), 2)


if __name__ == "__main__":
    unittest.main()
