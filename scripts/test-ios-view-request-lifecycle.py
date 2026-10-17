#!/usr/bin/env python3
"""确保页面请求的出现、失效和任务取消由共享 View modifier 统一处理。"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
TASK = ROOT / "platforms/ios/App/Sources/app/ViewRequestTask.swift"
TARGETS = [
    ROOT / "platforms/ios/App/Sources/dictionary/CloudDictionaryFilesView.swift",
    ROOT / "platforms/ios/App/Sources/dictionary/CloudDictionaryApplyView.swift",
    ROOT / "platforms/ios/App/Sources/voice/AISkinGenerationView.swift",
]


class ViewRequestLifecycleContract(unittest.TestCase):
    def test_modifier_owns_common_visibility_lifecycle(self):
        source = TASK.read_text()
        self.assertIn("struct ViewRequestLifecycle: ViewModifier", source)
        self.assertIn(".onAppear {", source)
        self.assertIn("request.appear()", source)
        self.assertIn("onAppear()", source)
        self.assertIn("ViewRequestTask.disappear(request: $request, pending: $pending)", source)
        self.assertIn("func trackViewRequest(", source)

    def test_migrated_pages_use_shared_lifecycle(self):
        for path in TARGETS:
            source = path.read_text()
            self.assertIn(".trackViewRequest(request: $request, pending: $pending, onDisappear: {", source, path.name)
            self.assertNotIn(".onAppear { request.appear() }", source, path.name)
            self.assertNotIn("ViewRequestTask.disappear(request: $request, pending: $pending)", source, path.name)


if __name__ == "__main__":
    unittest.main()
