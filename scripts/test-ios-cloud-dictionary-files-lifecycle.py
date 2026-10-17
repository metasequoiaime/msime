#!/usr/bin/env python3
"""校验云词库文件页的请求代次与离页清理契约。"""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/dictionary/CloudDictionaryFilesView.swift").read_text()
GENERATION = Path("platforms/ios/App/Sources/app/ViewRequestGeneration.swift").read_text()
TASK = Path("platforms/ios/App/Sources/app/ViewRequestTask.swift").read_text()


class CloudDictionaryFilesLifecycleTests(unittest.TestCase):
    def test_page_owns_task_and_shared_generation(self):
        self.assertIn("@State private var request = ViewRequestGeneration()", SOURCE)
        self.assertIn(".trackViewRequest(request: $request, pending: $pending, onDisappear: {", SOURCE)
        self.assertNotIn(".onAppear { request.appear() }", SOURCE)
        self.assertNotIn("ViewRequestTask.disappear(request: $request, pending: $pending)", SOURCE)
        self.assertIn("request.wrappedValue.disappear()", TASK)
        self.assertIn("pending.wrappedValue?.cancel()", TASK)
        self.assertIn("ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending", SOURCE)
        self.assertIn("isVisible = false\n    invalidate()", GENERATION)

    def test_run_fences_late_work_and_cleanup(self):
        self.assertIn("run(_ action: @escaping @MainActor (UUID) async throws -> Void)", SOURCE)
        self.assertIn("guard request.isVisible else { return }", SOURCE)
        self.assertIn("try await session.credentials(matchingUserID: accountID, matchingSessionID: sessionID)", SOURCE)
        self.assertIn("if request.wrappedValue.isCurrent(id)", TASK)
        self.assertIn("busy.wrappedValue = false", TASK)
        self.assertIn("pending.wrappedValue = nil", TASK)
        self.assertIn("guard isCurrent(id) else { throw CancellationError() }", GENERATION)

    def test_async_state_writes_check_the_current_generation(self):
        self.assertGreaterEqual(SOURCE.count("try request.checkCurrent(id)"), 6)
        self.assertIn("try Task.checkCancellation()", GENERATION)


if __name__ == "__main__":
    unittest.main()
