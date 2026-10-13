#!/usr/bin/env python3
"""离开 iOS AI 皮肤生成页后，旧任务不能卡住或覆盖重建页面。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/voice/AISkinGenerationView.swift"
GENERATION = ROOT / "platforms/ios/App/Sources/app/ViewRequestGeneration.swift"
TASK = ROOT / "platforms/ios/App/Sources/app/ViewRequestTask.swift"


class IOSAISkinGenerationViewLifecycleContract(unittest.TestCase):
    def test_disappear_invalidates_pending_generation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("@State private var request = ViewRequestGeneration()", source)
        self.assertIn(".onAppear { request.appear() }", source)
        self.assertIn("ViewRequestTask.cancel(task: $pending)", source)
        disappear = re.search(r"\.onDisappear \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(disappear, "AI skin view must invalidate work when it disappears")
        body = disappear.group("body")
        self.assertIn("ViewRequestTask.disappear(request: $request, pending: $pending)", body)
        task = TASK.read_text(encoding="utf-8")
        self.assertIn("request.wrappedValue.disappear()", task)
        self.assertIn("pending.wrappedValue?.cancel()", task)
        self.assertIn("pending.wrappedValue = nil", task)
        self.assertIn("busy = false", body)

    def test_generation_results_and_progress_are_bound_to_the_operation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("ViewRequestTask.run(request: $request, busy: $busy, message: $message,", source)
        self.assertIn("pending: $pending", source)
        self.assertNotIn("let id = request.begin()", source)
        self.assertRegex(source, r"performGeneration\(requestID: id, description:")
        self.assertRegex(source, r"if request.isCurrent\(id\) \{ completed = \$0 \}")
        generation = re.search(r"private func performGeneration\(.*?\n  \}", source, re.S)
        self.assertIsNotNone(generation, "could not locate AI skin generation task")
        body = generation.group(0)
        self.assertIn("try request.checkCurrent(id)", body)
        self.assertGreaterEqual(body.count("request.isCurrent(id)"), 4)
        generation = GENERATION.read_text(encoding="utf-8")
        self.assertIn("try Task.checkCancellation()", generation)
        self.assertIn("guard isCurrent(id) else { throw CancellationError() }", generation)
        self.assertIn("if request.wrappedValue.isCurrent(id)", TASK.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
