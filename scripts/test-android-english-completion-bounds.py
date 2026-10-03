#!/usr/bin/env python3
"""Android 在 JNI 复制前拒绝过大的英文补全资源路径。"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/core/NativeClient.java"
text = SOURCE.read_text()

if "ENGLISH_COMPLETION_RESOURCES_LIMIT = 4 * 1024" not in text:
    raise SystemExit("English completion resource limit is missing")

method = re.search(
    r"public static String englishCompletions\(.*?\n    \}", text, re.DOTALL
)
if not method or "resourcesBytes.length > ENGLISH_COMPLETION_RESOURCES_LIMIT" not in method.group(0):
    raise SystemExit("English completions must reject oversized resources before JNI")

print("Android English completion resources are bounded before JNI")
