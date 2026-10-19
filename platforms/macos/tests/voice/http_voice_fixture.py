"""Synthetic loopback-only HTTP fixture; never log request bodies or headers."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from socketserver import TCPServer
import json
import subprocess
import sys
import threading
import time

errors = []
counts = {}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        try:
            size = int(self.headers["Content-Length"])
            # The long upload is the one request allowed near the 20 MiB batch budget.
            assert 0 < size < (21 * 1024 * 1024 if self.path == "/asr-long" else 65536)
            body = self.rfile.read(size)
            while len(body) < size:
                more = self.rfile.read(size - len(body))
                assert more
                body += more
            assert self.headers["Authorization"] == "Bearer fixture-token"
            counts[self.path] = counts.get(self.path, 0) + 1
            if self.path == "/asr":
                assert b"fixture-model" in body and b"RIFF" in body
                assert b"changed-after-snapshot" not in body
                assert b'name="language"\r\n\r\nen\r\n' in body
                response = {"text": "synthetic transcript"}
            elif self.path == "/asr-error":
                response = {"text": "synthetic transcript", "error": {"message": "synthetic failure"}}
            elif self.path in ("/asr-incomplete", "/asr-filtered-alias"):
                response = {"choices": [{"message": {"content": "synthetic transcript"},
                                         "finish_reason": "length" if self.path == "/asr-incomplete" else "content_filter"}]}
                if self.path == "/asr-filtered-alias":
                    response["text"] = "synthetic transcript"
            elif self.path in ("/asr-denied", "/asr-trace"):
                assert b"RIFF" in body
                if self.path == "/asr-denied":
                    self.send_response(401)
                    self.end_headers()
                    self.wfile.write(json.dumps({"error": {"message": "Incorrect synthetic key"}}).encode())
                else:
                    self.send_response(500)
                    self.send_header("X-SiliconCloud-Trace-Id", "synthetic-trace")
                    self.end_headers()
                return
            elif self.path == "/asr-long":
                riff = body.index(b"RIFF")
                assert body[riff + 36:riff + 40] == b"data"
                samples = int.from_bytes(body[riff + 40:riff + 44], "little") // 2
                assert 44 + samples * 2 <= 20 * 1024 * 1024
                response = {"text": f"synthetic long {samples}"}
            else:
                document = json.loads(body)
                assert document["messages"][0]["content"] == "synthetic prompt"
                response = {"choices": [{"message": {"content": "synthetic polished"}}]}
            if self.path == "/polish-timeout":
                time.sleep(6)
            self.send_response(500 if self.path == "/polish-failure" else 200)
            self.end_headers()
            self.wfile.write(json.dumps(response).encode())
        except (BrokenPipeError, ConnectionResetError):
            if self.path != "/polish-timeout":
                errors.append("unexpected disconnected fixture")
        except Exception:
            errors.append("fixture assertion failed")
            self.send_error(500)


class Server(ThreadingHTTPServer):
    def server_bind(self):
        TCPServer.server_bind(self)
        self.server_name = "localhost"
        self.server_port = self.server_address[1]


server = Server(("127.0.0.1", 0), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
try:
    # The six-second polish stall is waited out rather than abandoned, so the run needs room for it.
    result = subprocess.run([sys.argv[1], f"http://127.0.0.1:{server.server_port}"], timeout=60)
    assert result.returncode == 0 and not errors, (result.returncode, errors)
    assert counts == {"/asr": 3, "/asr-long": 1, "/asr-denied": 1, "/asr-trace": 2,
                      "/asr-error": 1, "/asr-incomplete": 1, "/asr-filtered-alias": 1,
                      "/polish": 1, "/polish-failure": 1, "/polish-timeout": 1}, counts
finally:
    server.shutdown()
    server.server_close()
