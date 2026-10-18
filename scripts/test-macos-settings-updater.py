#!/usr/bin/env python3
"""合成包验证设置应用更新配置、版本隔离、签名要求和框架符号链接。"""

import base64
import importlib.util
import json
import plistlib
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("settings_updater", ROOT / "platforms/macos/scripts/settings_updater.py")
updater = importlib.util.module_from_spec(spec)
spec.loader.exec_module(updater)


class SettingsUpdaterTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="msime-updater-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.app = self.root / "合成设置.app"
        self.native = self.root / "合成输入法.app"
        self.settings = {"CFBundleIdentifier": "test.synthetic.settings", "CFBundleVersion": "1", "CFBundleShortVersionString": "1.2.3"}
        self.metadata = {"CFBundleVersion": "1002003004", "SUPublicEDKey": base64.b64encode(bytes(range(32))).decode(), "SURequireSignedFeed": True, "SUVerifyUpdateBeforeExtraction": True, "SUEnableAutomaticChecks": True, "SUAutomaticallyUpdate": True}
        framework = self.native / "Contents/Frameworks/Sparkle.framework"
        (framework / "Versions/B").mkdir(parents=True)
        (framework / "Versions/B/Sparkle").write_bytes(b"synthetic-framework")
        (framework / "Versions/Current").symlink_to("B")
        (framework / "Sparkle").symlink_to("Versions/Current/Sparkle")
        self.write_plists()

    def write_plists(self):
        for app, data in ((self.app, self.settings), (self.native, self.metadata)):
            (app / "Contents").mkdir(parents=True, exist_ok=True)
            (app / "Contents/Info.plist").write_bytes(plistlib.dumps(data))

    def read_settings(self):
        return plistlib.loads((self.app / "Contents/Info.plist").read_bytes())

    def test_each_edition_has_its_own_settings_feed_and_preserves_framework_links(self):
        table = json.loads((ROOT / "shared/contracts/editions.json").read_text())
        for edition in table["editions"]:
            if not edition["platforms"].get("macos"):
                continue
            with self.subTest(edition=edition["id"]):
                self.metadata["SUFeedURL"] = f'https://updates.example.invalid/appcast-{edition["id"]}.xml'
                self.write_plists()
                self.assertTrue(updater.configure(self.app, self.native, edition["id"]))
                settings = self.read_settings()
                self.assertEqual(settings["SUFeedURL"], f'https://updates.example.invalid/appcast-settings-{edition["id"]}.xml')
                self.assertEqual(settings["CFBundleVersion"], self.metadata["CFBundleVersion"])
                self.assertEqual(settings["CFBundleIdentifier"], self.settings["CFBundleIdentifier"])
                self.assertEqual(settings["SUPublicEDKey"], self.metadata["SUPublicEDKey"])
                self.assertTrue(settings["SURequireSignedFeed"])
                self.assertTrue(settings["SUVerifyUpdateBeforeExtraction"])
                self.assertFalse(settings["SUEnableAutomaticChecks"])
                framework = self.app / "Contents/Frameworks/Sparkle.framework"
                self.assertTrue((framework / "Sparkle").is_symlink())
                self.assertTrue((framework / "Versions/Current").is_symlink())
                self.assertEqual((framework / "Sparkle").read_bytes(), b"synthetic-framework")

    def test_no_feed_build_does_not_advertise_updates(self):
        self.settings.update({key: "synthetic-stale" for key in updater.UPDATE_KEYS})
        self.write_plists()
        self.assertFalse(updater.configure(self.app, self.native, "full"))
        settings = self.read_settings()
        self.assertFalse(set(updater.UPDATE_KEYS) & set(settings))
        self.assertEqual(settings["CFBundleVersion"], self.metadata["CFBundleVersion"])

    def test_other_edition_feed_and_unsigned_updates_are_rejected_without_mutating_app(self):
        for fields in (
            {"SUFeedURL": "https://updates.example.invalid/appcast-japanese.xml"},
            {"SUFeedURL": "https://updates.example.invalid/appcast-full.xml", "SURequireSignedFeed": False},
            {"SUFeedURL": "https://updates.example.invalid/appcast-full.xml", "SUVerifyUpdateBeforeExtraction": False},
            {"SUFeedURL": "https://updates.example.invalid/appcast-full.xml", "SUPublicEDKey": ""},
            {"SUFeedURL": "http://updates.example.invalid/appcast-full.xml"},
        ):
            with self.subTest(fields=fields):
                original = dict(self.metadata)
                self.metadata.update(fields)
                self.write_plists()
                with self.assertRaises(ValueError):
                    updater.configure(self.app, self.native, "full")
                self.assertEqual(self.read_settings(), self.settings)
                self.metadata = original


if __name__ == "__main__":
    unittest.main()
