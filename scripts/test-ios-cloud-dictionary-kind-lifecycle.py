#!/usr/bin/env python3
"""切换云词库类型时，旧请求不能覆盖新类型页面。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/dictionary/CloudDictionaryView.swift"
GENERATION = ROOT / "platforms/ios/App/Sources/app/ViewRequestGeneration.swift"
TASK = ROOT / "platforms/ios/App/Sources/app/ViewRequestTask.swift"


class IOSCloudDictionaryKindLifecycleContract(unittest.TestCase):
    def test_kind_change_cancels_and_invalidates_previous_request(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("@State private var request = ViewRequestGeneration()", source)
        self.assertIn(".task { request.appear();", source)
        change = re.search(r"\.onChange\(of: kind\) \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(change, "kind changes must invalidate the previous dictionary request")
        body = change.group("body")
        self.assertIn("ViewRequestTask.cancel(task: $pending)", body)
        self.assertLess(body.index("ViewRequestTask.cancel(task: $pending)"), body.index("request.invalidate()"))
        task = TASK.read_text(encoding="utf-8")
        cancel = re.search(r"static func cancel\(task: Binding<Task<Void, Never>\?>\) \{(?P<body>.*?)\n  \}", task, re.S)
        self.assertIsNotNone(cancel)
        self.assertIn("task.wrappedValue?.cancel()", cancel.group("body"))
        self.assertIn("task.wrappedValue = nil", cancel.group("body"))

    def test_disappearance_invalidates_pending_request(self):
        source = SOURCE.read_text(encoding="utf-8")
        disappear = re.search(r"\.onDisappear \{(?P<body>.*?)\n    \}", source, re.S)
        self.assertIsNotNone(disappear, "leaving the dictionary page must invalidate its request")
        body = disappear.group("body")
        self.assertIn("ViewRequestTask.disappear(request: $request, pending: $pending)", body)
        task = TASK.read_text(encoding="utf-8")
        self.assertIn("request.wrappedValue.disappear()", task)
        self.assertIn("pending.wrappedValue?.cancel()", task)
        self.assertIn("pending.wrappedValue = nil", task)
        self.assertIn("busy = false", body)

    def test_load_result_is_bound_to_requested_kind_and_generation(self):
        source = SOURCE.read_text(encoding="utf-8")
        load = re.search(r"private func load\(offset: Int\).*?\n  \}", source, re.S)
        self.assertIsNotNone(load, "could not locate CloudDictionaryView.load")
        body = load.group(0)
        self.assertIn("guard let id = request.current() else { throw CancellationError() }", body)
        self.assertIn("let requestedKind = kind", body)
        self.assertIn("requestedKind", body)
        self.assertIn("try request.checkCurrent(id)", body)
        generation = GENERATION.read_text(encoding="utf-8")
        self.assertIn("func current() -> UUID?", generation)
        self.assertIn("mutating func invalidate()", generation)

    def test_old_task_cannot_clear_new_task_busy_state(self):
        source = SOURCE.read_text(encoding="utf-8")
        run = re.search(r"private func run\(.*?\n  \}", source, re.S)
        self.assertIsNotNone(run, "could not locate CloudDictionaryView.run")
        body = run.group(0)
        self.assertIn("ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending", body)
        self.assertEqual(body.count("try request.checkCurrent(id)"), 2)
        self.assertIn("let id = request.wrappedValue.begin()", TASK.read_text(encoding="utf-8"))
        self.assertIn("if request.wrappedValue.isCurrent(id)", TASK.read_text(encoding="utf-8"))

    def test_file_import_can_finish_while_parent_page_is_hidden(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertRegex(source, re.compile(
            r"CloudDictionaryFilesView\(.*?imported: \{ try await refreshAfterImport\(\) \}",
            re.S,
        ))
        refresh = re.search(r"private func refreshAfterImport\(\).*?\n  \}", source, re.S)
        self.assertIsNotNone(refresh)
        self.assertIn("guard request.isVisible else { return }", refresh.group(0))
        self.assertIn("try await load(offset: 0)", refresh.group(0))

    def test_editor_save_checks_generation_around_remote_write(self):
        source = SOURCE.read_text(encoding="utf-8")
        save = re.search(r"private func save\(.*?\n  \}", source, re.S)
        self.assertIsNotNone(save)
        body = save.group(0)
        self.assertIn("guard let id = request.current() else { throw CancellationError() }", body)
        self.assertEqual(body.count("try request.checkCurrent(id)"), 2)
        self.assertIn("try await load(offset: 0)", body)


if __name__ == "__main__":
    unittest.main()
