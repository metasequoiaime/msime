#!/usr/bin/env python3
"""Read only synthetic GTK clipboard selections inside isolated Xvfb."""
import concurrent.futures
import ctypes
import importlib.machinery
import importlib.util
from pathlib import Path
import os
import tempfile
from unittest import mock
import subprocess
import select
import sys
import time
import unittest
import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, GLib, Gtk

READER = Path(sys.argv.pop(1)).resolve()
STRING_OWNER = Path(sys.argv.pop(1)).resolve()
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
loader = importlib.machinery.SourceFileLoader("monitor", str(ROOT / "scripts" / "msime-linux-clipboard-monitor"))
spec = importlib.util.spec_from_loader(loader.name, loader)
monitor = importlib.util.module_from_spec(spec)
loader.exec_module(monitor)


class NativeRead(unittest.TestCase):
    def setUp(self):
        self.clipboard = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)
        self.addCleanup(self.clipboard.clear)

    def read(self):
        # GTK must serve selection requests while the subprocess reads stdout.
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
            job = pool.submit(subprocess.run, [str(READER), "--read"], capture_output=True, timeout=3)
            while not job.done():
                while GLib.MainContext.default().iteration(False):
                    pass
                time.sleep(0.001)
            return job.result()

    def test_unicode_and_line_endings(self):
        text = "synthetic 中文🙂\nsecond line\r\n"
        self.clipboard.set_text(text, -1)
        result = self.read()
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, text.encode())
        self.assertEqual(result.stderr, b"")

    def test_large_incremental_transfer_is_bounded(self):
        text = "synthetic🙂汉" * 100000
        self.clipboard.set_text(text, -1)
        result = self.read()
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, text.encode()[:12000])
        captured = monitor.history_text(result.stdout, complete=False)
        self.assertTrue(text.startswith(captured))
        self.assertLessEqual(len(captured.encode("utf-16-le")), 8000)

    def test_latin1_string_fallback(self):
        owner = subprocess.Popen([str(STRING_OWNER)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 stderr=subprocess.DEVNULL)
        try:
            owner.stdin.write(b"synthetic caf\xe9\n")
            owner.stdin.close()
            self.assertTrue(select.select([owner.stdout], [], [], 3)[0])
            self.assertEqual(owner.stdout.readline(), b"ready\n")
            result = self.read()
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "synthetic café\n".encode())
        finally:
            owner.terminate()
            owner.wait(timeout=3)
            owner.stdout.close()

    def test_explicit_incr_handshake_and_bounded_chunks(self):
        for repeats in (100, 4000):
            with self.subTest(repeats=repeats):
                text = ("synthetic🙂汉" * repeats).encode()
                owner = subprocess.Popen([str(STRING_OWNER), "--incr"], stdin=subprocess.PIPE,
                                         stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
                try:
                    owner.stdin.write(text)
                    owner.stdin.close()
                    self.assertTrue(select.select([owner.stdout], [], [], 3)[0])
                    self.assertEqual(owner.stdout.readline(), b"ready\n")
                    result = self.read()
                    self.assertEqual(result.returncode, 0)
                    self.assertEqual(result.stdout, text[:12000])
                    self.assertEqual(owner.stdout.readline(), b"incr\n")
                finally:
                    owner.terminate()
                    owner.wait(timeout=3)
                    owner.stdout.close()

    def test_monitor_reads_without_external_clipboard_tools(self):
        text = "synthetic native-only clipboard"
        self.clipboard.set_text(text, -1)
        with tempfile.TemporaryDirectory(prefix="msime-native-reader-") as directory:
            root = Path(directory)
            (root / "msime-linux-clipboard-watch-x11").symlink_to(READER)
            with mock.patch.object(monitor, "__file__", str(root / "monitor.py")), \
                 mock.patch.dict(os.environ, {"PATH": "", "WAYLAND_DISPLAY": ""}), \
                 concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
                job = pool.submit(monitor.clipboard_text)
                while not job.done():
                    while GLib.MainContext.default().iteration(False):
                        pass
                    time.sleep(0.001)
                self.assertEqual(job.result(), text)

    def test_no_owner_returns_no_text(self):
        self.clipboard.clear()
        result = self.read()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, b"")

    def test_unresponsive_owner_has_one_deadline(self):
        x11 = ctypes.CDLL("libX11.so.6")
        pointer, number = ctypes.c_void_p, ctypes.c_ulong
        for name, args, result in (
            ("XOpenDisplay", [ctypes.c_char_p], pointer),
            ("XDefaultRootWindow", [pointer], number),
            ("XInternAtom", [pointer, ctypes.c_char_p, ctypes.c_int], number),
            ("XSetSelectionOwner", [pointer, number, number, number], ctypes.c_int),
            ("XSync", [pointer, ctypes.c_int], ctypes.c_int),
            ("XCloseDisplay", [pointer], ctypes.c_int),
        ):
            function = getattr(x11, name)
            function.argtypes, function.restype = args, result
        display = x11.XOpenDisplay(None)
        self.assertTrue(display)
        try:
            atom = x11.XInternAtom(display, b"CLIPBOARD", 0)
            x11.XSetSelectionOwner(display, atom, x11.XDefaultRootWindow(display), 0)
            x11.XSync(display, 0)
            started = time.monotonic()
            result = self.read()
            self.assertLess(time.monotonic() - started, 1.5)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, b"")
        finally:
            x11.XCloseDisplay(display)


if __name__ == "__main__":
    unittest.main()
