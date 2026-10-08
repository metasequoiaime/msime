#!/usr/bin/env python3
"""确保 GroupCard.Row 使用自身的启用策略，而不是把它当成 View。"""

from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
ANDROID_HOME = ROOT / "platforms/android/java/app/msime/android/home"


class AndroidRowEnabledPolicyTest(unittest.TestCase):
    def test_group_card_rows_are_not_passed_to_view_policy(self):
        declaration = re.compile(r"\bGroupCard\.Row\s+(\w+)\b")
        view_policy_call = re.compile(r"\bViewPolicy\.setEnabled\(\s*(\w+)\s*,")
        violations = []

        for source_path in sorted(ANDROID_HOME.glob("*.java")):
            source = source_path.read_text(encoding="utf-8")
            row_names = set(declaration.findall(source))
            for match in view_policy_call.finditer(source):
                if match.group(1) in row_names:
                    line = source.count("\n", 0, match.start()) + 1
                    violations.append(f"{source_path.relative_to(ROOT)}:{line}: {match.group(0)}")

        self.assertEqual([], violations, "GroupCard.Row 必须调用自身的 setEnabled：\n" + "\n".join(violations))


if __name__ == "__main__":
    unittest.main()
