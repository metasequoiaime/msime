#!/usr/bin/env python3
"""校验 iOS 设置同步页共用请求代次并隔离离页旧任务。"""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/settings/SettingsSyncView.swift"
GENERATION = ROOT / "platforms/ios/App/Sources/app/ViewRequestGeneration.swift"
TASK = ROOT / "platforms/ios/App/Sources/app/ViewRequestTask.swift"
APPLY = ROOT / "platforms/ios/App/Sources/settings/IOSCloudSettings.swift"


class IOSSettingsSyncViewLifecycleContract(unittest.TestCase):
    def test_disappear_invalidates_pending_operation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("@State private var request = ViewRequestGeneration()", source)
        self.assertIn(".trackViewRequest(request: $request, pending: $pending, onDisappear: {", source)
        self.assertNotIn("ViewRequestTask.disappear(request: $request, pending: $pending)", source)
        self.assertIn("busy = false", source)
        task = TASK.read_text(encoding="utf-8")
        self.assertIn("request.wrappedValue.disappear()", task)
        self.assertIn("pending.wrappedValue?.cancel()", task)
        self.assertIn("pending.wrappedValue = nil", task)

    def test_results_use_the_shared_operation_generation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertEqual(source.count("ViewRequestTask.run(request: $request, busy: $busy, message: $message,"), 3)
        self.assertEqual(source.count("pending: $pending"), 4)
        self.assertNotIn("let id = request.begin()", source)
        self.assertGreaterEqual(source.count("request.checkCurrent(id)"), 8)
        self.assertGreaterEqual(source.count("request.isCurrent(id)"), 9)
        # 每个远端等待（Rust 请求、账号核对）回来以后，下一行先核对代次，再写页面状态或发下一个请求。
        lines = [line.strip() for line in source.splitlines() if line.strip()]
        awaits = [i for i, line in enumerate(lines) if "try await " in line]
        self.assertGreaterEqual(len(awaits), 8)
        for i in awaits:
            self.assertEqual(lines[i + 1], "try request.checkCurrent(id)", f"no generation check after: {lines[i]}")
        self.assertIn("guard request.isCurrent(id) else { return }", source)
        generation = GENERATION.read_text(encoding="utf-8")
        self.assertIn("try Task.checkCancellation()", generation)
        self.assertIn("guard isCurrent(id) else { throw CancellationError() }", generation)
        task = TASK.read_text(encoding="utf-8")
        self.assertIn("try request.wrappedValue.checkCurrent(id)", task)
        self.assertIn("if request.wrappedValue.isCurrent(id)", task)

    def test_all_operations_still_keep_their_business_entry_points(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("load(requestID: id)", source)
        # C 上拿不到会话代次（#8201 4d），上传和应用只按读文档时的 user.id 绑定：应用前核对账号，上传把账号交给 Rust 的 preferences_put。
        self.assertIn("apply(cloud: cloud, userID: loadedUserID, requestID: id)", source)
        self.assertIn("upload(cloud: cloud, schema: schema, userID: loadedUserID, requestID: id)", source)
        self.assertEqual(source.count("guard try await account.status()?.id == userID else { throw CancellationError() }"), 3)
        self.assertIn('"operation": "preferences_put", "document": settings, "revision": revision, "user_id": userID', APPLY.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
