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

    def shorten_retry(self):
        # 把重起 watcher 的 30 秒间隔改成 1 秒，测试不必等满。
        source = self.monitor.read_text()
        self.assertIn("WATCH_RETRY_DELAY = 30\n", source)
        self.monitor.write_text(source.replace("WATCH_RETRY_DELAY = 30\n", "WATCH_RETRY_DELAY = 1\n"))

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

    # 与 wl-clipboard 在不提供 data-control 的合成器上一样：一启动就在 stderr 写下原因、以状态 1 退出。
    FAILING_WATCH = '''
import sys
with log.open("a") as output:
    output.write("watch\\n" if "--watch" in sys.argv else "read\\n")
if "--watch" in sys.argv:
    sys.stderr.write("Watch mode requires a compositor that supports the data-control protocol\\n")
    raise SystemExit(1)
sys.stdout.write("synthetic clipboard")
'''

    # 提供 data-control 的合成器上 watcher 后来退出：先被信号杀掉，再一次以别的原因非零退出（连不上合成器），之后正常运行。
    CRASHING_WATCH = '''
import os, signal, sys, time
with log.open("a") as output:
    output.write("watch\\n")
starts = log.read_text().splitlines().count("watch")
if starts == 1:
    time.sleep(0.3)
    os.kill(os.getpid(), signal.SIGKILL)
if starts == 2:
    sys.stderr.write("Failed to connect to a Wayland server\\n")
    raise SystemExit(1)
time.sleep(30)
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

    def test_crashed_watcher_is_retried_instead_of_disabled(self):
        # 只有「合成器不提供 data-control」才放弃 `wl-paste --watch`；崩溃、被信号杀掉、其他非零退出都照旧隔一段时间重起。没有 DISPLAY 时这一点决定采集是否还在：误判成不支持 data-control 就再也不起 watcher，整个会话不再记剪贴板。
        self.shorten_retry()
        self.write_wl_paste(self.CRASHING_WATCH)
        process = self.start_monitor()
        try:
            self.wait_for(self.wl_log, count=3, timeout=8)
            time.sleep(1)
            self.assertEqual(self.wl_log.read_text().splitlines(), ["watch", "watch", "watch"])
            self.assertNotIn("wl-paste --watch 不可用", self.stderr.read_text())
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
