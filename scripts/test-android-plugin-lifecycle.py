#!/usr/bin/env python3
"""确保 Tauri Android 插件销毁时停止自己的后台线程。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]


class AndroidPluginLifecycleContract(unittest.TestCase):
    def test_account_plugin_stops_bootstrap_worker(self):
        source = (ROOT / "platforms/android/java/app/msime/android/account/AccountPlugin.kt").read_text()
        self.assertRegex(source, r"override\s+fun\s+onDestroy\(\)\s*\{")
        self.assertIn("bootstrapWorker.shutdownNow()", source)

    def test_voice_plugin_stops_polling_worker(self):
        source = (ROOT / "platforms/android/java/app/msime/android/voice/VoicePlugin.kt").read_text()
        self.assertRegex(source, r"override\s+fun\s+onDestroy\(\)\s*\{")
        self.assertIn("worker.shutdownNow()", source)

    def test_voice_plugin_cancels_active_job_before_shutdown(self):
        source = (ROOT / "platforms/android/java/app/msime/android/voice/VoicePlugin.kt").read_text()
        destroy = source[source.index("override fun onDestroy()"):source.index("private fun store()")]
        self.assertIn("activeJob.getAndSet(null)", destroy)
        self.assertIn("current.invoke.reject(\"cancelled\", \"cancelled\")", destroy)
        self.assertIn("VoiceRecognitionActivity.cancelActive()", destroy)


if __name__ == "__main__":
    unittest.main()
