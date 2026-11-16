#!/usr/bin/env python3
"""Exercise clipboard ownership events on an isolated Xvfb display."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
import signal
import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, GLib, Gtk

ROOT = Path(__file__).resolve().parents[2]
WATCHER = Path(sys.argv.pop(1)).resolve()


class ClipboardEvents(unittest.TestCase):
    def test_identical_copies_and_disable_reenable(self):
        clipboard = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)
        self.addCleanup(clipboard.clear)
        initial = "synthetic identical clipboard"
        clipboard.set_text(initial, -1)
        def pump():
            while GLib.MainContext.default().iteration(False):
                pass
        def copy_again():
            clipboard.set_text(initial, -1)
            pump()
        with tempfile.TemporaryDirectory(prefix="msime-x11-clipboard-") as directory:
            root = Path(directory)
            monitor = root / "msime-linux-clipboard-monitor"
            shutil.copyfile(ROOT / "scripts" / monitor.name, monitor)
            shutil.copyfile(ROOT / "scripts" / "msime_crash_capture.py", root / "msime_crash_capture.py")
            (root / "msime-linux-clipboard-watch-x11").symlink_to(WATCHER)
            log = root / "captures"
            def executable(name, source):
                path = root / name
                path.write_text("#!" + sys.executable + "\n" + source)
                path.chmod(0o700)
            executable("msime-linux-clipboard-capture", "import os, sys\n"
                       "import json\n"
                       "text = sys.stdin.buffer.read().decode('utf-8')\n"
                       "with open(os.environ['MSIME_TEST_CAPTURES'], 'a') as log: log.write(json.dumps(text) + '\\n')\n")
            options = root / "runtime.json"
            options.write_text(json.dumps({"preferences_directory": str(root)}))
            def enable(value):
                temporary = root / "preferences.tmp"
                temporary.write_text(json.dumps({"format_version": 1, "preferences": {"clipboard_history": value}}))
                temporary.replace(root / "preferences.json")
            enable(True)
            env = {**os.environ, "PATH": str(root) + os.pathsep + os.environ["PATH"],
                   "MSIME_TEST_CAPTURES": str(log)}
            env.pop("WAYLAND_DISPLAY", None)
            process = subprocess.Popen([sys.executable, str(monitor), str(options)], env=env,
                                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            def count():
                return len(log.read_text().splitlines()) if log.exists() else 0
            def wait_count(expected):
                deadline = time.monotonic() + 4
                while count() < expected and process.poll() is None and time.monotonic() < deadline:
                    pump()
                    time.sleep(0.001)
                self.assertIsNone(process.poll(), "monitor exited")
                self.assertEqual(count(), expected)
            try:
                wait_count(1)
                copy_again()
                wait_count(2)
                copy_again()
                wait_count(3)
                # 保存进程停顿时，监听器仍要把三次正文分别排队，不能只剩最后一次。
                os.kill(process.pid, signal.SIGSTOP)
                texts = ["synthetic alpha\nline", "synthetic beta 中文", "synthetic gamma🙂"]
                try:
                    for text in texts:
                        clipboard.set_text(text, -1)
                        deadline = time.monotonic() + 0.2
                        while time.monotonic() < deadline:
                            pump()
                            time.sleep(0.001)
                finally:
                    os.kill(process.pid, signal.SIGCONT)
                wait_count(6)
                values = [json.loads(line) for line in log.read_text().splitlines()]
                self.assertEqual(values, [initial] * 3 + texts)
                enable(False)
                time.sleep(1)
                copy_again()
                time.sleep(1)
                self.assertEqual(count(), 6)
                enable(True)
                wait_count(7)
            finally:
                process.terminate()
                process.wait(timeout=4)


if __name__ == "__main__":
    unittest.main()
