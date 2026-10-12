#!/usr/bin/env python3
"""Contract checks for page-owned community editor operations."""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/community/CommunityResourcesView.swift").read_text()


class CommunityResourceEditorLifecycleTests(unittest.TestCase):
    def test_editor_owns_visibility_and_request_generation(self):
        editor = SOURCE.split("struct CommunityResourceEditor", 1)[1]
        self.assertIn("@State private var requestID = UUID()", editor)
        self.assertIn("@State private var visible = false", editor)
        self.assertIn("@State private var pending: Task<Void, Never>?", editor)
        self.assertIn(".onAppear { visible = true }", editor)
        self.assertIn("visible = false", editor)

    def test_import_and_publish_share_cancellation_fence(self):
        editor = SOURCE.split("struct CommunityResourceEditor", 1)[1]
        self.assertIn("run(_ action: @escaping @MainActor (UUID) async throws -> Void)", editor)
        self.assertIn("guard visible, requestID == id else { throw CancellationError() }", editor)
        self.assertGreaterEqual(editor.count("try ensureCurrent(id)"), 2)
        self.assertGreaterEqual(editor.count("run { id in"), 2)


if __name__ == "__main__":
    unittest.main()
