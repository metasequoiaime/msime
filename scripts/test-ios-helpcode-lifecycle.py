#!/usr/bin/env python3
"""辅助码页的导入、删除和目录刷新必须在页面消失后失效。"""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "platforms/ios/App/Sources/settings/HelpcodeSettingsView.swift").read_text()
GENERATION = (ROOT / "platforms/ios/App/Sources/app/ViewRequestGeneration.swift").read_text()
TASK = (ROOT / "platforms/ios/App/Sources/app/ViewRequestTask.swift").read_text()


class HelpcodeLifecycleTests(unittest.TestCase):
    def test_page_owns_and_cancels_pending_work(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", SOURCE)
        self.assertIn("@State private var request = ViewRequestGeneration()", SOURCE)
        self.assertIn("request.appear()", SOURCE)
        self.assertIn("ViewRequestTask.disappear(request: $request, pending: $pending)", SOURCE)
        self.assertIn(".onDisappear", SOURCE)
        self.assertIn("request.wrappedValue.disappear()", TASK)
        self.assertIn("pending.wrappedValue?.cancel()", TASK)
        self.assertIn("pending.wrappedValue = nil", TASK)

    def test_keyboard_test_target_includes_the_helpcode_request_dependency(self):
        project = (ROOT / "platforms/ios/project.yml").read_text()
        target = project.rsplit("  MSIMEKeyboardTests:", 1)[1].split("  MSIMEServiceTests:", 1)[0]
        self.assertIn("- path: App/Sources/settings/HelpcodeSettingsView.swift", target)
        self.assertIn("- path: App/Sources/app/ViewRequestGeneration.swift", target)
        self.assertIn("- path: App/Sources/app/ViewRequestTask.swift", target)

    def test_all_async_results_require_current_page_lifecycle(self):
        self.assertGreaterEqual(SOURCE.count("guard (try? request.checkCurrent(id)) != nil else { return }"), 6)
        self.assertEqual(SOURCE.count("ViewRequestTask.claim(request: $request, pending: $pending)"), 3)
        self.assertIn("ViewRequestTask.clear(request: $request, pending: $pending, id: id)", SOURCE)
        self.assertIn("return request.wrappedValue.begin()", TASK)
        self.assertIn("if request.wrappedValue.isCurrent(id) { pending.wrappedValue = nil }", TASK)
        self.assertIn("try Task.checkCancellation()", GENERATION)


if __name__ == "__main__":
    unittest.main()
