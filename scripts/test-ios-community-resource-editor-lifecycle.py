#!/usr/bin/env python3
"""校验社区资源编辑页的请求代次和离页清理契约。"""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/community/CommunityResourcesView.swift").read_text()


class CommunityResourceEditorLifecycleTests(unittest.TestCase):
    def test_editor_owns_visibility_and_request_generation(self):
        editor = SOURCE.split("struct CommunityResourceEditor", 1)[1]
        self.assertIn("@State private var request = ViewRequestGeneration()", editor)
        self.assertIn("@State private var pending: Task<Void, Never>?", editor)
        self.assertIn(".onAppear { request.appear() }", editor)
        self.assertIn("request.disappear()", editor)

    def test_import_and_publish_share_cancellation_fence(self):
        editor = SOURCE.split("struct CommunityResourceEditor", 1)[1]
        self.assertIn("run(_ action: @escaping @MainActor (UUID) async throws -> Void)", editor)
        self.assertIn("let id = request.begin()", editor)
        self.assertGreaterEqual(editor.count("try request.checkCurrent(id)"), 2)
        self.assertGreaterEqual(editor.count("run { id in"), 2)


if __name__ == "__main__":
    unittest.main()
