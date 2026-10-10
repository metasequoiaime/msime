#!/usr/bin/env python3
"""Verify Wayland watcher failure fallback and event capture behavior."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[2]


class WaylandWatch(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="msime-wayland-watch-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.monitor = self.root / "msime-linux-clipboard-monitor"
        shutil.copyfile(ROOT / "scripts" / self.monitor.name, self.monitor)
        self.monitor.chmod(0o700)
        self.options = self.root / "runtime.json"
        self.options.write_text(json.dumps({"preferences_directory": str(self.root)}))
        (self.root / "preferences.json").write_text(json.dumps({
            "format_version": 1, "preferences": {"clipboard_history": True}}))
        self.capture_log = self.root / "captures"
        (self.root / "msime-linux-clipboard-capture").write_text(
            "#!/usr/bin/env python3\n"
            "import os, sys\n"
            "sys.stdin.buffer.read()\n"
            "with open(os.environ['MSIME_TEST_CAPTURE_LOG'], 'a') as log: log.write('capture\\n')\n")
        (self.root / "msime-linux-clipboard-capture").chmod(0o700)
        self.wl_log = self.root / "wl-paste.log"
        self.env = {
            **{key: value for key, value in os.environ.items()
               if key not in ("DISPLAY",)},
            "PATH": str(self.root) + ":" + os.environ["PATH"],
            "WAYLAND_DISPLAY": "synthetic-wayland",
            "MSIME_TEST_WL_LOG": str(self.wl_log),
            "MSIME_TEST_CAPTURE_LOG": str(self.capture_log),
        }

    def write_wl_paste(self, source):
        path = self.root / "wl-paste"
        path.write_text("#!/usr/bin/env python3\nfrom pathlib import Path\nimport os\n"
                        "log = Path(os.environ['MSIME_TEST_WL_LOG'])\n" + source)
        path.chmod(0o700)

    def write_x11_watch(self):
        # 合成的 XWayland 监听器：监听模式先报一次事件再挂起，`--read` 模式交出固定文本并记一笔。
        path = self.root / "msime-linux-clipboard-watch-x11"
        path.write_text("#!/usr/bin/env python3\nfrom pathlib import Path\nimport os, sys, time\n"
                        "log = Path(os.environ['MSIME_TEST_WL_LOG'])\n"
                        "if sys.argv[1:] == ['--read']:\n"
                        "    with log.open('a') as output: output.write('x11-read\\n')\n"
                        "    sys.stdout.write('synthetic xwayland clipboard')\n"
                        "    raise SystemExit(0)\n"
                        "print(flush=True)\n"
                        "time.sleep(30)\n")
        path.chmod(0o700)

    def start_monitor(self):
        self.stderr = self.root / "monitor.stderr"
        with self.stderr.open("wb") as stderr:
            return subprocess.Popen([sys.executable, str(self.monitor), str(self.options)],
                                    env=self.env, stdout=subprocess.DEVNULL, stderr=stderr)

    def wait_for(self, path, count=1, timeout=4):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if path.exists() and len(path.read_text().splitlines()) >= count:
                return
            time.sleep(0.02)
        self.fail(f"timed out waiting for {count} lines in {path}")

    def stop_monitor(self, process):
        process.terminate()
        process.wait(timeout=4)

    FAILING_WATCH = '''
import sys
with log.open("a") as output:
    output.write("watch\\n" if "--watch" in sys.argv else "read\\n")
if "--watch" in sys.argv:
    raise SystemExit(7)
sys.stdout.write("synthetic clipboard")
'''

    def test_failed_watcher_never_reads_through_wl_paste(self):
        # 没有 data-control 的合成器上（GNOME/Mutter），`wl-paste` 读剪贴板要抢焦点，会打断输入法预编辑（#6514）。
        self.write_wl_paste(self.FAILING_WATCH)
        process = self.start_monitor()
        try:
            self.wait_for(self.wl_log)
            time.sleep(2.5)
            self.assertEqual(self.wl_log.read_text().splitlines(), ["watch"])
            self.assertFalse(self.capture_log.exists())
            self.assertIn("wl-paste --watch", self.stderr.read_text())
        finally:
            self.stop_monitor(process)

    def test_failed_watcher_switches_to_xwayland(self):
        self.write_wl_paste(self.FAILING_WATCH)
        self.write_x11_watch()
        self.env["DISPLAY"] = ":synthetic"
        process = self.start_monitor()
        try:
            self.wait_for(self.capture_log)
            time.sleep(1)
            self.assertEqual(self.wl_log.read_text().splitlines(), ["watch", "x11-read"])
            self.assertEqual(self.capture_log.read_text().splitlines(), ["capture"])
        finally:
            self.stop_monitor(process)

    def test_working_watcher_delivers_event_without_polling(self):
        self.write_wl_paste('''
import os, subprocess, sys, time
with log.open("a") as output:
    output.write("watch\\n")
command = sys.argv[sys.argv.index("--watch") + 1:]
subprocess.run(command, input=b"synthetic watcher clipboard", env=os.environ.copy())
time.sleep(30)
''')
        process = self.start_monitor()
        try:
            self.wait_for(self.capture_log)
            time.sleep(1)
            self.assertFalse(any(line == "read" for line in self.wl_log.read_text().splitlines()))
        finally:
            self.stop_monitor(process)


if __name__ == "__main__":
    unittest.main()
