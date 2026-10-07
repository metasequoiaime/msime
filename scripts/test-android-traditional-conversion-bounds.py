#!/usr/bin/env python3
"""Android traditional conversion text must be bounded before JNI copies."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kTraditionalConversionLimit = 1 * 1024 * 1024;" not in text:
    raise SystemExit("Android traditional conversion limit is missing")

marker = "Java_app_msime_android_NativeClient_simplifiedToTraditionalRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("simplifiedToTraditionalRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "length > kTraditionalConversionLimit" not in body:
    raise SystemExit("simplifiedToTraditionalRaw must reject oversized arrays before JNI copies")

print("Android traditional conversion text is bounded before JNI copies")
