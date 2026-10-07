#!/usr/bin/env python3
"""Android typing-statistics requests must be bounded before JNI copies their byte arrays."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kTypingStatisticsRequestLimit = 65536;" not in text:
    raise SystemExit("Android typing-statistics request limit is missing")

marker = "Java_app_msime_android_NativeClient_typingStatisticsRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("typingStatisticsRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "bounded_request(env, request, kTypingStatisticsRequestLimit" not in body:
    raise SystemExit("typingStatisticsRaw must reject oversized requests before JNI copies")

print("Android typing-statistics requests are bounded before JNI copies")
