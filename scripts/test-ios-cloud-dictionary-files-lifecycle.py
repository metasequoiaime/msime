#!/usr/bin/env python3
"""Contract checks for page-owned cloud dictionary file operations."""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/dictionary/CloudDictionaryFilesView.swift").read_text()


class CloudDictionaryFilesLifecycleTests(unittest.TestCase):
    def test_page_owns_an_operation_generation_and_visibility(self):
        self.assertIn("@State private var requestID = UUID()", SOURCE)
        self.assertIn("@State private var visible = false", SOURCE)
        self.assertIn(".onAppear { visible = true }", SOURCE)
        self.assertIn("visible = false", SOURCE)
        self.assertIn("requestID = UUID()", SOURCE)

    def test_run_fences_late_work_and_cleanup(self):
        self.assertIn("run(_ action: @escaping @MainActor (UUID) async throws -> Void)", SOURCE)
        self.assertIn("guard visible else { return }", SOURCE)
        self.assertIn("guard visible, requestID == id else { throw CancellationError() }", SOURCE)
        self.assertIn("if requestID == id {\n          busy = false\n          pending = nil", SOURCE)

    def test_async_state_writes_check_the_current_generation(self):
        self.assertGreaterEqual(SOURCE.count("try ensureCurrent(id)"), 6)
        self.assertIn("try Task.checkCancellation()", SOURCE)


if __name__ == "__main__":
    unittest.main()
