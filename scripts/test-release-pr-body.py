#!/usr/bin/env python3
"""回归验证发布 PR 正文对累计提交、中文长主题及固定内容超限的处理。"""

import importlib.util
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("release_pr_body", ROOT / "scripts/release-pr-body.py")
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
PREFIX = "## 版本号\n\n| macos | 0.52.0 | 0.53.0 |\n\n## 合并方式\n\n请用 merge commit 合并。\n"
URL = "https://github.com/example/project/compare/" + "1" * 40 + "..." + "2" * 40


class ReleasePrBodyTests(unittest.TestCase):
    def assert_structure(self, body: str) -> None:
        self.assertLessEqual(len(body.encode("utf-8")), MODULE.BODY_BYTE_BUDGET)
        self.assertIn(PREFIX.rstrip(), body)
        self.assertEqual(body.count("<details>"), 1)
        self.assertEqual(body.count("</details>"), 1)
        self.assertTrue(body.endswith(f"[查看全部提交与改动]({URL})\n"))

    def test_small_history_keeps_all_subjects(self) -> None:
        body = MODULE.render_body(PREFIX, ["修复甲", "增加乙"], 2, URL)
        self.assert_structure(body)
        self.assertIn("共 2 个提交，展示最近 2 项", body)
        self.assertIn("- 修复甲\n- 增加乙\n", body)

    def test_many_commits_keep_recent_entries_and_total_count(self) -> None:
        subjects = [f"合成提交 {i:04d}" for i in range(2000)]
        body = MODULE.render_body(PREFIX, subjects, len(subjects), URL)
        self.assert_structure(body)
        self.assertIn("共 2000 个提交，展示最近 100 项", body)
        self.assertIn("- 合成提交 0099\n", body)
        self.assertNotIn("合成提交 0100", body)

    def test_unicode_byte_budget_keeps_whole_entries(self) -> None:
        subject = "中文改动😀" * 1000
        body = MODULE.render_body(PREFIX, [subject] * 100, 100, URL)
        self.assert_structure(body)
        self.assertGreater(body.count("- " + subject + "\n"), 0)
        self.assertLess(body.count("- " + subject + "\n"), 100)

    def test_one_oversized_subject_preserves_metadata_and_link(self) -> None:
        body = MODULE.render_body(PREFIX, ["超长主题" * 30000], 1, URL)
        self.assert_structure(body)
        self.assertIn("共 1 个提交，展示最近 0 项", body)

    def test_empty_history(self) -> None:
        body = MODULE.render_body(PREFIX, [], 0, URL)
        self.assert_structure(body)
        self.assertIn("共 0 个提交，展示最近 0 项", body)

    def test_oversized_metadata_fails_before_body_is_written(self) -> None:
        with self.assertRaisesRegex(ValueError, "未推送分支"):
            MODULE.render_body("版本说明" * 30000, [], 0, URL)

    @unittest.skipUnless(shutil.which("git"), "未安装 git，跳过正文 CLI 的临时仓库验证")
    def test_cli_reads_history_and_replaces_the_prefix_file(self) -> None:
        with tempfile.TemporaryDirectory(prefix="msime-release-pr-test-") as temporary:
            directory = Path(temporary)

            def git(*args: str) -> str:
                return subprocess.check_output(["git", *args], cwd=directory, encoding="utf-8").strip()

            git("init", "--quiet")
            git("config", "user.name", "Release Fixture")
            git("config", "user.email", "release-fixture@example.invalid")
            git("-c", "commit.gpgsign=false", "commit", "--quiet", "--allow-empty", "-m", "合成基线")
            since = git("rev-parse", "HEAD")
            for subject in ("合成修复甲", "合成修复乙"):
                git("-c", "commit.gpgsign=false", "commit", "--quiet", "--allow-empty", "-m", subject)
            develop = git("rev-parse", "HEAD")
            prefix_file = directory / "body.md"
            prefix_file.write_text(PREFIX, encoding="utf-8")
            command = [sys.executable, str(ROOT / "scripts/release-pr-body.py"),
                       "--prefix-file", str(prefix_file), "--output", str(prefix_file),
                       "--since", since, "--develop", develop, "--commit-count", "2",
                       "--repo-url", "https://github.com/example/project/"]
            subprocess.run(command, cwd=directory, check=True, capture_output=True, text=True)
            body = prefix_file.read_text(encoding="utf-8")
            self.assertLessEqual(len(body.encode("utf-8")), MODULE.BODY_BYTE_BUDGET)
            self.assertIn("- 合成修复乙\n- 合成修复甲\n", body)
            self.assertIn(f"https://github.com/example/project/compare/{since}...{develop}", body)

            # 同一个输出文件已经有超长固定内容时，生成失败不得覆盖它。
            oversized = "版本说明" * 30000
            prefix_file.write_text(oversized, encoding="utf-8")
            result = subprocess.run(command, cwd=directory, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("未推送分支", result.stderr)
            self.assertEqual(prefix_file.read_text(encoding="utf-8"), oversized)


if __name__ == "__main__":
    unittest.main()
