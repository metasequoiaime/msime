"""Loopback synthetic text fixture; never log request bodies or credentials."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import socketserver
import subprocess
import sys
import threading
import time


# HTTPServer.server_bind resolves the bound address with socket.getfqdn, which waits on reverse DNS before this loopback server exists - 35 s on the macOS CI runners. Nothing reads server_name, so bind without it.
class LoopbackHTTPServer(ThreadingHTTPServer):
    def server_bind(self):
        socketserver.TCPServer.server_bind(self)
        self.server_name, self.server_port = self.server_address[:2]


errors = []
counts = {}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        try:
            size = int(self.headers["Content-Length"])
            assert 0 < size < 65536
            data = json.loads(self.rfile.read(size))
            assert self.headers["Authorization"] == "Bearer fixture-token"
            assert data["model"] == "fixture-model"
            assert data["messages"][0]["content"] == "synthetic prompt"
            assert data["messages"][1]["content"] == "<asr_text>\nsynthetic transcript\n</asr_text>"
            counts[self.path] = counts.get(self.path, 0) + 1
            if self.path == "/stall-headers":
                time.sleep(6)
            self.send_response(500 if self.path == "/failure" else 200)
            self.end_headers()
            if self.path == "/stall-body":
                self.wfile.flush()
                time.sleep(6)
            value = "" if self.path == "/empty" else "synthetic polished"
            payload = {"choices": [{"message": {"content": value}}]}
            if self.path in ("/length", "/content_filter"):
                payload["choices"][0]["finish_reason"] = self.path[1:]
            elif self.path == "/error":
                payload["error"] = {"message": "synthetic failure"}
            self.wfile.write(json.dumps(payload).encode())
        except (BrokenPipeError, ConnectionResetError):
            if not self.path.startswith("/stall-"):
                errors.append("unexpected disconnected fixture")
        except Exception:
            errors.append("fixture assertion failed")
            self.send_error(500)


server = LoopbackHTTPServer(("127.0.0.1", 0), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
try:
    # Two six-second stalls are waited out rather than abandoned, so the whole run needs room for them.
    result = subprocess.run([sys.argv[1], f"http://127.0.0.1:{server.server_port}"], timeout=60)
    assert result.returncode == 0 and not errors
    assert counts == {"/polish": 1, "/failure": 1, "/empty": 1,
                      "/length": 1, "/content_filter": 1, "/error": 1,
                      "/stall-headers": 1, "/stall-body": 1}
finally:
    server.shutdown()
    server.server_close()
