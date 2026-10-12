#!/usr/bin/env python3
"""Contract checks for page-owned community resource detail operations."""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/community/CommunityResourcesView.swift").read_text()


class CommunityResourceDetailLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.detail = SOURCE.split("struct CommunityResourceDetail", 1)[1].split(
            "private extension PersonalWordKind", 1
        )[0]

    def test_detail_owns_visibility_and_pending_work(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", self.detail)
        self.assertIn("@State private var requestID = UUID()", self.detail)
        self.assertIn("@State private var visible = false", self.detail)
        self.assertIn(".onAppear { visible = true", self.detail)
        self.assertIn(".onDisappear", self.detail)
        self.assertIn("pending?.cancel()", self.detail)

    def test_detail_actions_fence_late_results(self):
        self.assertIn("run(_ action: @escaping @MainActor (UUID) async throws -> Void)", self.detail)
        self.assertIn("guard visible, requestID == id else { throw CancellationError() }", self.detail)
        self.assertGreaterEqual(self.detail.count("try ensureCurrent(id)"), 5)
        self.assertIn("catch is CancellationError", self.detail)


if __name__ == "__main__":
    unittest.main()
