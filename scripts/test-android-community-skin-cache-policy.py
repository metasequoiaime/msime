#!/usr/bin/env python3
"""社区皮肤缓存读取复用共享有界 body 策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/community/CommunitySkinCache.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    region = source.split("    public static List<Entry> read(", 1)[1].split(
        "    private static String text", 1
    )[0]
    if "HttpBodyPolicy.readBounded" not in region:
        print(f"{SOURCE}: 没有使用 HttpBodyPolicy.readBounded", file=sys.stderr)
        return 1
    if "Files.readAllBytes" in region:
        print(f"{SOURCE}: 仍使用无界 Files.readAllBytes", file=sys.stderr)
        return 1
    print("Android community skin cache reads use the shared bounded body policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
