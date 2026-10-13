#!/usr/bin/env python3
"""校验云词库应用页的请求代次与离页清理契约。"""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/dictionary/CloudDictionaryApplyView.swift").read_text()
GENERATION = Path("platforms/ios/App/Sources/app/ViewRequestGeneration.swift").read_text()


class CloudDictionaryApplyLifecycleTests(unittest.TestCase):
    def test_page_owns_task_and_shared_generation(self):
        self.assertIn("@State private var request = ViewRequestGeneration()", SOURCE)
        self.assertIn(".onAppear { request.appear() }", SOURCE)
        self.assertIn("request.disappear()\n      pending?.cancel()", SOURCE)
        self.assertGreaterEqual(SOURCE.count("let id = request.begin()"), 2)
        self.assertIn("currentID = UUID()", GENERATION)
        self.assertIn("isVisible = false\n    invalidate()", GENERATION)

    def test_download_and_enqueue_fence_late_completions(self):
        self.assertIn("guard request.isVisible, !busy, let version = state.localVersion else { return }", SOURCE)
        self.assertIn("guard request.isVisible, !busy, let preview, let version = expectedVersion else { return }", SOURCE)
        self.assertGreaterEqual(SOURCE.count("try request.checkCurrent(id)"), 4)
        self.assertGreaterEqual(SOURCE.count("if request.isCurrent(id)"), 4)
        self.assertGreaterEqual(SOURCE.count("pending = nil"), 3)
        self.assertIn("try Task.checkCancellation()", GENERATION)
        self.assertIn("guard isCurrent(id) else { throw CancellationError() }", GENERATION)


if __name__ == "__main__":
    unittest.main()
