#!/usr/bin/env python3
"""校验云词库文件页的请求代次与离页清理契约。"""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/dictionary/CloudDictionaryFilesView.swift").read_text()
GENERATION = Path("platforms/ios/App/Sources/app/ViewRequestGeneration.swift").read_text()
TASK = Path("platforms/ios/App/Sources/app/ViewRequestTask.swift").read_text()
DICTIONARY = Path("platforms/ios/App/Sources/dictionary/IOSCloudDictionary.swift").read_text()


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
        # 页面按打开时的 user.id 绑定（#8201 4c）：每个远端请求都经 IOSCloudDictionary，它在请求前后核对账号，退出或换号时是 CancellationError。
        self.assertIn("private var dictionary: IOSCloudDictionary { IOSCloudDictionary(accountID: accountID) }", SOURCE)
        self.assertNotIn("BackendRustAccount.current()", SOURCE)
        self.assertNotIn("account.cloudDictionary(", SOURCE)
        self.assertGreaterEqual(SOURCE.count("try await dictionary.object(") + SOURCE.count("try await dictionary.request("), 5)
        self.assertRegex(DICTIONARY, r"try await requireAccount\(account\)\n\s*let data = try await account\.cloudDictionary\(action, snapshotDirectory: try Self\.transferDirectory\(\)\)\n\s*try await requireAccount\(account\)")
        self.assertIn("guard current == accountID else { throw CancellationError() }", DICTIONARY)
        self.assertIn("if request.wrappedValue.isCurrent(id)", TASK)
        self.assertIn("busy.wrappedValue = false", TASK)
        self.assertIn("pending.wrappedValue = nil", TASK)
        self.assertIn("guard isCurrent(id) else { throw CancellationError() }", GENERATION)

    def test_async_state_writes_check_the_current_generation(self):
        self.assertGreaterEqual(SOURCE.count("try request.checkCurrent(id)"), 6)
        self.assertIn("try Task.checkCancellation()", GENERATION)


if __name__ == "__main__":
    unittest.main()
