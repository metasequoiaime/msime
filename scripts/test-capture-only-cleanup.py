#!/usr/bin/env python3
"""验证辅助进程清理轮询对 Windows 文件占用的处理，不放宽清空断言。"""

import errno
from pathlib import Path
import runpy
import sys
import unittest
from unittest.mock import Mock, patch


ROOT = Path(__file__).resolve().parents[1]
with patch.object(sys, "argv", ["capture_only.py", "synthetic-helper"]):
    HELPER = runpy.run_path(str(ROOT / "platforms/common/tests/capture_only.py"))["Helper"]
GLOBALS = HELPER.wait_cleared.__globals__


class CleanupPollingTests(unittest.TestCase):
    def helper(self, events):
        helper = HELPER.__new__(HELPER)
        helper.events = Mock(side_effect=events)
        helper.records = Mock(return_value=[])
        return helper

    def test_windows_crt_access_denied_retries_until_empty(self):
        helper = self.helper([PermissionError(errno.EACCES, "synthetic busy"),
                              [{"kind": "synthetic"}], []])
        with patch.object(GLOBALS["platform"], "system", return_value="Windows"), \
                patch.object(GLOBALS["time"], "sleep") as sleep:
            helper.wait_cleared()
        self.assertEqual(helper.events.call_count, 3)
        self.assertEqual(sleep.call_count, 2)
        helper.records.assert_called_once()

    def test_native_windows_errors_retry(self):
        for code in (5, 32):
            with self.subTest(code=code):
                error = PermissionError(errno.EACCES, "synthetic busy")
                error.winerror = code
                helper = self.helper([error, []])
                with patch.object(GLOBALS["time"], "sleep"):
                    helper.wait_cleared()
                self.assertEqual(helper.events.call_count, 2)

    def test_other_native_error_propagates(self):
        error = PermissionError(errno.EACCES, "synthetic other error")
        error.winerror = 1314
        helper = self.helper([error])
        with patch.object(GLOBALS["platform"], "system", return_value="Windows"):
            with self.assertRaises(PermissionError):
                helper.wait_cleared()

    def test_unix_access_denied_propagates(self):
        helper = self.helper([PermissionError(errno.EACCES, "synthetic denied")])
        with patch.object(GLOBALS["platform"], "system", return_value="Linux"):
            with self.assertRaises(PermissionError):
                helper.wait_cleared()

    def test_windows_access_denied_still_times_out(self):
        helper = self.helper([PermissionError(errno.EACCES, "synthetic busy")])
        with patch.object(GLOBALS["platform"], "system", return_value="Windows"), \
                patch.object(GLOBALS["time"], "monotonic", side_effect=[0, 1, 11]), \
                patch.object(GLOBALS["time"], "sleep"):
            with self.assertRaisesRegex(AssertionError, "did not clear"):
                helper.wait_cleared()


if __name__ == "__main__":
    unittest.main()
