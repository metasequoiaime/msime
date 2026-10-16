#!/usr/bin/env python3
"""验证宿主类型迁出入口后，可选回调的按钮门禁仍生效。"""

import contextlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    "settings_action_guard", Path(__file__).with_name("test-settings-action-guard.py")
)
GUARD = importlib.util.module_from_spec(spec)
spec.loader.exec_module(GUARD)


class SettingsActionGuardTests(unittest.TestCase):
    def check(self, button, contract='  openVoice?: () => Promise<void>;'):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "index.tsx"
            client = root / "settings-client.ts"
            source.write_text(button, encoding="utf-8")
            client.write_text("export interface SettingsClient {\n" + contract + "\n}", encoding="utf-8")
            output = io.StringIO()
            errors = io.StringIO()
            with patch.object(GUARD, "ROOT", root), patch.object(GUARD, "SOURCE", source), patch.object(
                GUARD, "CLIENT_SOURCE", client
            ), contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
                result = GUARD.main()
            return result, output.getvalue(), errors.getvalue()

    def test_unguarded_leaf_callback_fails(self):
        result, _, errors = self.check(
            'type Props = {\n  onReplay?: () => void;\n};\n'
            '<button onClick={() => client.openVoice()}>打开</button>'
        )
        self.assertEqual(result, 1)
        self.assertIn("optional client.openVoice", errors)

    def test_guarded_leaf_callbacks_pass(self):
        for button in (
            '<button disabled={!client.openVoice} onClick={() => client.openVoice()}>打开</button>',
            '<button onClick={() => client.openVoice?.()}>打开</button>',
            '{client.openVoice && <button onClick={() => client.openVoice()}>打开</button>}',
        ):
            with self.subTest(button=button):
                result, output, errors = self.check(button)
                self.assertEqual(result, 0, errors)
                self.assertIn("1 optional callbacks", output)

    def test_missing_callbacks_fail(self):
        result, _, errors = self.check("<button>打开</button>", contract="")
        self.assertEqual(result, 1)
        self.assertIn("no optional client callbacks", errors)

    def test_existing_entry_callbacks_remain_checked(self):
        result, output, errors = self.check(
            'type Props = {\n  onReplay?: () => void;\n};\n'
            '<button onClick={() => client.onReplay()}>打开</button>'
        )
        self.assertEqual(result, 1)
        self.assertIn("optional client.onReplay", errors)


if __name__ == "__main__":
    unittest.main()
