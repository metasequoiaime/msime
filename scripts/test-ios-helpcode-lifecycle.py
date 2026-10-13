#!/usr/bin/env python3
"""辅助码页的导入、删除和目录刷新必须在页面消失后失效。"""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "platforms/ios/App/Sources/settings/HelpcodeSettingsView.swift").read_text()


class HelpcodeLifecycleTests(unittest.TestCase):
    def test_page_owns_and_cancels_pending_work(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", SOURCE)
        self.assertIn("@State private var visible = false", SOURCE)
        self.assertIn("@State private var lifecycle = UUID()", SOURCE)
        self.assertIn(".onDisappear", SOURCE)
        self.assertIn("pending?.cancel()", SOURCE)

    def test_all_async_results_require_current_page_lifecycle(self):
        guard = "guard visible, lifecycle == id, !Task.isCancelled else { return }"
        self.assertGreaterEqual(SOURCE.count(guard), 3)
        self.assertGreaterEqual(SOURCE.count("guard !Task.isCancelled else { return }"), 3)


if __name__ == "__main__":
    unittest.main()
