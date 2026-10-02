#!/usr/bin/env python3
"""Windows-parity regressions for Linux AI candidate reuse."""
import base64
import importlib.machinery
import importlib.util
import io
import json
from pathlib import Path
from types import SimpleNamespace
import sys
import threading
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
loader = importlib.machinery.SourceFileLoader(
    "ai_cache_online_provider", str(ROOT / "scripts" / "msime-linux-online-provider"))
spec = importlib.util.spec_from_loader(loader.name, loader)
provider = importlib.util.module_from_spec(spec)
loader.exec_module(provider)


def query(generation=1, context="", prompt="first", segments=None):
    return {
        "ai_eligible": True,
        "generation": generation,
        "session_id": generation,
        "ai_context": context,
        "pinyin_segments": segments or ["ni", "hao"],
        "ai_assistant": {
            "enabled": True,
            "provider": "synthetic",
            "endpoint": "https://ai.invalid/v1/chat/completions",
            "model": "synthetic-model",
            "prompt_custom_1": prompt,
            "candidate_limit": 3,
        },
    }


class AiCandidateCache(unittest.TestCase):
    def setUp(self):
        self.state = SimpleNamespace(cache={}, lock=threading.Lock())
        self.config = {
            "provider": "synthetic",
            "endpoint": "https://ai.invalid/v1/chat/completions",
            "model": "synthetic-model",
            "token": "synthetic-token",
        }

    def configured(self, request):
        return provider.configured_ai(
            request, Path("/private/synthetic.json"), self.state.cache,
            self.state.lock)

    def test_success_is_reused_across_generation_context_and_prompt(self):
        rows = [{"text": "你好", "source": 1}]
        with mock.patch.object(provider, "load_ai_config",
                               return_value=self.config), \
                mock.patch.object(provider, "ai", return_value=rows) as ai:
            self.assertEqual(self.configured(query()), rows)
            self.assertEqual(
                self.configured(query(generation=9, context="private context",
                                      prompt="changed")), rows)
        self.assertEqual(ai.call_count, 1)

    def test_provider_identity_and_segments_partition_cache(self):
        rows = [{"text": "你好", "source": 1}]
        changed_model = query()
        changed_model["ai_assistant"]["model"] = "other-model"
        changed_segments = query(segments=["nin", "hao"])
        with mock.patch.object(provider, "load_ai_config",
                               return_value=self.config), \
                mock.patch.object(provider, "ai", return_value=rows) as ai:
            self.configured(query())
            self.configured(changed_model)
            self.configured(changed_segments)
        self.assertEqual(ai.call_count, 3)

    def test_empty_result_is_not_cached(self):
        with mock.patch.object(provider, "load_ai_config",
                               return_value=self.config), \
                mock.patch.object(provider, "ai",
                                  side_effect=[None, [{"text": "重试", "source": 1}]]) as ai:
            self.assertIsNone(self.configured(query()))
            self.assertEqual(self.configured(query()),
                             [{"text": "重试", "source": 1}])
        self.assertEqual(ai.call_count, 2)

    def test_cache_is_bounded_without_retaining_request_secrets(self):
        key = provider.ai_cache_key(query(context="sensitive", prompt="private"))
        self.assertNotIn("sensitive", key)
        self.assertNotIn("private", key)
        self.assertNotIn("synthetic-token", key)
        self.state.cache.update({str(index): [] for index in range(
            provider.MAX_AI_CACHE_ENTRIES)})
        rows = [{"text": "你好", "source": 1}]
        with mock.patch.object(provider, "load_ai_config",
                               return_value=self.config), \
                mock.patch.object(provider, "ai", return_value=rows):
            self.configured(query())
        self.assertEqual(len(self.state.cache), 1)

    def test_cache_probe_returns_a_hit_without_the_network(self):
        rows = [{"text": "你好", "source": 1}]
        with mock.patch.object(provider, "load_ai_config",
                               return_value=self.config), \
                mock.patch.object(provider, "ai", return_value=rows) as ai:
            self.configured(query())
            probe = query(generation=2)
            probe["ai_cache_only"] = True
            self.assertEqual(self.configured(probe), rows)
        self.assertEqual(ai.call_count, 1)

    def test_cache_probe_miss_stays_local(self):
        probe = query()
        probe["ai_cache_only"] = True
        with mock.patch.object(provider, "load_ai_config",
                               return_value=self.config), \
                mock.patch.object(provider, "ai") as ai:
            self.assertIsNone(self.configured(probe))
        ai.assert_not_called()
        self.assertEqual(self.state.cache, {})

    def test_cache_probe_never_asks_the_cloud(self):
        probe = {"cloud_eligible": True, "query_text": "nihao", "ai_cache_only": True}
        with mock.patch.object(provider, "fetch") as fetch:
            self.assertIsNone(provider.cloud(probe))
        fetch.assert_not_called()


def run_worker(request):
    """Feed one request to the HTTP worker in-process and return (exit code, fetch_direct kwargs)."""
    stdin = SimpleNamespace(buffer=io.BytesIO(json.dumps(request).encode()))
    stdout = SimpleNamespace(buffer=io.BytesIO())
    with mock.patch.object(provider.sys, "stdin", stdin), \
            mock.patch.object(provider.sys, "stdout", stdout), \
            mock.patch.object(provider.signal, "signal"), \
            mock.patch.object(provider.signal, "setitimer") as alarm, \
            mock.patch.object(provider, "fetch_direct",
                              return_value=b'{"choices":[]}') as direct:
        code = provider.http_request_worker()
    sent = direct.call_args.kwargs if direct.called else None
    return code, sent, alarm


def worker_fetch(url, timeout, body=None, token=None, extra_headers=None, connect_timeout=None):
    """Stand-in for fetch() that runs the real worker validation, so a caller asking for a deadline the worker refuses fails here the way it fails in production."""
    body = body if isinstance(body, bytes) else None if body is None else json.dumps(body).encode()
    request = {"url": url, "timeout": timeout,
               "body": base64.b64encode(body).decode("ascii") if body is not None else None,
               "token": token, "extra_headers": extra_headers}
    if connect_timeout is not None:
        request["connect_timeout"] = connect_timeout
    code, _, _ = run_worker(request)
    if code != 0:
        raise ValueError("HTTP request failed")
    return {"choices": [{"message": {"content": json.dumps(
        {"candidates": [{"text": "你好"}]}, ensure_ascii=False)}}]}


class AiRequestBudget(unittest.TestCase):
    def test_windows_budget(self):
        # Windows ai_assistant.cpp: CURLOPT_TIMEOUT_MS 8000, CURLOPT_CONNECTTIMEOUT_MS 2500.
        self.assertEqual(provider.AI_REQUEST_TIMEOUT, 8.0)
        self.assertEqual(provider.AI_CONNECT_TIMEOUT, 2.5)

    def test_worker_accepts_the_ai_budget_and_arms_its_alarm(self):
        code, sent, alarm = run_worker({"url": "https://ai.invalid/", "timeout": 8.0,
                                        "connect_timeout": 2.5, "body": None})
        self.assertEqual(code, 0)
        self.assertEqual(sent["timeout"], 8.0)
        self.assertEqual(sent["connect_timeout"], 2.5)
        alarm.assert_called_once_with(provider.signal.ITIMER_REAL, 8.0)

    def test_worker_rejects_deadlines_outside_the_budget(self):
        for request in ({"timeout": 8.5}, {"timeout": 0}, {"timeout": "8"},
                        {"timeout": 2.0, "connect_timeout": 2.5},
                        {"timeout": 8.0, "connect_timeout": 0},
                        {"timeout": 8.0, "connect_timeout": "2.5"}):
            code, sent, _ = run_worker({"url": "https://ai.invalid/", "body": None, **request})
            self.assertEqual(code, 1, request)
            self.assertIsNone(sent, request)

    def test_candidate_request_uses_the_ai_budget(self):
        with mock.patch.object(provider, "fetch", side_effect=worker_fetch) as fetch:
            rows = provider.ai(query(), {"provider": "synthetic",
                                         "endpoint": "https://ai.invalid/v1/chat/completions",
                                         "model": "synthetic-model", "token": "synthetic-token"})
        self.assertEqual(rows, [{"text": "你好", "source": 1}])
        self.assertEqual(fetch.call_args.args[1], provider.AI_REQUEST_TIMEOUT)
        self.assertEqual(fetch.call_args.kwargs["connect_timeout"], provider.AI_CONNECT_TIMEOUT)

    def test_polish_test_asks_for_a_deadline_the_worker_accepts(self):
        # ai_polish_test once asked for 8 s while the worker capped at 7 s, so every press came back empty.
        profile = {"provider": "synthetic", "endpoint": "https://ai.invalid/v1/chat/completions",
                   "model": "synthetic-model", "token": "synthetic-token"}
        with mock.patch.object(provider, "select_ai_profile", return_value=profile), \
                mock.patch.object(provider, "fetch", side_effect=worker_fetch):
            result = provider.ai_polish_test(
                {"provider": "synthetic", "endpoint": profile["endpoint"],
                 "model": "synthetic-model", "prompt": "p", "text": "t"},
                SimpleNamespace(ai_config_path=Path("/private/synthetic.json")))
        self.assertNotEqual(result, {"text": ""})

    def test_connect_limit_covers_only_the_handshake(self):
        connection = provider.ConnectBoundedHTTPSConnection(
            "ai.invalid", timeout=8.0, connect_timeout=2.5)
        seen = []

        def connect(self):
            seen.append(self.timeout)
            self.sock = mock.Mock()

        with mock.patch.object(provider.http.client.HTTPSConnection, "connect", connect):
            connection.connect()
        self.assertEqual(seen, [2.5])
        self.assertEqual(connection.timeout, 8.0)
        connection.sock.settimeout.assert_called_once_with(8.0)

    def test_connect_limit_never_extends_a_shorter_request(self):
        connection = provider.ConnectBoundedHTTPSConnection(
            "ai.invalid", timeout=1.0, connect_timeout=2.5)
        seen = []

        def connect(self):
            seen.append(self.timeout)
            self.sock = mock.Mock()

        with mock.patch.object(provider.http.client.HTTPSConnection, "connect", connect):
            connection.connect()
        self.assertEqual(seen, [1.0])



def request_line(kind, size, query=None):
    """One request line whose JSON, without the newline, is exactly size bytes, serialised the way serde_json writes it."""
    query = dict(query or {}, text="")
    encode = lambda value: json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode()
    base = len(encode({"version": 1, "kind": kind, "query": query}))
    query["text"] = "a" * (size - base)
    line = encode({"version": 1, "kind": kind, "query": query})
    assert len(line) == size
    return line + b"\n"


class RequestLineLimit(unittest.TestCase):
    """UnixSocketProvider::ai_test sends up to 32768 bytes; every other request is capped at 16384."""

    def read(self, line):
        return provider.read_request(io.BytesIO(line))

    def test_polish_test_between_16_and_32_kib_is_read(self):
        for size in (16385, 24576, 32768):
            request = self.read(request_line("ai_test", size))
            self.assertIsNotNone(request, size)
            self.assertEqual(request["kind"], "ai_test")

    def test_polish_test_over_the_client_limit_is_refused(self):
        self.assertIsNone(self.read(request_line("ai_test", 32769)))

    def test_other_requests_keep_the_16_kib_limit(self):
        self.assertIsNotNone(self.read(request_line("online", 16384)))
        for kind in ("online", "translation", "ai_models", "credential_test"):
            self.assertIsNone(self.read(request_line(kind, 16385)), kind)

    def test_truncated_line_is_refused(self):
        self.assertIsNone(self.read(request_line("ai_test", 20000)[:-1]))

    def test_longest_polish_test_the_client_allows_gets_an_answer(self):
        # An 8192-byte prompt and an 8192-byte sample, the most ai_test accepts, used to be dropped without a reply.
        profile = {"provider": "synthetic", "endpoint": "https://ai.invalid/v1/chat/completions",
                   "model": "synthetic-model", "token": "synthetic-token"}
        line = json.dumps({"version": 1, "kind": "ai_test", "query": {
            "provider": "synthetic", "endpoint": profile["endpoint"], "model": "synthetic-model",
            "prompt": "提" * 2730, "text": "字" * 2730}}, ensure_ascii=False,
            separators=(",", ":")).encode() + b"\n"
        self.assertGreater(len(line), 16384)
        request = self.read(line)
        self.assertIsNotNone(request)
        with mock.patch.object(provider, "select_ai_profile", return_value=profile), \
                mock.patch.object(provider, "fetch", side_effect=worker_fetch):
            result = provider.ai_polish_test(
                request["query"], SimpleNamespace(ai_config_path=Path("/private/synthetic.json")))
        self.assertNotEqual(result, {"text": ""})


if __name__ == "__main__":
    unittest.main()
