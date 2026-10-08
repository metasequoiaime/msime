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


def check_model_request_user_agent() -> None:
    # api.everyapi.ai sits behind Cloudflare, which answers urllib's default `Python-urllib/3.x` with 403 (error 1010) before the request reaches the API; every release from android-v0.2.0 to android-v0.3.0 fell back to the raw commit list because of it.
    sent: list[object] = []

    def capture(request: object, timeout: float) -> object:
        sent.append(request)
        raise OSError("captured")

    original = MODULE.urllib.request.urlopen
    MODULE.urllib.request.urlopen = capture
    MODULE.os.environ["EVERYAPI_RELEASE_NOTES_TOKEN"] = "test-token"
    try:
        MODULE.model_notes("Android", "1.2.3", [{"subject": "修复键盘崩溃", "commit": "1234567890", "paths": ["platforms/android/java/app.kt"], "body": ""}])
    finally:
        MODULE.urllib.request.urlopen = original
        del MODULE.os.environ["EVERYAPI_RELEASE_NOTES_TOKEN"]
    check(len(sent) == 1, "模型请求没有发出")
    agent = sent[0].get_header("User-agent") or ""
    check(bool(agent) and not agent.startswith("Python-urllib"), f"模型请求用了会被 Cloudflare 拒绝的 User-Agent：{agent!r}")


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
    check_model_request_user_agent()
    print("release notes platform filtering: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
