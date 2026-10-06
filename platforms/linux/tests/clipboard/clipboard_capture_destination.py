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
loader = importlib.machinery.SourceFileLoader("clipboard_monitor", str(ROOT / "scripts" / "msime-linux-clipboard-monitor"))
spec = importlib.util.spec_from_loader(loader.name, loader)
monitor = importlib.util.module_from_spec(spec)
loader.exec_module(monitor)


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
