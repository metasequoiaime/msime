#!/usr/bin/env python3
"""Ensure ProfilePage does not retain transient busy state across view recreation."""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/ProfilePage.java"
BASE = ROOT / "platforms/android/java/app/msime/android/home/ColumnDetailPage.java"


class AndroidProfileViewLifecycleContract(unittest.TestCase):
    def test_destroy_view_clears_busy_state(self):
        source = SOURCE.read_text(encoding="utf-8")
        start = source.index("@Override protected void onContentDestroyed()")
        end = source.index("\n    @Override protected void onContentReady()", start)
        body = source[start:end]
        self.assertIn("generation++;", body)
        self.assertIn("loaded = null;", body)
        self.assertIn("avatarPickerSessionId = null;", body)
        self.assertIn("busy = false;", body)
        base = BASE.read_text(encoding="utf-8")
        self.assertIn("onContentDestroyed();", base)


if __name__ == "__main__":
    unittest.main()
