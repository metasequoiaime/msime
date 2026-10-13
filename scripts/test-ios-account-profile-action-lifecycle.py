#!/usr/bin/env python3
"""离开 iOS 个人资料页后，旧账号操作不能继续回写页面或触发退出回调。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/account/AccountProfileView.swift"


class IOSAccountProfileActionLifecycleContract(unittest.TestCase):
    def test_action_task_is_owned_and_invalidated(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, r"@State private var operation: Task<Void, Never>\?")
        self.assertRegex(source, r"@State private var request = ViewRequestGeneration\(\)")
        disappear = re.search(r"\.onDisappear \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(disappear, "profile page must invalidate account actions when it disappears")
        body = disappear.group("body")
        self.assertIn("request.disappear()", body)
        self.assertIn("operation?.cancel()", body)

    def test_action_completion_is_bound_to_current_generation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, re.compile(r"let id = request\.begin\(\)", re.S))
        self.assertRegex(source, r"operation = Task \{")
        task = re.search(r"operation = Task \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(task, "account action task must be stored for cancellation")
        body = task.group("body")
        self.assertIn("request.isCurrent(id)", body)
        self.assertIn("try request.checkCurrent(id)", body)
        self.assertIn("operation = nil", body)
        self.assertIn("onSignedOut()", body)


if __name__ == "__main__":
    unittest.main()
