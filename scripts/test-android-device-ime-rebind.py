import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]


CALLERS = (
    "platforms/android/tests/device/candidate/AiCandidateSeatDeviceSmoke.java",
    "platforms/android/tests/device/clipboard/ClipboardEditDeviceSmoke.java",
    "platforms/android/tests/device/keyboard/EmptyEditorBackspaceDeviceSmoke.java",
    "platforms/android/tests/device/settings/HostLaunchDeviceSmoke.java",
    "platforms/android/tests/device/settings/SettingsDeviceSmoke.java",
    "platforms/android/tests/device/settings/SettingsLifecycleSmoke.java",
    "platforms/android/tests/device/voice/HandwritingDeviceSmoke.java",
    "platforms/android/tests/device/voice/TypingStatisticsDeviceSmoke.java",
)


class AndroidDeviceImeRebindTests(unittest.TestCase):
    def test_all_rebind_callers_use_the_stable_shared_helper(self):
        for relative in CALLERS:
            source = (ROOT / relative).read_text()
            self.assertIn("restartIme(", source, relative)
            self.assertNotIn('ime disable ', source, relative)
            self.assertNotIn('ime enable ', source, relative)
            self.assertNotIn('ime set ', source, relative)

    def test_shared_helper_accepts_product_specific_component_ids(self):
        source = (ROOT / "platforms/android/tests/device/core/DeviceSmoke.java").read_text()
        self.assertIn("protected final void restartIme() throws Exception", source)
        self.assertIn("protected final void restartIme(String ime) throws Exception", source)
        self.assertIn('restartIme("app.msime.android/.MSIMEInputService")', source)


if __name__ == "__main__":
    unittest.main()
