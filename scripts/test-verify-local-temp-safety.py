#!/usr/bin/env python3
"""临时文件创建失败时必须停止门禁，清理不得匹配工作区隐藏文件。"""

from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "scripts/verify-local.sh").read_text(encoding="utf-8")
STARTUP = SOURCE[SOURCE.index("failed=0\n"):SOURCE.index("note() {")]


@unittest.skipUnless(shutil.which("bash"), "缺少 bash")
class TempSafety(unittest.TestCase):
    def run_startup(self, fail_at, body):
        with tempfile.TemporaryDirectory(prefix="verify temp safety ") as directory:
            root = Path(directory)
            (root / "owned").mkdir()
            for name in [".git", ".gitignore", "keep.txt"]:
                (root / name).write_text("synthetic sentinel")
            shell = r'''
mktemp() {
  local calls=0
  [ ! -f calls ] || calls=$(cat calls)
  calls=$((calls + 1))
  printf '%s' "$calls" > calls
  if [ "$calls" = "$FAIL_AT" ]; then
    printf 'synthetic mktemp failure\n' >&2
    return 1
  fi
  local result="$PWD/owned/temp-$calls"
  : > "$result"
  printf '%s\n' "$result"
}
'''
            environment = {"PATH": os.environ["PATH"] if os.name == "nt" else "/usr/bin:/bin",
                           "FAIL_AT": str(fail_at)}
            if os.name == "nt":
                for key in ("SystemRoot", "TEMP", "TMP"):
                    if key in os.environ:
                        environment[key] = os.environ[key]
            result = subprocess.run(
                ["bash", "-c", shell + STARTUP + body],
                cwd=root,
                env=environment,
                capture_output=True,
                text=True,
                # Windows 上还要启动 Git Bash 及其外部工具，断言仍逐项核对清理结果。
                timeout=30 if os.name == "nt" else 5,
            )
            self.assertTrue((root / ".git").exists(), "清理删除了工作区 .git")
            self.assertTrue((root / ".gitignore").exists(), "清理删除了工作区隐藏文件")
            self.assertTrue((root / "keep.txt").exists())
            self.assertEqual(list((root / "owned").iterdir()), [], "临时文件未清理")
            return result, (root / "continued").exists()

    def test_first_temp_failure_stops_before_checks(self):
        result, continued = self.run_startup(1, "touch continued")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(continued, "临时文件创建失败后仍继续门禁")

    def test_second_temp_failure_cleans_the_first_file(self):
        result, continued = self.run_startup(2, "touch continued")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(continued)

    def test_success_cleans_suffix_files_and_preserves_workspace(self):
        result, continued = self.run_startup(0, ': > "$collected.log"\ntouch continued')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(continued)

    def test_empty_collector_never_expands_hidden_workspace_paths(self):
        result, continued = self.run_startup(0, 'rm -- "$collected"\ncollected=""\ntouch continued')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(continued)


if __name__ == "__main__":
    unittest.main()
