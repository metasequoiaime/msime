#!/usr/bin/env python3
"""Synthetic NiuTrans credential normalization and cache regressions."""
import importlib.machinery
import importlib.util
import threading
from types import SimpleNamespace
import unittest
from unittest import mock
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
loader = importlib.machinery.SourceFileLoader("online_provider", str(ROOT / "scripts" / "msime-linux-online-provider"))
spec = importlib.util.spec_from_loader(loader.name, loader)
provider = importlib.util.module_from_spec(spec)
loader.exec_module(provider)


class NiuTransCredentials(unittest.TestCase):
    def test_trimmed_credentials_share_cache_identity(self):
        server = SimpleNamespace(translation_cache={}, translation_lock=threading.Lock())
        query = {"candidates": ["测试"], "target_language": "en", "provider": "niutrans"}
        padded = {**query, "niutrans": {"enabled": True, "app_id": " synthetic-app ", "apikey": "\tsynthetic-key\n"}}
        clean = {**query, "niutrans": {"enabled": True, "app_id": "synthetic-app", "apikey": "synthetic-key"}}
        with mock.patch.object(provider, "fetch", return_value={"tgtText": "synthetic gloss"}) as fetch:
            self.assertEqual(provider.translations(padded, server), [{"text": "测试", "translation": "synthetic gloss"}])
            self.assertEqual(provider.translations(clean, server), [{"text": "测试", "translation": "synthetic gloss"}])
        self.assertEqual(fetch.call_count, 1)


if __name__ == "__main__":
    unittest.main()
