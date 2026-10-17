#!/usr/bin/env python3
"""校验社区资源详情页的请求代次和离页清理契约。"""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/community/CommunityResourcesView.swift").read_text()
TASK = Path("platforms/ios/App/Sources/app/ViewRequestTask.swift")


class CommunityResourceDetailLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.detail = SOURCE.split("struct CommunityResourceDetail", 1)[1].split(
            "private extension PersonalWordKind", 1
        )[0]

    def test_detail_owns_visibility_and_pending_work(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", self.detail)
        self.assertIn("@State private var request = ViewRequestGeneration()", self.detail)
        self.assertIn(".trackViewRequest(request: $request, pending: $pending, onDisappear: {", self.detail)
        self.assertNotIn(".onAppear { request.appear()", self.detail)
        self.assertNotIn("ViewRequestTask.disappear(request: $request, pending: $pending)", self.detail)

    def test_detail_actions_fence_late_results(self):
        self.assertIn("run(_ action: @escaping @MainActor (UUID) async throws -> Void)", self.detail)
        self.assertIn("ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending", self.detail)
        self.assertGreaterEqual(self.detail.count("try request.checkCurrent(id)"), 5)
        helper = TASK.read_text()
        self.assertIn("let id = request.wrappedValue.begin()", helper)
        self.assertIn("catch is CancellationError", helper)


if __name__ == "__main__":
    unittest.main()
