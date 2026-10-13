#!/usr/bin/env python3
"""Contract checks for login sheet work that must stop when its view disappears."""
from pathlib import Path
import unittest


SOURCE = Path("platforms/ios/App/Sources/account/AccountCodeLoginView.swift").read_text()
SETTINGS = Path("platforms/ios/App/Sources/account/AccountSettingsView.swift").read_text()
NAVIGATION = Path("platforms/ios/App/Sources/app/AppNavigation.swift").read_text()


class LoginSheetLifecycleTests(unittest.TestCase):
    def test_model_invalidates_inflight_login_work(self):
        model = SOURCE.split("enum CodeLoginChannel", 1)[0]
        self.assertIn("private var generation = UUID()", model)
        self.assertIn("func invalidate()", model)
        self.assertIn("generation = UUID()", model)
        self.assertIn("guard generation == id else { throw CancellationError() }", model)
        self.assertIn("catch is CancellationError", model)

    def test_code_login_view_invalidates_model_and_pending_task(self):
        view = SOURCE.split("struct CodeLoginView", 1)[1]
        self.assertIn("model.invalidate()", view)
        self.assertIn("pending?.cancel()", view)
        self.assertIn(".onDisappear", view)

    def test_login_sheet_invalidates_model_when_dismissed(self):
        sheet = NAVIGATION.split("struct AccountLoginSheet", 1)[1].split(
            "struct AccountLoginOptions", 1
        )[0]
        self.assertIn(".onDisappear { model.invalidate() }", sheet)

    def test_account_section_owns_prepare_and_reload_tasks(self):
        section = SETTINGS.split("struct AppleAccountSection", 1)[1].split(
            "struct AccountProfileEditor", 1
        )[0]
        self.assertIn("@State private var pending: Task<Void, Never>?", section)
        self.assertIn("@State private var request = ViewRequestGeneration()", section)
        self.assertIn("request.appear()", section)
        self.assertIn("request.disappear()", section)
        self.assertIn("pending?.cancel()", section)
        self.assertIn(".onDisappear", section)
        self.assertIn("try request.checkCurrent(id)", section)


if __name__ == "__main__":
    unittest.main()
