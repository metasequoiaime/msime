#!/usr/bin/env python3
"""校验社区资源列表的请求代次和离页清理契约。"""
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
        self.assertIn("@State private var request = ViewRequestGeneration()", self.view)
        self.assertIn(".onAppear { request.appear(); Task", self.view)
        self.assertIn("pending?.cancel()", self.view)
        self.assertIn("request.disappear()", self.view)

    def test_late_list_results_are_fenced_after_disappearance(self):
        self.assertIn("let id = request.begin(); busy = true", self.view)
        self.assertIn("try request.checkCurrent(id)", self.view)
        self.assertIn("pending = Task", self.view)
        self.assertIn("onPublished: { startLoad() }", self.view)


if __name__ == "__main__":
    unittest.main()
