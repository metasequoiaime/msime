#!/usr/bin/env python3
"""公告字段直接复用共享 JSON 策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
POLICY = ROOT / "platforms/android/java/app/msime/android/core/NoticeFieldPolicy.java"
BANNER = ROOT / "platforms/android/java/app/msime/android/home/NoticeBanner.java"
SMOKE = ROOT / "platforms/android/tests/home/NoticeBannerSmoke.java"


def main() -> int:
    banner = BANNER.read_text(encoding="utf-8")
    smoke = SMOKE.read_text(encoding="utf-8")
    if POLICY.exists():
        print(f"{POLICY}: 不应保留公告 JSON 转发类", file=sys.stderr)
        return 1
    if "NoticeFieldPolicy" in banner:
        print(f"{BANNER}: 不应继续调用公告 JSON 转发类", file=sys.stderr)
        return 1
    if "Class.forName" in smoke or "NoticeFieldPolicy" in smoke:
        print(f"{SMOKE}: 不应通过反射检查已移除的转发类", file=sys.stderr)
        return 1
    if "JsonPolicy.strictString" not in banner or "JsonPolicy.strictString" not in smoke:
        print(f"{SMOKE}: 应直接检查 JsonPolicy.strictString", file=sys.stderr)
        return 1
    print("Android notices use the shared JSON string policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
