#!/usr/bin/env python3
"""Verify clipboard watcher descendants stop even after their leader exits."""
import ctypes
import importlib.machinery
import importlib.util
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
loader = importlib.machinery.SourceFileLoader("clipboard_monitor", str(ROOT / "scripts" / "msime-linux-clipboard-monitor"))
spec = importlib.util.spec_from_loader(loader.name, loader)
monitor = importlib.util.module_from_spec(spec)
loader.exec_module(monitor)
CHILD = '''import os, signal, sys, time
from pathlib import Path
signal.signal(signal.SIGTERM, signal.SIG_IGN)
ready = Path(sys.argv[1] + ".tmp")
ready.write_text(str(os.getpid()))
ready.replace(sys.argv[1])
time.sleep(30)
'''
LEADER = '''import subprocess, sys, time
from pathlib import Path
subprocess.Popen([sys.executable, "-c", sys.argv[1], sys.argv[2]])
while not Path(sys.argv[2]).exists(): time.sleep(0.01)
if sys.argv[3] == "alive": time.sleep(30)
'''


@unittest.skipUnless(sys.platform == "linux", "requires Linux child-subreaper support")
class WatchLifecycle(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Adopt fixture descendants so the test also reaps every synthetic child.
        if ctypes.CDLL(None).prctl(36, 1, 0, 0, 0) != 0:
            raise RuntimeError("could not enable fixture child reaping")

    def check_shutdown(self, mode):
        with tempfile.TemporaryDirectory(prefix="msime-watch-") as directory:
            pid_file = Path(directory) / "child.pid"
            leader = subprocess.Popen([sys.executable, "-c", LEADER, CHILD, str(pid_file), mode],
                                      start_new_session=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            child = None
            try:
                deadline = time.monotonic() + 3
                while not pid_file.exists() and time.monotonic() < deadline:
                    time.sleep(0.01)
                self.assertTrue(pid_file.exists(), "synthetic descendant did not start")
                child = int(pid_file.read_text())
                if mode == "exited":
                    self.assertEqual(leader.wait(timeout=3), 0)
                else:
                    self.assertIsNone(leader.poll())
                monitor.stop_watch(leader)
                deadline = time.monotonic() + 2
                reaped = 0
                while time.monotonic() < deadline:
                    reaped, _ = os.waitpid(child, os.WNOHANG)
                    if reaped:
                        child = None
                        break
                    time.sleep(0.01)
                self.assertNotEqual(reaped, 0, "watcher descendant survived stop_watch")
                monitor.stop_watch(leader)  # Already-gone groups are harmless.
            finally:
                try:
                    os.killpg(leader.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                leader.wait(timeout=3)
                if child is not None:
                    os.waitpid(child, 0)

    def test_exited_leader_still_stops_descendants(self):
        self.check_shutdown("exited")

    def test_leader_exit_on_term_still_stops_resistant_descendants(self):
        self.check_shutdown("alive")


if __name__ == "__main__":
    unittest.main()
