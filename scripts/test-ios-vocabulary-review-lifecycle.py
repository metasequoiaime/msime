#!/usr/bin/env python3
"""Contract checks for vocabulary review work stopping after the page disappears."""
from pathlib import Path
import unittest

SOURCE = Path("platforms/ios/App/Sources/settings/VocabularyReviewSettingsView.swift").read_text()
GENERATION = Path("platforms/ios/App/Sources/app/ViewRequestGeneration.swift").read_text()


class VocabularyReviewLifecycleTests(unittest.TestCase):
    def test_review_page_owns_and_invalidates_background_work(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", SOURCE)
        self.assertIn("@State private var request = ViewRequestGeneration()", SOURCE)
        self.assertIn("request.appear()", SOURCE)
        self.assertIn(".onDisappear", SOURCE)
        self.assertIn("pending?.cancel()", SOURCE)
        self.assertIn("request.disappear()", SOURCE)

    def test_detached_results_require_current_page_generation(self):
        self.assertEqual(SOURCE.count("let id = request.begin()"), 2)
        self.assertGreaterEqual(SOURCE.count("try request.checkCurrent(id)"), 2)
        self.assertGreaterEqual(SOURCE.count("request.isCurrent(id)"), 2)
        self.assertIn("try Task.checkCancellation()", GENERATION)
        self.assertIn("catch is CancellationError", SOURCE)


if __name__ == "__main__":
    unittest.main()
