#!/usr/bin/env python3
"""偏好覆盖扫描按路径展开同类型兄弟项、可选结构，并在递归环处收尾。"""

import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "preference_coverage", ROOT / "platforms/macos/tests/settings/preference_coverage.py"
)
COVERAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COVERAGE)


class PreferenceCoverageTest(unittest.TestCase):
    def test_shared_struct_is_checked_under_each_owner(self):
        structs = COVERAGE.struct_fields("""
pub struct Preferences {
    pub first: Shared,
    pub second: Shared,
}
pub struct Shared {
    pub probe: bool,
}
""")
        self.assertEqual(
            COVERAGE.preference_paths(structs, set()),
            ["first", "first.probe", "second", "second.probe"],
        )

    def test_optional_struct_is_checked_but_lists_are_not_unwrapped(self):
        structs = {
            "Preferences": [("design", "Option<Shared>"), ("list", "Vec<Shared>")],
            "Shared": [("probe", "bool")],
        }
        self.assertEqual(
            COVERAGE.preference_paths(structs, set()), ["design", "design.probe", "list"]
        )

    def test_cycle_stops_at_current_path_without_skipping_siblings(self):
        structs = {
            "Preferences": [("first", "Shared"), ("second", "Shared")],
            "Shared": [("cycle", "Option<Shared>"), ("probe", "bool")],
        }
        self.assertEqual(
            COVERAGE.preference_paths(structs, set()),
            ["first", "first.cycle", "first.probe", "second", "second.cycle", "second.probe"],
        )

    def test_exemption_applies_only_to_its_owner_path(self):
        structs = {
            "Preferences": [("first", "Shared"), ("second", "Shared")],
            "Shared": [("probe", "bool")],
        }
        self.assertEqual(
            COVERAGE.preference_paths(structs, {"first"}),
            ["first", "second", "second.probe"],
        )


if __name__ == "__main__":
    unittest.main()
