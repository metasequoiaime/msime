#!/usr/bin/env python3
"""用真实 HTTP worker 验证 Linux 转写拒绝明确失败的正文。"""
import base64
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
    "voice_asr_reply_provider", str(ROOT / "platforms/linux/scripts/msime-linux-voice-provider"))
spec = importlib.util.spec_from_loader(loader.name, loader)
voice = importlib.util.module_from_spec(spec)
loader.exec_module(voice)

TRANSCRIPT = "synthetic complete transcript"
PCM = b"\0\0" * 160


class LoopbackHTTPServer(http.server.HTTPServer):
    def server_bind(self):
        # 回环服务无需反向 DNS，避免主机网络影响合成测试。
        socketserver.TCPServer.server_bind(self)
        self.server_name, self.server_port = self.server_address[:2]


class AsrReplyTests(unittest.TestCase):
    def test_explicit_failure_rejects_all_text_aliases_without_retry(self):
        cases = {}
        for shape in ("text", "transcription", "result", "chat"):
            for status in ("legacy", "stop", "null-finish", "null-error", "length", "content_filter",
                           "object-error", "empty-error", "string-error", "false-error",
                           "zero-error", "blank-error"):
                choice = {"message": {"content": TRANSCRIPT}} if shape == "chat" else {}
                reply = {"choices": [choice]}
                if shape in ("text", "transcription"):
                    reply[shape] = TRANSCRIPT
                elif shape == "result":
                    reply["result"] = {"text": TRANSCRIPT}
                if status in ("stop", "length", "content_filter"):
                    choice["finish_reason"] = status
                elif status == "null-finish":
                    choice["finish_reason"] = None
                elif status.endswith("-error"):
                    reply["error"] = {"null-error": None, "object-error": {"message": "synthetic failure"},
                                      "empty-error": {}, "string-error": "synthetic failure",
                                      "false-error": False, "zero-error": 0, "blank-error": ""}[status]
                accepted = status in ("legacy", "stop", "null-finish", "null-error")
                cases[f"/{shape}/{status}"] = (reply, accepted, "bailian" if shape == "chat" else "openai")
        # SiliconFlow 只应重试网络类故障，协议明确失败不能再次请求。
        cases["/siliconflow/error"] = ({"text": TRANSCRIPT, "error": {}}, False, "siliconflow")
        counts = collections.Counter()
        errors = []

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_POST(self):
                try:
                    counts[self.path] += 1
                    body = self.rfile.read(int(self.headers["Content-Length"]))
                    assert self.headers["Authorization"] == "Bearer synthetic-token"
                    reply, _accepted, provider = cases[self.path]
                    if provider == "bailian":
                        request = json.loads(body)
                        assert self.headers["Content-Type"] == "application/json"
                        assert request["model"] == "synthetic-model"
                        assert request["stream"] is False
                        assert request["messages"][0]["role"] == "user"
                        part = request["messages"][0]["content"][0]
                        assert part["type"] == "input_audio"
                        assert part["input_audio"]["data"] == (
                            "data:audio/wav;base64," + base64.b64encode(voice.wav_audio(PCM)).decode("ascii"))
                    else:
                        assert self.headers["Content-Type"].startswith("multipart/form-data; boundary=")
                        assert b'name="model"\r\n\r\nsynthetic-model\r\n' in body
                        assert b'name="file"; filename="audio.wav"' in body
                        assert b"RIFF" in body and b"WAVE" in body
                    response = json.dumps(reply).encode()
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Content-Length", str(len(response)))
                    self.end_headers()
                    self.wfile.write(response)
                except Exception:
                    # 请求与异常正文不进入测试输出。
                    errors.append("invalid synthetic request")
                    self.send_error(500)

        # 子进程继承代理环境，回环请求必须直接访问本测试服务。
        with patch.dict(os.environ, {"NO_PROXY": "127.0.0.1", "no_proxy": "127.0.0.1"}), \
                LoopbackHTTPServer(("127.0.0.1", 0), Handler) as server:
            worker = threading.Thread(target=server.serve_forever, daemon=True)
            worker.start()
            try:
                for path, (_reply, accepted, provider) in cases.items():
                    with self.subTest(path=path):
                        config = {"provider": provider, "model": "synthetic-model", "token": "synthetic-token",
                                  "endpoint": f"http://127.0.0.1:{server.server_port}{path}"}
                        if accepted:
                            self.assertEqual(voice.transcribe(PCM, config, "auto", voice.Session()), TRANSCRIPT)
                        else:
                            with self.assertRaises(voice.VoiceRequestError) as caught:
                                voice.transcribe(PCM, config, "auto", voice.Session())
                            self.assertIs(caught.exception.retryable, False)
                            self.assertEqual(str(caught.exception), "voice request failed")
                        self.assertEqual(counts[path], 1)
                self.assertEqual(errors, [])
            finally:
                server.shutdown()
                worker.join()


if __name__ == "__main__":
    unittest.main()
