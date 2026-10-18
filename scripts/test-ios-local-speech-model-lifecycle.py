#!/usr/bin/env python3
"""Local voice model work must be cancelled when its settings section disappears."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/settings/LocalSpeechModelsView.swift"


class IOSLocalSpeechModelLifecycleContract(unittest.TestCase):
    def test_manager_tracks_and_cancels_background_work(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, r"private var refreshTask: Task<Void, Never>\?")
        self.assertRegex(source, r"private var operationTasks: \[String: Task<Void, Never>\]")
        cancel = re.search(r"func cancelPending\(\).*?\n  \}", source, re.S)
        self.assertIsNotNone(cancel, "manager must expose cancellation for page disappearance")
        body = cancel.group(0)
        self.assertIn("refreshTask?.cancel()", body)
        self.assertIn("operationTasks.values.forEach { $0.cancel() }", body)
        self.assertIn("downloads.values.forEach { $0.cancel() }", body)

    def test_section_cancels_manager_work_on_disappearance(self):
        source = SOURCE.read_text(encoding="utf-8")
        section = source[source.index("struct LocalSpeechModelsSection"):]
        self.assertIn(".onDisappear { manager.cancelPending() }", section)

    def test_install_and_remove_ignore_cancelled_generations(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("private var operationGeneration = OperationGeneration()", source)
        self.assertRegex(source, r"let generation = operationGeneration.current\(\)")
        self.assertGreaterEqual(len(re.findall(r"operationGeneration.isCurrent\(generation\)", source)), 2)
        self.assertIn("operationGeneration.invalidate()", source)
        self.assertIn("try Task.checkCancellation()", source)


if __name__ == "__main__":
    unittest.main()
