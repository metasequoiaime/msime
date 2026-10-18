#!/usr/bin/env python3
"""离开 iOS 云剪贴板页后，旧任务不能卡住或覆盖重建页面。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/keyboard/CloudClipboardView.swift"
REFRESH = ROOT / "platforms/ios/App/Sources/keyboard/IOSCloudClipboard.swift"
GENERATION = ROOT / "platforms/ios/App/Sources/app/ViewRequestGeneration.swift"
TASK = ROOT / "platforms/ios/App/Sources/app/ViewRequestTask.swift"


class IOSCloudClipboardViewLifecycleContract(unittest.TestCase):
    def test_disappear_invalidates_pending_operation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("@State private var request = ViewRequestGeneration()", source)
        self.assertIn(".task { run { _ in nil } }", source)
        self.assertIn(".trackViewRequest(request: $request, pending: $pending, onDisappear: {", source)
        self.assertNotIn("ViewRequestTask.disappear(request: $request, pending: $pending)", source)
        self.assertIn("busy = false", source)
        task = TASK.read_text(encoding="utf-8")
        self.assertIn("request.wrappedValue.disappear()", task)
        self.assertIn("pending.wrappedValue?.cancel()", task)
        self.assertIn("pending.wrappedValue = nil", task)

    def test_results_are_bound_to_the_operation_generation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("ViewRequestTask.run(request: $request, busy: $busy, message: $message,", source)
        self.assertIn("pending: $pending", source)
        self.assertNotIn("let id = request.begin()", source)
        self.assertRegex(source, r"execute\(action, requestID: id\)")
        execute = re.search(r"private func execute\(.*?\n  \}", source, re.S)
        self.assertIsNotNone(execute, "could not locate CloudClipboardView.execute")
        body = execute.group(0)
        # 操作、列表和设备状态在 Rust 那边一次刷新里做完（#8201 4c），页面只有这一个 await：它回来以后先核对代次再写状态，每个失败分支也先核对。
        self.assertRegex(body, r"try await clipboard\.refresh\(boundTo: accountID, search: search\)[^\n]*\n\s*try request\.checkCurrent\(id\)\n\s*accountID = result\.accountID")
        self.assertEqual(body.count("await "), 2, "a new await in execute needs its own generation check")
        self.assertGreaterEqual(body.count("guard request.isCurrent(id) else { return }"), 4)
        # 换号、退出在请求前后都要核对，结果才不会落到另一个账号的页面上。
        refresh = re.search(r"func refresh\(boundTo.*?\n  \}", REFRESH.read_text(encoding="utf-8"), re.S)
        self.assertIsNotNone(refresh, "could not locate IOSCloudClipboard.refresh")
        refresh = refresh.group(0)
        before = refresh.index("guard let current = try await account.status()?.id, accountID == nil || accountID == current else { throw CancellationError() }")
        after = refresh.index("guard try await account.status()?.id == current else { throw CancellationError() }")
        self.assertLess(before, refresh.index("try await action(send)"))
        self.assertGreater(after, refresh.index('"operation": "list"'))
        generation = GENERATION.read_text(encoding="utf-8")
        self.assertIn("try Task.checkCancellation()", generation)
        self.assertIn("guard isCurrent(id) else { throw CancellationError() }", generation)
        self.assertIn("if request.wrappedValue.isCurrent(id)", TASK.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
