#!/usr/bin/env python3
"""切换云词库类型时，旧请求不能覆盖新类型页面。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/dictionary/CloudDictionaryView.swift"


class IOSCloudDictionaryKindLifecycleContract(unittest.TestCase):
    def test_kind_change_cancels_and_invalidates_previous_request(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, r"@State private var requestID = UUID\(\)")
        change = re.search(r"\.onChange\(of: kind\) \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(change, "kind changes must invalidate the previous dictionary request")
        body = change.group("body")
        self.assertIn("pending?.cancel()", body)
        self.assertIn("requestID = UUID()", body)

    def test_disappearance_invalidates_pending_request(self):
        source = SOURCE.read_text(encoding="utf-8")
        disappear = re.search(r"\.onDisappear \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(disappear, "leaving the dictionary page must invalidate its request")
        body = disappear.group("body")
        self.assertIn("requestID = UUID()", body)
        self.assertIn("pending?.cancel()", body)
        self.assertIn("busy = false", body)

    def test_load_result_is_bound_to_requested_kind_and_generation(self):
        source = SOURCE.read_text(encoding="utf-8")
        load = re.search(r"private func load\(offset: Int\).*?\n  \}", source, re.S)
        self.assertIsNotNone(load, "could not locate CloudDictionaryView.load")
        body = load.group(0)
        self.assertIn("let id = requestID", body)
        self.assertIn("let requestedKind = kind", body)
        self.assertIn("requestedKind", body)
        self.assertIn("requestID == id", body)

    def test_old_task_cannot_clear_new_task_busy_state(self):
        source = SOURCE.read_text(encoding="utf-8")
        run = re.search(r"private func run\(.*?\n  \}", source, re.S)
        self.assertIsNotNone(run, "could not locate CloudDictionaryView.run")
        body = run.group(0)
        self.assertRegex(body, re.compile(r"let id = UUID\(\).*?requestID = id", re.S))
        self.assertIn("if requestID == id", body)


if __name__ == "__main__":
    unittest.main()
