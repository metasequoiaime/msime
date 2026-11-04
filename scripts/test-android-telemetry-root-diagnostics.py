#!/usr/bin/env python3
"""运行真实遥测夹具，确认 adb 提权失败时保留诊断并停止设备修改。"""

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
DEVICE = ROOT / "platforms/android/tests/device"
SCRIPTS = ("telemetry-watch.py", "telemetry-consent.py", "telemetry-anr.py")
FAKE_ADB = '''#!{python}
import os
from pathlib import Path
import sys

args = sys.argv[3:]
with Path(os.environ["FAKE_ADB_CALLS"]).open("a") as calls:
    calls.write(" ".join(args) + "\\n")
if args == ["emu", "avd", "name"]:
    print("msime-client-test\\nOK")
elif args == ["shell", "getprop ro.build.version.sdk"]:
    print("35")
elif args == ["root"]:
    print("synthetic root stdout", flush=True)
    print("synthetic root stderr", file=sys.stderr, flush=True)
    sys.exit(7)
else:
    sys.exit("unexpected command after failed root")
'''


class RootDiagnosticsTests(unittest.TestCase):
    @unittest.skipIf(os.name == "nt", "合成 ADB 夹具需要 POSIX 可执行脚本")
    def test_failed_root_preserves_both_streams_and_stops(self):
        for script in SCRIPTS:
            with self.subTest(script=script), tempfile.TemporaryDirectory() as temp:
                base = Path(temp)
                adb = base / "platform-tools/adb"
                adb.parent.mkdir()
                adb.write_text(FAKE_ADB.format(python=sys.executable), encoding="utf-8")
                adb.chmod(0o755)
                calls = base / "calls"
                env = dict(os.environ, ANDROID_SDK_ROOT=temp, FAKE_ADB_CALLS=str(calls))
                result = subprocess.run(
                    [sys.executable, str(DEVICE / script), "emulator-5580"],
                    env=env, capture_output=True, text=True, timeout=10,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("synthetic root stdout", result.stdout)
                self.assertIn("synthetic root stderr", result.stderr)
                self.assertIn("exit status 7", result.stderr)
                recorded = calls.read_text(encoding="utf-8").splitlines()
                self.assertEqual(recorded.count("root"), 1)
                self.assertEqual(recorded[-1], "root")
                self.assertFalse(any("wait-for-device" in call for call in recorded))


if __name__ == "__main__":
    unittest.main()
