#!/usr/bin/env python3
"""Polling capture must honor the latest synthetic runtime configuration."""
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
loader = importlib.machinery.SourceFileLoader("clipboard_monitor", str(ROOT / "scripts" / "msime-linux-clipboard-monitor"))
spec = importlib.util.spec_from_loader(loader.name, loader)
monitor = importlib.util.module_from_spec(spec)
loader.exec_module(monitor)


class CloudReceive(unittest.TestCase):
    @staticmethod
    def item(second):
        return {"text": f"合成远端 {second}", "updated_at": f"2026-11-13T00:00:{second:02d}Z"}

    def test_existing_text_establishes_watermark_then_accepts_new_copies(self):
        policy = monitor.CloudReceivePolicy()
        self.assertFalse(policy.consider(self.item(1), "合成本机"))
        self.assertTrue(policy.consider(self.item(2), "合成本机"))
        self.assertTrue(policy.consider(self.item(2), "合成本机"))
        policy.received(self.item(2))
        self.assertFalse(policy.consider(self.item(1), self.item(2)["text"]))
        self.assertFalse(policy.consider(self.item(3), "合成并发复制"))
        self.assertTrue(policy.consider(self.item(4), "合成并发复制"))

    def test_empty_page_and_nontext_read_are_distinct(self):
        policy = monitor.CloudReceivePolicy()
        self.assertFalse(policy.consider(None, "合成本机"))
        self.assertTrue(policy.consider(self.item(1), "合成本机"))
        self.assertFalse(policy.consider(self.item(2), None))
        self.assertFalse(policy.consider(self.item(3), None))
        self.assertFalse(policy.consider({"text": "合成", "updated_at": "synthetic-time"}, "合成本机"))
        self.assertFalse(policy.consider(self.item(4), "合成本机"))
        self.assertTrue(policy.consider(self.item(5), "合成本机"))


class CloudAutomaticSync(unittest.TestCase):
    def setUp(self):
        self.binding = {"source": "account", "user_id": "synthetic-a", "revision": 1}
        self.text = "合成启动前文本"
        self.page = {"enabled": True, "items": []}
        self.calls = []
        self.during_list = lambda: None
        self.sync = monitor.CloudAutoSync(self.request)
        self.writes = []

    def request(self, action):
        self.calls.append(action)
        if action["operation"] == "binding":
            return dict(self.binding)
        if action["operation"] == "list":
            self.during_list()
            return self.page
        return {}

    def poll(self):
        def write(text):
            self.writes.append(text)
            self.text = text
            return True
        self.sync.poll(lambda: self.text, write)

    def adds(self):
        return [c for c in self.calls if c["operation"] == "add"]

    def test_startup_and_binding_switch_never_upload_existing_text(self):
        self.poll()
        self.poll()
        self.assertEqual(self.adds(), [])
        self.text = "合成新复制"
        self.poll()
        self.assertEqual(self.adds(), [{"operation": "add", "text": self.text,
                                       "expected_binding": self.binding}])
        self.binding = {**self.binding, "user_id": "synthetic-b"}
        self.poll()
        self.assertEqual(len(self.adds()), 1)
        self.text = "合成第二账号复制"
        self.poll()
        self.assertEqual(self.adds()[-1]["expected_binding"]["user_id"], "synthetic-b")

    def test_remote_copy_is_written_once_without_upload_echo(self):
        self.poll()
        item = CloudReceive.item(1)
        self.page["items"] = [item]
        self.poll()
        self.poll()
        self.assertEqual(self.writes, [item["text"]])
        self.assertEqual(self.adds(), [])

    def test_write_without_matching_readback_does_not_consume_remote_copy(self):
        self.poll()
        self.page["items"] = [CloudReceive.item(1)]
        attempts = []
        def write(text):
            attempts.append(text)
            return True
        self.sync.poll(lambda: self.text, write)
        self.sync.poll(lambda: self.text, write)
        self.assertEqual(attempts, [self.page["items"][0]["text"]] * 2)
        self.assertEqual(self.adds(), [])

    def test_local_copy_during_request_wins_and_uploads_current_text(self):
        self.poll()
        self.page["items"] = [CloudReceive.item(1)]
        self.during_list = lambda: setattr(self, "text", "合成并发复制")
        self.poll()
        self.assertEqual(self.writes, [])
        self.assertEqual(self.adds()[0]["text"], "合成并发复制")

    def test_source_switch_during_request_discards_old_reply_and_copy(self):
        self.poll()
        self.text = "合成待发复制"
        self.page["items"] = [CloudReceive.item(1)]
        self.during_list = lambda: self.binding.update(user_id="synthetic-b")
        self.poll()
        self.assertEqual(self.writes, [])
        self.assertEqual(self.adds(), [])
        self.during_list = lambda: None
        self.poll()
        self.assertEqual(self.adds(), [])

    def test_disabled_and_unreadable_clipboards_are_never_sent(self):
        self.page["enabled"] = False
        self.poll()
        self.text = "合成关闭期间复制"
        self.poll()
        self.page["enabled"] = True
        self.poll()
        self.text = None
        self.poll()
        self.assertEqual(self.adds(), [])
        self.assertEqual(self.writes, [])


class EmptyClipboardStatus(unittest.TestCase):
    def test_only_explicit_empty_status_is_an_empty_clipboard(self):
        command = [sys.executable, "-c", "raise SystemExit(3)"]
        self.assertEqual(monitor.read_command(command, empty_status=3), "")
        self.assertIsNone(monitor.read_command(command))
        self.assertIsNone(monitor.read_command([sys.executable, "-c", "raise SystemExit(1)"], empty_status=3))

    def test_empty_clipboard_accepts_a_new_remote_copy(self):
        policy = monitor.CloudReceivePolicy()
        self.assertFalse(policy.consider(None, ""))
        self.assertTrue(policy.consider(CloudReceive.item(1), ""))
        self.assertFalse(policy.consider(CloudReceive.item(2), None))


class CaptureDestination(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="msime-clipboard-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.options = self.root / "runtime.json"
        self.first = self.root / "first"
        self.second = self.root / "second"
        for path in (self.first, self.second):
            path.mkdir()
            self.preferences(path, True)
        self.configure(self.first)

    def preferences(self, directory, enabled):
        (directory / "preferences.json").write_text(json.dumps({
            "format_version": 1, "preferences": {"clipboard_history": enabled}}))

    def configure(self, directory):
        self.options.write_text(json.dumps({"preferences_directory": str(directory)}))

    def run_poll(self, during_read, iterations=1):
        reads = 0
        def clipboard():
            nonlocal reads
            reads += 1
            during_read(reads)
            return "synthetic clipboard fixture"
        sleeps = 0
        def sleep(_):
            nonlocal sleeps
            sleeps += 1
            if sleeps >= iterations:
                raise KeyboardInterrupt()
        with mock.patch.object(sys, "argv", ["monitor", str(self.options)]), \
             mock.patch.object(monitor.signal, "signal"), \
             mock.patch.object(monitor.shutil, "which", return_value=None), \
             mock.patch.object(monitor, "clipboard_text", side_effect=clipboard), \
             mock.patch.object(monitor.time, "sleep", side_effect=sleep), \
             mock.patch.object(monitor.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as capture:
            with self.assertRaises(KeyboardInterrupt):
                monitor.main()
        return capture.call_args_list

    def test_destination_switch_discards_old_read_and_recovers(self):
        calls = self.run_poll(lambda count: self.configure(self.second) if count == 1 else None, 2)
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0].args[0][-1], str(self.second))

    def test_disable_during_read_prevents_capture(self):
        calls = self.run_poll(lambda _: self.preferences(self.first, False))
        self.assertEqual(calls, [])

    def test_corrupt_configuration_during_read_prevents_capture(self):
        calls = self.run_poll(lambda _: self.options.write_text("{"))
        self.assertEqual(calls, [])

    def test_document_rejects_a_symlinked_options_file(self):
        outside = self.root / "outside-runtime.json"
        outside.write_text(json.dumps({"preferences_directory": str(self.first)}))
        self.options.unlink()
        self.options.symlink_to(outside)
        with self.assertRaises(OSError):
            monitor.document(self.options)

    def test_unchanged_destination_captures_once(self):
        calls = self.run_poll(lambda _: None, 2)
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0].args[0][-1], str(self.first))
        self.assertEqual(calls[0].kwargs["input"], b"synthetic clipboard fixture")


if __name__ == "__main__":
    unittest.main()
