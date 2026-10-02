#!/usr/bin/env python3
"""Pick the polishing prompt the way the Windows voice service does."""
import importlib.machinery
import importlib.util
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
loader = importlib.machinery.SourceFileLoader(
    "voice_polish_provider", str(ROOT / "scripts" / "msime-linux-voice-provider"))
spec = importlib.util.spec_from_loader(loader.name, loader)
voice = importlib.util.module_from_spec(spec)
loader.exec_module(voice)


class ProviderPolishPrompt(unittest.TestCase):
    def test_slots_and_presets(self):
        self.assertEqual(voice.polish_prompt({"polish_prompt_id": "custom_1", "polish_prompt_custom_1": "一号"}), "一号")
        self.assertEqual(voice.polish_prompt({"polish_prompt_id": "custom_2", "polish_prompt_custom_2": "二号"}), "二号")
        self.assertEqual(voice.polish_prompt({"polish_prompt_id": "custom_3"}), voice.PROMPTS["cleanup"])
        self.assertEqual(voice.polish_prompt({}), voice.PROMPTS["cleanup"])
        # "custom" names no slot, so it gets the built-in cleanup prompt rather than slot one.
        self.assertEqual(voice.polish_prompt({"polish_prompt_id": "custom", "polish_prompt_custom_1": "一号"}), voice.PROMPTS["cleanup"])
        for preset in voice.PROMPTS:
            with self.subTest(preset=preset):
                self.assertEqual(voice.polish_prompt({"polish_prompt_id": preset}), voice.PROMPTS[preset])


if __name__ == "__main__":
    unittest.main()
