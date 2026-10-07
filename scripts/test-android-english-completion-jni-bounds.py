#!/usr/bin/env python3
"""Android English completion JNI requests must be bounded before either copy."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

for declaration in (
    "constexpr jsize kEnglishCompletionRequestLimit = 16384;",
    "constexpr jsize kEnglishCompletionResourcesLimit = 4096;",
):
    if declaration not in text:
        raise SystemExit(f"missing Android English completion bound: {declaration}")

marker = "Java_app_msime_android_NativeClient_englishCompletionsRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("englishCompletionsRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "kEnglishCompletionRequestLimit" not in body or "kEnglishCompletionResourcesLimit" not in body:
    raise SystemExit("englishCompletionsRaw must reject oversized arrays before JNI copies")

print("Android English completion requests are bounded before JNI copies")
