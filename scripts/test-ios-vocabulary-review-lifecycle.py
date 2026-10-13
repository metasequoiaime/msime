#!/usr/bin/env python3
"""Contract checks for vocabulary review work stopping after the page disappears."""
from pathlib import Path
import unittest

SOURCE = Path("platforms/ios/App/Sources/settings/VocabularyReviewSettingsView.swift").read_text()
GENERATION = Path("platforms/ios/App/Sources/app/ViewRequestGeneration.swift").read_text()
TASK = Path("platforms/ios/App/Sources/app/ViewRequestTask.swift").read_text()


class VocabularyReviewLifecycleTests(unittest.TestCase):
    def test_review_page_owns_and_invalidates_background_work(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", SOURCE)
        self.assertIn("@State private var request = ViewRequestGeneration()", SOURCE)
        self.assertIn("request.appear()", SOURCE)
        self.assertIn(".onDisappear", SOURCE)
        self.assertIn("ViewRequestTask.disappear(request: $request, pending: $pending)", SOURCE)
        self.assertIn("request.wrappedValue.disappear()", TASK)
        self.assertIn("pending.wrappedValue?.cancel()", TASK)
        self.assertIn("pending.wrappedValue = nil", TASK)

    def test_detached_results_require_current_page_generation(self):
        self.assertEqual(SOURCE.count("ViewRequestTask.run(request: $request, busy: $busy, pending: $pending,"), 2)
        self.assertEqual(SOURCE.count("prepare: { message = \"\" }"), 2)
        self.assertNotIn("let id = request.begin()", SOURCE)
        self.assertGreaterEqual(SOURCE.count("try request.checkCurrent(id)"), 2)
        self.assertIn("let id = request.wrappedValue.begin()", TASK)
        self.assertIn("if request.wrappedValue.isCurrent(id)", TASK)
        self.assertIn("try Task.checkCancellation()", GENERATION)
        self.assertIn("catch is CancellationError", TASK)


if __name__ == "__main__":
    unittest.main()
