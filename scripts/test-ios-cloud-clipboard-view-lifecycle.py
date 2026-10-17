#!/usr/bin/env python3
"""离开 iOS 云剪贴板页后，旧任务不能卡住或覆盖重建页面。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/keyboard/CloudClipboardView.swift"
GENERATION = ROOT / "platforms/ios/App/Sources/app/ViewRequestGeneration.swift"
TASK = ROOT / "platforms/ios/App/Sources/app/ViewRequestTask.swift"


class IOSCloudClipboardViewLifecycleContract(unittest.TestCase):
    def test_disappear_invalidates_pending_operation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("@State private var request = ViewRequestGeneration()", source)
        self.assertIn(".task { run { _, _ in nil } }", source)
        self.assertIn(".trackViewRequest(request: $request, pending: $pending, onDisappear: {", source)
        self.assertNotIn("ViewRequestTask.disappear(request: $request, pending: $pending)", source)
        self.assertIn("busy = false", source)
        task = TASK.read_text(encoding="utf-8")
        self.assertIn("request.wrappedValue.disappear()", task)
        self.assertIn("pending.wrappedValue?.cancel()", task)
        self.assertIn("pending.wrappedValue = nil", task)

    def test_results_are_bound_to_the_operation_generation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("ViewRequestTask.run(request: $request, busy: $busy, message: $message,", source)
        self.assertIn("pending: $pending", source)
        self.assertNotIn("let id = request.begin()", source)
        self.assertRegex(source, r"execute\(action, requestID: id\)")
        execute = re.search(r"private func execute\(.*?\n  \}", source, re.S)
        self.assertIsNotNone(execute, "could not locate CloudClipboardView.execute")
        body = execute.group(0)
        self.assertGreaterEqual(body.count("try request.checkCurrent(id)"), 5)
        self.assertGreaterEqual(body.count("request.isCurrent(id)"), 4)
        generation = GENERATION.read_text(encoding="utf-8")
        self.assertIn("try Task.checkCancellation()", generation)
        self.assertIn("guard isCurrent(id) else { throw CancellationError() }", generation)
        self.assertIn("if request.wrappedValue.isCurrent(id)", TASK.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
