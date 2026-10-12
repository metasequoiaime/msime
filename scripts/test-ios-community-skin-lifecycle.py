#!/usr/bin/env python3
"""Contract checks for page-owned community skin operations."""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/community/SkinCommunityView.swift").read_text()


class CommunitySkinLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.list_view = SOURCE.split("struct SkinCommunityView", 1)[1].split(
            "private struct CommunitySkinCard", 1
        )[0]
        self.detail = SOURCE.split("struct CommunitySkinDetail", 1)[1].split(
            "struct CommunityPublishView", 1
        )[0]
        self.publish = SOURCE.split("struct CommunityPublishView", 1)[1].split(
            "// A value-based preview", 1
        )[0]

    def test_list_fences_late_load_and_install_results(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", self.list_view)
        self.assertIn("@State private var visible = false", self.list_view)
        self.assertIn("@State private var takeTasks: [String: Task<Void, Never>] = [:]", self.list_view)
        self.assertIn(".onAppear { visible = true", self.list_view)
        self.assertIn(".onDisappear", self.list_view)
        self.assertIn("pending?.cancel()", self.list_view)
        self.assertIn("takeTasks.values.forEach { $0.cancel() }", self.list_view)
        self.assertIn("guard visible, requestID == id else { throw CancellationError() }", self.list_view)
        self.assertIn("try ensureCurrent(id)", self.list_view)
        self.assertIn("catch is CancellationError", self.list_view)

    def test_detail_fences_late_actions(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", self.detail)
        self.assertIn("@State private var visible = false", self.detail)
        self.assertIn(".onDisappear", self.detail)
        self.assertIn("pending?.cancel()", self.detail)
        self.assertIn("run(_ action: @escaping @MainActor (UUID) async throws -> Void)", self.detail)
        self.assertIn("guard visible, requestID == id else { throw CancellationError() }", self.detail)
        self.assertGreaterEqual(self.detail.count("try ensureCurrent(id)"), 4)
        self.assertIn("catch is CancellationError", self.detail)

    def test_publish_cancels_after_leaving(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", self.publish)
        self.assertIn("@State private var visible = false", self.publish)
        self.assertIn(".onDisappear", self.publish)
        self.assertIn("pending?.cancel()", self.publish)
        self.assertIn("guard visible, requestID == id else { throw CancellationError() }", self.publish)
        self.assertIn("catch is CancellationError", self.publish)


if __name__ == "__main__":
    unittest.main()
