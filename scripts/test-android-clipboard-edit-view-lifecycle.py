#!/usr/bin/env python3
"""Ensure ClipboardEditPage clears transient save state with its view."""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/ClipboardEditPage.java"
BASE = ROOT / "platforms/android/java/app/msime/android/home/ColumnDetailPage.java"


class AndroidClipboardEditViewLifecycleContract(unittest.TestCase):
    def test_destroy_view_clears_saving_state(self):
        source = SOURCE.read_text(encoding="utf-8")
        start = source.index("@Override protected void onContentDestroyed()")
        end = source.index("\n    private void load", start)
        body = source[start:end]
        self.assertIn("save = null;", body)
        self.assertIn("saving = false;", body)
        self.assertIn("onContentDestroyed();", BASE.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
