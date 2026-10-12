#!/usr/bin/env python3
"""Ensure FeedbackPage cancels view-bound submission work during view teardown."""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/FeedbackPage.java"


class AndroidFeedbackViewLifecycleContract(unittest.TestCase):
    def test_destroy_view_clears_sending_and_cancels_submission(self):
        source = SOURCE.read_text(encoding="utf-8")
        start = source.index("@Override public void onDestroyView()")
        end = source.index("\n    private void chooseType", start)
        body = source[start:end]
        self.assertIn("sending = false;", body)
        self.assertIn("submitTask.cancel(true);", body)

    def test_submission_keeps_a_cancellable_future(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("Future<?> submitTask;", source)
        self.assertIn("submitTask = NetworkTask.run(this", source)


if __name__ == "__main__":
    unittest.main()
