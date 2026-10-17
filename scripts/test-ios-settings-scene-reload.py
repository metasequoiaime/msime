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
REQUEST_AWARE = [
    ROOT / "platforms/ios/App/Sources/settings" / name
    for name in ("CommonPhrasesSettingsView.swift", "HelpcodeSettingsView.swift")
]


class SettingsSceneReloadContract(unittest.TestCase):
    def test_all_settings_pages_use_shared_reload_modifier(self):
        for path in TARGETS:
            source = path.read_text()
            self.assertIn(".reloadOnAppearAndActive(reload)", source, path.name)
            self.assertNotIn("@Environment(\\.scenePhase)", source, path.name)

    def test_modifier_owns_both_lifecycle_hooks(self):
        source = (ROOT / "platforms/ios/App/Sources/settings/SettingsScenePhaseReload.swift").read_text()
        self.assertEqual(source.count("@Environment(\\.scenePhase)"), 1)
        self.assertEqual(source.count(".onChange(of: scenePhase)"), 1)
        self.assertIn("prepare?()\n        reload()", source)
        self.assertIn("phase == .active", source)

    def test_app_icon_uses_shared_reload_modifier(self):
        source = APP_ICON.read_text()
        self.assertIn(".reloadOnAppearAndActive(model.refresh)", source)
        self.assertNotIn("@Environment(\\.scenePhase)", source)

    def test_request_aware_pages_use_shared_reload_modifier(self):
        for path in REQUEST_AWARE:
            source = path.read_text()
            self.assertIn(".reloadOnAppearAndActive(reload, onAppear: { request.appear() })", source, path.name)
            self.assertNotIn("@Environment(\\.scenePhase)", source, path.name)

    def test_appearance_and_layout_pages_use_shared_reload_modifier(self):
        for name, reload in (
            ("SkinAppearanceSettingsView.swift", "reloadThemes"),
            ("KeyboardLayoutSettingsView.swift", "readPreferences"),
        ):
            folder = "keyboard" if name.startswith("Keyboard") else "settings"
            path = ROOT / "platforms/ios/App/Sources" / folder / name
            source = path.read_text()
            self.assertIn(f".reloadOnAppearAndActive({reload})", source, name)
            self.assertNotIn("@Environment(\\.scenePhase)", source, name)

    def test_tablet_settings_reads_scheme_once_before_routing_on_appear(self):
        path = ROOT / "platforms/ios/App/Sources/keyboard/TabletSettingsView.swift"
        source = path.read_text()
        self.assertIn(
            ".reloadOnAppearAndActive {\n      scheme = InputSchemePreference.scheme\n      show(router.settingsPage)\n    }",
            source,
        )
        self.assertNotIn("@Environment(\\.scenePhase)", source)
        self.assertNotIn(".onChange(of: scenePhase)", source)

    def test_skin_settings_uses_shared_reload_modifier(self):
        path = ROOT / "platforms/ios/App/Sources/settings/FeatureSettingsViews.swift"
        source = path.read_text()
        self.assertIn(".reloadOnAppearAndActive({ reloadLibrary(); reloadThemes() })", source)

    def test_input_settings_uses_shared_reload_modifier(self):
        path = ROOT / "platforms/ios/App/Sources/app/OnboardingView.swift"
        source = path.read_text()
        self.assertIn(".reloadOnAppearAndActive({", source)
        self.assertIn("resourcePacks.reload()", source)


if __name__ == "__main__":
    unittest.main()
