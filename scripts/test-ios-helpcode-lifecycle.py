#!/usr/bin/env python3
"""辅助码页的导入、删除和目录刷新必须在页面消失后失效。"""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "platforms/ios/App/Sources/settings/HelpcodeSettingsView.swift").read_text()
GENERATION = (ROOT / "platforms/ios/App/Sources/app/ViewRequestGeneration.swift").read_text()


class HelpcodeLifecycleTests(unittest.TestCase):
    def test_page_owns_and_cancels_pending_work(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", SOURCE)
        self.assertIn("@State private var request = ViewRequestGeneration()", SOURCE)
        self.assertIn("request.appear()", SOURCE)
        self.assertIn("request.disappear()", SOURCE)
        self.assertIn(".onDisappear", SOURCE)
        self.assertIn("pending?.cancel()", SOURCE)

    def test_all_async_results_require_current_page_lifecycle(self):
        self.assertGreaterEqual(SOURCE.count("guard (try? request.checkCurrent(id)) != nil else { return }"), 6)
        self.assertIn("return request.begin()", SOURCE)
        self.assertIn("if request.isCurrent(id) { pending = nil }", SOURCE)
        self.assertIn("try Task.checkCancellation()", GENERATION)


if __name__ == "__main__":
    unittest.main()
