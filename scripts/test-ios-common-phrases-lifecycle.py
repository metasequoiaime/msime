#!/usr/bin/env python3
"""常用语页的异步读写必须在页面消失后失效。"""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
VIEW = (ROOT / "platforms/ios/App/Sources/settings/CommonPhrasesSettingsView.swift").read_text()
STATE = (ROOT / "platforms/ios/SharedUI/dictionary/CommonPhrasesRequestState.swift").read_text()
GENERATION = (ROOT / "platforms/ios/App/Sources/app/ViewRequestGeneration.swift").read_text()
TASK = (ROOT / "platforms/ios/App/Sources/app/ViewRequestTask.swift").read_text()


class CommonPhrasesLifecycleTests(unittest.TestCase):
    def test_page_owns_and_invalidates_pending_work(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", VIEW)
        self.assertIn("@State private var request = ViewRequestGeneration()", VIEW)
        self.assertIn("request.appear()", VIEW)
        self.assertIn("ViewRequestTask.disappear(request: $request, pending: $pending)", VIEW)
        self.assertIn(".onDisappear", VIEW)
        self.assertIn("request.wrappedValue.disappear()", TASK)
        self.assertIn("pending.wrappedValue?.cancel()", TASK)
        self.assertIn("pending.wrappedValue = nil", TASK)
        self.assertIn("requestState.invalidate()", VIEW)
        self.assertIn("mutating func invalidate()", STATE)

    def test_async_results_require_current_page_lifecycle(self):
        self.assertGreaterEqual(VIEW.count("try request.checkCurrent(id)"), 4)
        self.assertEqual(VIEW.count("ViewRequestTask.claim(request: $request, pending: $pending)"), 2)
        self.assertIn("ViewRequestTask.clear(request: $request, pending: $pending, id: id)", VIEW)
        self.assertIn("return request.wrappedValue.begin()", TASK)
        self.assertIn("if request.wrappedValue.isCurrent(id) { pending.wrappedValue = nil }", TASK)
        self.assertIn("catch is CancellationError", VIEW)
        self.assertIn("try Task.checkCancellation()", GENERATION)


if __name__ == "__main__":
    unittest.main()
