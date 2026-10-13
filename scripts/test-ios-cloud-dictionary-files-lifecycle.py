#!/usr/bin/env python3
"""校验云词库文件页的请求代次与离页清理契约。"""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/dictionary/CloudDictionaryFilesView.swift").read_text()
GENERATION = Path("platforms/ios/App/Sources/app/ViewRequestGeneration.swift").read_text()


class CloudDictionaryFilesLifecycleTests(unittest.TestCase):
    def test_page_owns_task_and_shared_generation(self):
        self.assertIn("@State private var request = ViewRequestGeneration()", SOURCE)
        self.assertIn(".onAppear { request.appear() }", SOURCE)
        self.assertIn("request.disappear()\n      pending?.cancel()", SOURCE)
        self.assertIn("let id = request.begin()", SOURCE)
        self.assertIn("isVisible = false\n    currentID = UUID()", GENERATION)

    def test_run_fences_late_work_and_cleanup(self):
        self.assertIn("run(_ action: @escaping @MainActor (UUID) async throws -> Void)", SOURCE)
        self.assertIn("guard request.isVisible else { return }", SOURCE)
        self.assertIn("if request.isCurrent(id) {\n          busy = false\n          pending = nil", SOURCE)
        self.assertIn("guard isCurrent(id) else { throw CancellationError() }", GENERATION)

    def test_async_state_writes_check_the_current_generation(self):
        self.assertGreaterEqual(SOURCE.count("try request.checkCurrent(id)"), 6)
        self.assertIn("try Task.checkCancellation()", GENERATION)


if __name__ == "__main__":
    unittest.main()
