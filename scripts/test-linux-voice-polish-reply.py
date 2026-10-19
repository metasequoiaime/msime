#!/usr/bin/env python3
"""用合成 HTTP 回复核对 Linux 语音润色的完整性与原文回退。"""
import collections
import http.server
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import signal
import socketserver
import sys
import threading
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
if not hasattr(signal, "SIGALRM") or not hasattr(signal, "setitimer"):
    print("skipped: Linux 语音 HTTP worker 需要 Unix 信号")
    sys.exit(0)
sys.path.insert(0, str(ROOT / "platforms/linux/scripts"))
loader = importlib.machinery.SourceFileLoader(
    "voice_polish_reply_provider", str(ROOT / "platforms/linux/scripts/msime-linux-voice-provider"))
spec = importlib.util.spec_from_loader(loader.name, loader)
voice = importlib.util.module_from_spec(spec)
loader.exec_module(voice)

TRANSCRIPT = "synthetic complete transcript"
POLISHED = "synthetic polished text"


class LoopbackHTTPServer(http.server.HTTPServer):
    def server_bind(self):
        # 测试只需要回环地址，不让 HTTPServer 的反向 DNS 查询拖延启动。
        socketserver.TCPServer.server_bind(self)
        self.server_name, self.server_port = self.server_address[:2]


class PolishReplyTests(unittest.TestCase):
    def test_http_reply_status_preserves_transcript_or_accepts_complete_text(self):
        def reply(**fields):
            return {"choices": [{"message": {"content": POLISHED}, **fields}]}

        cases = {
            "/length": (reply(finish_reason="length"), TRANSCRIPT),
            "/filter": (reply(finish_reason="content_filter"), TRANSCRIPT),
            "/error": ({**reply(), "error": {"message": "synthetic failure"}}, TRANSCRIPT),
            "/empty-error": ({**reply(), "error": {}}, TRANSCRIPT),
            "/false-error": ({**reply(), "error": False}, TRANSCRIPT),
            "/zero-error": ({**reply(), "error": 0}, TRANSCRIPT),
            "/empty-string-error": ({**reply(), "error": ""}, TRANSCRIPT),
            "/stop": (reply(finish_reason="stop"), POLISHED),
            "/legacy": (reply(), POLISHED),
            "/null-finish": (reply(finish_reason=None), POLISHED),
            "/null-error": ({**reply(finish_reason="stop"), "error": None}, POLISHED),
            "/empty-choices": ({"choices": []}, TRANSCRIPT),
        }
        counts = collections.Counter()
        errors = []

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_POST(self):
                try:
                    request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                    assert self.headers["Authorization"] == "Bearer synthetic-token"
                    assert request["model"] == "synthetic-model"
                    assert request["stream"] is False
                    assert request["messages"][1]["content"] == f"<asr_text>\n{TRANSCRIPT}\n</asr_text>"
                    counts[self.path] += 1
                    body = json.dumps(cases[self.path][0]).encode()
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)
                except Exception:
                    # 不向进程输出请求体、请求头或异常诊断。
                    errors.append("invalid synthetic request")
                    self.send_error(500)

        # HTTP worker 继承当前环境，合成回环请求不经过用户配置的代理。
        with patch.dict(os.environ, {"NO_PROXY": "127.0.0.1", "no_proxy": "127.0.0.1"}), \
                LoopbackHTTPServer(("127.0.0.1", 0), Handler) as server:
            worker = threading.Thread(target=server.serve_forever, daemon=True)
            worker.start()
            try:
                for path, (_reply, expected) in cases.items():
                    with self.subTest(path=path):
                        config = {"provider": "openai", "model": "synthetic-model",
                                  "token": "synthetic-token",
                                  "endpoint": f"http://127.0.0.1:{server.server_port}{path}"}
                        self.assertEqual(voice.polish(TRANSCRIPT, config, {"polish_enabled": True}), expected)
                        self.assertEqual(counts[path], 1)
                self.assertEqual(errors, [])
            finally:
                server.shutdown()
                worker.join()


if __name__ == "__main__":
    unittest.main()
