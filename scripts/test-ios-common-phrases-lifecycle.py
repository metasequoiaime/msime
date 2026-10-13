#!/usr/bin/env python3
"""常用语页的异步读写必须在页面消失后失效。"""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
VIEW = (ROOT / "platforms/ios/App/Sources/settings/CommonPhrasesSettingsView.swift").read_text()
STATE = (ROOT / "platforms/ios/SharedUI/dictionary/CommonPhrasesRequestState.swift").read_text()


class CommonPhrasesLifecycleTests(unittest.TestCase):
    def test_page_owns_and_invalidates_pending_work(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", VIEW)
        self.assertIn("@State private var visible = false", VIEW)
        self.assertIn("@State private var lifecycle = UUID()", VIEW)
        self.assertIn(".onDisappear", VIEW)
        self.assertIn("pending?.cancel()", VIEW)
        self.assertIn("requestState.invalidate()", VIEW)
        self.assertIn("mutating func invalidate()", STATE)

    def test_async_results_require_current_page_lifecycle(self):
        guard = "guard visible, lifecycle == id, !Task.isCancelled else { return }"
        self.assertGreaterEqual(VIEW.count(guard), 2)
        self.assertIn("catch is CancellationError", VIEW)


if __name__ == "__main__":
    unittest.main()
