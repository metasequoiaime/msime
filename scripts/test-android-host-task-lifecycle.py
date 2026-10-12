#!/usr/bin/env python3
"""确保宿主与网络异步结果只投递给发起任务的 Fragment 视图。"""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"


class HostTaskLifecycleContract(unittest.TestCase):
    def test_shared_delivery_checks_the_original_view_and_lifecycle(self):
        source = (HOME / "ViewResultDelivery.java").read_text()
        self.assertRegex(source, r"View\s+view\s*=\s*fragment\.getView\(\)")
        self.assertIn("fragment.getViewLifecycleOwner().getLifecycle()", source)
        self.assertIn("fragment.isAdded()", source)
        self.assertIn("fragment.getView() != view", source)
        self.assertIn("lifecycle.getCurrentState().isAtLeast(Lifecycle.State.CREATED)", source)

    def test_host_and_network_tasks_use_the_same_view_delivery(self):
        for name in ("HostTask.java", "NetworkTask.java"):
            with self.subTest(name=name):
                source = (HOME / name).read_text()
                self.assertIn("ViewResultDelivery.capture(fragment)", source)
                self.assertIn("owner.post(", source)


if __name__ == "__main__":
    unittest.main()
