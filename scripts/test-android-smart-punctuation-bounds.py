#!/usr/bin/env python3
"""Android smart-punctuation requests must be bounded before JNI copies."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kSmartPunctuationRequestLimit = 4096;" not in text:
    raise SystemExit("Android smart-punctuation request limit is missing")

for method in ("smartPunctuationArmRaw", "smartPunctuationDecideRaw"):
    marker = f"Java_app_msime_android_NativeClient_{method}"
    start = text.find(marker)
    if start < 0:
        raise SystemExit(f"{method} JNI export is missing")
    end = text.find("\n}", start)
    body = text[start:end]
    if "length > kSmartPunctuationRequestLimit" not in body:
        raise SystemExit(f"{method} must reject oversized arrays before JNI copies")

print("Android smart-punctuation requests are bounded before JNI copies")
