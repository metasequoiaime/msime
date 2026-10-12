#!/usr/bin/env python3
"""The keyboard test target must compile DesignKit's group header dependency."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
DESIGN = (ROOT / "platforms/ios/App/Sources/app/DesignKit.swift").read_text(encoding="utf-8")
SETTINGS = (ROOT / "platforms/ios/App/Sources/app/SettingsDesign.swift").read_text(encoding="utf-8")
PROJECT = (ROOT / "platforms/ios/project.yml").read_text(encoding="utf-8")


class IOSSettingsGroupHeaderLinkage(unittest.TestCase):
    def test_header_is_defined_in_designkit_built_by_keyboard_tests(self):
        test_sources = PROJECT.split("  MSIMEKeyboardTests:", 2)[-1].split("  MSIMEServiceTests:", 1)[0]
        self.assertIn("path: App/Sources/app/DesignKit.swift", test_sources)
        self.assertIn("struct SettingsGroupHeader: View", DESIGN)
        self.assertNotIn("struct SettingsGroupHeader: View", SETTINGS)


if __name__ == "__main__":
    unittest.main()
