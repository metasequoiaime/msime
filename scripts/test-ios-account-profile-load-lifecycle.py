#!/usr/bin/env python3
"""Contract checks for page-owned account profile loading and nickname saves."""
from pathlib import Path
import re
import unittest


SOURCE = Path("platforms/ios/App/Sources/account/AccountProfileView.swift").read_text()
GENERATION = Path("platforms/ios/App/Sources/app/ViewRequestGeneration.swift").read_text()


class AccountProfileLoadLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.profile = SOURCE.split("struct AccountProfileEditor", 1)[1].split(
            "private struct AccountNicknameEditor", 1
        )[0]
        self.nickname = SOURCE.split("private struct AccountNicknameEditor", 1)[1]

    def test_profile_load_is_owned_by_visible_page(self):
        self.assertIn("@State private var loadTask: Task<Void, Never>?", self.profile)
        self.assertIn("@State private var request = ViewRequestGeneration()", self.profile)
        self.assertRegex(self.profile, r"\.task \{\s*request\.appear\(\)\s*startLoad\(\)")
        self.assertIn("loadTask?.cancel()", self.profile)
        self.assertIn("try request.checkCurrent(id)", self.profile)
        self.assertIn("request.disappear()", self.profile)
        self.assertIn("catch is CancellationError", self.profile)

    def test_nickname_save_is_cancelled_when_editor_leaves(self):
        self.assertIn("@State private var operation: Task<Void, Never>?", self.nickname)
        self.assertIn("@State private var request = ViewRequestGeneration()", self.nickname)
        self.assertIn("request.appear()", self.nickname)
        self.assertIn("request.disappear()", self.nickname)
        self.assertIn("operation?.cancel()", self.nickname)
        self.assertIn("try request.checkCurrent(id)", self.nickname)
        self.assertIn("catch is CancellationError", self.nickname)
        self.assertIn("try Task.checkCancellation()", GENERATION)


if __name__ == "__main__":
    unittest.main()
