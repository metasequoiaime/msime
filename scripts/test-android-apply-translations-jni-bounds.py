#!/usr/bin/env python3
"""Android translation result JNI buffers must be bounded before copying."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

declaration = "constexpr jsize kApplyTranslationsLimit = 1 * 1024 * 1024;"
if declaration not in text:
    raise SystemExit(f"missing Android translation result bound: {declaration}")

marker = "Java_app_msime_android_NativeClient_applyTranslationsRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("applyTranslationsRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "kApplyTranslationsLimit" not in body:
    raise SystemExit("applyTranslationsRaw must reject oversized buffers before JNI copies")

print("Android translation result JNI buffer is bounded before copy")
