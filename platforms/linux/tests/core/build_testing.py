#!/usr/bin/env python3
"""核对 CMake 实际生成的目标图，防止关闭测试后仍重复编译测试源。"""

import json
from pathlib import Path
import sys


def targets(build_dir):
    reply = build_dir / ".cmake/api/v1/reply"
    index = json.loads(max(reply.glob("index-*.json")).read_text())
    model_file = reply / index["reply"]["codemodel-v2"]["jsonFile"]
    model = json.loads(model_file.read_text())
    source_root = Path(model["paths"]["source"])
    repo_root = source_root.parent.parent
    production = set()
    tests = set()
    for configuration in model["configurations"]:
        for entry in configuration["targets"]:
            target = json.loads((reply / entry["jsonFile"]).read_text())
            if target["type"] == "UTILITY":
                continue
            sources = [
                (source_root / source["path"]).resolve()
                for source in target.get("sources", [])
            ]
            is_test = any(
                path.is_relative_to(repo_root) and "tests" in path.relative_to(repo_root).parts
                for path in sources
            )
            destination = tests if is_test else production
            destination.add(target["name"])
    return production, tests


enabled_production, enabled_tests = targets(Path(sys.argv[1]))
disabled_production, disabled_tests = targets(Path(sys.argv[2]))
assert enabled_tests, "BUILD_TESTING=ON 没有生成测试目标"
assert {
    "ibus-engine-smoke", "msime-linux-online-provider-contract", "fcitx5-native-test"
} <= enabled_tests, enabled_tests
assert not disabled_tests, f"BUILD_TESTING=OFF 仍生成测试目标：{sorted(disabled_tests)}"
assert enabled_production == disabled_production, (
    f"测试开关改变了生产目标：{sorted(enabled_production ^ disabled_production)}"
)
assert {
    "msime-linux-ibus", "msime-fcitx5", "msime-linux-prepare",
    "msime-linux-online", "msime-voice-local"
} <= disabled_production, disabled_production
print(
    f"BUILD_TESTING：保留 {len(disabled_production)} 个生产目标，"
    f"排除 {len(enabled_tests)} 个测试目标"
)
