#!/usr/bin/env python3
"""iOS service tests 编译使用设计修饰器的设置页面时必须带上 DesignKit。"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
PROJECT = ROOT / "platforms/ios/project.yml"


class IOSServiceTestDesignKitContract(unittest.TestCase):
    def test_service_tests_include_design_kit_source(self):
        project = PROJECT.read_text(encoding="utf-8")
        target = re.search(r"^  MSIMEServiceTests:\n(?P<body>.*?)(?=^  MSIME|\Z)", project, re.M | re.S)
        self.assertIsNotNone(target, "could not locate MSIMEServiceTests target")
        self.assertIn("- path: App/Sources/app/DesignKit.swift", target.group("body"))


if __name__ == "__main__":
    unittest.main()
