#!/usr/bin/env python3
"""校验内嵌社区回复模板区的请求和添加任务生命周期。"""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/community/CommunityResourcesView.swift").read_text()


class CommunityReplySectionLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.section = SOURCE.split("struct CommunityReplyTemplatesSection", 1)[1].split(
            "struct CommunityResourceDetail", 1
        )[0]

    def test_section_owns_visibility_and_pending_work(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", self.section)
        self.assertIn("@State private var addTasks: [String: Task<Void, Never>]", self.section)
        self.assertIn("@State private var request = ViewRequestGeneration()", self.section)
        self.assertIn(".onAppear { request.appear()", self.section)
        self.assertIn(".onDisappear", self.section)
        self.assertIn("pending?.cancel()", self.section)
        self.assertIn("addTasks.values.forEach { $0.cancel() }", self.section)
        self.assertIn("adding.removeAll()", self.section)

    def test_late_section_results_are_fenced(self):
        self.assertIn("guard request.isVisible, addGeneration == generation else { throw CancellationError() }", self.section)
        self.assertIn("try ensureVisible(generation)", self.section)
        self.assertIn("catch is CancellationError", self.section)
        self.assertIn("if request.isCurrent(id) { pending = nil }", self.section)

    def test_reply_install_checks_cancellation_between_side_effects(self):
        helper = SOURCE.split("enum CommunityReplyInstall", 1)[1].split(
            "struct CommunityReplyTemplatesSection", 1
        )[0]
        self.assertGreaterEqual(helper.count("try Task.checkCancellation()"), 2)


if __name__ == "__main__":
    unittest.main()
