#!/usr/bin/env python3
"""宿主存储不重复定义共享 JSON 状态策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/HostStore.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    if "strictOk(Object value)" in source:
        print(f"{SOURCE}: 不应保留 strictOk 转发方法", file=sys.stderr)
        return 1
    if "JsonPolicy.objectValue(root)" not in source:
        print(f"{SOURCE}: 宿主响应应直接调用 JsonPolicy.objectValue", file=sys.stderr)
        return 1
    print("Android 宿主存储未重复定义共享 JSON 状态策略")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
