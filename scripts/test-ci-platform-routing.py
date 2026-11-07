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
NATIVE_OUTPUTS = (
    "android", "linux", "harmonyos", "windows", "web", "engine",
    "engine_capi", "ui", "plugins", "android_device",
)
PLATFORM_ROUTES = {
    "android": ({"android"}, False, False),
    "common": ({"linux", "windows"}, True, False),
    "harmony": ({"harmonyos"}, False, False),
    "ios": (set(), False, True),
    "linux": ({"linux"}, False, False),
    "macos": (set(), True, False),
    "windows": ({"windows"}, False, False),
}


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
    def assert_native_route(self, path: str, active: set[str]):
        expected = {key: "true" if key in active else "false" for key in NATIVE_OUTPUTS}
        self.assertEqual(expected, route("ci-platforms.yml", path))

    def test_every_platform_directory_has_a_reviewed_route(self):
        actual = {entry.name for entry in (ROOT / "platforms").iterdir() if entry.is_dir()}
        self.assertEqual(set(PLATFORM_ROUTES), actual, "新增平台目录后，请扩展路由测试")

    def test_shared_engine_reaches_all_hosts(self):
        self.assert_native_route(
            "crates/engine/src/lib.rs",
            {"android", "linux", "harmonyos", "windows", "web", "engine", "plugins", "engine_capi"},
        )
        self.assertEqual("true", route("ci-macos.yml", "crates/engine/src/lib.rs")["build"])
        self.assertEqual("true", route("ci-ios.yml", "crates/engine/src/lib.rs")["build"])

    def test_platform_only_changes_do_not_build_other_hosts(self):
        for directory, (native, macos, ios) in PLATFORM_ROUTES.items():
            path = f"platforms/{directory}/src/example.txt"
            with self.subTest(path=path):
                self.assert_native_route(path, native)
                self.assertEqual(macos, route("ci-macos.yml", path)["build"] == "true")
                self.assertEqual(ios, route("ci-ios.yml", path)["build"] == "true")

    def test_licenses_locks_and_build_scripts(self):
        cases = (
            ("LICENSE", {"web"}, True, False),
            ("THIRD_PARTY_NOTICES.txt", set(), True, False),
            ("resources/opencc.lock.json", {"android", "linux", "harmonyos", "windows", "web", "engine", "plugins"}, True, True),
            ("resources/onnxruntime-minimal.lock.json", {"android", "linux"}, True, True),
            ("scripts/fetch_opencc.py", {"android", "linux", "harmonyos", "windows", "web", "engine", "plugins"}, True, True),
            ("scripts/fetch_onnxruntime.py", {"android", "linux", "harmonyos", "windows", "web", "engine", "plugins"}, True, True),
        )
        for path, active, macos, ios in cases:
            with self.subTest(path=path):
                self.assert_native_route(path, active)
                self.assertEqual(macos, route("ci-macos.yml", path)["build"] == "true")
                self.assertEqual(ios, route("ci-ios.yml", path)["build"] == "true")


if __name__ == "__main__":
    unittest.main()
