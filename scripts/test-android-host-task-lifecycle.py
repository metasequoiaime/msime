#!/usr/bin/env python3
"""确保 Android 宿主异步结果不会写入重建后的 Fragment 视图。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/HostTask.java"


class HostTaskLifecycleContract(unittest.TestCase):
    def test_result_is_bound_to_the_view_that_started_the_work(self):
        source = SOURCE.read_text()
        self.assertRegex(source, r"View\s+ownerView\s*=\s*fragment\.getView\(\)")
        self.assertRegex(source, r"Lifecycle\s+ownerLifecycle\s*=\s*fragment\.getViewLifecycleOwner\(\)")
        self.assertRegex(source, r"deliver\(fragment,\s*ownerView,\s*ownerLifecycle,")
        self.assertRegex(source, r"fragment\.getView\(\)\s*!?=\s*ownerView")

        deliver = re.search(
            r"private static <T> void deliver\((?P<args>.*?)\) \{(?P<body>.*?)\n\s*\}",
            source,
            re.S,
        )
        self.assertIsNotNone(deliver, "could not locate HostTask delivery guard")
        self.assertIn("ownerLifecycle.getCurrentState()", deliver.group("body"))


if __name__ == "__main__":
    unittest.main()
