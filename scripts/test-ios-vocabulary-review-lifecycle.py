#!/usr/bin/env python3
"""Contract checks for vocabulary review work stopping after the page disappears."""
from pathlib import Path
import unittest

SOURCE = Path("platforms/ios/App/Sources/settings/VocabularyReviewSettingsView.swift").read_text()


class VocabularyReviewLifecycleTests(unittest.TestCase):
    def test_review_page_owns_and_invalidates_background_work(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", SOURCE)
        self.assertIn("@State private var visible = false", SOURCE)
        self.assertIn("@State private var generation = UUID()", SOURCE)
        self.assertIn(".onDisappear", SOURCE)
        self.assertIn("pending?.cancel()", SOURCE)
        self.assertIn("generation = UUID()", SOURCE)

    def test_detached_results_require_current_page_generation(self):
        self.assertGreaterEqual(SOURCE.count("guard visible, generation == id, !Task.isCancelled else { return }"), 2)
        self.assertIn("catch is CancellationError", SOURCE)


if __name__ == "__main__":
    unittest.main()
