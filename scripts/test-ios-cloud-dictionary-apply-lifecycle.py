#!/usr/bin/env python3
"""Contract checks for page-owned cloud dictionary apply operations."""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/dictionary/CloudDictionaryApplyView.swift").read_text()


class CloudDictionaryApplyLifecycleTests(unittest.TestCase):
    def test_page_owns_generation_and_visibility(self):
        self.assertIn("@State private var requestID = UUID()", SOURCE)
        self.assertIn("@State private var visible = false", SOURCE)
        self.assertIn(".onAppear { visible = true }", SOURCE)
        self.assertIn("visible = false", SOURCE)
        self.assertGreaterEqual(SOURCE.count("requestID = UUID()"), 2)

    def test_download_and_enqueue_fence_late_completions(self):
        self.assertIn("guard visible, !busy, let version = state.localVersion else { return }", SOURCE)
        self.assertIn("guard visible, !busy, let preview, let version = expectedVersion else { return }", SOURCE)
        self.assertIn("guard visible, requestID == id else { throw CancellationError() }", SOURCE)
        self.assertGreaterEqual(SOURCE.count("try ensureCurrent(id)"), 4)
        self.assertGreaterEqual(SOURCE.count("if requestID == id"), 2)
        self.assertGreaterEqual(SOURCE.count("pending = nil"), 3)


if __name__ == "__main__":
    unittest.main()
