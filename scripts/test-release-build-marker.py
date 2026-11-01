#!/usr/bin/env python3
"""发布正文的隐藏构建标记与构建身份：release-build-marker.py 写出的那一行能被应用内检查更新读懂（crates/client-services/src/update_check.rs 的 BUILD_MARKER），三个桌面平台的发布 workflow 都在发布说明末尾追加它，并在编译 Rust 时导出 MSIME_BUILD_RUN。"""

import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/release-build-marker.py"
SPEC = importlib.util.spec_from_file_location("release_build_marker", SCRIPT)
MARKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MARKER)
SHA = "0123456789abcdef0123456789abcdef01234567"
WORKFLOWS = ("release-windows.yml", "release-linux.yml", "release-macos.yml")


def payload(line):
    match = re.fullmatch(r"<!-- msime-build (\{.*\}) -->", line)
    assert match, line
    return json.loads(match.group(1))


class MarkerTests(unittest.TestCase):
    def test_channel_follows_the_release_suffix(self):
        for release, channel in (("1.2.3", "stable"), ("1.2.3-beta", "beta"), ("1.2.3-alpha", "alpha")):
            self.assertEqual(payload(MARKER.marker(release, SHA, "38812345678")),
                             {"run": 38812345678, "source": SHA, "channel": channel})

    def test_the_rust_reader_finds_the_run(self):
        # update_check.rs 找 "<!-- msime-build " 之后到 " -->" 之前的 JSON，读其中的 run。
        line = MARKER.marker("1.2.3", SHA, "7")
        start = line.index("<!-- msime-build ") + len("<!-- msime-build ")
        self.assertEqual(json.loads(line[start:line.index(" -->", start)])["run"], 7)

    def test_bad_arguments_are_refused(self):
        for release, source, run in (("1.2", SHA, "1"), ("1.2.3-rc", SHA, "1"), ("1.2.3", SHA[:-1], "1"),
                                     ("1.2.3", SHA.upper(), "1"), ("1.2.3", SHA, "0"), ("1.2.3", SHA, "-1"),
                                     ("1.2.3", SHA, ""), ("1.2.3", SHA, "1" * 20)):
            with self.assertRaises(ValueError, msg=(release, source, run)):
                MARKER.marker(release, source, run)

    def test_command_line_appends_a_separated_line_or_nothing(self):
        done = subprocess.run([sys.executable, str(SCRIPT), "1.2.3-beta", SHA, "99"],
                              capture_output=True, text=True, check=False)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(done.stdout, "\n" + MARKER.marker("1.2.3-beta", SHA, "99") + "\n")
        failed = subprocess.run([sys.executable, str(SCRIPT), "1.2.3", SHA, "abc"],
                                capture_output=True, text=True, check=False)
        self.assertNotEqual(failed.returncode, 0)
        self.assertEqual(failed.stdout, "")


class WorkflowTests(unittest.TestCase):
    def test_each_desktop_release_exports_the_run_and_appends_the_marker(self):
        for name in WORKFLOWS:
            text = (ROOT / ".github/workflows" / name).read_text(encoding="utf-8")
            self.assertRegex(text, r"(?m)^env:\n  MSIME_BUILD_RUN: \$\{\{ github\.run_id \}\}$", name)
            self.assertIn('python3 scripts/release-build-marker.py "$RELEASE" "$(git rev-parse HEAD)" "$GITHUB_RUN_ID" >> release-notes.md', text, name)
            # 标记追加在发布说明写完之后、交给 publish-release.sh 之前。
            self.assertLess(text.index("release-build-marker.py"), text.index("bash scripts/publish-release.sh"), name)

    def test_the_linux_package_container_receives_the_run(self):
        text = (ROOT / "platforms/linux/package-container.sh").read_text(encoding="utf-8")
        self.assertIn('-e MSIME_BUILD_RUN="${MSIME_BUILD_RUN:-${GITHUB_RUN_ID:-}}"', text)

    def test_the_rust_side_reads_the_same_names(self):
        text = (ROOT / "crates/client-services/src/update_check.rs").read_text(encoding="utf-8")
        self.assertIn('option_env!("MSIME_BUILD_RUN")', text)
        self.assertIn('const BUILD_MARKER: &str = "<!-- msime-build ";', text)


if __name__ == "__main__":
    unittest.main()
