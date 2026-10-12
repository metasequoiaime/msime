#!/usr/bin/env python3
"""Ensure ExpressionPage does not retain a stale community install lock."""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/ExpressionPage.java"
BASE = ROOT / "platforms/android/java/app/msime/android/home/ColumnDetailPage.java"


class AndroidExpressionInstallViewLifecycleContract(unittest.TestCase):
    def test_destroy_view_clears_installing_state(self):
        source = SOURCE.read_text(encoding="utf-8")
        start = source.index("@Override protected void onContentDestroyed()")
        end = source.index("\n    @Override protected void reload()", start)
        body = source[start:end]
        self.assertIn("installing.clear();", body)
        base = BASE.read_text(encoding="utf-8")
        start = base.index("@Override public void onDestroyView()")
        end = base.index("\n    protected void onContentDestroyed()", start)
        self.assertIn("onContentDestroyed();", base[start:end])


if __name__ == "__main__":
    unittest.main()
