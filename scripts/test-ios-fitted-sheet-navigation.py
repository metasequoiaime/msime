#!/usr/bin/env python3
"""用 `fittedSheet()` 的 iOS 弹窗必须包在 `NavigationStack` 里。

`NavigationView` 里设的 `presentationDetents` 传不到 sheet 上：真机上个人词库编辑器加了 `fittedSheet()` 以后仍是整屏、没有拖动条，改成 `NavigationStack` 才贴合内容。这里按文件检查：调用了 `fittedSheet(` 的源文件里不能再有 `NavigationView`；精简录音页的 `fittedSheet` 写在 `ServiceSettingsView` 里，外层容器在 App 入口，单独检查。见 `.agents/notes/implemented/feature/2026-10-21-ios-sheets-fit-content.md`。
"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCES = [ROOT / "platforms/ios/App/Sources", ROOT / "shared/backend-ui"]
APP_ENTRY = ROOT / "platforms/ios/App/Sources/MetasequoiaImeApp.swift"


def swift_files():
    for root in SOURCES:
        yield from sorted(root.rglob("*.swift"))


class FittedSheetNavigationContract(unittest.TestCase):
    def test_fitted_sheets_are_not_inside_navigation_view(self):
        offenders = []
        users = 0
        for path in swift_files():
            if path.name == "FittedSheet.swift":
                continue
            text = path.read_text(encoding="utf-8")
            if ".fittedSheet(" not in text:
                continue
            users += 1
            if re.search(r"\bNavigationView\s*\{", text):
                offenders.append(str(path.relative_to(ROOT)))
        self.assertGreater(users, 0, "no fittedSheet() callers found; the contract lost its subject")
        self.assertEqual(offenders, [], "use NavigationStack around sheets that call fittedSheet()")

    def test_voice_recording_sheet_uses_navigation_stack(self):
        text = APP_ENTRY.read_text(encoding="utf-8")
        sheet = re.search(r"\.sheet\(isPresented: \$navigation\.recordsVoice\)\s*\{(?P<body>.*?)\n    \}", text, re.S)
        self.assertIsNotNone(sheet, "could not locate the recordsVoice sheet")
        self.assertIn("NavigationStack {", sheet.group("body"))
        self.assertNotIn("NavigationView", sheet.group("body"))


if __name__ == "__main__":
    unittest.main()
