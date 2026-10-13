#!/usr/bin/env python3
"""确保 iOS 设置页共用同一条写入失败提示。"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = ROOT / "platforms/ios/App/Sources"
MESSAGE = "设置没有保存，键盘可能正在写入同一份设置，请再试一次。"


class SettingsSaveFailureContract(unittest.TestCase):
    def test_message_has_one_definition_and_all_users_reference_it(self):
        swift = list(SOURCE_ROOT.rglob("*.swift"))
        definitions = [p for p in swift if 'static let message = "' in p.read_text()]
        self.assertEqual([p.name for p in definitions], ["SettingsSaveFailure.swift"])
        users = [p for p in swift if "SettingsSaveFailure.message" in p.read_text()]
        self.assertGreaterEqual(len(users), 16)
        self.assertEqual(sum(p.read_text().count(MESSAGE) for p in swift), 1)


if __name__ == "__main__":
    unittest.main()
