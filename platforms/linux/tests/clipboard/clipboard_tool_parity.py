#!/usr/bin/env python3
"""Synthetic Windows/shared-store text parity through the installed history CLI."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

TOOL = str(Path(sys.argv.pop(1)).resolve())


class ClipboardTool(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="msime-history-")
        self.addCleanup(directory.cleanup)
        self.path = Path(directory.name) / "history.json"

    def run_tool(self, operation, *args, data=None):
        return subprocess.run([TOOL, str(self.path), operation, *args], input=data,
                              capture_output=True, timeout=3)

    def test_preserves_unicode_limit_and_whitespace(self):
        for text in ("汉" * 4001, "a" * 3999 + "🙂", "🙂" * 2001,
                     "synthetic\r\nline\n \t", "synthetic\r\0"):
            with self.subTest(length=len(text)):
                normalized = text.rstrip("\0\r").encode("utf-16-le")[:8000].decode("utf-16-le", errors="ignore")
                result = self.run_tool("add-stdin", data=text.encode())
                self.assertEqual(result.returncode, 0)
                self.assertEqual(self.run_tool("get", "0").stdout, normalized.encode())
                self.assertEqual(json.loads(self.path.read_text())[0], normalized)

    def test_existing_history_survives_unrelated_mutation(self):
        original = ["汉" * 4000, "🙂" * 2000, "synthetic\r\nline\n \t"]
        self.path.write_text(json.dumps(original))
        self.assertEqual(json.loads(self.run_tool("list").stdout), original)
        self.assertEqual(self.run_tool("add", "synthetic new").returncode, 0)
        self.assertEqual(self.run_tool("remove-index", "0").returncode, 0)
        self.assertEqual(json.loads(self.path.read_text()), original)

    def test_invalid_input_does_not_change_history(self):
        original = '["synthetic saved"]'
        self.path.write_text(original)
        for data in (b"synthetic\0embedded", b"\xff"):
            result = self.run_tool("add-stdin", data=data)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stderr, b"")
            self.assertEqual(self.path.read_text(), original)

    def test_index_operations_reject_trailing_characters(self):
        self.path.write_text('["synthetic saved"]')
        for operation in ("get", "remove-index"):
            result = self.run_tool(operation, "0junk")
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stderr, b"")
        self.assertEqual(json.loads(self.path.read_text()), ["synthetic saved"])

    def test_lock_symlink_is_rejected(self):
        outside = self.path.parent / "outside.lock"
        outside.write_text("synthetic lock target")
        Path(str(self.path) + ".lock").symlink_to(outside)
        result = self.run_tool("list")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        self.assertEqual(outside.read_text(), "synthetic lock target")
        self.assertFalse(self.path.exists())

    def test_parent_symlink_is_rejected_before_locking(self):
        outside = self.path.parent / "outside-state"
        outside.mkdir()
        linked = self.path.parent / "linked-state"
        linked.symlink_to(outside, target_is_directory=True)
        linked_path = linked / "history.json"
        result = subprocess.run([TOOL, str(linked_path), "list"],
                                capture_output=True, timeout=3)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        self.assertFalse((outside / "history.json.lock").exists())

    def test_add_does_not_create_below_parent_symlink(self):
        outside = self.path.parent / "outside-state"
        outside.mkdir()
        linked = self.path.parent / "linked-state"
        linked.symlink_to(outside, target_is_directory=True)
        # 以 root 身份运行时（Linux 容器里就是这样），root 自己不对外开放的目录里的链接会被当成受信任的系统链接（见 `src/core/SafePath.h`）；把目录改成其他人可写，这条链接就成了任何人都可能放进去的链接。
        self.path.parent.chmod(0o777)
        linked_path = linked / "new-dir" / "history.json"
        result = subprocess.run([TOOL, str(linked_path), "add", "synthetic"],
                                capture_output=True, timeout=3)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        self.assertFalse((outside / "new-dir").exists())


if __name__ == "__main__":
    unittest.main()
