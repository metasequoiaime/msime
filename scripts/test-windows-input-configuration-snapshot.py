#!/usr/bin/env python3
"""检查 Windows 输入入口只消费已发布的配置，后台实现另有行为回归。"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
TSF = ROOT / "platforms/windows/tsf"


class InputConfigurationSnapshot(unittest.TestCase):
    def test_key_and_focus_helpers_do_not_read_files(self):
        source = (TSF / "Utils/FanyUtils.cpp").read_text()
        start = source.index("ReadSharedPreferences()")
        end = source.index("std::optional<GameCompatibility> ReadConfiguredGameCompatibility()", start)
        helpers = source[start:end]
        self.assertIn("cached_host_configuration()", helpers)
        for forbidden in ("msime_client_load_preferences", "msime_client_try_load_preferences",
                          "default_host_options_json", "default_state_directory", "is_regular_file"):
            self.assertNotIn(forbidden, helpers)

    def test_timer_uses_worker_and_applies_snapshot_on_owner(self):
        source = (TSF / "IME/MetasequoiaIME.cpp").read_text()
        start = source.index("if (wParam == TIMER_REFRESH_HOST_PREFERENCES)")
        end = source.index("if (wParam == TIMER_DEFERRED_FOCUS_LOSS)", start)
        timer = source[start:end]
        self.assertIn("refresh_host_configuration()", timer)
        self.assertIn("pIME->_ApplyHostPreferences(configuration)", timer)
        self.assertIn("pIME->_PollPreferenceTransition()", timer)
        apply_start = source.index("bool CMetasequoiaIME::_ApplyHostPreferences(")
        apply_end = source.index("void CMetasequoiaIME::_PollPreferenceTransition()", apply_start)
        apply = source[apply_start:apply_end]
        self.assertIn("host->update_preferences(configuration->preferenceSnapshot", apply)
        self.assertNotIn("reload_preferences", apply)
        self.assertNotIn("default_state_directory", apply)
        self.assertNotIn("reload_preferences", timer)
        self.assertNotIn("default_state_directory", timer)
        activation = source[source.index("STDAPI CMetasequoiaIME::ActivateEx"):]
        self.assertLess(activation.index("initialize_host_configuration()"),
                        activation.index("ReadConfiguredGameCompatibility()"))

    def test_game_activation_keeps_fresh_exclusion_rules(self):
        source = (TSF / "Utils/FanyUtils.cpp").read_text()
        start = source.index("std::optional<GameCompatibility> ReadConfiguredGameCompatibility()")
        end = source.index("void SendKeys", start)
        self.assertIn("load_activation_preferences()", source[start:end])

    def test_profile_setup_uses_same_snapshot(self):
        source = (TSF / "Composition/CompositionProcessorEngine.cpp").read_text()
        start = source.index("BOOL CCompositionProcessorEngine::SetupLanguageProfile")
        end = source.index("\nExit:", start)
        setup = source[start:end]
        self.assertIn("configuration->hostOptions", setup)
        self.assertIn("update_preferences(configuration->preferenceSnapshot", setup)
        self.assertNotIn("reload_preferences", setup)
        self.assertNotIn("default_host_options_json", setup)


if __name__ == "__main__":
    unittest.main()
