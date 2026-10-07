#!/usr/bin/env python3
"""Android Doubao PCM JNI inputs must be bounded before copying."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

declaration = "constexpr jsize kDoubaoAudioPcmLimit = 1 * 1024 * 1024;"
if declaration not in text:
    raise SystemExit(f"missing Android Doubao PCM bound: {declaration}")

marker = "Java_app_msime_android_NativeClient_doubaoAudioFrameRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("doubaoAudioFrameRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "kDoubaoAudioPcmLimit" not in body:
    raise SystemExit("doubaoAudioFrameRaw must reject oversized PCM before JNI copies")

print("Android Doubao PCM JNI input is bounded before copy")
