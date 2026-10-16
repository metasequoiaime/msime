#!/usr/bin/env python3
"""验证偏好字段门禁会展开独立模块中的组合类型。"""

import contextlib
import importlib.util
import io
import pathlib
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location(
    "preferences_parity", pathlib.Path(__file__).with_name("test-preferences-field-parity.py")
)
PARITY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PARITY)


class IntersectionParityTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        root = pathlib.Path(self.directory.name)
        original_ui, original_rust = PARITY.UI, PARITY.RUST
        self.addCleanup(setattr, PARITY, "UI", original_ui)
        self.addCleanup(setattr, PARITY, "RUST", original_rust)
        PARITY.UI = root / "index.tsx"
        PARITY.RUST = root / "preferences.rs"
        PARITY.UI.write_text(
            "export type Preferences = Shared & { scheme: string; child?: { nested: boolean }; };"
        )
        self.shared = root / "shared.ts"
        self.shared.write_text(
            "export type Base = { chinese_punctuation: boolean; };\n"
            "export type Shared = Base & { smart_punctuation?: boolean; };"
        )

    def test_imported_and_transitive_fields_are_checked_without_nested_keys(self):
        bodies = PARITY.page_object_bodies()
        self.assertEqual(
            set(PARITY.ts_members(bodies["Preferences"])),
            {"scheme", "child", "chinese_punctuation", "smart_punctuation"},
        )

    def test_unmapped_field_in_imported_base_still_fails_parity(self):
        PARITY.RUST.write_text(
            "pub struct Preferences {\n    pub scheme: String,\n    pub child: bool,\n}\n"
        )
        errors = io.StringIO()
        with contextlib.redirect_stderr(errors):
            self.assertEqual(PARITY.main(), 1)
        self.assertIn("chinese_punctuation", errors.getvalue())
        self.assertIn("never takes effect", errors.getvalue())

    def preferences_in_leaf_module(self):
        leaf = PARITY.UI.parent / "core" / "preferences-model.ts"
        leaf.parent.mkdir()
        leaf.write_text(PARITY.UI.read_text())
        PARITY.UI.write_text('export type { Preferences } from "./core/preferences-model";')
        return leaf

    def test_preferences_in_leaf_module_still_expands_imported_bases(self):
        self.preferences_in_leaf_module()
        bodies = PARITY.page_object_bodies()
        self.assertEqual(
            set(PARITY.ts_members(bodies["Preferences"])),
            {"scheme", "child", "chinese_punctuation", "smart_punctuation"},
        )

    def test_missing_field_in_leaf_preferences_still_fails_parity(self):
        leaf = self.preferences_in_leaf_module()
        leaf.write_text(leaf.read_text().replace("scheme: string;", ""))
        PARITY.RUST.write_text(
            "pub struct Preferences {\n"
            "    pub scheme: String,\n"
            "    pub child: bool,\n"
            "    pub chinese_punctuation: bool,\n"
            "    pub smart_punctuation: bool,\n"
            "}\n"
        )
        errors = io.StringIO()
        with contextlib.redirect_stderr(errors):
            self.assertEqual(PARITY.main(), 1)
        self.assertIn("['scheme'] exist in client_core::preferences::Preferences", errors.getvalue())

    def test_missing_base_is_rejected_instead_of_skipping_its_fields(self):
        self.shared.write_text("")
        with self.assertRaisesRegex(ValueError, "Shared"):
            PARITY.page_object_bodies()

    def test_cyclic_bases_are_rejected(self):
        self.shared.write_text("export type Shared = Preferences & { flag: boolean; };")
        with self.assertRaisesRegex(ValueError, "Preferences"):
            PARITY.page_object_bodies()


if __name__ == "__main__":
    unittest.main()
