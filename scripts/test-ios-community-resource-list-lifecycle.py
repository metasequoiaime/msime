#!/usr/bin/env python3
"""校验社区资源列表的请求代次和离页清理契约。"""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/community/CommunityResourcesView.swift").read_text()
TASK = Path("platforms/ios/App/Sources/app/ViewRequestTask.swift").read_text()


class CommunityResourceListLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.view = SOURCE.split("struct CommunityResourcesView", 1)[1].split(
            "struct CommunityResourceDetail", 1
        )[0]
        self.reply = SOURCE.split("struct CommunityReplyTemplatesSection", 1)[1].split(
            "struct CommunityResourceDetail", 1
        )[0]

    def test_list_owns_visibility_and_pending_work(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", self.view)
        self.assertIn("@State private var request = ViewRequestGeneration()", self.view)
        self.assertIn(".trackViewRequest(request: $request, pending: $pending, onAppear: {", self.view)
        self.assertIn("Task { await refreshInstalled() }", self.view)
        self.assertNotIn(".onAppear { request.appear(); Task", self.view)
        self.assertNotIn("ViewRequestTask.disappear(request: $request, pending: $pending)", self.view)
        self.assertIn(".trackViewRequest(request: $request, pending: $pending, onAppear: {", self.reply)
        self.assertNotIn("ViewRequestTask.disappear(request: $request, pending: $pending)", self.reply)
        self.assertIn("request.wrappedValue.disappear()", TASK)
        self.assertIn("pending.wrappedValue?.cancel()", TASK)
        self.assertIn("pending.wrappedValue = nil", TASK)

    def test_late_list_results_are_fenced_after_disappearance(self):
        self.assertIn("ViewRequestTask.startLoad(request: $request, pending: $pending) { id in", self.view)
        self.assertIn("CommunityUnseenItems.filter(existing: items, incoming: page.items)", self.view)
        self.assertIn("guard request.isCurrent(id) else { return }", self.view)
        self.assertIn("try request.checkCurrent(id)", self.view)
        self.assertIn("ViewRequestTask.startLoad(request: $request, pending: $pending) { id in", self.reply)
        self.assertIn("if request.wrappedValue.isCurrent(id) { pending.wrappedValue = nil }", TASK)
        self.assertIn("onPublished: { startLoad() }", self.view)


if __name__ == "__main__":
    unittest.main()
