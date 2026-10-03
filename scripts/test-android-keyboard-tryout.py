#!/usr/bin/env python3
"""Keep the Android tryout model loader usable after a successful response."""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/KeyboardTryoutActivity.java"


class KeyboardTryoutSourceContract(unittest.TestCase):
    def test_model_loader_restores_controls_after_success(self):
        source = SOURCE.read_text()
        success = re.search(
            r"models\.clear\(\);(?P<body>.*?)\n\s*\}\);\n\s*\}\s*catch",
            source,
            re.S,
        )
        self.assertIsNotNone(success, "could not locate the model-load success callback")
        body = success.group("body")
        self.assertRegex(body, r"load\.setEnabled\(true\)")
        self.assertRegex(body, r"send\.setEnabled\([^;]*field\.length\(\)\s*>\s*0")


if __name__ == "__main__":
    unittest.main()
