#!/usr/bin/env python3
"""The packaged provider's model listing and polish test. Synthetic responses only.

Neither request may take an endpoint or a token from the settings page: the page
names a provider, the private owner-only file holds the credential, and the two
have to agree first. That is the whole point of keeping AI credentials out of the
shell on this platform, so it is what these cases check.
"""
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import re
from types import SimpleNamespace
import sys
import tempfile
import unittest
import unittest.mock

ROOT = Path(__file__).resolve().parents[2]
REPOSITORY = ROOT.parents[1]
sys.path.insert(0, str(ROOT / "scripts"))


def load(name, filename):
    loader = importlib.machinery.SourceFileLoader(name, str(ROOT / "scripts" / filename))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


online = load("ai_service_online_provider", "msime-linux-online-provider")

PRIVATE = {
    "provider": "synthetic",
    "endpoint": "https://service.example.invalid/openai/v1/chat/completions",
    "model": "synthetic-model",
    "token": "synthetic-token",
}


class AiServiceContract(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="msime-ai-service-")
        self.addCleanup(self.directory.cleanup)
        path = Path(self.directory.name) / "ai-provider.json"
        path.write_text(json.dumps(PRIVATE))
        path.chmod(0o600)
        self.server = SimpleNamespace(ai_config_path=str(path))
        self.requests = []
        self.original_fetch = online.fetch
        self.addCleanup(setattr, online, "fetch", self.original_fetch)

    def respond(self, document):
        def fetch(url, timeout, body=None, token=None, extra_headers=None, **kwargs):
            self.requests.append({"url": url, "body": body, "token": token})
            return document

        online.fetch = fetch

    def test_model_catalogue_sits_beside_the_configured_endpoint(self):
        # 复用端点自己的 API 前缀而不是假定一个，这与 client-providers 的 ai::endpoint::models_url 为自己持有 token 的宿主所做的一致。
        self.assertEqual(
            online.ai_models_url(PRIVATE["endpoint"]),
            "https://service.example.invalid/openai/v1/models",
        )
        self.assertEqual(
            online.ai_models_url("https://service.example.invalid/chat/completions"),
            "https://service.example.invalid/models",
        )

    def test_models_are_listed_with_the_private_token_and_bounded(self):
        self.respond(
            {
                "data": [
                    {"id": "alpha"},
                    {"id": "alpha"},
                    {"id": ""},
                    {"id": "bad\nname"},
                    {"id": "beta"},
                    "not an object",
                ]
            }
        )
        result = online.ai_models(
            {"provider": "synthetic", "endpoint": PRIVATE["endpoint"]}, self.server
        )
        # Duplicates, empties and control characters are dropped; order is kept.
        self.assertEqual(result, {"models": ["alpha", "beta"]})
        self.assertEqual(self.requests[0]["url"], "https://service.example.invalid/openai/v1/models")
        self.assertEqual(self.requests[0]["token"], PRIVATE["token"])
        self.assertIsNone(self.requests[0]["body"])

    def test_models_refuse_a_provider_or_endpoint_the_private_file_disagrees_with(self):
        self.respond({"data": [{"id": "alpha"}]})
        for query in (
            {"provider": "other", "endpoint": PRIVATE["endpoint"]},
            {"provider": "synthetic", "endpoint": "https://elsewhere.example.invalid/v1/chat"},
            {"provider": "synthetic"},
            {},
        ):
            self.assertEqual(online.ai_models(query, self.server), {"models": []})
        # Nothing left the process for any of them.
        self.assertEqual(self.requests, [])

    def test_endpoint_policy_matches_the_shared_cases(self):
        # 与 crates/client-providers/src/ai/endpoint.rs 跑同一组用例；这里不派生来源键，只核对 result。
        contract = json.loads((REPOSITORY / "shared/contracts/ai-endpoint/cases.json").read_text())
        self.assertGreater(len(contract["cases"]), 40)
        for case in contract["cases"]:
            with self.subTest(endpoint=case["endpoint"]):
                self.assertEqual(online.ai_endpoint_check(case["endpoint"]), case["result"])
        # 非常规写法的 IPv4 不当作局域网地址。
        for endpoint in ("http://010.0.0.1/v1", "http://0x7f000001/v1", "http://10.1/v1"):
            self.assertNotEqual(online.ai_endpoint_check(endpoint), "allowed", endpoint)

    def test_a_local_http_service_is_reached_and_a_public_one_is_not(self):
        path = Path(self.server.ai_config_path)
        local = {**PRIVATE, "endpoint": "http://192.168.1.20:1234/v1/chat/completions"}
        path.write_text(json.dumps(local))
        self.respond({"data": [{"id": "local-model"}]})
        query = {"provider": "synthetic", "endpoint": local["endpoint"]}
        self.assertEqual(online.ai_models(query, self.server), {"models": ["local-model"]})
        self.assertEqual(self.requests[0]["url"], "http://192.168.1.20:1234/v1/models")
        self.assertEqual(self.requests[0]["token"], "synthetic-token")

        self.requests.clear()
        public = {**PRIVATE, "endpoint": "http://service.example.invalid/v1/chat/completions"}
        path.write_text(json.dumps(public))
        query = {"provider": "synthetic", "endpoint": public["endpoint"]}
        # 公网的 http 接口连配置都读不进来，Token 不会离开本机。
        with self.assertRaises(ValueError):
            online.ai_models(query, self.server)
        self.assertEqual(self.requests, [])

    def test_a_local_http_service_is_reached_without_the_environment_proxy(self):
        def proxies(url):
            # 空的 ProxyHandler 没有任何 *_open 方法，不会进 opener，但它挡住了 build_opener 默认按环境变量加的那一个；所以这里合并 opener 里所有代理表来看。
            merged = {}
            for handler in online.request_opener(url).handlers:
                if isinstance(handler, online.urllib.request.ProxyHandler):
                    merged.update(handler.proxies)
            return merged

        environment = {"http_proxy": "http://proxy.example.invalid:3128",
                       "https_proxy": "http://proxy.example.invalid:3128"}
        with unittest.mock.patch.dict(online.os.environ, environment):
            # 带 Token 的局域网 http 请求直连，不能明文经过代理。
            self.assertEqual(proxies("http://192.168.1.20:1234/v1/models"), {})
            # https 照旧使用环境变量里的代理。
            self.assertEqual(proxies("https://service.example.invalid/v1/models")["https"],
                             "http://proxy.example.invalid:3128")

    def test_polish_sends_the_page_text_under_the_private_credential(self):
        self.respond({"choices": [{"message": {"content": "  polished  "}}]})
        result = online.ai_polish_test(
            {
                "provider": "synthetic",
                "endpoint": PRIVATE["endpoint"],
                "model": PRIVATE["model"],
                "prompt": "synthetic prompt",
                "text": "synthetic input",
            },
            self.server,
        )
        self.assertEqual(result, {"text": "polished"})
        sent = self.requests[0]
        self.assertEqual(sent["url"], PRIVATE["endpoint"])
        self.assertEqual(sent["token"], PRIVATE["token"])
        self.assertEqual(sent["body"]["model"], PRIVATE["model"])
        self.assertEqual(
            [message["content"] for message in sent["body"]["messages"]],
            ["synthetic prompt", "synthetic input"],
        )

    def test_polish_requires_the_model_to_match_and_real_text(self):
        self.respond({"choices": [{"message": {"content": "polished"}}]})
        base = {
            "provider": "synthetic",
            "endpoint": PRIVATE["endpoint"],
            "model": PRIVATE["model"],
            "prompt": "synthetic prompt",
            "text": "synthetic input",
        }
        # The model is compared here, unlike the listing: a polish request runs on
        # one specific model and the settings page must be naming the configured
        # one.
        for override in (
            {"model": "another-model"},
            {"text": "   "},
            {"text": ""},
            {"text": None},
            {"text": "x" * 8193},
            {"prompt": "p" * 8193},
        ):
            self.assertEqual(
                online.ai_polish_test({**base, **override}, self.server), {"text": ""}
            )
        self.assertEqual(self.requests, [])

    def test_polish_and_models_survive_a_service_answering_nonsense(self):
        for document in ({}, {"choices": []}, {"choices": [{}]}, [], "text", None):
            self.respond(document)
            self.assertEqual(
                online.ai_polish_test(
                    {
                        "provider": "synthetic",
                        "endpoint": PRIVATE["endpoint"],
                        "model": PRIVATE["model"],
                        "prompt": "p",
                        "text": "t",
                    },
                    self.server,
                ),
                {"text": ""},
            )
            self.assertEqual(
                online.ai_models(
                    {"provider": "synthetic", "endpoint": PRIVATE["endpoint"]}, self.server
                ),
                {"models": []},
            )

    def test_thinking_is_disabled_for_the_providers_that_need_it_said(self):
        for provider, key in (("deepseek", "thinking"), ("siliconflow", "enable_thinking")):
            path = Path(self.directory.name) / f"{provider}.json"
            path.write_text(json.dumps({**PRIVATE, "provider": provider}))
            path.chmod(0o600)
            server = SimpleNamespace(ai_config_path=str(path))
            self.requests.clear()
            self.respond({"choices": [{"message": {"content": "polished"}}]})
            online.ai_polish_test(
                {
                    "provider": provider,
                    "endpoint": PRIVATE["endpoint"],
                    "model": PRIVATE["model"],
                    "prompt": "p",
                    "text": "t",
                },
                server,
            )
            self.assertIn(key, self.requests[0]["body"])

    def test_default_preferences_send_the_builtin_associative_prompt(self):
        # AiAssistantPreferences::default() with AI switched on: slot one selected and every prompt field empty. Before the fallback this sent an empty system message, so the model had no reason to answer JSON and no candidate ever parsed.
        options = {
            "enabled": True,
            "provider": PRIVATE["provider"],
            "model": PRIVATE["model"],
            "endpoint": PRIVATE["endpoint"],
            "candidate_limit": 3,
            "prompt_id": "custom_1",
            "prompt_custom_1": "",
            "prompt_custom_2": "",
            "prompt_custom_3": "",
        }
        query = {"ai_eligible": True, "ai_assistant": options,
                 "pinyin_segments": ["shu", "ru", "fa"], "ai_context": ""}
        answer = {"candidates": [{"text": "输入法", "type": "chinese", "confidence": 0.9},
                                 {"text": "输入法", "type": "chinese", "confidence": 0.5},
                                 {"text": "书入法", "type": "chinese", "confidence": 0.1}]}
        sent = []

        def fetch(url, timeout, body=None, token=None, **kwargs):
            sent.append(body)
            return {"choices": [{"message": {"content": json.dumps(answer, ensure_ascii=False)}}]}

        online.fetch = fetch
        # 合并字典用 {**a, **b} 而不是 a | b：后者要 Python 3.9，legacy 包的运行时验收在 Python 3.7 上跑这个测试。
        # A prompt someone cleared by hand, whitespace included, is still "not customised".
        for blank in ("", "  \n"):
            sent.clear()
            rows = online.ai({**query, "ai_assistant": {**options, "prompt_custom_1": blank}}, PRIVATE)
            self.assertEqual([row["text"] for row in rows], ["输入法", "书入法"])
            system = sent[0]["messages"][0]
            self.assertEqual(system["role"], "system")
            self.assertTrue(system["content"].strip())
            self.assertIn("json", system["content"].lower())
            self.assertIn("candidates", system["content"])
        # A prompt the user did write is sent untouched.
        sent.clear()
        online.ai({**query, "ai_assistant": {**options, "prompt_custom_1": "synthetic prompt"}}, PRIVATE)
        self.assertEqual(sent[0]["messages"][0]["content"], "synthetic prompt")
        # 第二、第三个槽位为空时同样得到内置文本，而不是第一个槽位的文本，与 client-providers 的做法一致；"custom" 不指向任何槽位，什么都不发送。
        for slot in ("custom_2", "custom_3"):
            sent.clear()
            online.ai({**query, "ai_assistant": {**options, "prompt_id": slot, "prompt_custom_1": "synthetic prompt"}}, PRIVATE)
            self.assertEqual(sent[0]["messages"][0]["content"], online.DEFAULT_AI_PROMPT)
        sent.clear()
        online.ai({**query, "ai_assistant": {**options, "prompt_id": "custom", "prompt_custom_1": "synthetic prompt"}}, PRIVATE)
        self.assertEqual(sent, [])

    def test_builtin_prompt_matches_client_core(self):
        # The provider cannot import the Rust constant at run time, so keep the two copies from drifting apart here. The Rust literal only uses escapes JSON shares (\n and \").
        source = (REPOSITORY / "crates/client-providers/src/ai.rs").read_text()
        literal = re.search(r'pub const DEFAULT_CANDIDATE_PROMPT: &str = ("(?:[^"\\]|\\.)*");', source)
        self.assertIsNotNone(literal)
        self.assertEqual(online.DEFAULT_AI_PROMPT, json.loads(literal.group(1)))


    def ollama_query(self, **overrides):
        options = {
            "enabled": True,
            "provider": "ollama",
            "model": "qwen3:8b",
            "endpoint": "http://localhost:11434/v1/chat/completions",
            "candidate_limit": 3,
            "prompt_id": "custom_1",
            "prompt_custom_1": "",
            "prompt_custom_2": "",
            "prompt_custom_3": "",
            **overrides,
        }
        return {"ai_eligible": True, "ai_assistant": options,
                "pinyin_segments": ["shu", "ru", "fa"], "ai_context": ""}

    def test_ollama_uses_the_page_endpoint_and_model_without_a_token(self):
        # Ollama 不鉴权，设置页不显示 Token，私有文件里没有它的条目：地址和模型取自设置页，请求不带 Authorization。
        answer = {"candidates": [{"text": "输入法", "type": "chinese", "confidence": 0.9}]}
        self.respond({"choices": [{"message": {"content": json.dumps(answer, ensure_ascii=False)}}]})
        query = self.ollama_query()
        cache, lock = {}, online.threading.Lock()
        for path in (None, self.server.ai_config_path):
            self.requests.clear()
            rows = online.configured_ai(query, path, cache if path else None, lock)
            self.assertEqual([row["text"] for row in rows], ["输入法"])
            self.assertEqual(self.requests[0]["url"], "http://localhost:11434/v1/chat/completions")
            self.assertEqual(self.requests[0]["body"]["model"], "qwen3:8b")
            self.assertIsNone(self.requests[0]["token"])
        # 没有凭据也能进缓存，下一次命中不再发请求。
        self.assertEqual(len(cache), 1)
        self.requests.clear()
        self.assertEqual([row["text"] for row in online.configured_ai(query, self.server.ai_config_path, cache, lock)],
                         ["输入法"])
        self.assertEqual(self.requests, [])

        self.respond({"choices": [{"message": {"content": json.dumps({"translation": "input method"})}}]})
        self.assertEqual(online.ai_translation("输入法", "en", query["ai_assistant"], None), "input method")
        self.assertIsNone(self.requests[-1]["token"])

        page = {"provider": "ollama", "endpoint": query["ai_assistant"]["endpoint"], "model": ""}
        self.respond({"data": [{"id": "qwen3:8b"}]})
        no_private_file = SimpleNamespace(ai_config_path=None)
        self.assertEqual(online.ai_models(page, no_private_file), {"models": ["qwen3:8b"]})
        self.assertEqual(self.requests[-1]["url"], "http://localhost:11434/v1/models")
        self.assertIsNone(self.requests[-1]["token"])

        self.respond({"choices": [{"message": {"content": "OK"}}]})
        self.assertTrue(online.ai_credential_test({**page, "model": "qwen3:8b"}, None))
        self.assertIsNone(self.requests[-1]["token"])
        # 润色测试必须有模型。
        self.requests.clear()
        self.assertEqual(online.ai_polish_test({**page, "prompt": "", "text": "hi"}, no_private_file), {"text": ""})
        self.assertEqual(self.requests, [])

    def test_a_request_without_a_token_carries_no_authorization_header(self):
        sent = []

        class Response:
            def __enter__(self):
                return self

            def __exit__(self, *args):
                return False

            def read(self, size):
                return b"{}"

        class Opener:
            def open(self, request, timeout):
                sent.append(dict(request.header_items()))
                return Response()

        with unittest.mock.patch.object(online, "request_opener", lambda url, connect_timeout=None: Opener()):
            online.fetch_direct("http://localhost:11434/v1/models", 1.0, None, None)
            online.fetch_direct("https://service.example.invalid/v1/models", 1.0, None, "synthetic-token")
        self.assertNotIn("Authorization", sent[0])
        self.assertEqual(sent[1]["Authorization"], "Bearer synthetic-token")

    def test_ollama_refuses_a_public_http_endpoint_and_bad_page_values(self):
        self.respond({"data": [{"id": "never"}]})
        public = "http://service.example.invalid/v1/chat/completions"
        self.assertIsNone(online.configured_ai(self.ollama_query(endpoint=public), None))
        self.assertIsNone(online.ai_translation("输入法", "en", self.ollama_query(endpoint=public)["ai_assistant"], None))
        self.assertEqual(online.ai_models({"provider": "ollama", "endpoint": public}, self.server), {"models": []})
        self.assertFalse(online.ai_credential_test({"provider": "ollama", "endpoint": public, "model": "m"}, None))
        for bad in ({"model": ""}, {"model": "bad\nmodel"}, {"endpoint": ""}, {"endpoint": 7}):
            self.assertIsNone(online.configured_ai(self.ollama_query(**bad), None))
        self.assertEqual(self.requests, [])

    def test_other_providers_still_need_the_private_file(self):
        # 只有 Ollama 免 Token：别的服务没有私有条目时什么都不发，也不会拿设置页给的地址去请求。
        self.respond({"choices": [{"message": {"content": "{}"}}]})
        for provider in ("synthetic-other", "custom", "openai"):
            query = self.ollama_query(provider=provider, endpoint="https://service.example.invalid/v1/chat/completions")
            self.assertIsNone(online.configured_ai(query, self.server.ai_config_path))
            self.assertIsNone(online.configured_ai(query, None))
            self.assertIsNone(online.ai_translation("输入法", "en", query["ai_assistant"], None))
            page = {"provider": provider, "endpoint": query["ai_assistant"]["endpoint"], "model": "m"}
            self.assertEqual(online.ai_models(page, SimpleNamespace(ai_config_path=None)), {"models": []})
            self.assertFalse(online.ai_credential_test(page, None))
        self.assertEqual(self.requests, [])


if __name__ == "__main__":
    unittest.main()
