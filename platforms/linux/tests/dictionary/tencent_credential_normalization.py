#!/usr/bin/env python3
"""Synthetic Tencent credential normalization and signing regressions."""
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import threading
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
loader = importlib.machinery.SourceFileLoader("online_provider", str(ROOT / "scripts" / "msime-linux-online-provider"))
spec = importlib.util.spec_from_loader(loader.name, loader)
provider = importlib.util.module_from_spec(spec)
loader.exec_module(provider)


class TencentCredentials(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="msime-tencent-")
        self.addCleanup(directory.cleanup)
        self.path = Path(directory.name) / "tencent.json"
        self.clean = {"secret_id": "synthetic-local-id", "secret_key": "synthetic-local-key"}

    def save(self, config):
        self.path.write_text(json.dumps(config))
        self.path.chmod(0o600)

    def test_copy_whitespace_preserves_signature(self):
        self.save(self.clean)
        clean = provider.load_tencent_config(self.path)
        for whitespace in (" ", "\t", "\r", "\n", " \t\r\n"):
            with self.subTest(whitespace=repr(whitespace)):
                self.save({key: whitespace + value + whitespace for key, value in self.clean.items()})
                normalized = provider.load_tencent_config(self.path)
                self.assertTrue(normalized == clean, "normalized credential snapshot differs")
                with mock.patch.object(provider.time, "time", return_value=1800000000), \
                     mock.patch.object(provider, "fetch", return_value={"Response": {"TargetTextList": ["synthetic"]}}) as fetch:
                    provider.tencent_translation(clean, ["测试"], "zh", "en", 2.5)
                    provider.tencent_translation(normalized, ["测试"], "zh", "en", 2.5)
                self.assertTrue(fetch.call_args_list[0] == fetch.call_args_list[1], "signed requests differ")

    def test_invalid_values_remain_rejected(self):
        for value in ("", " \t\r\n", " <YOUR_TENCENT_SECRET_ID>\n", "\tFAKESECRET_example ",
                      "synthetic inner space", "synthetic\nline", "synthetic\x00", "\vsynthetic", "\u00a0synthetic"):
            for key in self.clean:
                with self.subTest(case=repr(value), field=key):
                    self.save({**self.clean, key: value})
                    with self.assertRaises(ValueError):
                        provider.load_tencent_config(self.path)

    def test_reload_reuses_cache_after_whitespace_only_change(self):
        server = SimpleNamespace(tencent_config_path=self.path, translation_cache={},
                                 translation_lock=threading.Lock())
        query = {"candidates": ["测试"], "target_language": "en", "provider": "tencent"}
        self.save(self.clean)
        with mock.patch.object(provider, "fetch", return_value={"Response": {"TargetTextList": ["synthetic"]}}) as fetch:
            first = provider.translations(query, server)
            self.assertEqual(first, [{"text": "测试", "translation": "synthetic"}])
            self.save({key: " \n" + value + "\t" for key, value in self.clean.items()})
            self.assertEqual(provider.translations(query, server), first)
            self.assertEqual(fetch.call_count, 1)
            self.save({**self.clean, "secret_key": "synthetic-replacement-key"})
            self.assertEqual(provider.translations(query, server), first)
            self.assertEqual(fetch.call_count, 2)
            self.save({**self.clean, "secret_key": " <YOUR_TENCENT_SECRET_KEY> "})
            self.assertEqual(provider.translations(query, server), [])
            self.assertEqual(fetch.call_count, 2)


if __name__ == "__main__":
    unittest.main()
