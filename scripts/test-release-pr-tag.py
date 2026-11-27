#!/usr/bin/env python3
"""release-pr.sh --tag 的契约：每个发版 tag 单独推一次（GitHub 对一次推上来超过三个的 tag 不产生 push 事件，发布工作流一个都不会启动），推上去的是附注 tag；--beta bare 打不带编号的 -beta；Android 同一个 X.Y.Z 只打一个测试版 tag。用一个带 pre-receive 钩子的本地裸仓库当远端，记下每次推送带了几个 ref。"""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/release-pr.sh"
PLATFORMS = ["android", "ios", "macos", "linux", "windows"]
HOOK = """#!/bin/sh
count=0
while read -r old new ref; do count=$((count + 1)); done
echo "$count" >> "$PUSH_LOG"
"""


@unittest.skipUnless(shutil.which("git") and shutil.which("bash"), "needs git and bash")
class ReleasePrTagTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        self.log = self.tmp / "pushes.log"
        self.env = {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "HOME": str(self.tmp),
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_AUTHOR_NAME": "Release Test",
            "GIT_AUTHOR_EMAIL": "release-test@example.invalid",
            "GIT_COMMITTER_NAME": "Release Test",
            "GIT_COMMITTER_EMAIL": "release-test@example.invalid",
            "PUSH_LOG": str(self.log),
        }
        self.remote = self.tmp / "remote.git"
        self.work = self.tmp / "work"
        self.git("init", "--quiet", "--bare", str(self.remote), cwd=self.tmp)
        hook = self.remote / "hooks/pre-receive"
        hook.write_text(HOOK)
        hook.chmod(0o755)
        self.git("init", "--quiet", "-b", "main", str(self.work), cwd=self.tmp)
        for platform in PLATFORMS:
            path = self.work / "platforms" / platform / "version.txt"
            path.parent.mkdir(parents=True)
            path.write_text("1.0.0\n")
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", "initial")
        self.git("remote", "add", "origin", str(self.remote))
        self.git("push", "--quiet", "origin", "main")
        self.log.unlink()

    def git(self, *args: str, cwd: Path | None = None) -> str:
        result = subprocess.run(["git", *args], cwd=cwd or self.work, env=self.env, capture_output=True, text=True, timeout=60)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout

    def release(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(["bash", str(SCRIPT), "--tag", *args], cwd=self.work, env=self.env, capture_output=True, text=True, timeout=120)

    def remote_tags(self) -> list[str]:
        return sorted(self.git("for-each-ref", "--format=%(refname:short) %(objecttype)", "refs/tags", cwd=self.remote).splitlines())

    def test_every_tag_is_pushed_on_its_own(self) -> None:
        result = self.release(*PLATFORMS)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.log.read_text().split(), ["1"] * len(PLATFORMS))
        self.assertEqual(self.remote_tags(), sorted(f"{platform}-v1.0.0 tag" for platform in PLATFORMS))

    def test_bare_beta_and_numbered_beta(self) -> None:
        result = self.release("--beta", "bare", "linux")
        self.assertEqual(result.returncode, 0, result.stderr)
        result = self.release("--beta", "2", "windows")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.remote_tags(), ["linux-v1.0.0-beta tag", "windows-v1.0.0-beta.2 tag"])
        result = self.release("--beta", "01", "linux")
        self.assertEqual(result.returncode, 2)

    def test_android_takes_one_beta_per_version(self) -> None:
        result = self.release("--beta", "1", "android")
        self.assertEqual(result.returncode, 0, result.stderr)
        for beta in ("2", "bare"):
            result = self.release("--beta", beta, "android")
            self.assertEqual(result.returncode, 1, result.stdout)
            self.assertIn("Android", result.stderr)
        self.assertEqual(self.remote_tags(), ["android-v1.0.0-beta.1 tag"])

    def test_dry_run_pushes_nothing(self) -> None:
        result = self.release("--dry-run", *PLATFORMS)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(self.log.exists())
        self.assertEqual(self.remote_tags(), [])


if __name__ == "__main__":
    unittest.main()
