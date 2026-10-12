#!/usr/bin/env python3
"""Contract checks for page-owned account profile loading and nickname saves."""
from pathlib import Path
import re
import unittest


SOURCE = Path("platforms/ios/App/Sources/account/AccountProfileView.swift").read_text()


class AccountProfileLoadLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.profile = SOURCE.split("struct AccountProfileEditor", 1)[1].split(
            "private struct AccountNicknameEditor", 1
        )[0]
        self.nickname = SOURCE.split("private struct AccountNicknameEditor", 1)[1]

    def test_profile_load_is_owned_by_visible_page(self):
        self.assertIn("@State private var loadTask: Task<Void, Never>?", self.profile)
        self.assertIn("@State private var loadGeneration = UUID()", self.profile)
        self.assertIn("@State private var visible = false", self.profile)
        self.assertRegex(self.profile, r"\.task \{\s*visible = true\s*startLoad\(\)")
        self.assertIn("loadTask?.cancel()", self.profile)
        self.assertIn("guard visible, loadGeneration == id else { throw CancellationError() }", self.profile)
        self.assertIn("catch is CancellationError", self.profile)

    def test_nickname_save_is_cancelled_when_editor_leaves(self):
        self.assertIn("@State private var operation: Task<Void, Never>?", self.nickname)
        self.assertIn("@State private var saveGeneration = UUID()", self.nickname)
        self.assertIn("@State private var visible = false", self.nickname)
        self.assertIn("saveGeneration = UUID()", self.nickname)
        self.assertIn("operation?.cancel()", self.nickname)
        self.assertIn("guard visible, saveGeneration == id, !Task.isCancelled else { return }", self.nickname)
        self.assertIn("catch is CancellationError", self.nickname)


if __name__ == "__main__":
    unittest.main()
