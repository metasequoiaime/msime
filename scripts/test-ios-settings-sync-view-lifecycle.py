#!/usr/bin/env python3
"""离开 iOS 设置同步页后，旧任务不能卡住或覆盖重建页面。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/settings/SettingsSyncView.swift"


class IOSSettingsSyncViewLifecycleContract(unittest.TestCase):
    def test_disappear_invalidates_pending_operation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, r"@State private var requestID = UUID\(\)")
        disappear = re.search(r"\.onDisappear \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(disappear, "settings sync view must invalidate work when it disappears")
        body = disappear.group("body")
        self.assertIn("requestID = UUID()", body)
        self.assertIn("busy = false", body)
        self.assertIn("pending?.cancel()", body)

    def test_results_are_bound_to_the_operation_generation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, re.compile(r"let id = UUID\(\).*?requestID = id", re.S))
        self.assertRegex(source, r"load\(requestID: id\)")
        load = re.search(r"private func load\(.*?\n  \}", source, re.S)
        self.assertIsNotNone(load, "could not locate SettingsSyncView.load")
        body = load.group(0)
        self.assertGreaterEqual(len(re.findall(r"requestID == id", body)), 2)

    def test_upload_and_apply_are_also_bound_to_the_operation_generation(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, r"upload\(cloud: cloud, schema: schema, userID: loadedUserID, sessionID: loadedSessionID, requestID: id\)")
        self.assertRegex(source, r"apply\(cloud: cloud, userID: loadedUserID, sessionID: loadedSessionID, requestID: id\)")
        upload = re.search(r"private func upload\(.*?\n  \}", source, re.S)
        apply = re.search(r"private func apply\(.*?\n  \}", source, re.S)
        self.assertIsNotNone(upload, "could not locate SettingsSyncView.upload")
        self.assertIsNotNone(apply, "could not locate SettingsSyncView.apply")
        self.assertGreaterEqual(len(re.findall(r"requestID == id", upload.group(0))), 3)
        self.assertGreaterEqual(len(re.findall(r"requestID == id", apply.group(0))), 2)


if __name__ == "__main__":
    unittest.main()
