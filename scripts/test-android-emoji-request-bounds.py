#!/usr/bin/env python3
"""Android emoji catalog requests must be bounded before JNI copies either array."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

for declaration in (
    "constexpr jsize kEmojiQueryLimit = 16384;",
    "constexpr jsize kEmojiResourcesLimit = 4096;",
):
    if declaration not in text:
        raise SystemExit(f"missing Android emoji bound: {declaration}")

marker = "Java_app_msime_android_NativeClient_emojiCatalogRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("emojiCatalogRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "kEmojiQueryLimit" not in body or "kEmojiResourcesLimit" not in body:
    raise SystemExit("emojiCatalogRaw must reject oversized arrays before JNI copies")

print("Android emoji catalog requests are bounded before JNI copies")
