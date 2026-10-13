#!/usr/bin/env python3
"""Both iOS diagnostic switches share the nested preference binding."""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SETTINGS = ROOT / "platforms/ios/App/Sources/settings"


class DiagnosticLogBindingContract(unittest.TestCase):
    def test_both_pages_use_shared_nested_binding(self):
        for name in ("DiagnosticLogSettingsView.swift", "DeveloperOptionsView.swift"):
            source = (SETTINGS / name).read_text()
            self.assertIn('SharedPreferenceBinding.nested("diagnostic_log", key: "server"', source, name)
            self.assertNotIn('diagnostic["server"] = value', source, name)


if __name__ == "__main__":
    unittest.main()
