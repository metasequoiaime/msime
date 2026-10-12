#!/usr/bin/env python3
"""Contract checks for page-owned account settings operations."""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/account/AccountSettingsView.swift").read_text()


class AccountSettingsLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.view = SOURCE.split("struct AccountSettingsView", 1)[1].split(
            "struct AppleAccountSection", 1
        )[0]

    def test_reload_is_owned_by_visible_page(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", self.view)
        self.assertIn("@State private var operation: Task<Void, Never>?", self.view)
        self.assertIn("@State private var requestID = UUID()", self.view)
        self.assertIn("@State private var visible = false", self.view)
        self.assertIn("startReload()", self.view)
        self.assertIn("pending?.cancel()", self.view)
        self.assertIn("operation?.cancel()", self.view)
        self.assertIn("guard visible, requestID == id else { throw CancellationError() }", self.view)
        self.assertIn("catch is CancellationError", self.view)

    def test_account_actions_and_sync_ignore_late_results(self):
        self.assertIn("operation = Task", self.view)
        self.assertIn("guard visible, requestID == id else { return }", self.view)
        self.assertIn("syncTask", self.view)
        self.assertIn("syncGeneration", self.view)
        self.assertIn("syncTask?.cancel()", self.view)


if __name__ == "__main__":
    unittest.main()
