#!/usr/bin/env python3
"""用真实子进程核对 Cargo 契约锁的互斥和进程退出后的释放。"""

from pathlib import Path
import os
import subprocess
import sys
import tempfile
import time
import unittest

from contract_cargo import contract_cargo_lock

WORKER = r'''
from pathlib import Path
import sys
sys.path.insert(0, sys.argv[1])
from contract_cargo import contract_cargo_lock
Path(sys.argv[3]).write_text("ready")
with contract_cargo_lock(Path(sys.argv[2])):
    Path(sys.argv[4]).write_text("acquired")
    sys.stdin.buffer.read(1)
'''


class ContractCargoLockTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="msime-contract-lock-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.lock = self.root / "cargo.lock"
        self.workers = []
        self.addCleanup(self.stop_workers)

    def stop_workers(self):
        for process in self.workers:
            if process.poll() is None:
                process.terminate()
            process.wait(timeout=10)
            process.stdin.close()
            process.stderr.close()

    def worker(self, name):
        ready = self.root / (name + ".ready")
        acquired = self.root / (name + ".acquired")
        process = subprocess.Popen([sys.executable, "-c", WORKER, str(Path(__file__).resolve().parent),
            str(self.lock), str(ready), str(acquired)], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE, close_fds=True)
        self.workers.append(process)
        self.wait_for(ready, process)
        return process, acquired

    def wait_for(self, path, process):
        deadline = time.monotonic() + (30 if os.name == "nt" else 5)
        while not path.exists():
            if process.poll() is not None:
                self.fail(process.stderr.read().decode("utf-8", errors="replace"))
            if time.monotonic() >= deadline:
                self.fail("合成锁进程未在有界时间内到达预期状态")
            time.sleep(0.02)

    def finish(self, process):
        process.stdin.write(b"x")
        process.stdin.flush()
        self.assertEqual(process.wait(timeout=10), 0)

    def test_a_second_process_waits_until_the_holder_releases(self):
        with contract_cargo_lock(self.lock):
            process, acquired = self.worker("waiting")
            time.sleep(0.2)
            self.assertFalse(acquired.exists(), "另一进程在持锁期间进入了临界区")
        self.wait_for(acquired, process)
        self.finish(process)

    def test_terminating_a_holder_does_not_leave_a_stale_lock(self):
        holder, first = self.worker("holder")
        self.wait_for(first, holder)
        waiting, second = self.worker("after-exit")
        time.sleep(0.2)
        self.assertFalse(second.exists(), "第二个持锁者提前进入了临界区")
        holder.terminate()
        holder.wait(timeout=10)
        self.wait_for(second, waiting)
        self.finish(waiting)


if __name__ == "__main__":
    unittest.main()
