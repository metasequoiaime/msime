#!/usr/bin/env python3
"""Android polish prompts must be bounded before JNI copies their byte arrays."""

from pathlib import Path

source = (Path(__file__).resolve().parents[1] / "platforms/android/native/client_jni.cpp").read_text(encoding="utf-8")
start = source.find("Java_app_msime_android_NativeClient_polishPromptRaw")
if start < 0:
    raise SystemExit("Android polish prompt JNI export is missing")
end = source.find("\n}", start)
body = source[start:end]
for declaration in (
    "constexpr jsize kPolishPromptIdLimit = 256;",
    "constexpr jsize kPolishPromptCustomLimit = 8192;",
):
    if declaration not in source:
        raise SystemExit(f"missing Android polish prompt bound: {declaration}")
for array, limit in (
    ("id", "kPolishPromptIdLimit"),
    ("custom1", "kPolishPromptCustomLimit"),
    ("custom2", "kPolishPromptCustomLimit"),
    ("custom3", "kPolishPromptCustomLimit"),
):
    check = f"{array} && env->GetArrayLength({array}) > {limit}"
    copy = f"utf8(env, {array})"
    if check not in body or copy not in body or body.index(check) > body.index(copy):
        raise SystemExit(f"{array} must be bounded before its JNI copy")
print("Android polish prompt JNI inputs are bounded before copies")
