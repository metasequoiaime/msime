#!/usr/bin/env python3
"""Ensure DeveloperPage drops transient work and full tokens with its view."""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/DeveloperPage.java"
BASE = ROOT / "platforms/android/java/app/msime/android/home/ColumnDetailPage.java"


class AndroidDeveloperViewLifecycleContract(unittest.TestCase):
    def test_destroy_view_clears_busy_state_and_fresh_token(self):
        source = SOURCE.read_text(encoding="utf-8")
        start = source.index("@Override protected void onContentDestroyed()")
        end = source.index("\n    @Override protected void reload()", start)
        body = source[start:end]
        self.assertIn("generation++;", body)
        self.assertIn('cloudAccountId = "";', body)
        self.assertIn('cloudSessionId = "";', body)
        self.assertIn("busy = false;", body)
        self.assertIn("confirming = false;", body)
        self.assertIn("freshToken = null;", body)
        base = BASE.read_text(encoding="utf-8")
        start = base.index("@Override public void onDestroyView()")
        end = base.index("\n    protected void onContentDestroyed()", start)
        self.assertIn("onContentDestroyed();", base[start:end])


if __name__ == "__main__":
    unittest.main()
