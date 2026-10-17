#!/usr/bin/env python3
"""在线 provider 只问用户选中的那个翻译服务，「水杉账号」的释义经 Rust 的 msime_client_account translate。

用户改选别的服务或关闭以后，有效的腾讯凭据文件仍留在磁盘上，所以这里每个用例都放着一份：不是明确选了腾讯的查询，一样不能发给腾讯。第三方服务都经 provider 自己的 `fetch` 出网，记下它就知道问了谁；「水杉账号」由 Rust 发请求，换成假的 RustAccountApi 或假的 `_call`。不联网。
"""
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

TENCENT = "https://tmt.tencentcloudapi.com/"
NIUTRANS = "https://api.niutrans.com/v2/text/translate"
CUSTOM = "https://translation.example.invalid/translate"


def respond(url, timeout, body=None, token=None, extra_headers=None):
    if url == TENCENT:
        return {"Response": {"TargetTextList": ["synthetic tencent"]}}
    if url == NIUTRANS:
        return {"tgtText": "synthetic niutrans"}
    return {"data": "synthetic custom"}


class TranslationProviderSelection(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="msime-translation-selection-")
        self.addCleanup(directory.cleanup)
        path = Path(directory.name) / "tencent-provider.json"
        path.write_text(json.dumps({"secret_id": "synthetic-local-id", "secret_key": "synthetic-local-key"}))
        path.chmod(0o600)
        self.assertTrue(provider.load_tencent_config(path), "fixture Tencent credential must be usable")
        self.server = SimpleNamespace(tencent_config_path=path, translation_cache={},
                                      translation_lock=threading.Lock())

    def test_provider_does_not_duplicate_rust_account_storage_or_authentication(self):
        source = Path(provider.__file__).read_text()
        self.assertNotIn("/v1/auth/", source)
        self.assertNotIn("anonymous-account.json", source)
        self.assertNotIn("anonymous-session.json", source)

    def contacted(self, query):
        query = {"generation": 1, "candidates": ["测试"], "target_language": "en", **query}
        with mock.patch.object(provider, "fetch", side_effect=respond) as fetch:
            result = provider.translations(query, self.server)
        return result, [call.args[0] for call in fetch.call_args_list]

    def test_unusable_or_disabled_selection_never_reaches_tencent(self):
        for name, query in (
            ("translation off", {"provider": "none"}),
            # host-api omits a NiuTrans or custom block whose configuration is incomplete, but still names the selection.
            ("NiuTrans without credentials", {"provider": "niutrans"}),
            ("NiuTrans disabled block", {"provider": "niutrans",
                                         "niutrans": {"enabled": False, "app_id": "", "apikey": ""}}),
            ("custom without endpoint", {"provider": "custom"}),
            ("custom disabled block", {"provider": "custom",
                                       "custom_translation": {"enabled": False, "endpoint": "", "api_key": ""}}),
            ("unknown service", {"provider": "deepl"}),
            ("malformed service", {"provider": ["tencent"]}),
            # The field is required: a query without it is malformed, whatever blocks it carries.
            ("missing service", {}),
            ("missing service with a usable block", {"niutrans": {"enabled": True, "app_id": "synthetic-app",
                                                                  "apikey": "synthetic-key"}}),
            ("missing service with the account flag", {"translation_account": True}),
        ):
            with self.subTest(case=name):
                result, urls = self.contacted(query)
                self.assertEqual(result, [])
                self.assertEqual(urls, [])

    def test_only_the_selected_service_is_asked(self):
        niutrans = {"enabled": True, "app_id": "synthetic-app", "apikey": "synthetic-key"}
        custom = {"enabled": True, "endpoint": CUSTOM, "api_key": ""}
        for name, query, expected, translation in (
            ("Tencent", {"provider": "tencent"}, TENCENT, "synthetic tencent"),
            # A stray block for another service must not redirect a Tencent selection.
            ("Tencent with stray NiuTrans block", {"provider": "tencent", "niutrans": niutrans}, TENCENT,
             "synthetic tencent"),
            ("NiuTrans", {"provider": "niutrans", "niutrans": niutrans}, NIUTRANS, "synthetic niutrans"),
            ("custom", {"provider": "custom", "custom_translation": custom}, CUSTOM, "synthetic custom"),
        ):
            with self.subTest(case=name):
                self.server.translation_cache.clear()
                result, urls = self.contacted(query)
                self.assertEqual(result, [{"text": "测试", "translation": translation}])
                self.assertEqual(urls, [expected])

    def test_sentence_requests_allow_one_long_item_without_changing_candidate_limits(self):
        sentence = "这是一个超过普通候选词限制但仍在整句请求上限内的合成测试句子。" * 9
        custom = {"enabled": True, "endpoint": CUSTOM, "api_key": ""}
        result, urls = self.contacted({"provider": "custom", "custom_translation": custom,
                                       "sentence": True, "candidates": [sentence]})
        self.assertEqual(result, [{"text": sentence, "translation": "synthetic custom"}])
        self.assertEqual(urls, [CUSTOM])

        result, urls = self.contacted({"provider": "custom", "custom_translation": custom,
                                       "candidates": [sentence]})
        self.assertEqual(result, [])
        self.assertEqual(urls, [])

    def test_explicit_account_selection_asks_rust_and_sends_nothing_itself(self):
        # 请求、会话和规范化都在 Rust 的 msime_client_account translate 里，provider 自己的 fetch 一次也不用。
        account = mock.Mock()
        account.directory = Path("/synthetic/account")
        account.translate.return_value = ["synthetic account"]
        self.server.rust_account = account
        before = provider.time.monotonic()
        result, urls = self.contacted({"provider": "none", "translation_account": True})
        self.assertEqual(result, [{"text": "测试", "translation": "synthetic account"}])
        self.assertEqual(urls, [])
        texts, source, target, deadline = account.translate.call_args.args
        self.assertEqual((texts, source, target), (["测试"], "zh", "en"))
        # 交给 Rust 的是这一次 translations 的总期限（6 秒），每次请求的超时由 RustAccountApi 从里面算。
        self.assertGreater(deadline, before + 5)
        self.assertLessEqual(deadline, provider.time.monotonic() + 6)

    def test_failed_account_translation_is_negatively_cached(self):
        account = mock.Mock()
        account.directory = Path("/synthetic/account")
        account.translate.return_value = None
        self.server.rust_account = account
        result, urls = self.contacted({"provider": "none", "translation_account": True})
        self.assertEqual((result, urls), ([], []))
        result, urls = self.contacted({"provider": "none", "translation_account": True})
        self.assertEqual((result, urls), ([], []))
        account.translate.assert_called_once()

    def test_ai_selection_reaches_the_ai_translation(self):
        # 原来 options 在赋值之前就被读，选了 AI 的候选翻译每次都抛 UnboundLocalError。
        assistant = {"enabled": True, "provider": "synthetic"}
        self.server.ai_config_path = None
        with mock.patch.object(provider, "ai_translation", return_value="synthetic ai") as translate:
            result = provider.translations({"generation": 1, "candidates": ["测试"], "target_language": "en",
                                            "provider": "ai", "ai_assistant": assistant}, self.server)
        self.assertEqual(result, [{"text": "测试", "translation": "synthetic ai"}])
        translate.assert_called_once()
        self.assertIs(translate.call_args.args[2], assistant)


AGENT = "msime-linux/1.4.0 (unknown; Synthetic 1; edition=full)"


class AccountTranslation(unittest.TestCase):
    """RustAccountApi.translate：经 msime_client_account 的 translate 问，会话都不可用时才补注册匿名账号，10 分钟内只补一次。"""

    def setUp(self):
        self.api = provider.RustAccountApi("/synthetic/account")
        self.calls = []
        # 每次 msime_client_account 依次取一个回复，用完以后都是最后一个。
        self.translate_replies = []
        self.registration = {"ok": True, "value": True}

    def call(self, name, payload):
        self.calls.append((name, payload))
        if name == "msime_client_client_agent":
            return {"ok": True, "value": AGENT}
        if name == "msime_client_ensure_anonymous_account_with_agent":
            return self.registration
        if name == "msime_client_account":
            return self.translate_replies.pop(0) if len(self.translate_replies) > 1 else self.translate_replies[0]
        raise AssertionError(name)

    def translate(self, texts=("水杉",), budget=6.0):
        with mock.patch.object(self.api, "_call", side_effect=self.call):
            return self.api.translate(list(texts), "zh", "en", provider.time.monotonic() + budget)

    def names(self):
        return [name for name, _ in self.calls if name != "msime_client_client_agent"]

    @staticmethod
    def ok(*values):
        return {"ok": True, "value": {"translations": list(values)}}

    def test_the_request_is_msime_client_account_translate(self):
        self.translate_replies = [self.ok("sequoia", None)]
        self.assertEqual(self.translate(("水杉", "测试")), ["sequoia", None])
        self.assertEqual(self.names(), ["msime_client_account"])
        self.assertEqual(self.calls[-1][1], {
            "directory": "/synthetic/account", "client_agent": AGENT,
            "action": {"operation": "translate", "texts": ["水杉", "测试"], "source_lang": "zh",
                       "target_lang": "en", "timeout_ms": 2500}})

    def test_timeout_follows_the_remaining_budget_within_rust_limits(self):
        self.translate_replies = [self.ok("sequoia")]
        self.translate(budget=1.7)
        self.assertTrue(1600 <= self.calls[-1][1]["action"]["timeout_ms"] <= 1700, self.calls[-1])
        # Rust 不收不到 1000 的 timeout_ms，期限只剩一点时按 1 秒问。
        self.translate(budget=0.2)
        self.assertEqual(self.calls[-1][1]["action"]["timeout_ms"], 1000)
        self.calls.clear()
        self.assertIsNone(self.translate(budget=-0.1))
        self.assertEqual(self.names(), [])

    def test_a_usable_session_needs_no_registration(self):
        # 原来每次释义之前都先注册匿名账号，注册失败时已经登录的用户也拿不到释义。
        self.registration = {"ok": False, "error": "account_unavailable"}
        self.translate_replies = [self.ok("sequoia")]
        self.assertEqual(self.translate(), ["sequoia"])
        self.assertEqual(self.names(), ["msime_client_account"])

    def test_unauthorized_registers_once_and_retries(self):
        unauthorized = {"ok": False, "error": "account_unauthorized"}
        self.translate_replies = [unauthorized, self.ok("sequoia")]
        self.assertEqual(self.translate(), ["sequoia"])
        self.assertEqual(self.names(), ["msime_client_account", "msime_client_ensure_anonymous_account_with_agent",
                                        "msime_client_account"])
        self.assertEqual(self.calls[2][1], {"directory": "/synthetic/account", "client_agent": AGENT})
        # 10 分钟之内再遇到 account_unauthorized 不再注册。
        self.calls.clear()
        self.translate_replies = [unauthorized]
        self.assertIsNone(self.translate())
        self.assertEqual(self.names(), ["msime_client_account"])
        # 过了 ANONYMOUS_REGISTRATION_RETRY 才再补一次。
        self.calls.clear()
        self.api._registered_at = provider.time.monotonic() - provider.ANONYMOUS_REGISTRATION_RETRY - 1
        self.translate_replies = [unauthorized, self.ok("sequoia")]
        self.assertEqual(self.translate(), ["sequoia"])
        self.assertEqual(self.names(), ["msime_client_account", "msime_client_ensure_anonymous_account_with_agent",
                                        "msime_client_account"])

    def test_a_failed_registration_is_not_retried_for_ten_minutes(self):
        self.registration = {"ok": False, "error": "account_network"}
        self.translate_replies = [{"ok": False, "error": "account_unauthorized"}]
        self.assertIsNone(self.translate())
        self.assertIsNone(self.translate())
        self.assertEqual(self.names(), ["msime_client_account", "msime_client_ensure_anonymous_account_with_agent",
                                        "msime_client_account"])

    def test_other_failures_do_not_register(self):
        for error in ("account_network", "account_unavailable", "account_rate_limited"):
            with self.subTest(error=error):
                self.calls.clear()
                self.translate_replies = [{"ok": False, "error": error}]
                self.assertIsNone(self.translate())
                self.assertEqual(self.names(), ["msime_client_account"])

    def test_a_reply_of_the_wrong_length_is_a_failure(self):
        self.translate_replies = [self.ok("sequoia", "extra")]
        self.assertIsNone(self.translate())

if __name__ == "__main__":
    unittest.main()
