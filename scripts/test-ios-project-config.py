import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class IOSProjectConfigTests(unittest.TestCase):
    def test_keyboard_cancels_delayed_gloss_work_when_hidden(self):
        controller = (
            ROOT / "platforms/ios/KeyboardExtension/Sources/keyboard/KeyboardViewController.swift"
        ).read_text()
        disappear_start = controller.index("  override func viewWillDisappear")
        disappear = controller[disappear_start : controller.index("\n  private func", disappear_start)]
        self.assertIn("candidateGlossTimer?.invalidate()", disappear)
        self.assertIn("candidateGlossTimer = nil", disappear)

    def test_candidate_translation_cancel_stops_inflight_tasks(self):
        store = (
            ROOT / "platforms/ios/KeyboardExtension/Sources/candidate/CandidateTranslationStore.swift"
        ).read_text()
        self.assertIn("private var tasks: [UUID: Task<Void, Never>] = [:]", store)
        self.assertIn("for task in tasks.values { task.cancel() }", store)
        self.assertIn("tasks.removeAll()", store)
        self.assertIn("tasks[id] = task", store)
        self.assertIn("tasks[id] = nil", store)


if __name__ == "__main__":
    unittest.main()
