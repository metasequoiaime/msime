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
        self.assertIn(".trackViewRequest(request: $request, pending: $pending, onAppear: {", source)
        self.assertIn("if !loaded { run { _ in nil } }", source)
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
        refresh_source = REFRESH.read_text(encoding="utf-8")
        refresh = re.search(r"func refresh\(boundTo.*?\n  \}", refresh_source, re.S)
        self.assertIsNotNone(refresh, "could not locate IOSCloudClipboard.refresh")
        refresh = refresh.group(0)
        # 先问共享层走哪条路：配置了中继或网盘就只走中继，不发账号请求。
        self.assertLess(refresh.index("IOSCloudClipboardRelay.selected("), refresh.index("account.status()"))
        before = refresh.index("guard accountID == nil || accountID == current else { throw CancellationError() }")
        self.assertLess(refresh.index("guard let current = try await account.status()?.id else"), before)
        after = refresh.index("guard try await account.status()?.id == current else { throw CancellationError() }")
        self.assertLess(before, refresh.index("try await action(send)"))
        self.assertGreater(after, refresh.index('"operation": "list"'))
        # 中继和网盘同样在操作前核对绑定、读完列表后核对偏好里还是那一个。
        relay = re.search(r"private func refreshRelay\(boundTo.*?\n  \}", refresh_source, re.S)
        self.assertIsNotNone(relay, "could not locate IOSCloudClipboard.refreshRelay")
        relay = relay.group(0)
        relay_before = relay.index("guard accountID == nil || accountID == relay.binding else { throw CancellationError() }")
        self.assertLess(relay_before, relay.index("try await action(send)"))
        self.assertGreater(relay.index("guard (try? relay.requireCurrent()) != nil else { throw CancellationError() }"),
                           relay.index('"operation": "list"'))
        generation = GENERATION.read_text(encoding="utf-8")
        self.assertIn("try Task.checkCancellation()", generation)
        self.assertIn("guard isCurrent(id) else { throw CancellationError() }", generation)
        self.assertIn("if request.wrappedValue.isCurrent(id)", TASK.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
