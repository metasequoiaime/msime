#!/usr/bin/env python3
"""验证 Android JNI 响应直接复用共享 UTF-8 文本策略。"""

from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/core/NativeClient.java"


def main() -> None:
    source = SOURCE.read_text(encoding="utf-8")
    errors = []
    if "private static String text(byte[] value)" in source:
        errors.append(f"{SOURCE}: 仍保留 UTF-8 响应转发方法")
    if re.search(r"(?<![.\w])text\(", source):
        errors.append(f"{SOURCE}: JNI 响应仍通过本地 text 方法解码")
    if source.count("TextPolicy.utf8(") < 80:
        errors.append(f"{SOURCE}: JNI 响应没有统一直接使用 TextPolicy.utf8")
    if errors:
        for error in errors:
            print(error)
        raise SystemExit(1)
    print("Android JNI responses use the shared UTF-8 text policy")


if __name__ == "__main__":
    main()
