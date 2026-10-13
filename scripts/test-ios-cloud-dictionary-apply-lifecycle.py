#!/usr/bin/env python3
"""校验云词库应用页的请求代次与离页清理契约。"""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/dictionary/CloudDictionaryApplyView.swift").read_text()
GENERATION = Path("platforms/ios/App/Sources/app/ViewRequestGeneration.swift").read_text()
TASK = Path("platforms/ios/App/Sources/app/ViewRequestTask.swift").read_text()


class CloudDictionaryApplyLifecycleTests(unittest.TestCase):
    def test_page_owns_task_and_shared_generation(self):
        self.assertIn("@State private var request = ViewRequestGeneration()", SOURCE)
        self.assertIn(".onAppear { request.appear() }", SOURCE)
        self.assertIn("ViewRequestTask.disappear(request: $request, pending: $pending)", SOURCE)
        self.assertIn("request.wrappedValue.disappear()", TASK)
        self.assertIn("pending.wrappedValue?.cancel()", TASK)
        self.assertGreaterEqual(SOURCE.count("ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending"), 2)
        self.assertIn("currentID = UUID()", GENERATION)
        self.assertIn("isVisible = false\n    invalidate()", GENERATION)

    def test_download_and_enqueue_fence_late_completions(self):
        self.assertIn("guard request.isVisible, !busy, let version = state.localVersion else { return }", SOURCE)
        self.assertIn("guard request.isVisible, !busy, let preview, let version = expectedVersion else { return }", SOURCE)
        self.assertGreaterEqual(SOURCE.count("try request.checkCurrent(id)"), 4)
        self.assertIn("if request.wrappedValue.isCurrent(id)", TASK)
        self.assertIn("pending.wrappedValue = nil", TASK)
        self.assertIn("try Task.checkCancellation()", GENERATION)
        self.assertIn("guard isCurrent(id) else { throw CancellationError() }", GENERATION)


if __name__ == "__main__":
    unittest.main()
