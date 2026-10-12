#!/usr/bin/env python3
"""Contract checks for page-owned community resource list operations."""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/community/CommunityResourcesView.swift").read_text()


class CommunityResourceListLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.view = SOURCE.split("struct CommunityResourcesView", 1)[1].split(
            "struct CommunityResourceDetail", 1
        )[0]

    def test_list_owns_visibility_and_pending_work(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", self.view)
        self.assertIn("@State private var visible = false", self.view)
        self.assertIn(".onAppear { visible = true; Task", self.view)
        self.assertIn("pending?.cancel()", self.view)
        self.assertIn("requestID = UUID()", self.view)

    def test_late_list_results_are_fenced_after_disappearance(self):
        self.assertIn("guard visible, requestID == id else { throw CancellationError() }", self.view)
        self.assertIn("try ensureCurrent(id)", self.view)
        self.assertIn("pending = Task", self.view)
        self.assertIn("onPublished: { startLoad() }", self.view)


if __name__ == "__main__":
    unittest.main()
