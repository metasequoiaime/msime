#!/usr/bin/env python3
"""Linux 执行共享云候选描述符，不复制 Google 协议或响应路径。"""
import importlib.machinery
import importlib.util
from pathlib import Path
import sys
import json
import io
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
loader = importlib.machinery.SourceFileLoader(
    "cloud_timeout_provider", str(ROOT / "scripts" / "msime-linux-online-provider"))
spec = importlib.util.spec_from_loader(loader.name, loader)
provider = importlib.util.module_from_spec(spec)
loader.exec_module(provider)


class CloudTimeoutParity(unittest.TestCase):
    def test_cloud_request_uses_two_second_total_budget(self):
        result = ["SUCCESS", [["nihao", ["你好"]]]]
        query = {"cloud_candidates": True, "cloud_eligible": True,
                 "query_text": "nihao", "scheme": 0,
                 "cloud_request": {"url": "https://inputtools.google.com/request?text=nihao", "method": "GET", "headers": {},
                                   "body_utf8": "", "timeout_ms": 2000, "connect_timeout_ms": 2000}}
        with mock.patch.object(provider, "fetch", return_value=result) as fetch:
            self.assertEqual(json.loads(provider.cloud(query)["cloud_response"]), result)
        self.assertEqual(fetch.call_args.args[1], 2.0)
        self.assertEqual(provider.CLOUD_REQUEST_TIMEOUT, 2.0)

    def test_custom_post_and_cache_pass_through_without_parsing(self):
        descriptor = {"url": "http://127.0.0.1:8764/candidates", "method": "POST", "headers": {"Authorization": "Bearer synthetic"},
                      "body_utf8": '{"query":"nihao"}', "timeout_ms": 200, "connect_timeout_ms": 200}
        query = {"cloud_candidates": True, "cloud_eligible": True, "cloud_request": descriptor}
        with mock.patch.object(provider, "fetch", return_value={"data": ["合成候选"]}) as fetch:
            self.assertEqual(json.loads(provider.cloud(query)["cloud_response"]), {"data": ["合成候选"]})
            fetch.assert_called_once_with(descriptor["url"], 0.2, descriptor["body_utf8"].encode(),
                                          extra_headers=descriptor["headers"], connect_timeout=0.2, max_response_bytes=262144)
        descriptor["cached_response"] = '{"candidates":[{"text":"合成缓存"}]}'
        with mock.patch.object(provider, "fetch") as fetch:
            self.assertEqual(provider.cloud(query)["cloud_response"], descriptor["cached_response"])
            fetch.assert_not_called()
        descriptor.pop("cached_response")
        descriptor["url"] = "http://public.example.invalid/api"
        with mock.patch.object(provider, "fetch") as fetch:
            self.assertIsNone(provider.cloud(query))
            fetch.assert_not_called()

    def test_online_line_has_room_for_full_config_and_keeps_other_limits(self):
        def line(kind, padding):
            return (json.dumps({"kind": kind, "query": {"synthetic": "x" * padding}}) + "\n").encode()
        self.assertIsNotNone(provider.read_request(io.BytesIO(line("online", 20000))))
        self.assertIsNone(provider.read_request(io.BytesIO(line("translation", 20000))))
        self.assertIsNone(provider.read_request(io.BytesIO(line("online", 131072))))
        self.assertEqual(provider.HTTP_WORKER_MAX_TIMEOUT, 10.0)


if __name__ == "__main__":
    unittest.main()
