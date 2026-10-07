#!/usr/bin/env python3
"""Android typing-statistics switch paths must be bounded before JNI copies."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kTypingStatisticsDirectoryLimit = 16384;" not in text:
    raise SystemExit("Android typing-statistics directory limit is missing")

marker = "Java_app_msime_android_NativeClient_typingStatisticsEnabledRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("typingStatisticsEnabledRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "kTypingStatisticsDirectoryLimit" not in body:
    raise SystemExit("typingStatisticsEnabledRaw must reject oversized arrays before JNI copies")

print("Android typing-statistics switch paths are bounded before JNI copies")
