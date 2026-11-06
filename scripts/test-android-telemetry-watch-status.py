#!/usr/bin/env python3
"""platforms/android/tests/device/telemetry-watch.py 怎样读夹具状态、怎样判定 :ime 被重启。

#10952：API 35 上 :ime 全程是同一个进程，验收却报 fixture restarted instead of observing consent，因为某一次 adb shell cat 读空或失败被当成空文档，pid 变成 None 就算重启。这里用合成的 adb 结果驱动读取与核对两个函数，不连设备，任何机器上都能跑。
"""

import importlib.util
import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("telemetry_watch", ROOT / "platforms/android/tests/device/telemetry-watch.py")
WATCH = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WATCH)

STATUS = {"enabled": True, "session_id": "s1", "capture": "c1", "watching": True, "pid": 6116}


def result(returncode=0, stdout=b"", stderr=b""):
    return subprocess.CompletedProcess(["adb"], returncode, stdout, stderr)


def document(value=STATUS):
    return result(stdout=json.dumps(value).encode() + b"\n")


MISSING = result(1, stderr=b"cat: /data/user/0/app.msime.android/files/synthetic-telemetry-watch/status.json: No such file or directory\n")


class Reads:
    """按顺序回放 adb 结果，记录调用次数与等待。"""

    def __init__(self, *results):
        self.results = list(results)
        self.calls = 0
        self.sleeps = []

    def cat(self):
        self.calls += 1
        return self.results.pop(0)

    def sleep(self, seconds):
        self.sleeps.append(seconds)


class ReadStatus(unittest.TestCase):
    def read(self, *results):
        reads = Reads(*results)
        current, last = WATCH.read_status(reads.cat, reads.sleep)
        return current, last, reads

    def test_transient_empty_output_is_retried(self):
        current, _, reads = self.read(result(), document())
        self.assertEqual(current, STATUS)
        self.assertEqual(reads.calls, 2)

    def test_transient_adb_failure_is_retried(self):
        current, _, reads = self.read(result(255, stderr=b"error: closed\n"), document())
        self.assertEqual(current, STATUS)
        self.assertEqual(reads.calls, 2)

    def test_undecodable_output_is_retried(self):
        current, _, _ = self.read(result(stdout=b'{"enabled": tr'), document())
        self.assertEqual(current, STATUS)

    def test_missing_file_returns_empty_without_retry(self):
        # start() 在夹具写出第一份状态前一直拿到空文档继续等，不能被重试拖慢。
        current, last, reads = self.read(MISSING)
        self.assertEqual(current, {})
        self.assertIs(last, MISSING)
        self.assertEqual((reads.calls, reads.sleeps), (1, []))

    def test_persistent_failure_returns_empty_with_last_result(self):
        failure = result(1, stderr=b"error: device offline\n")
        current, last, reads = self.read(*[failure] * WATCH.STATUS_ATTEMPTS)
        self.assertEqual(current, {})
        self.assertIs(last, failure)
        self.assertEqual(reads.calls, WATCH.STATUS_ATTEMPTS)
        self.assertEqual(len(reads.sleeps), WATCH.STATUS_ATTEMPTS - 1)


class ProcessMismatch(unittest.TestCase):
    def check(self, current, status_result=None, pidof=b"6116\n"):
        return WATCH.process_mismatch(6116, current, status_result or document(current), result(stdout=pidof))

    def test_same_process_passes(self):
        self.assertIsNone(self.check(STATUS))

    def test_one_empty_read_during_stable_capture_is_not_a_restart(self):
        # #10952 的现场：稳定性循环里某一次 cat 读空，下一次就读到同一个 pid。
        reads = Reads(result(), document())
        current, last = WATCH.read_status(reads.cat, reads.sleep)
        self.assertIsNone(WATCH.process_mismatch(6116, current, last, result(stdout=b"6116\n")))

    def test_different_status_pid_is_a_restart(self):
        message = self.check(dict(STATUS, pid=7001), pidof=b"7001\n")
        self.assertIn("fixture restarted instead of observing consent", message)
        self.assertIn("7001", message)

    def test_unreadable_status_with_same_ime_reports_raw_output(self):
        unreadable = result(1, stderr=b"error: closed\n")
        message = self.check({}, unreadable)
        self.assertNotIn("restarted", message)
        self.assertIn("status unreadable", message)
        self.assertIn("returncode=1", message)
        self.assertIn("error: closed", message)

    def test_changed_ime_pid_is_reported(self):
        message = self.check(STATUS, pidof=b"7001\n")
        self.assertIn("actual :ime process changed", message)
        self.assertIn("7001", message)

    def test_missing_ime_is_reported(self):
        self.assertIn("actual :ime process not found", self.check(STATUS, pidof=b""))


if __name__ == "__main__":
    unittest.main()
