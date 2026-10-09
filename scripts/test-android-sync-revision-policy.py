#!/usr/bin/env python3
"""Android 同步版本号统一复用非负整数解析策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/account/SyncApi.java"
SMOKE = ROOT / "platforms/android/tests/core/SyncApiSmoke.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    smoke = SMOKE.read_text(encoding="utf-8")
    for method in ("phraseRevision", "snapshotRevisionValue"):
        if f"static long {method}(Object value)" in source:
            print(f"{SOURCE}: 不应保留 {method} 转发方法", file=sys.stderr)
            return 1
        if f"SyncApi.{method}" in smoke:
            print(f"{SMOKE}: 不应测试已移除的 {method} 转发方法", file=sys.stderr)
            return 1
    if "phraseRevision(root.opt(\"revision\"))" in source:
        print(f"{SOURCE}: 常用语版本号应直接使用 preferenceRevision", file=sys.stderr)
        return 1
    if "snapshotRevisionValue(" in source:
        print(f"{SOURCE}: 快照版本号应直接使用 preferenceRevision", file=sys.stderr)
        return 1
    required = (
        "long nextRevision = preferenceRevision(next);",
        "return preferenceRevision(revision);",
    )
    if source.count('preferenceRevision(root.opt("revision"))') < 2 or any(
            expression not in source for expression in required):
        print(f"{SOURCE}: 同步版本号没有统一复用 preferenceRevision", file=sys.stderr)
        return 1
    if smoke.count("SyncApi.preferenceRevision") < 2:
        print(f"{SMOKE}: 缺少 preferenceRevision 合同检查", file=sys.stderr)
        return 1
    print("Android 同步版本号已统一复用非负整数解析策略")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
