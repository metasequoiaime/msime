#!/usr/bin/env python3
"""A missing or outdated optional dependency leaves the voice service running.

Starts the real provider with synthetic credentials, a fake recorder and either no websockets package or a stub of a release without the sync keepalive arguments. The service must stay up, a Doubao request must get the voice_dependency_missing line, and a batch-profile request must still reach recording. Linux only: the provider authenticates peers with SO_PEERCRED.
"""
import json
from pathlib import Path
import select
import signal
import socket
import subprocess
import sys
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[2]
PROVIDER = ROOT / "scripts" / "msime-linux-voice-provider"
CONFIG = {
    "asr": {"provider": "openai", "token": "fixture-token", "endpoint": "https://127.0.0.1:9/v1"},
    "asr_profiles": {"doubao": {"provider": "doubao", "token": "fixture-token",
                                "endpoint": "wss://127.0.0.1:9/asr", "doubao_auth_mode": "api_key"}},
}
# Only the keywords a pre-15 sync client takes: no ping_interval or ping_timeout.
OLD_CLIENT = '''class ClientConnection:
    def recv(self, timeout=None):
        pass


def connect(uri, *, additional_headers=None, user_agent_header=None, compression=None, open_timeout=None,
            close_timeout=None, max_size=None, max_queue=None, logger=None, create_connection=None, **kwargs):
    pass
'''
# 按 16 kHz、16-bit 单声道实时输出静音，避免瞬间填满整个录音预算。
FAKE_RECORDER = f"#!{sys.executable}\n" + '''import os
import time

while True:
    os.write(1, bytes(640))
    time.sleep(0.02)
'''


@unittest.skipUnless(sys.platform == "linux", "requires SO_PEERCRED peer authentication")
class DegradedStart(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.root.chmod(0o700)
        self.config = self.root / "voice-provider.json"
        self.config.write_text(json.dumps(CONFIG), encoding="utf-8")
        self.config.chmod(0o600)
        self.socket = self.root / "voice.sock"
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.packages = self.root / "packages"
        self.packages.mkdir()
        self.process = None

    def tearDown(self):
        if self.process and self.process.poll() is None:
            self.process.send_signal(signal.SIGTERM)
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
        if self.process:
            self.process.stderr.close()
        self.directory.cleanup()

    def install_recorder(self):
        recorder = self.bin / "parec"
        recorder.write_text(FAKE_RECORDER)
        recorder.chmod(0o755)

    def install_old_websockets(self):
        package = self.packages / "websockets"
        (package / "sync").mkdir(parents=True)
        (package / "__init__.py").write_text("")
        (package / "sync" / "__init__.py").write_text("")
        (package / "sync" / "client.py").write_text(OLD_CLIENT)
        metadata = self.packages / "websockets-14.2.dist-info"
        metadata.mkdir()
        (metadata / "METADATA").write_text("Metadata-Version: 2.1\nName: websockets\nVersion: 14.2\n")

    def start(self, *extra):
        # -S keeps any websockets in the system site-packages out of reach, so the package directory alone decides what is installed. PATH holds only the fake recorder; cue players are optional.
        environment = {"PATH": str(self.bin), "PYTHONPATH": str(self.packages), "LC_ALL": "C.UTF-8"}
        self.process = subprocess.Popen(
            [sys.executable, "-S", str(PROVIDER), str(self.socket), "--config", str(self.config),
             "--capture", "pulse", *extra],
            env=environment, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                self.fail("provider exited: " + self.process.stderr.read().decode())
            if self.socket.exists():
                return
            time.sleep(0.05)
        self.fail("provider socket did not appear")

    def connect(self):
        client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        client.settimeout(10)
        client.connect(str(self.socket))
        return client

    def voice(self, generation, options):
        client = self.connect()
        request = {"version": 1, "kind": "voice",
                   "query": {"language": "zh-cn", "generation": generation, "stream": True,
                             "events": ["status"], "options": dict(options, sound_enabled=False)}}
        client.sendall(json.dumps(request).encode() + b"\n")
        return client, client.makefile("rb")

    def read(self, lines):
        line = lines.readline()
        self.assertTrue(line.endswith(b"\n"), "provider closed without a reply")
        return json.loads(line)

    def assert_missing(self, generation, options, detail):
        client, lines = self.voice(generation, options)
        with client, lines:
            reply = self.read(lines)
            self.assertEqual(reply, {"generation": generation, "text": "", "type": "final", "ok": False,
                                     "error": "voice_dependency_missing", "detail": detail})
            self.assertEqual(lines.readline(), b"", "one line, then the connection closes")

    def assert_batch_records(self, generation):
        client, lines = self.voice(generation, {})
        with client, lines:
            status = self.read(lines)
            self.assertEqual(status, {"generation": generation, "type": "status", "phase": "recording", "ok": True})
            # 控制客户端稍晚发出取消时，录音也应保持活动，不能自行进入识别。
            self.assertEqual(select.select([client], [], [], 0.25)[0], [],
                             "recording must remain active until cancelled")
            with self.connect() as control:
                control.sendall(json.dumps({"version": 1, "kind": "voice_cancel",
                                            "query": {"generation": generation}}).encode() + b"\n")
            reply = self.read(lines)
            self.assertEqual(reply, {"generation": generation, "text": "", "type": "final", "ok": True})

    def assert_running(self):
        self.assertIsNone(self.process.poll(), "voice service stays up")
        with self.connect():
            pass

    def test_without_websockets_only_doubao_fails(self):
        self.install_recorder()
        self.start()
        self.assert_running()
        self.assert_missing(1, {"asr_provider": "doubao"}, "websockets")
        self.assert_batch_records(2)
        self.assert_running()

    def test_websockets_without_sync_keepalive_only_doubao_fails(self):
        self.install_recorder()
        self.install_old_websockets()
        self.start()
        self.assert_missing(3, {"asr_provider": "doubao"}, "websockets")
        self.assert_batch_records(4)
        self.assert_running()

    def test_without_recorder_requests_name_it(self):
        self.start()
        self.assert_running()
        self.assert_missing(5, {}, "recorder")
        self.assert_missing(6, {"capture_backend": "alsa"}, "recorder")
        self.assert_running()
        self.process.send_signal(signal.SIGTERM)
        self.process.wait(timeout=10)
        log = self.process.stderr.read().decode()
        self.assertIn("recording unavailable", log)
        self.assertIn("Doubao unavailable", log)

    def test_invalid_configuration_still_exits_2(self):
        self.install_recorder()
        self.process = subprocess.Popen(
            [sys.executable, "-S", str(PROVIDER), str(self.socket), "--config", str(self.config),
             "--max-recording-seconds", "0"],
            env={"PATH": str(self.bin)}, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        self.assertEqual(self.process.wait(timeout=10), 2)


if __name__ == "__main__":
    unittest.main()
