#!/usr/bin/env python3
"""核对文档入口与三个必需检查 workflow 的路径过滤。"""

from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS = ROOT / ".github/workflows"
IGNORED = ("ci.yml", "ci-macos.yml", "ci-ios.yml")
EVENTS = ("push", "pull_request")


def path_list(workflow: str, event: str, key: str) -> tuple[str, ...]:
    lines = (WORKFLOWS / workflow).read_text(encoding="utf-8").splitlines()
    event_line = f"  {event}:"
    try:
        start = lines.index(event_line, lines.index("on:")) + 1
    except ValueError as error:
        raise AssertionError(f"{workflow}: 缺少 {event} 事件") from error
    section = []
    for line in lines[start:]:
        if line and not line.startswith("    "):
            break
        section.append(line)
    marker = f"    {key}:"
    try:
        first = section.index(marker) + 1
    except ValueError as error:
        raise AssertionError(f"{workflow}: {event} 缺少 {key}") from error
    paths = []
    for line in section[first:]:
        match = re.fullmatch(r"      - ([^#]+?)(?:\s+#.*)?", line)
        if not match:
            break
        paths.append(match.group(1).strip(" '\""))
    if not paths or len(paths) != len(set(paths)):
        raise AssertionError(f"{workflow}: {event}.{key} 为空或有重复项")
    return tuple(paths)


class CIPathFilters(unittest.TestCase):
    def test_document_paths_match_every_ignored_list(self):
        for event in EVENTS:
            expected = path_list("ci-docs.yml", event, "paths")
            for workflow in IGNORED:
                with self.subTest(event=event, workflow=workflow):
                    self.assertEqual(expected, path_list(workflow, event, "paths-ignore"))

    def test_build_inputs_are_not_document_paths(self):
        paths = path_list("ci-docs.yml", "pull_request", "paths")
        for build_input in (
            "LICENSE", "THIRD_PARTY_NOTICES.txt", "Cargo.lock",
            "resources/opencc.lock.json", "resources/onnxruntime-minimal.lock.json",
            "scripts/fetch_opencc.py", "platforms/android/java/Keyboard.java",
        ):
            with self.subTest(path=build_input):
                self.assertFalse(any(Path(build_input).match(pattern) for pattern in paths))


if __name__ == "__main__":
    unittest.main()
