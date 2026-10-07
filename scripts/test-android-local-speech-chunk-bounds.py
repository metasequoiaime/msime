#!/usr/bin/env python3
"""Android local speech chunks must be bounded before native allocation."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jint kLocalSpeechChunkLimit = 1600;" not in text:
    raise SystemExit("Android local speech chunk limit is missing")

marker = "Java_app_msime_android_NativeClient_localSpeechAcceptRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("localSpeechAcceptRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "count > kLocalSpeechChunkLimit" not in body:
    raise SystemExit("localSpeechAcceptRaw must reject oversized chunks before allocation")

print("Android local speech chunks are bounded before native allocation")
