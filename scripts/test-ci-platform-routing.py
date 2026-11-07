#!/usr/bin/env python3
"""用合成路径执行 workflow 中实际运行的 Bash 分类规则。"""

from pathlib import Path
import os
import re
import subprocess
import textwrap
import unittest


ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS = ROOT / ".github/workflows"
PLATFORMS = {"android", "common", "harmony", "ios", "linux", "macos", "windows"}


def decision_script(workflow: str) -> str:
    source = (WORKFLOWS / workflow).read_text(encoding="utf-8")
    match = re.search(r"(?m)^      - id: decide\s*$", source)
    if match is None:
        match = re.search(r"(?m)^        id: decide\s*$", source)
    if match is None:
        raise AssertionError(f"{workflow}: 缺少 decide 步骤")
    block = re.search(r"(?m)^        run: \|\s*$", source[match.end():])
    if block is None:
        raise AssertionError(f"{workflow}: 缺少 decide.run")
    remainder = source[match.end() + block.end():]
    lines = []
    for line in remainder.splitlines():
        if line.strip() and not line.startswith("          "):
            break
        lines.append(line)
    script = textwrap.dedent("\n".join(lines))
    anchor = "shared=false;" if workflow == "ci-platforms.yml" else "while IFS= read -r path; do"
    if anchor not in script:
        raise AssertionError(f"{workflow}: 分类入口已变化，请更新测试")
    return script[script.index(anchor):]


def route(workflow: str, *paths: str) -> dict[str, str]:
    env = dict(os.environ, GITHUB_OUTPUT="/dev/stdout", CHANGED="\n".join(paths))
    result = subprocess.run(
        ["bash", "-c", 'set -euo pipefail\nchanged="$CHANGED"\n' + decision_script(workflow)],
        env=env, cwd=ROOT, text=True, capture_output=True, check=True,
    )
    return dict(re.findall(r"(?m)^([a-z_]+)=(true|false)$", result.stdout))


class CIPlatformRouting(unittest.TestCase):
    def test_every_platform_directory_has_a_reviewed_route(self):
        actual = {entry.name for entry in (ROOT / "platforms").iterdir() if entry.is_dir()}
        self.assertEqual(PLATFORMS, actual, "新增平台目录后，请扩展路由测试")

    def test_shared_engine_reaches_all_hosts(self):
        native = route("ci-platforms.yml", "crates/engine/src/lib.rs")
        for key in ("android", "linux", "harmonyos", "windows", "web", "engine", "plugins", "engine_capi"):
            self.assertEqual("true", native[key], key)
        self.assertEqual("true", route("ci-macos.yml", "crates/engine/src/lib.rs")["build"])
        self.assertEqual("true", route("ci-ios.yml", "crates/engine/src/lib.rs")["build"])

    def test_platform_only_changes_do_not_build_other_hosts(self):
        cases = (
            ("android", "android"), ("harmony", "harmonyos"),
            ("linux", "linux"), ("windows", "windows"),
            ("ios", None), ("macos", None),
        )
        for directory, expected in cases:
            path = f"platforms/{directory}/src/example.txt"
            with self.subTest(path=path):
                native = route("ci-platforms.yml", path)
                active = {key for key in ("android", "linux", "harmonyos", "windows", "web", "engine", "plugins") if native[key] == "true"}
                self.assertEqual({expected} if expected else set(), active)
                self.assertEqual(directory == "macos", route("ci-macos.yml", path)["build"] == "true")
                self.assertEqual(directory == "ios", route("ci-ios.yml", path)["build"] == "true")

    def test_licenses_locks_and_build_scripts(self):
        cases = (
            ("LICENSE", {"web"}, True, False),
            ("THIRD_PARTY_NOTICES.txt", set(), True, False),
            ("resources/opencc.lock.json", {"android", "linux", "harmonyos", "windows", "web", "engine"}, True, True),
            ("resources/onnxruntime-minimal.lock.json", {"android", "linux"}, True, True),
            ("scripts/fetch_opencc.py", {"android", "linux", "harmonyos", "windows", "web", "engine"}, True, True),
            ("scripts/fetch_onnxruntime.py", {"android", "linux", "harmonyos", "windows", "web", "engine"}, True, True),
        )
        for path, required, macos, ios in cases:
            with self.subTest(path=path):
                native = route("ci-platforms.yml", path)
                for key in required:
                    self.assertEqual("true", native[key], key)
                self.assertEqual(macos, route("ci-macos.yml", path)["build"] == "true")
                self.assertEqual(ios, route("ci-ios.yml", path)["build"] == "true")


if __name__ == "__main__":
    unittest.main()
