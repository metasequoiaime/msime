#!/usr/bin/env python3
"""关于页许可文本读取复用共享有界 body 策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/AboutPage.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    region = source.split("    private void showNotice(", 1)[1].split(
        "    private", 1
    )[0]
    if "HttpBodyPolicy.readBounded" not in region:
        print(f"{SOURCE}: 没有使用 HttpBodyPolicy.readBounded", file=sys.stderr)
        return 1
    if "ByteArrayOutputStream" in region or "in.read(buffer)" in region:
        print(f"{SOURCE}: 仍保留自定义许可读取循环", file=sys.stderr)
        return 1
    if "NOFOLLOW_LINKS" not in region:
        print(f"{SOURCE}: 下载资源包许可读取没有拒绝符号链接", file=sys.stderr)
        return 1
    print("Android about notices use the shared bounded body policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
