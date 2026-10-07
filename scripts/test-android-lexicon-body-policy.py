#!/usr/bin/env python3
"""词库导入读取器复用共享有界 body 策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/LexiconPage.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    region = source.split("    @Nullable private static byte[] readAll(", 1)[1].split(
        "    static String displayName", 1
    )[0]
    if "HttpBodyPolicy.readBounded" not in region:
        print(f"{SOURCE}: 没有使用 HttpBodyPolicy.readBounded", file=sys.stderr)
        return 1
    if "ByteArrayOutputStream" in region or "while ((read = input.read(buffer)) != -1)" in region:
        print(f"{SOURCE}: 仍保留自定义文件读取循环", file=sys.stderr)
        return 1
    print("Android lexicon imports use the shared bounded body policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
