#!/usr/bin/env python3
"""离开 iOS 开发者选项页后，诊断任务不能回写已经失效的页面状态。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/settings/DeveloperOptionsView.swift"


class IOSDeveloperOptionsLifecycleContract(unittest.TestCase):
    def test_view_owns_and_invalidates_diagnostic_tasks(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, r"@State private var pending: Task<Void, Never>\?")
        self.assertRegex(source, r"@State private var lifecycle = OperationGeneration\(\)")
        disappear_bodies = re.findall(r"\.onDisappear \{(?P<body>.*?)\n    \}", source, re.S)
        body = next((body for body in disappear_bodies if "ViewRequestTask.cancel(generation: $lifecycle, task: $pending)" in body), None)
        self.assertIsNotNone(body, "developer options must invalidate work when it disappears")
        self.assertIn("ViewRequestTask.cancel(generation: $lifecycle, task: $pending)", body)
        task = (ROOT / "platforms/ios/App/Sources/app/ViewRequestTask.swift").read_text()
        generation = (ROOT / "platforms/ios/App/Sources/app/ViewRequestGeneration.swift").read_text()
        self.assertIn("generation.wrappedValue.cancel(task: &task.wrappedValue)", task)
        self.assertIn("task?.cancel()", generation)
        self.assertIn("task = nil", generation)

    def test_export_and_reset_results_are_bound_to_current_lifecycle(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, re.compile(r"let id = lifecycle.current\(\).*?pending = Task", re.S))
        export = re.search(r"private func export\(\).*?(?=\n  private func reset\(\))", source, re.S)
        reset = re.search(r"private func reset\(\).*?(?=\n}\n\n/// 已写好|\Z)", source, re.S)
        self.assertIsNotNone(export, "could not locate DeveloperOptionsView.export")
        self.assertIsNotNone(reset, "could not locate DeveloperOptionsView.reset")
        self.assertIn("lifecycle.isCurrent(id)", export.group(0))
        self.assertIn("lifecycle.isCurrent(id)", reset.group(0))
        self.assertIn("pending = Task", export.group(0))
        self.assertIn("pending = Task", reset.group(0))
        self.assertLess(
            reset.group(0).index("DeveloperDiagnostics.resetLocalPreferences()"),
            reset.group(0).index("guard lifecycle.isCurrent(id)"),
            "reset must finish its persistent side effects even after the page disappears",
        )


if __name__ == "__main__":
    unittest.main()
