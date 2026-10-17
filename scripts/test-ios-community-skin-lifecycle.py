#!/usr/bin/env python3
"""校验社区皮肤列表、详情和发布页的请求生命周期。"""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/community/SkinCommunityView.swift").read_text()
TASK = Path("platforms/ios/App/Sources/app/ViewRequestTask.swift")


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
        self.assertIn("@State private var request = ViewRequestGeneration()", self.list_view)
        self.assertIn("@State private var takeTasks: [String: Task<Void, Never>] = [:]", self.list_view)
        self.assertIn("@State private var takeGeneration = OperationGeneration()", self.list_view)
        self.assertIn("@State private var accountGeneration = OperationGeneration()", self.list_view)
        self.assertIn("takeGeneration.invalidate()", self.list_view)
        self.assertIn("accountGeneration.invalidate()", self.list_view)
        self.assertIn(".trackViewRequest(request: $request, pending: $pending, onAppear: {", self.list_view)
        self.assertIn("library = CustomSkinLibrary.designs", self.list_view)
        self.assertNotIn("ViewRequestTask.disappear(request: $request, pending: $pending)", self.list_view)
        self.assertIn("ViewRequestTask.startLoad(request: $request, pending: $pending) { id in", self.list_view)
        self.assertIn("CommunityUnseenItems.filter(existing: skins, incoming: page.skins)", self.list_view)
        self.assertIn("if scheduledID == nil { pending = nil }", self.list_view)
        self.assertIn("takeTasks.values.forEach { $0.cancel() }", self.list_view)
        self.assertIn("guard request.isCurrent(id) else { throw CancellationError() }", self.list_view)
        self.assertIn("try request.checkCurrent(id)", self.list_view)
        self.assertIn("catch is CancellationError", self.list_view)

    def test_detail_fences_late_actions(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", self.detail)
        self.assertIn("@State private var request = ViewRequestGeneration()", self.detail)
        self.assertIn(".trackViewRequest(request: $request, pending: $pending, onDisappear: {", self.detail)
        self.assertNotIn("ViewRequestTask.disappear(request: $request, pending: $pending)", self.detail)
        self.assertIn("run(_ action: @escaping @MainActor (UUID) async throws -> Void)", self.detail)
        self.assertIn("ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending", self.detail)
        self.assertGreaterEqual(self.detail.count("Button { run { id in"), 3)
        self.assertGreaterEqual(self.detail.count("try request.checkCurrent(id)"), 4)
        helper = TASK.read_text()
        self.assertIn("let id = request.wrappedValue.begin()", helper)
        self.assertIn("catch is CancellationError", helper)
        self.assertIn("request.wrappedValue.disappear()", helper)
        self.assertIn("pending.wrappedValue?.cancel()", helper)
        self.assertIn("pending.wrappedValue = nil", helper)

    def test_publish_cancels_after_leaving(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", self.publish)
        self.assertIn("@State private var request = ViewRequestGeneration()", self.publish)
        self.assertIn(".trackViewRequest(request: $request, pending: $pending, onAppear: {", self.publish)
        self.assertIn("onDisappear: {", self.publish)
        self.assertNotIn("ViewRequestTask.disappear(request: $request, pending: $pending)", self.publish)
        self.assertIn("ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending", self.publish)
        self.assertIn("catch is CancellationError", TASK.read_text())


if __name__ == "__main__":
    unittest.main()
