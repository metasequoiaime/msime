"""Exercise the real shared curl adapter against synthetic loopback responses."""
import collections
import http.server
import json
import os
import socketserver
import subprocess
import sys
import threading


# HTTPServer.server_bind resolves the bound address with socket.getfqdn, which waits on reverse DNS before this loopback server exists - 35 s on the macOS CI runners. Nothing reads server_name, so bind without it.
class LoopbackHTTPServer(http.server.HTTPServer):
    def server_bind(self):
        socketserver.TCPServer.server_bind(self)
        self.server_name, self.server_port = self.server_address[:2]


REDIRECT_TARGETS = {"asr": "/asr/success", "polish": "/polish/success"}


class SharedVoiceHandler(http.server.BaseHTTPRequestHandler):
    counts = collections.Counter()

    def log_message(self, *_args):
        pass

    def do_POST(self):
        self.rfile.read(int(self.headers.get("Content-Length", "0")))
        self.counts[self.path] += 1
        mode, scenario = self.path.strip("/").split("/")
        status = 200
        payload = {"text": "synthetic result"} if mode == "asr" else {
            "choices": [{"message": {"content": "synthetic result"}}]}
        body = json.dumps(payload).encode()
        if scenario == "reject":
            status = 401
        elif scenario == "redirect":
            status = 307
        elif scenario == "retry" and self.counts[self.path] == 1:
            status = 503
        elif scenario == "malformed":
            body = b"not-json"
        elif scenario == "oversized":
            # Still valid JSON: without the byte limit this would be accepted.
            body = b" " * (1024 * 1024 + 1) + body
        self.send_response(status)
        if status == 307:
            # Pick the target from a fixed table instead of echoing the request path, so no header value is ever built from request data.
            self.send_header("Location", REDIRECT_TARGETS[mode])
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        try:
            self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError):
            pass  # The response-size guard may close the connection early.


with LoopbackHTTPServer(("127.0.0.1", 0), SharedVoiceHandler) as server:
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    environment = dict(os.environ, NO_PROXY="127.0.0.1", no_proxy="127.0.0.1")
    try:
        for mode in ("asr", "polish"):
            for scenario in ("success", "reject", "redirect", "malformed", "oversized"):
                path = f"/{mode}/{scenario}"
                url = f"http://127.0.0.1:{server.server_port}{path}"
                subprocess.run([sys.argv[1], mode, "openai", url,
                                "success" if scenario == "success" else "failure"],
                               check=True, timeout=10, env=environment)
                assert SharedVoiceHandler.counts[path] == 1
            assert SharedVoiceHandler.counts[f"/{mode}/success"] == 1
        url = f"http://127.0.0.1:{server.server_port}/asr/retry"
        subprocess.run([sys.argv[1], "asr", "siliconflow", url, "success"],
                       check=True, timeout=10, env=environment)
        assert SharedVoiceHandler.counts["/asr/retry"] == 2
    finally:
        server.shutdown()
        worker.join(timeout=5)
