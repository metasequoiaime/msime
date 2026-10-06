#!/usr/bin/env python3
"""验证发布说明只选择目标平台和纯共享改动。"""

from __future__ import annotations

import importlib.util
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("release_notes", ROOT / "scripts/generate-release-notes.py")
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def check(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def main() -> int:
    check(MODULE.platform_relevant(["platforms/android/java/app.kt"], "android"), "Android 路径没有被选中")
    check(MODULE.platform_relevant(["crates/engine/src/lib.rs"], "ios"), "纯共享改动没有被所有平台选中")
    check(MODULE.platform_relevant(["platforms/common/src/host.rs"], "windows"), "平台公共改动没有被所有平台选中")
    check(MODULE.platform_relevant(["apps/desktop/src-tauri/src/shared/voice.rs"], "android"), "Tauri 公共改动没有被所有平台选中")
    check(MODULE.platform_relevant(["platforms/android/java/app.kt", "packages/ui/src/index.tsx"], "android"), "平台与共享改动没有被选中")
    check(not MODULE.platform_relevant(["platforms/ios/App/App.swift"], "android"), "iOS 改动混入 Android 说明")
    check(not MODULE.platform_relevant(["platforms/ios/App/App.swift", "packages/ui/src/index.tsx"], "android"), "带 iOS 路径的共享改动混入 Android 说明")
    notes = MODULE.fallback_notes("Android", "1.2.3", [{"subject": "修复键盘崩溃", "commit": "1234567890"}])
    check(notes.startswith("## Android 1.2.3\n"), "确定性说明没有平台标题")
    check("修复键盘崩溃" in notes and "12345678" in notes, "确定性说明没有提交内容")
    print("release notes platform filtering: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
