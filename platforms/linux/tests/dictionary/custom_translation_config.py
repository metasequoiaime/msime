#!/usr/bin/env python3
"""Real socket/HTTP coverage using only synthetic translation fixtures."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import socket
import socketserver
import subprocess
import sys
import tempfile
import threading
import time
import unittest


# HTTPServer.server_bind resolves the bound address with socket.getfqdn, which waits on reverse DNS before this loopback server exists - 35 s on the macOS CI runners. Nothing reads server_name, so bind without it.
class LoopbackHTTPServer(ThreadingHTTPServer):
    def server_bind(self):
        socketserver.TCPServer.server_bind(self)
        self.server_name, self.server_port = self.server_address[:2]


ROOT = Path(__file__).resolve().parents[2]


class CustomTranslationConfig(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="msime-custom-")
        self.addCleanup(directory.cleanup)
        self.address = Path(directory.name) / "provider.sock"
        self.calls = []
        calls = self.calls
        class HTTPHandler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_POST(self):
                body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                calls.append((self.path, self.headers.get("Authorization"), body))
                payload = json.dumps({"data": "", "translation": "synthetic " + body["source_lang"]}).encode()
                self.send_response(200)
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)
        http = LoopbackHTTPServer(("127.0.0.1", 0), HTTPHandler)
        thread = threading.Thread(target=http.serve_forever, daemon=True)
        thread.start()
        def stop_http():
            http.shutdown()
            http.server_close()
            thread.join(timeout=3)
        self.addCleanup(stop_http)
        self.endpoint = "http://127.0.0.1:" + str(http.server_port) + "/translate"
        process = subprocess.Popen([sys.executable, str(ROOT / "scripts" / "msime-linux-online-provider"), str(self.address)],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        def stop_provider():
            if process.poll() is None:
                process.terminate()
            process.wait(timeout=5)
        self.addCleanup(stop_provider)
        deadline = time.monotonic() + 3
        while not self.address.exists() and process.poll() is None and time.monotonic() < deadline:
            time.sleep(0.01)
        self.assertIsNone(process.poll())
        self.assertTrue(self.address.exists())

    def request(self, endpoint, token):
        query = {"candidates": ["测试", "synthetic"], "target_language": "fr",
                 "custom_translation": {"enabled": True, "endpoint": endpoint, "api_key": token}}
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(8)
            client.connect(str(self.address))
            client.sendall(json.dumps({"version": 1, "kind": "translation", "query": query}).encode() + b"\n")
            with client.makefile("rb") as reader:
                return json.loads(reader.readline(131073))["translations"]

    def credential_test(self, endpoint, token):
        query = {"service": "translation.custom",
                 "config": {"endpoint": endpoint, "api_key": token}}
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(8)
            client.connect(str(self.address))
            client.sendall(json.dumps({"version": 1, "kind": "credential_test",
                                       "query": query}).encode() + b"\n")
            with client.makefile("rb") as reader:
                return json.loads(reader.readline(4097))

    def test_normalized_requests_and_cache_identity(self):
        expected = [{"text": "测试", "translation": "synthetic ZH"},
                    {"text": "synthetic", "translation": "synthetic EN"}]
        self.assertEqual(self.request(self.endpoint + " ", " synthetic-local-key "), expected)
        self.assertEqual(len(self.calls), 2)
        for path, authorization, _ in self.calls:
            self.assertEqual(path, "/translate")
            self.assertEqual(authorization, "Bearer synthetic-local-key")
        self.assertEqual([(body["source_lang"], body["target_lang"]) for _, _, body in self.calls],
                         [("ZH", "FR"), ("EN", "ZH")])
        self.assertEqual(self.request(self.endpoint, "synthetic-local-key"), expected)
        self.assertEqual(len(self.calls), 2)
        self.assertEqual(self.request(self.endpoint, "synthetic-replacement-key"), expected)
        self.assertEqual(len(self.calls), 4)

    def test_whitespace_only_key_omits_authorization(self):
        self.assertEqual(len(self.request(self.endpoint, " \t\r\n")), 2)
        self.assertTrue(all(authorization is None for _, authorization, _ in self.calls))

    def test_credential_test_uses_the_same_bounded_provider_path(self):
        self.assertEqual(self.credential_test(self.endpoint, "synthetic-local-key"),
                         {"ok": True, "message": "连接成功，当前配置有效。"})
        self.assertEqual(len(self.calls), 1)
        self.assertEqual(self.calls[0][1], "Bearer synthetic-local-key")

    def test_unsupported_endpoint_does_not_send_http(self):
        for endpoint in ("file:///synthetic", "ftp://127.0.0.1/translate", " ",
                         "http://translate.example.test/translate",
                         "http://localhost.example.test/translate",
                         "http://127.0.0.2/translate"):
            self.assertEqual(self.request(endpoint, "synthetic-local-key"), [])
            self.assertFalse(self.credential_test(endpoint, "synthetic-local-key")["ok"])
        self.assertEqual(self.calls, [])


if __name__ == "__main__":
    unittest.main()
