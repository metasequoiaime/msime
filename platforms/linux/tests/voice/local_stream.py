#!/usr/bin/env python3
"""On-device recognition in the Linux voice service: the helper session, the warm helper pool, model and hotword validation, and the private file rules for the `local` provider. Runs against a fake helper, so it needs neither the sherpa-onnx runtime nor a model, and runs on any Unix."""
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import threading
import unittest


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import msime_voice_local as local  # noqa: E402

loader = importlib.machinery.SourceFileLoader(
    "local_voice_provider", str(ROOT / "scripts" / "msime-linux-voice-provider"))
spec = importlib.util.spec_from_loader(loader.name, loader)
voice = importlib.util.module_from_spec(spec)
loader.exec_module(voice)

FAKE_HELPER = Path(__file__).resolve().parent / "local_fake_helper.py"


class LocalFixture(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="msime-local-")
        self.root = Path(self.directory.name)
        self.log = self.root / "helper.log"
        self.helper = self.root / "msime-voice-local"
        self.helper.write_text('#!/bin/sh\nexec "%s" "%s" "$@"\n' % (sys.executable, FAKE_HELPER))
        self.helper.chmod(0o755)
        self.environment = dict(os.environ)
        os.environ["FAKE_HELPER_LOG"] = str(self.log)
        self.pool = local.HelperPool(self.helper)
        self.model = self.root / "model"
        self.model.mkdir()
        (self.model / local.MANIFEST).write_text(json.dumps({"id": "x-asr", "hotwords": "native"}))

    def tearDown(self):
        self.pool.close()
        os.environ.clear()
        os.environ.update(self.environment)
        self.directory.cleanup()

    def requests(self):
        if not self.log.exists():
            return []
        return [json.loads(line) for line in self.log.read_text(encoding="utf-8").splitlines()]

    def stream(self, model=None, hotwords=(), cancelled=None):
        return local.LocalStream(self.pool, str(model or self.model), "zh-cn", list(hotwords),
                                 cancelled or threading.Event())


class LocalStreamTest(LocalFixture):
    def test_helper_discards_the_rest_of_an_oversized_output_line(self):
        fixture = self.root / "oversized-helper.py"
        fixture.write_text(
            "import json, sys\n"
            "sys.stdout.write(json.dumps({'type': 'hello', 'available': True}) + '\\n')\n"
            "sys.stdout.write('x' * (" + str(local.MAX_LINE + 1) + ") + "
            "json.dumps({'type': 'final', 'id': 1, 'text': 'forged'}) + '\\n')\n"
            "sys.stdout.write(json.dumps({'type': 'partial', 'id': 1, 'text': 'valid'}) + '\\n')\n"
            "sys.stdout.flush()\n"
        )
        helper = local.Helper([sys.executable, str(fixture)])
        try:
            helper.hello()
            self.assertEqual(helper.events.get(timeout=2), {"type": "partial", "id": 1, "text": "valid"})
        finally:
            helper.kill()

    def test_partials_final_and_the_start_request(self):
        stream = self.stream(hotwords=[{"text": "水杉", "pinyin": "shui shan"}])
        try:
            # 2.5 s of audio in uneven pieces; an odd trailing byte never reaches the helper.
            for _ in range(10):
                stream.feed(b"\0" * 8001)
            seen = []
            final = stream.finish(lambda: seen.append(stream.latest()))
            self.assertEqual(final, "水杉输入法")
            self.assertTrue(stream.done.is_set())
        finally:
            stream.close()
        requests = self.requests()
        start = requests[0]
        self.assertEqual({key: start[key] for key in ("op", "id", "model", "language", "hotwords", "threads")},
                         {"op": "start", "id": 1, "model": str(self.model), "language": "zh-cn",
                          "hotwords": ["水杉"], "threads": 0})
        audio = [request["bytes"] for request in requests if request["op"] == "audio"]
        self.assertEqual(sum(audio), 80010 // 2 * 2)
        self.assertTrue(all(size % 2 == 0 and size <= local.CHUNK_BYTES for size in audio))
        self.assertEqual(requests[-1]["op"], "finish")

    def test_latest_reports_partials_while_recording(self):
        stream = self.stream()
        try:
            stream.feed(b"\0" * 32000 * 2)
            for _ in range(50):
                if stream.drain(0.1) or stream.text == "听到2秒":
                    break
            self.assertEqual(stream.latest(), "听到2秒")
        finally:
            stream.close()

    def test_a_clean_session_keeps_the_helper_warm(self):
        first = self.stream()
        first.feed(b"\0" * 6400)
        self.assertEqual(first.finish(lambda: None), "水杉输入法")
        first.close()
        second = self.stream()
        second.feed(b"\0" * 6400)
        self.assertEqual(second.finish(lambda: None), "水杉输入法")
        second.close()
        pids = {request["pid"] for request in self.requests()}
        self.assertEqual(len(pids), 1, "the second recording reuses the first helper")
        starts = [request["id"] for request in self.requests() if request["op"] == "start"]
        self.assertEqual(starts, [1, 2], "each session has its own id")

    def test_cancel_confirms_and_the_helper_is_reused(self):
        cancelled = threading.Event()
        stream = self.stream(cancelled=cancelled)
        stream.feed(b"\0" * 6400)
        cancelled.set()
        self.assertEqual(stream.finish(lambda: None), "")
        stream.close()
        self.assertEqual(self.requests()[-1]["op"], "cancel")
        self.assertIsNotNone(self.pool.idle, "a confirmed cancel keeps the helper")
        again = self.stream()
        self.assertEqual(again.finish(lambda: None), "水杉输入法")
        again.close()
        self.assertEqual(len({request["pid"] for request in self.requests()}), 1)

    def test_a_helper_error_fails_the_recording(self):
        broken = self.root / "broken"
        broken.mkdir()
        stream = self.stream(model=broken)
        try:
            with self.assertRaises(ValueError):
                stream.finish(lambda: None)
        finally:
            stream.close()

    def test_a_crashed_helper_is_not_reused(self):
        crash = self.root / "crash"
        crash.mkdir()
        stream = self.stream(model=crash)
        stream.feed(b"\0" * 6400)
        with self.assertRaises(ValueError):
            stream.finish(lambda: None)
        stream.close()
        self.assertIsNone(self.pool.idle)

    def test_missing_runtime_or_helper_is_a_missing_dependency(self):
        os.environ["FAKE_HELPER_AVAILABLE"] = "0"
        with self.assertRaisesRegex(local.LocalUnavailable, "libsherpa-onnx-c-api"):
            self.stream()
        with self.assertRaises(local.LocalUnavailable):
            local.LocalStream(local.HelperPool(self.root / "absent"), str(self.model), "zh-cn", [], threading.Event())
        with self.assertRaises(local.LocalUnavailable):
            local.LocalStream(local.HelperPool(None), str(self.model), "zh-cn", [], threading.Event())

    def test_the_helper_is_told_when_to_exit(self):
        self.assertEqual(local.HelperPool(self.helper).command(), [str(self.helper), "--idle-exit", "600"])
        self.assertEqual(local.HelperPool(self.helper, "/opt/lib.so").command()[-2:], ["--runtime", "/opt/lib.so"])


class ModelAndHotwords(LocalFixture):
    def test_model_manifest(self):
        self.assertEqual(local.model_manifest(str(self.model))["hotwords"], "native")
        for path in (None, "", "model", "/tmp/\x01model", "/" + "x" * 4096):
            with self.subTest(path=path), self.assertRaises(ValueError):
                local.model_manifest(path)
        whisper = self.root / "ggml-base.bin"
        whisper.write_bytes(b"\0")
        with self.assertRaises(ValueError):
            local.model_manifest(str(whisper))
        empty = self.root / "empty"
        empty.mkdir()
        with self.assertRaises(OSError):
            local.model_manifest(str(empty))
        linked = self.root / "linked"
        linked.mkdir()
        (linked / local.MANIFEST).symlink_to(self.model / local.MANIFEST)
        with self.assertRaises(OSError):
            local.model_manifest(str(linked))
        external = self.root / "external"
        external.mkdir()
        (external / local.MANIFEST).write_text(json.dumps({"hotwords": "native"}))
        linked_model = self.root / "linked-model"
        linked_model.symlink_to(external, target_is_directory=True)
        with self.assertRaises(ValueError):
            local.model_manifest(str(linked_model))
        nested = external / "nested"
        nested.mkdir()
        (nested / local.MANIFEST).write_text("{}")
        # 放在其他人可写的目录里：CI 容器以 root 运行，root 建的链接在只有属主可写的目录里会被当成系统安装的可信链接。
        shared = self.root / "shared"
        shared.mkdir()
        shared.chmod(0o777)
        linked_parent = shared / "linked-parent"
        linked_parent.symlink_to(external, target_is_directory=True)
        with self.assertRaises(ValueError):
            local.model_manifest(str(linked_parent / "nested"))
        listed = self.root / "listed"
        listed.mkdir()
        (listed / local.MANIFEST).write_text("[]")
        with self.assertRaises(ValueError):
            local.model_manifest(str(listed))

    def test_parse_hotwords(self):
        self.assertEqual(local.parse_hotwords("水杉\tshui shan\n输入法\tshu ru fa\n水杉\tshui shan\n\n孤词\n"),
                         [{"text": "水杉", "pinyin": "shui shan"}, {"text": "输入法", "pinyin": "shu ru fa"}])
        self.assertEqual(local.parse_hotwords(None), [])
        self.assertEqual(len(local.parse_hotwords("\n".join("词%d\tci" % i for i in range(5000)))), local.MAX_HOTWORDS)

    def test_correction_without_the_host_library_keeps_the_text(self):
        hotwords = [{"text": "水杉", "pinyin": "shui shan"}]
        self.assertEqual(local.HotwordCorrector(None).correct("谁删", hotwords), "谁删")
        corrector = local.HotwordCorrector(self.root / "libmsime_host_api.so")
        self.assertEqual(corrector.correct("谁删", hotwords), "谁删")
        self.assertTrue(corrector.failed)
        self.assertEqual(corrector.correct("", hotwords), "")


class PrivateFile(unittest.TestCase):
    def load(self, document):
        with tempfile.TemporaryDirectory(prefix="msime-voice-") as directory:
            path = Path(directory) / "voice.json"
            path.write_text(json.dumps(document), encoding="utf-8")
            path.chmod(0o600)
            return voice.load_config(path)

    def test_local_needs_no_token(self):
        config = self.load({"asr": {"provider": "local"},
                            "polish": {"provider": "deepseek", "token": "sk-polish"}})
        self.assertEqual(config["asr"], {"provider": "local"})
        self.assertEqual(config["polish"]["endpoint"], voice.DEFAULT_ENDPOINTS["polish"]["deepseek"])
        config = self.load({"asr": {"provider": "openai", "token": "sk-asr"}, "asr_profiles": {"local": {}}})
        self.assertEqual(config["asr_profiles"]["local"], {"provider": "local"})

    def test_polishing_alone_is_a_valid_file(self):
        config = self.load({"polish": {"provider": "deepseek", "token": "sk-polish"}})
        self.assertIsNone(voice.select_profile(config, {"asr_provider": "openai"}, "asr"))
        self.assertEqual(voice.select_profile(config, {}, "polish")["provider"], "deepseek")

    def test_local_is_not_a_polishing_provider_and_cloud_still_needs_a_token(self):
        for document in ({"polish": {"provider": "local"}}, {"asr": {"provider": "openai"}}, {},
                         {"asr_profiles": {"local": {}}}):
            with self.subTest(document=document), self.assertRaises(ValueError):
                self.load(document)


class CredentialTest(LocalFixture):
    def test_local_checks_the_model_and_the_helper(self):
        server = type("Server", (), {"local_helpers": self.pool, "config_path": self.root / "absent.json"})()
        options = {"asr_provider": "local", "asr_model_path": str(self.model)}
        self.assertTrue(voice.credential_test({"service": "voice.asr", "config": options}, server)["ok"])
        self.assertFalse(voice.credential_test({"service": "voice.asr", "config": dict(options, asr_model_path="/absent")},
                                               server)["ok"])
        os.environ["FAKE_HELPER_AVAILABLE"] = "0"
        self.pool.close()
        result = voice.credential_test({"service": "voice.asr", "config": options}, server)
        self.assertFalse(result["ok"])
        self.assertIn("重新安装", result["message"])


if __name__ == "__main__":
    unittest.main()
