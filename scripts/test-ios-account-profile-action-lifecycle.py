#!/usr/bin/env python3
"""离开 iOS 个人资料页后，旧账号操作不能继续回写页面或触发退出回调。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/account/AccountProfileView.swift"
TASK = ROOT / "platforms/ios/App/Sources/app/ViewRequestTask.swift"


class IOSAccountProfileActionLifecycleContract(unittest.TestCase):
    def test_action_task_is_owned_and_invalidated(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, r"@State private var operation: Task<Void, Never>\?")
        self.assertRegex(source, r"@State private var request = ViewRequestGeneration\(\)")
        disappear = re.search(r"\.onDisappear \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(disappear, "profile page must invalidate account actions when it disappears")
        body = disappear.group("body")
        self.assertIn("ViewRequestTask.disappear(request: $request, pending: $operation)", body)
        task = TASK.read_text(encoding="utf-8")
        self.assertIn("request.wrappedValue.disappear()", task)
        self.assertIn("pending.wrappedValue?.cancel()", task)

    def test_action_completion_is_bound_to_current_generation(self):
        source = SOURCE.read_text(encoding="utf-8")
        perform = source.split("private func perform(", 1)[1].split("private func startLoad()", 1)[0]
        self.assertIn("ViewRequestTask.run(request: $request, busy: $busy, message: $actionMessage,", perform)
        self.assertIn("pending: $operation", perform)
        self.assertIn("isCancellation: AccountRequestPolicy.isCancellation", perform)
        self.assertIn("try request.checkCurrent(id)", perform)
        self.assertIn("onSignedOut()", perform)
        self.assertIn("try request.wrappedValue.checkCurrent(id)", TASK.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
