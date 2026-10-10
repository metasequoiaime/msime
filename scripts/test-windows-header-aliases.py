#!/usr/bin/env python3
"""在真实文件系统中验证 MinGW 头文件别名，所有输入均为合成值。"""

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
HELPER = ROOT / "platforms/windows/cross/add-header-aliases.py"


class HeaderAliasesTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def run_helper(self, *directories):
        result = subprocess.run(
            [sys.executable, str(HELPER), *map(str, directories)],
            capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_relative_aliases_keep_the_remainder_and_are_idempotent(self):
        for name in ("windows.h", "winUser.h", "a space.h"):
            (self.root / name).write_text("synthetic", encoding="utf-8")
        self.run_helper(self.root)
        for source, alias in (("windows.h", "Windows.h"), ("winUser.h", "WinUser.h"), ("a space.h", "A space.h")):
            self.assertEqual(os.readlink(self.root / alias), source)
            self.assertEqual((self.root / alias).read_text(), "synthetic")
        self.run_helper(self.root)
        self.assertEqual(len(list(self.root.iterdir())), 6)

    def test_existing_entries_are_preserved_including_broken_aliases(self):
        for name in ("custom.h", "linked.h", "broken.h"):
            (self.root / name).write_text("lower", encoding="utf-8")
        (self.root / "Custom.h").write_text("original", encoding="utf-8")
        (self.root / "Linked.h").symlink_to("custom.h")
        (self.root / "Broken.h").symlink_to("missing.h")
        self.run_helper(self.root)
        self.assertEqual((self.root / "Custom.h").read_text(), "original")
        self.assertEqual(os.readlink(self.root / "Linked.h"), "custom.h")
        self.assertEqual(os.readlink(self.root / "Broken.h"), "missing.h")

    def test_only_top_level_ascii_lowercase_h_entries_are_selected(self):
        for name in ("Upper.h", "1digit.h", "_private.h", ".hidden.h", "lower.hpp", "upper.H", "éclair.h"):
            (self.root / name).write_text("synthetic", encoding="utf-8")
        (self.root / "dangling.h").symlink_to("absent")
        (self.root / "nested").mkdir()
        (self.root / "nested/inside.h").write_text("synthetic", encoding="utf-8")
        before = set(self.root.iterdir())
        self.run_helper(self.root)
        self.assertEqual(set(self.root.iterdir()), before)
        self.assertFalse((self.root / "nested/Inside.h").exists())

    def test_valid_source_links_and_h_directories_keep_relative_targets(self):
        (self.root / "actual.txt").write_text("synthetic", encoding="utf-8")
        (self.root / "source.h").symlink_to("actual.txt")
        (self.root / "folder.h").mkdir()
        self.run_helper(self.root)
        self.assertEqual(os.readlink(self.root / "Source.h"), "source.h")
        self.assertEqual((self.root / "Source.h").read_text(), "synthetic")
        self.assertEqual(os.readlink(self.root / "Folder.h"), "folder.h")
        self.assertTrue((self.root / "Folder.h").is_dir())

    def test_multiple_missing_and_non_directory_paths(self):
        first = self.root / "first"
        second = self.root / "second"
        first.mkdir()
        second.mkdir()
        (first / "one.h").write_text("one", encoding="utf-8")
        (second / "two.h").write_text("two", encoding="utf-8")
        plain = self.root / "plain"
        plain.write_text("synthetic", encoding="utf-8")
        self.run_helper(first, self.root / "missing", plain, second)
        self.assertEqual(os.readlink(first / "One.h"), "one.h")
        self.assertEqual(os.readlink(second / "Two.h"), "two.h")


def main():
    if os.name != "posix":
        print("skipped: MinGW 镜像别名回归需要 POSIX 文件系统")
        return 0
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        (root / "case-probe").touch()
        if (root / "CASE-PROBE").exists():
            print("skipped: 文件系统不区分大小写；在 Linux 容器中运行头文件别名回归")
            return 0
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(HeaderAliasesTests)
    return 0 if unittest.TextTestRunner().run(suite).wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
