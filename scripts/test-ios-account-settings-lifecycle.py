#!/usr/bin/env python3
"""Contract checks for page-owned account settings operations."""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/account/AccountSettingsView.swift").read_text()
GENERATION = Path("platforms/ios/App/Sources/app/ViewRequestGeneration.swift").read_text()
TASK = Path("platforms/ios/App/Sources/app/ViewRequestTask.swift").read_text()


class AccountSettingsLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.view = SOURCE.split("struct AccountSettingsView", 1)[1].split(
            "struct AppleAccountSection", 1
        )[0]
        self.login = SOURCE.split("struct AppleAccountSection", 1)[1].split("struct MeRow", 1)[0]

    def test_reload_is_owned_by_visible_page(self):
        self.assertIn("@State private var pending: Task<Void, Never>?", self.view)
        self.assertIn("@State private var operation: Task<Void, Never>?", self.view)
        self.assertIn("@State private var request = ViewRequestGeneration()", self.view)
        self.assertIn("request.appear()", self.view)
        self.assertIn("request.disappear()", self.view)
        self.assertIn("startReload()", self.view)
        self.assertIn("pending?.cancel()", self.view)
        self.assertIn("operation?.cancel()", self.view)
        self.assertIn("try request.checkCurrent(id)", self.view)
        self.assertIn("guard request.isCurrent(id)", self.view)
        self.assertIn("catch is CancellationError", self.view)

    def test_account_actions_and_sync_ignore_late_results(self):
        self.assertIn("ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $operation,", self.view)
        self.assertIn("canceling: $pending, clearMessage: false,", self.view)
        self.assertIn("isCancellation: AccountRequestPolicy.isCancellation", self.view)
        self.assertEqual(self.view.count("let id = request.begin()"), 1)
        self.assertIn("syncTask", self.view)
        self.assertIn("@State private var syncGeneration = OperationGeneration()", self.view)
        self.assertIn("syncGeneration.invalidate()", self.view)
        self.assertEqual(self.view.count("let id = syncGeneration.begin()"), 2)
        self.assertIn("syncTask?.cancel()", self.view)

    def test_login_section_uses_shared_page_generation(self):
        self.assertIn("@State private var request = ViewRequestGeneration()", self.login)
        self.assertIn("request.appear()", self.login)
        self.assertIn("request.disappear()", self.login)
        self.assertIn("ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending,", self.login)
        self.assertIn("canceling: $pending, clearMessage: false,", self.login)
        self.assertIn("isCancellation: AccountRequestPolicy.isCancellation", self.login)
        self.assertEqual(self.login.count("let id = request.begin()"), 1)
        self.assertIn("try request.checkCurrent(id)", self.login)
        self.assertIn("model.invalidate()", self.login)
        self.assertIn("try Task.checkCancellation()", GENERATION)
        self.assertIn("try request.wrappedValue.checkCurrent(id)", TASK)


if __name__ == "__main__":
    unittest.main()
