#!/usr/bin/env python3
"""Windows-parity tests for per-item negative translation caching."""
import importlib.machinery
import importlib.util
import hashlib
from pathlib import Path
from types import SimpleNamespace
import sys
import threading
import time
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
loader = importlib.machinery.SourceFileLoader(
    "translation_cache_provider", str(ROOT / "scripts" / "msime-linux-online-provider"))
spec = importlib.util.spec_from_loader(loader.name, loader)
provider = importlib.util.module_from_spec(spec)
loader.exec_module(provider)


class TranslationCacheParity(unittest.TestCase):
    def setUp(self):
        self.server = SimpleNamespace(
            translation_cache={}, translation_lock=threading.Lock(),
            tencent_config_path=None)
        self.query = {
            "candidates": ["hello"],
            "target_language": "en",
            "provider": "custom",
            "custom_translation": {
                "enabled": True,
                "endpoint": "https://translation.invalid/",
                "api_key": "synthetic-key",
            },
        }

    def test_negative_result_is_reused_for_eight_minutes(self):
        with mock.patch.object(provider, "custom_translation", return_value=None) as translate:
            self.assertEqual(provider.translations(self.query, self.server), [])
            self.assertEqual(provider.translations(self.query, self.server), [])
        self.assertEqual(translate.call_count, 1)
        self.assertEqual(len(self.server.translation_cache), 1)
        expiry, value = next(iter(self.server.translation_cache.values()))
        self.assertIsNone(value)
        self.assertGreaterEqual(expiry - time.monotonic(), 479)

    def test_positive_result_survives_without_a_time_expiry(self):
        result = "你好"
        with mock.patch.object(provider, "custom_translation", return_value=result) as translate:
            expected = [{"text": "hello", "translation": result}]
            self.assertEqual(provider.translations(self.query, self.server), expected)
            self.assertEqual(provider.translations(self.query, self.server), expected)
        self.assertEqual(translate.call_count, 1)
        expiry, value = next(iter(self.server.translation_cache.values()))
        self.assertEqual(value, result)
        self.assertEqual(expiry, float("inf"))

    def test_translation_cache_capacity_matches_windows_worker(self):
        self.assertEqual(provider.MAX_TRANSLATION_CACHE_ENTRIES, 4096)
        self.assertEqual(provider.NEGATIVE_TRANSLATION_CACHE_TTL, 480)

    def test_normalized_credentials_reuse_cache_and_rotation_partitions_it(self):
        with mock.patch.object(provider, "custom_translation", return_value="你好") as translate:
            expected = provider.translations(self.query, self.server)
            self.query["custom_translation"]["api_key"] = " \tsynthetic-key\r\n"
            self.assertEqual(provider.translations(self.query, self.server), expected)
            self.assertEqual(translate.call_count, 1)
            self.query["custom_translation"]["api_key"] = "synthetic-rotated-key"
            self.assertEqual(provider.translations(self.query, self.server), expected)
            self.assertEqual(translate.call_count, 2)

    def test_credential_cache_identity_is_private_to_provider_lifetime(self):
        fresh_provider = importlib.util.module_from_spec(spec)
        loader.exec_module(fresh_provider)
        keys = []
        for module in (provider, fresh_provider):
            self.server.translation_cache.clear()
            with mock.patch.object(module, "custom_translation", return_value="你好"):
                module.translations(self.query, self.server)
            key = next(iter(self.server.translation_cache))
            fingerprint = key[0][1]
            self.assertNotIn(b"synthetic-key", fingerprint)
            self.assertNotEqual(fingerprint, hashlib.sha256(b"synthetic-key").digest())
            keys.append(key)
        self.assertNotEqual(keys[0], keys[1])

    def test_hmac_key_padding_cannot_alias_distinct_credentials(self):
        for value in (b"", b"synthetic-key", b"s" * 64):
            self.assertNotEqual(provider.credential_cache_fingerprint(value),
                                provider.credential_cache_fingerprint(value + b"\0"))


if __name__ == "__main__":
    unittest.main()
