#!/usr/bin/env python3
"""社区列表的初始已获取状态不能覆盖用户已经开始的安装动作。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/CommunityAdapter.java"


class AndroidCommunityActionRaceContract(unittest.TestCase):
    def test_set_actions_preserves_busy_entries(self):
        source = SOURCE.read_text(encoding="utf-8")
        method = re.search(r"public void setActions\(Map<String, Action> values\) \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(method, "could not locate CommunityAdapter.setActions")
        body = method.group("body")
        self.assertRegex(body, r"Action\.BUSY")
        self.assertRegex(body, r"continue;")
        self.assertIn("actions.put", body)


if __name__ == "__main__":
    unittest.main()
