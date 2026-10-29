#!/usr/bin/env python3
"""Contract checks for the explicit login consent on iOS (.agents/notes/implemented/bug-fix/2026-10-27-login-privacy-consent.md).

The consent rule itself is covered by LoginConsentPolicyTests and the UI test testLoginRequiresExplicitConsent, but that UI test skips when the live account service does not offer email sign-in, so a new sign-in button that forgets `confirmConsent()` would pass CI. These checks pin the wiring in the source instead: every sign-in request in AccountLoginOptions passes `confirmConsent()` first, the phone sheet only opens after it, and the model's sign-in methods are called from nowhere else.
"""
from pathlib import Path
import re
import unittest


SOURCES = Path("platforms/ios/App/Sources")
NAVIGATION_PATH = SOURCES / "app/AppNavigation.swift"
CODE_LOGIN_PATH = SOURCES / "account/AccountCodeLoginView.swift"
NAVIGATION = NAVIGATION_PATH.read_text()
POLICY = (SOURCES / "account/LoginConsentPolicy.swift").read_text()
SHARED = Path("packages/ui/src/account/login-consent.tsx").read_text()

SIGN_IN_CALL = re.compile(r"\bmodel\.(signInWithApple|signInWithGoogle|requestCode|signInWithCode)\(")
# The line that opens the closure or function a sign-in request runs in.
SCOPE_START = re.compile(r"\bfunc \w+\(|\bButton \{|onCompletion: \{")


def enclosing_scope(lines, index):
    """Lines from the nearest enclosing closure or function start down to `index`."""
    for start in range(index, -1, -1):
        if SCOPE_START.search(lines[start]):
            return lines[start:index + 1]
    raise AssertionError(f"no enclosing closure or function above line {index + 1}: {lines[index].strip()}")


class LoginConsentTests(unittest.TestCase):
    def options(self):
        return NAVIGATION.split("struct AccountLoginOptions", 1)[1]

    def test_no_implied_consent_wording(self):
        self.assertNotIn("登录即", NAVIGATION)
        self.assertIn("consentRow", self.options())

    def test_every_sign_in_request_confirms_consent_first(self):
        lines = self.options().splitlines()
        calls = [index for index, line in enumerate(lines) if SIGN_IN_CALL.search(line)]
        # Apple, Google, sending the email code and signing in with it.
        self.assertEqual(len(calls), 4)
        for index in calls:
            scope = enclosing_scope(lines, index)
            self.assertTrue(any("guard confirmConsent()" in line for line in scope),
                            f"sign-in request without a consent check first: {lines[index].strip()}")

    def test_phone_sheet_opens_only_after_consent(self):
        lines = self.options().splitlines()
        opens = [index for index, line in enumerate(lines) if "phoneChannel = .phone" in line]
        self.assertEqual(len(opens), 1)
        scope = enclosing_scope(lines, opens[0])
        self.assertTrue(any("guard confirmConsent()" in line for line in scope))

    def test_sign_in_methods_called_only_from_the_login_views(self):
        allowed = {NAVIGATION_PATH, CODE_LOGIN_PATH}
        for path in SOURCES.rglob("*.swift"):
            if path in allowed:
                continue
            self.assertIsNone(re.search(r"\.(signInWithApple|signInWithGoogle|requestCode|signInWithCode)\(", path.read_text()),
                              f"{path} starts a sign-in outside the consent-gated login views")

    def test_wording_matches_shared_login_consent(self):
        self.assertIn('export const LOGIN_CONSENT_PROMPT = "请先阅读并勾选同意《用户协议》和《隐私政策》";', SHARED)
        self.assertIn('aria-label="我已阅读并同意《用户协议》和《隐私政策》"', SHARED)
        self.assertIn('static let termsTitle = "《用户协议》"', POLICY)
        self.assertIn('static let privacyTitle = "《隐私政策》"', POLICY)
        self.assertIn('static let text = "我已阅读并同意\\(termsTitle)和\\(privacyTitle)"', POLICY)
        self.assertIn('static let prompt = "请先阅读并勾选同意\\(termsTitle)和\\(privacyTitle)"', POLICY)
        self.assertIn('URL(string: "https://msime.app/terms/")', POLICY)
        self.assertIn('URL(string: "https://msime.app/privacy/")', POLICY)


if __name__ == "__main__":
    unittest.main()
