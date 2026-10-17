#!/usr/bin/env python3
"""校验社区资源编辑页的请求代次和离页清理契约。"""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/community/CommunityResourcesView.swift").read_text()
TASK = Path("platforms/ios/App/Sources/app/ViewRequestTask.swift")


class CommunityResourceEditorLifecycleTests(unittest.TestCase):
    def test_editor_owns_visibility_and_request_generation(self):
        editor = SOURCE.split("struct CommunityResourceEditor", 1)[1]
        self.assertIn("@State private var request = ViewRequestGeneration()", editor)
        self.assertIn("@State private var pending: Task<Void, Never>?", editor)
        self.assertIn(".trackViewRequest(request: $request, pending: $pending, onDisappear: {", editor)
        self.assertNotIn(".onAppear { request.appear() }", editor)
        self.assertNotIn("ViewRequestTask.disappear(request: $request, pending: $pending)", editor)

    def test_import_and_publish_share_cancellation_fence(self):
        editor = SOURCE.split("struct CommunityResourceEditor", 1)[1]
        self.assertIn("run(_ action: @escaping @MainActor (UUID) async throws -> Void)", editor)
        self.assertIn("ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending", editor)
        self.assertGreaterEqual(editor.count("try request.checkCurrent(id)"), 2)
        self.assertGreaterEqual(editor.count("run { id in"), 2)
        self.assertIn("CommunityUnseenItems.filter(existing: words, incoming: imported.entries)", editor)
        self.assertIn("let id = request.wrappedValue.begin()", TASK.read_text())


if __name__ == "__main__":
    unittest.main()
