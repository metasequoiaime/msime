#!/usr/bin/env python3
"""校验 iOS 设置同步页共用请求代次并隔离离页旧任务。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/settings/SettingsSyncView.swift"
GENERATION = ROOT / "platforms/ios/App/Sources/app/ViewRequestGeneration.swift"
TASK = ROOT / "platforms/ios/App/Sources/app/ViewRequestTask.swift"


class IOSSettingsSyncViewLifecycleContract(unittest.TestCase):
    def test_disappear_invalidates_pending_operation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("@State private var request = ViewRequestGeneration()", source)
        disappear = re.search(r"\.onDisappear \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(disappear, "settings sync view must invalidate work when it disappears")
        body = disappear.group("body")
        self.assertIn("ViewRequestTask.disappear(request: $request, pending: $pending)", body)
        self.assertIn("busy = false", body)
        task = TASK.read_text(encoding="utf-8")
        self.assertIn("request.wrappedValue.disappear()", task)
        self.assertIn("pending.wrappedValue?.cancel()", task)
        self.assertIn("pending.wrappedValue = nil", task)

    def test_results_use_the_shared_operation_generation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertEqual(source.count("ViewRequestTask.run(request: $request, busy: $busy, message: $message,"), 3)
        self.assertEqual(source.count("pending: $pending"), 4)
        self.assertNotIn("let id = request.begin()", source)
        self.assertGreaterEqual(source.count("request.checkCurrent(id)"), 9)
        self.assertGreaterEqual(source.count("request.isCurrent(id)"), 7)
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
        self.assertIn("apply(cloud: cloud, userID: loadedUserID, sessionID: loadedSessionID, requestID: id)", source)
        self.assertIn("upload(cloud: cloud, schema: schema, userID: loadedUserID, sessionID: loadedSessionID, requestID: id)", source)


if __name__ == "__main__":
    unittest.main()
