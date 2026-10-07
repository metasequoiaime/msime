#!/usr/bin/env python3
"""Android online response JNI inputs must be bounded before either copy."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

for declaration in (
    "constexpr jsize kOnlineBodyLimit = 262144;",
    "constexpr jsize kOnlineCandidatesLimit = 16384;",
):
    if declaration not in text:
        raise SystemExit(f"missing Android online-response bound: {declaration}")

for method, first, second in (
    ("applyCloudResponseRaw", "kOnlineQueryLimit", "kOnlineBodyLimit"),
    ("applyOnlineCandidatesRaw", "kOnlineQueryLimit", "kOnlineCandidatesLimit"),
):
    marker = f"Java_app_msime_android_NativeClient_{method}"
    start = text.find(marker)
    if start < 0:
        raise SystemExit(f"{method} JNI export is missing")
    end = text.find("\n}", start)
    body = text[start:end]
    if first not in body or second not in body:
        raise SystemExit(f"{method} must reject oversized arrays before JNI copies")

print("Android online response inputs are bounded before JNI copies")
