#!/usr/bin/env python3
"""翻译服务页离开后重建时必须释放连接测试的临时锁。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/TranslationServicePage.java"
BASE = ROOT / "platforms/android/java/app/msime/android/home/ColumnDetailPage.java"


class AndroidTranslationServiceViewLifecycleContract(unittest.TestCase):
    def test_destroy_view_clears_connection_test_lock(self):
        source = SOURCE.read_text(encoding="utf-8")
        on_destroy = re.search(
            r"@Override protected void onContentDestroyed\(\) \{(?P<body>.*?)\n    \}",
            source,
            re.S,
        )
        self.assertIsNotNone(on_destroy, "TranslationServicePage must release view-bound state")
        self.assertIn("testing = false;", on_destroy.group("body"))
        base = BASE.read_text(encoding="utf-8")
        start = base.index("@Override public void onDestroyView()")
        end = base.index("\n    protected void onContentDestroyed()", start)
        self.assertIn("onContentDestroyed();", base[start:end])


if __name__ == "__main__":
    unittest.main()
