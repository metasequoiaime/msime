#!/usr/bin/env python3
"""Exercise clipboard ownership events on an isolated Xvfb display."""
import ctypes
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
WATCHER = Path(sys.argv.pop(1)).resolve()


class ClipboardEvents(unittest.TestCase):
    def test_identical_copies_and_disable_reenable(self):
        x11 = ctypes.CDLL("libX11.so.6")
        pointer, number = ctypes.c_void_p, ctypes.c_ulong
        signatures = {
            "XOpenDisplay": ([ctypes.c_char_p], pointer),
            "XDefaultRootWindow": ([pointer], number),
            "XInternAtom": ([pointer, ctypes.c_char_p, ctypes.c_int], number),
            "XCreateSimpleWindow": ([pointer, number, ctypes.c_int, ctypes.c_int, ctypes.c_uint,
                                     ctypes.c_uint, ctypes.c_uint, number, number], number),
            "XSetSelectionOwner": ([pointer, number, number, number], ctypes.c_int),
            "XSync": ([pointer, ctypes.c_int], ctypes.c_int),
            "XCloseDisplay": ([pointer], ctypes.c_int),
        }
        for name, (arguments, result) in signatures.items():
            function = getattr(x11, name)
            function.argtypes, function.restype = arguments, result
        display = x11.XOpenDisplay(None)
        self.assertTrue(display)
        self.addCleanup(x11.XCloseDisplay, display)
        root_window = x11.XDefaultRootWindow(display)
        clipboard = x11.XInternAtom(display, b"CLIPBOARD", 0)
        window = x11.XCreateSimpleWindow(display, root_window, 0, 0, 1, 1, 0, 0, 0)
        def copy_again():
            # Same owner and same text: this is invisible to text-only polling.
            x11.XSetSelectionOwner(display, clipboard, window, 0)
            x11.XSync(display, 0)
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
            executable("xclip", "import sys\nsys.stdout.write('synthetic identical clipboard')\n")
            executable("msime-linux-clipboard-capture", "import os, sys\n"
                       "assert sys.stdin.buffer.read() == b'synthetic identical clipboard'\n"
                       "with open(os.environ['MSIME_TEST_CAPTURES'], 'a') as log: log.write('capture\\n')\n")
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
                    time.sleep(0.01)
                self.assertIsNone(process.poll(), "monitor exited")
                self.assertEqual(count(), expected)
            try:
                wait_count(1)
                copy_again()
                wait_count(2)
                copy_again()
                wait_count(3)
                enable(False)
                time.sleep(1)
                copy_again()
                time.sleep(1)
                self.assertEqual(count(), 3)
                enable(True)
                wait_count(4)
            finally:
                process.terminate()
                process.wait(timeout=4)


if __name__ == "__main__":
    unittest.main()
