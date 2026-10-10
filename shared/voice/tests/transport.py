"""Exercise the real shared curl adapter against synthetic loopback responses."""
import base64
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
    bodies = {}

    def log_message(self, *_args):
        pass

    def do_POST(self):
        request = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        self.counts[self.path] += 1
        self.bodies[self.path] = (self.headers.get("Content-Type", ""), request)
        mode, scenario = self.path.strip("/").split("/")
        status = 200
        # chat 场景是阿里云百炼的回答形状：文字在 choices[0].message.content。
        payload = {"text": "synthetic result"} if mode == "asr" and scenario != "chat" else {
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
        # The upload itself: the model, the language component of the host's zh-CN tag for OpenAI, and the recording as a WAV file part. SiliconFlow is sent no language field at all.
        content_type, body = SharedVoiceHandler.bodies["/asr/success"]
        assert content_type.startswith("multipart/form-data; boundary="), content_type
        assert b'name="model"\r\n\r\nfixture\r\n' in body
        assert b'name="language"\r\n\r\nzh\r\n' in body
        assert b'name="file"; filename="audio.wav"\r\nContent-Type: audio/wav\r\n\r\nRIFF' in body
        assert b"WAVEfmt " in body
        assert b'name="language"' not in SharedVoiceHandler.bodies["/asr/retry"][1]
        # 阿里云百炼：JSON 请求体，录音是唯一一条 user 消息里的 Base64 数据 URL。
        url = f"http://127.0.0.1:{server.server_port}/asr/chat"
        subprocess.run([sys.argv[1], "asr", "bailian", url, "success"],
                       check=True, timeout=10, env=environment)
        content_type, body = SharedVoiceHandler.bodies["/asr/chat"]
        assert content_type == "application/json", content_type
        request = json.loads(body)
        assert request["model"] == "fixture" and request["stream"] is False
        assert len(request["messages"]) == 1 and request["messages"][0]["role"] == "user"
        part = request["messages"][0]["content"][0]
        assert part["type"] == "input_audio"
        data = part["input_audio"]["data"]
        assert data.startswith("data:audio/wav;base64,UklGR"), data[:40]
        assert base64.b64decode(data.split(",", 1)[1])[8:16] == b"WAVEfmt "
    finally:
        server.shutdown()
        worker.join(timeout=5)
