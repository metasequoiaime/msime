#!/usr/bin/env python3
"""The real voice service with on-device recognition: no private file, a fake recorder and either the fake helper or the built msime-voice-local without its runtime.

A `local` request must record, forward partials and the final transcript, and hand the model path, language and dictionary hotwords to the helper; an unusable model path fails the request; a helper whose sherpa-onnx runtime is missing answers voice_dependency_missing with detail local_asr and leaves the service running. Linux only: the provider authenticates peers with SO_PEERCRED.

Usage: provider_local.py <built msime-voice-local>
"""
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
PROVIDER = ROOT / "scripts" / "msime-linux-voice-provider"
FAKE_HELPER = Path(__file__).resolve().parent / "local_fake_helper.py"
BUILT_HELPER = None
# Endless silence; the recording limit ends it after two seconds.
# cat 按运行测试时的 PATH 解析，不假定 /bin/cat：Nix 构建沙箱里 /bin 下只有 sh。
FAKE_RECORDER = "#!/bin/sh\nexec %s /dev/zero\n" % shutil.which("cat")


@unittest.skipUnless(sys.platform == "linux", "requires SO_PEERCRED peer authentication")
class LocalProvider(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.root.chmod(0o700)
        self.socket = self.root / "voice.sock"
        self.bin = self.root / "bin"
        self.bin.mkdir()
        recorder = self.bin / "parec"
        recorder.write_text(FAKE_RECORDER)
        recorder.chmod(0o755)
        self.log = self.root / "helper.log"
        self.fake = self.root / "msime-voice-local"
        self.fake.write_text('#!/bin/sh\nexec "%s" "%s" "$@"\n' % (sys.executable, FAKE_HELPER))
        self.fake.chmod(0o755)
        self.model = self.root / "x-asr-zh-en-streaming"
        self.model.mkdir()
        (self.model / "msime-model.json").write_text(json.dumps({"id": "x-asr-zh-en-streaming", "hotwords": "native"}))
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

    def start(self, helper):
        # No --config file exists: on-device recognition must not need one. PATH holds only the fake recorder; cue players are optional.
        environment = {"PATH": str(self.bin), "LC_ALL": "C.UTF-8", "MSIME_VOICE_LOCAL_HELPER": str(helper),
                       "FAKE_HELPER_LOG": str(self.log)}
        self.process = subprocess.Popen(
            [sys.executable, "-S", str(PROVIDER), str(self.socket), "--config", str(self.root / "voice-provider.json"),
             "--capture", "pulse", "--max-recording-seconds", "2"],
            env=environment, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        for _ in range(200):
            if self.process.poll() is not None:
                self.fail("provider exited: " + self.process.stderr.read().decode())
            if self.socket.exists():
                return
            self.process_wait(0.05)
        self.fail("provider socket did not appear")

    def process_wait(self, seconds):
        try:
            self.process.wait(timeout=seconds)
        except subprocess.TimeoutExpired:
            pass

    def connect(self):
        client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        client.settimeout(30)
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

    def options(self, **extra):
        return dict({"asr_provider": "local", "asr_model_path": str(self.model),
                     "voice_hotwords": "水杉\tshui shan\n输入法\tshu ru fa"}, **extra)

    def requests(self):
        return [json.loads(line) for line in self.log.read_text(encoding="utf-8").splitlines()]

    def test_local_recording_streams_partials_and_the_final_transcript(self):
        self.start(self.fake)
        client, lines = self.voice(1, self.options())
        with client, lines:
            self.assertEqual(self.read(lines), {"generation": 1, "type": "status", "phase": "recording", "ok": True})
            replies = []
            while True:
                reply = self.read(lines)
                replies.append(reply)
                if reply["type"] == "final":
                    break
            self.assertEqual(replies[-1], {"generation": 1, "text": "水杉输入法", "type": "final", "ok": True})
            for reply in replies[:-1]:
                if reply["type"] == "partial":
                    self.assertRegex(reply["text"], r"^听到\d秒$")
                else:
                    self.assertEqual(reply, {"generation": 1, "type": "status", "phase": "recognizing", "ok": True})
            self.assertEqual(lines.readline(), b"")
        start = self.requests()[0]
        self.assertEqual((start["op"], start["model"], start["language"], start["hotwords"]),
                         ("start", str(self.model), "zh-cn", ["水杉", "输入法"]))
        self.assertEqual(self.requests()[-1]["op"], "finish")
        self.assertGreater(sum(request.get("bytes", 0) for request in self.requests()), 32000)

        # The next recording reuses the warm helper.
        client, lines = self.voice(2, self.options())
        with client, lines:
            while True:
                reply = self.read(lines)
                if reply["type"] == "final":
                    break
            self.assertEqual(reply["text"], "水杉输入法")
        self.assertEqual(len({request["pid"] for request in self.requests()}), 1)
        self.assertIsNone(self.process.poll(), "voice service stays up")

    def test_an_unusable_model_path_fails_the_request(self):
        self.start(self.fake)
        for path in ("relative/model", str(self.root / "absent"), str(self.bin / "parec")):
            with self.subTest(path=path):
                client, lines = self.voice(3, self.options(asr_model_path=path))
                with client, lines:
                    self.assertEqual(self.read(lines), {"generation": 3, "text": "", "type": "final", "ok": False})
                    self.assertEqual(lines.readline(), b"")
        self.assertFalse(self.log.exists(), "no helper starts for an unusable model")

    def test_the_built_helper_without_its_runtime_is_a_missing_dependency(self):
        if BUILT_HELPER is None:
            self.skipTest("no built msime-voice-local given")
        self.start(BUILT_HELPER)
        client, lines = self.voice(4, self.options())
        with client, lines:
            self.assertEqual(self.read(lines), {"generation": 4, "text": "", "type": "final", "ok": False,
                                                "error": "voice_dependency_missing", "detail": "local_asr"})
            self.assertEqual(lines.readline(), b"")
        self.assertIsNone(self.process.poll(), "voice service stays up")
        self.process.send_signal(signal.SIGTERM)
        self.process.wait(timeout=10)
        self.assertIn("local_asr unavailable: ", self.process.stderr.read().decode())


if __name__ == "__main__":
    if len(sys.argv) > 1 and not sys.argv[1].startswith("-"):
        BUILT_HELPER = Path(sys.argv.pop(1)).resolve()
        if not os.access(BUILT_HELPER, os.X_OK):
            sys.exit("not an executable helper: %s" % BUILT_HELPER)
    unittest.main()
