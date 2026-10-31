#!/usr/bin/env python3
"""对显式测试库验证真实 C ABI 到回环 HTTP；不启动产品或向生产后台发送。"""

import argparse
import ctypes
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import re
import tempfile
import threading


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", required=True, type=Path)
    parser.add_argument("--origin", required=True)
    parser.add_argument("--expect-rejected", action="store_true",
                        help="核对正式库或缺配置的测试库拒绝握手，完全不发送")
    args = parser.parse_args()
    match = re.fullmatch(r"http://127\.0\.0\.1:([1-9][0-9]{0,4})", args.origin)
    if not match or not 0 < int(match[1]) < 65536:
        parser.error("origin 必须是规范的 http://127.0.0.1:<非零端口>")
    port = int(match[1])
    library = ctypes.CDLL(str(args.library.resolve(strict=True)))
    library.msime_client_string_free.argtypes = [ctypes.c_void_p]
    library.msime_client_string_free.restype = None

    def call(name, document):
        function = getattr(library, "msime_client_telemetry_" + name)
        function.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
        function.restype = ctypes.c_void_p
        data = json.dumps(document).encode()
        pointer = function(data, len(data))
        assert pointer, name
        try:
            return json.loads(ctypes.string_at(pointer))
        finally:
            library.msime_client_string_free(pointer)

    def success(name, document):
        result = call(name, document)
        assert result.get("ok") is True, (name, result)
        return result["value"]

    class Handler(BaseHTTPRequestHandler):
        events = []
        failure_pending = True
        unexpected = []

        def log_message(self, *_args):
            pass

        def do_POST(self):
            if self.path != "/v1/telemetry/events":
                self.unexpected.append(self.path)
                self.send_error(500)
                return
            assert self.headers.get("Authorization") is None
            assert self.headers.get("Proxy-Authorization") is None
            assert self.headers.get("Cookie") is None
            length = int(self.headers["Content-Length"])
            assert 0 < length <= 32 * 1024
            event = json.loads(self.rfile.read(length))
            self.events.append(event)
            status, headers = 204, {}
            if event["kind"] == "crash" and Handler.failure_pending:
                Handler.failure_pending = False
                status, headers = 503, {"Retry-After": "0"}
            self.send_response(status)
            for key, value in headers.items():
                self.send_header(key, value)
            self.send_header("Content-Length", "0")
            self.end_headers()

    with tempfile.TemporaryDirectory(prefix="msime-loopback-") as temporary:
        directory = Path(temporary) / "telemetry"
        request = {"directory": str(directory), "platform": "linux", "version": "0.0.101",
                   "component": "synthetic-helper", "capture_only": True,
                   "enabled": True, "test_origin": args.origin}
        with ThreadingHTTPServer(("127.0.0.1", port), Handler) as server:
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                handshake = call("begin", request)
                if args.expect_rejected:
                    assert handshake.get("ok") is False, handshake
                    assert call("flush", request).get("ok") is False
                    assert not Handler.events and not Handler.unexpected
                    assert not directory.exists()
                    print("PASS: 不支持隔离握手的库在建队列和发送前拒绝请求")
                    return
                assert handshake.get("ok") is True, handshake
                wrong = dict(request, test_origin=args.origin + "/")
                # 先用完全不联网的 begin 证明确实解析测试字段；旧库若忽略未知字段，应在任何 flush 前停止。
                assert call("begin", wrong).get("ok") is False
                assert call("flush", wrong).get("ok") is False
                assert not Handler.events
                start = handshake["value"]
                record = Path(start["crash_record_path"])
                record.write_text(start["crash_record_header"]
                                  + "Synthetic isolated fault\nsynthetic.dll+0x10\n")
                restarted = dict(request, version="0.0.102")
                success("begin", restarted)
                assert not record.exists()
                failed = success("flush", restarted)
                assert failed["remaining"] >= 1, failed
                first = Handler.events[-1]
                assert first["kind"] == "crash" and first["version"] == "0.0.101", first
                assert "Synthetic isolated fault" in first["message"]
                assert first["stack"] == "synthetic.dll+0x10"
                assert temporary not in json.dumps(first)
                # 重试沿用故障事件身份和原版本；网络错误不生成第二个故障。
                delivered = success("flush", restarted)
                assert delivered["remaining"] == 0 and delivered["sent"] >= 1, delivered
                assert Handler.events[-1] == first
                assert sum(event["kind"] == "crash" for event in Handler.events) == 2
                assert not Handler.unexpected
                count = len(Handler.events)
                success("flush", dict(restarted, enabled=False))
                assert len(Handler.events) == count
                print("PASS: C ABI→HTTP、原版本补报、503 重试同一事件、无凭据、关闭后不发送")
            finally:
                server.shutdown()
                thread.join(timeout=5)
                assert not thread.is_alive()


if __name__ == "__main__":
    main()
