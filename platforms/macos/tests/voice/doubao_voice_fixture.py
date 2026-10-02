"""Loopback WebSocket fixture; only synthetic PCM, never log headers or bodies."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import base64
import gzip
import hashlib
import json
import socketserver
import struct
import subprocess
import sys
import threading


# HTTPServer.server_bind resolves the bound address with socket.getfqdn, which waits on reverse DNS before this loopback server exists - 35 s on the macOS CI runners. Nothing reads server_name, so bind without it.
class LoopbackHTTPServer(ThreadingHTTPServer):
    def server_bind(self):
        socketserver.TCPServer.server_bind(self)
        self.server_name, self.server_port = self.server_address[:2]


errors = []
counts = {}


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def exact(self, size):
        data = self.rfile.read(size)
        if len(data) != size:
            raise EOFError()
        return data

    def receive(self):
        first, second = self.exact(2)
        if first & 15 == 8:
            raise EOFError()
        assert first == 0x82 and second & 128
        size = second & 127
        if size == 126:
            size = struct.unpack(">H", self.exact(2))[0]
        elif size == 127:
            size = struct.unpack(">Q", self.exact(8))[0]
        assert size < 65536
        mask = self.exact(4)
        payload = bytes(value ^ mask[i % 4] for i, value in enumerate(self.exact(size)))
        assert payload[0] == 0x11 and payload[2:4] == b"\x11\0"
        sequence, length = struct.unpack(">iI", payload[4:12])
        assert length == len(payload) - 12
        return payload[1], sequence, gzip.decompress(payload[12:])

    def send(self, payload):
        header = b"\x82"
        if len(payload) < 126:
            header += bytes([len(payload)])
        elif len(payload) < 65536:
            header += b"\x7e" + struct.pack(">H", len(payload))
        else:
            header += b"\x7f" + struct.pack(">Q", len(payload))
        self.wfile.write(header + payload)
        self.wfile.flush()

    def transcript(self, text, final=False):
        body = gzip.compress(json.dumps({"result": {"text": text}}).encode())
        self.send(bytes([0x11, 0x93 if final else 0x91, 0x11, 0]) +
                  struct.pack(">iI", -2 if final else 2, len(body)) + body)

    def do_GET(self):
        try:
            counts[self.path] = counts.get(self.path, 0) + 1
            assert self.path != "/leaked"
            if self.path == "/redirect":
                self.send_response(302)
                self.send_header("Location", f"http://127.0.0.1:{self.server.server_port}/leaked")
                self.send_header("Content-Length", "0")
                self.end_headers()
                return
            legacy = self.path in ("/legacy", "/trimmed-legacy")
            assert self.headers.get("X-Api-Key") == (None if legacy else "fixture-token")
            assert self.headers.get("X-Api-App-Key") == ("stale-fixture-app" if legacy else None)
            assert self.headers.get("X-Api-Access-Key") == ("fixture-token" if legacy else None)
            assert self.headers["X-Api-Resource-Id"] == "volc.bigasr.sauc.duration"
            assert len(self.headers["X-Api-Request-Id"]) == 36
            accept = base64.b64encode(hashlib.sha1((self.headers["Sec-WebSocket-Key"] +
                "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
            self.send_response(101)
            self.send_header("Upgrade", "websocket")
            self.send_header("Connection", "Upgrade")
            self.send_header("Sec-WebSocket-Accept", accept)
            self.end_headers()
            self.connection.settimeout(40)
            kind, sequence, raw = self.receive()
            assert kind == 0x11 and sequence == 1
            request = json.loads(raw)["request"]
            assert request["enable_itn"] is False and request["enable_punc"] is False
            assert request["enable_ddc"] is True and request["corpus"]["boosting_table_id"] == "fixture-table"
            if self.path == "/malformed":
                self.send(b"invalid")
            elif self.path == "/server-error":
                self.send(b"\x11\xf0\0\0" + struct.pack(">II", 45000001, 9) + b"synthetic")
            elif self.path == "/oversized":
                self.send(bytes(1024 * 1024 + 1))
            elif self.path == "/silent":
                kind, sequence, pcm = self.receive()
                assert kind == 0x23 and sequence == -2 and not pcm
                self.receive()  # The native finish deadline must close the socket.
            elif self.path == "/invalid-pcm":
                self.receive()
            elif self.path == "/long":
                # A stream well past the old 60 s cap arrives whole, as the MSIME-Windows client sends it.
                samples = 0
                while True:
                    kind, sequence, pcm = self.receive()
                    assert kind in (0x21, 0x23)
                    samples += len(pcm) // 2
                    if kind == 0x23:
                        break
                self.transcript(f"synthetic long {samples}", True)
            else:
                kind, sequence, pcm = self.receive()
                assert kind == 0x21 and sequence == 2 and pcm == b"\xff\x1f" * 3200
                self.transcript("synthetic partial")
                if self.path in ("/cancel", "/drop"):
                    self.receive()
                else:
                    kind, sequence, pcm = self.receive()
                    assert kind == 0x23 and sequence == -3 and pcm == b"\xff\x1f" * 19
                    self.transcript("synthetic final", True)
            self.close_connection = True
        except (EOFError, ConnectionError):
            self.close_connection = True
        except Exception:
            errors.append("WebSocket fixture assertion failed")
            self.close_connection = True


server = LoopbackHTTPServer(("127.0.0.1", 0), Handler)
# A handler can sit in a 40 s socket read; joining it on close outlived ctest's limit, so a hang was killed before this script could say anything.
server.daemon_threads = True
server.block_on_close = False
threading.Thread(target=server.serve_forever, daemon=True).start()
try:
    client = subprocess.Popen([sys.argv[1], f"ws://127.0.0.1:{server.server_port}"])
    try:
        returncode = client.wait(timeout=60)
    except subprocess.TimeoutExpired:
        # Every wait in the client fails on its own deadline, so reaching this means a thread is stuck; its stack is the only useful output.
        subprocess.run(["sample", str(client.pid), "3"], stdout=sys.stderr, stderr=sys.stderr)
        client.kill()
        raise
    assert returncode == 0 and not errors
    assert all(counts.get(path) == 1 for path in
               ("/api", "/legacy", "/unset", "/masked-app", "/trimmed", "/trimmed-legacy",
                "/malformed", "/server-error", "/oversized", "/redirect", "/cancel", "/drop", "/silent", "/long"))
    assert "/leaked" not in counts
finally:
    server.shutdown()
    server.server_close()
