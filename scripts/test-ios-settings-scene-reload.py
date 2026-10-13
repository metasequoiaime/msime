#!/usr/bin/env python3
"""确保设置页共用出现和回到前台时的重新加载修饰符。"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
TARGETS = [
    ROOT / "platforms/ios/App/Sources/settings" / name
    for name in (
        "VoiceSettingsView.swift", "HandwritingSettingsView.swift", "ExpressionSettingsView.swift",
        "CandidateOptionsSettingsView.swift", "DeveloperOptionsView.swift", "ClipboardHistorySettingsView.swift",
        "DiagnosticLogSettingsView.swift", "LocalModeSettingsView.swift", "DictionarySettingsView.swift",
        "PunctuationSettingsView.swift", "CustomThemeCandidateSection.swift", "TypingStatisticsView.swift",
    )
] + [ROOT / "platforms/ios/App/Sources/keyboard/KeyboardToolbarSettingsView.swift"]
APP_ICON = ROOT / "platforms/ios/App/Sources/settings/AppIconSettingsView.swift"


class SettingsSceneReloadContract(unittest.TestCase):
    def test_all_settings_pages_use_shared_reload_modifier(self):
        for path in TARGETS:
            source = path.read_text()
            self.assertIn(".reloadOnAppearAndActive(reload)", source, path.name)
            self.assertNotIn("@Environment(\\.scenePhase)", source, path.name)

    def test_modifier_owns_both_lifecycle_hooks(self):
        source = (ROOT / "platforms/ios/App/Sources/settings/SettingsScenePhaseReload.swift").read_text()
        self.assertIn(".onAppear(perform: reload)", source)
        self.assertIn(".onChange(of: scenePhase)", source)
        self.assertIn("phase == .active", source)

    def test_app_icon_uses_shared_reload_modifier(self):
        source = APP_ICON.read_text()
        self.assertIn(".reloadOnAppearAndActive(model.refresh)", source)
        self.assertNotIn("@Environment(\\.scenePhase)", source)


if __name__ == "__main__":
    unittest.main()
