#!/usr/bin/env python3
"""语音页「测试连接」走网络线程池，不占设置页读写共享偏好的那条串行线程。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/VoicePage.java"
BASE = ROOT / "platforms/android/java/app/msime/android/home/ColumnDetailPage.java"


class VoiceCredentialTestThreadContract(unittest.TestCase):
    def test_probe_runs_on_the_network_pool(self):
        source = SOURCE.read_text()
        # The probe blocks for up to about 15 seconds; on HostTask.run it would queue every settings page's store reads and writes behind it.
        self.assertRegex(source, r"HostTask\.runNetwork\(this,\s*context\s*->\s*ServiceCredentialTest\.voice\(")
        self.assertNotRegex(source, r"HostTask\.run\(this,\s*context\s*->\s*ServiceCredentialTest\.voice\(")

    def test_repeated_taps_do_not_stack_probes(self):
        source = SOURCE.read_text()
        test_row = re.search(r"private void testRow\(.*?\n    \}\n", source, re.S)
        self.assertIsNotNone(test_row, "could not locate VoicePage.testRow")
        body = test_row.group(0)
        self.assertRegex(body, r"if \(!testing\.add\(service\)\) return;")
        self.assertIn("testing.remove(service);", body)
        self.assertLess(body.index("testing.add(service)"), body.index("HostTask.runNetwork("))
        on_destroy = re.search(r"protected void onContentDestroyed\(\) \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(on_destroy, "could not locate VoicePage.onContentDestroyed")
        self.assertIn("testing.clear();", on_destroy.group("body"))
        self.assertIn("onContentDestroyed();", BASE.read_text())


if __name__ == "__main__":
    unittest.main()
