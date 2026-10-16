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

for name, correction in (("englishCompletions", "false"), ("englishCorrections", "true")):
    method = re.search(rf"public static String {name}\(.*?\n    \}}", text, re.DOTALL)
    if not method or f"englishCandidates(prefix, resources, {correction})" not in method.group(0):
        raise SystemExit(f"{name} must use the bounded English query")

method = re.search(r"private static String englishCandidates\(.*?\n    \}", text, re.DOTALL)
if not method:
    raise SystemExit("Bounded English query is missing")
body = method.group(0)
guard = body.find("resourcesBytes.length > ENGLISH_COMPLETION_RESOURCES_LIMIT")
call = body.find("englishCompletionsRaw(requestBytes, resourcesBytes)")
if guard < 0 or call < guard:
    raise SystemExit("English queries must reject oversized resources before JNI")

print("Android English completion resources are bounded before JNI")
