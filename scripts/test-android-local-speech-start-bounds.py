#!/usr/bin/env python3
"""Android local speech start JNI inputs must be bounded before copying."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

for declaration in (
    "constexpr jsize kLocalSpeechModelPathLimit = 4096;",
    "constexpr jsize kLocalSpeechLanguageLimit = 256;",
    "constexpr jsize kLocalSpeechHotwordsLimit = 256 * 1024;",
):
    if declaration not in text:
        raise SystemExit(f"missing Android local speech start bound: {declaration}")

marker = "Java_app_msime_android_NativeClient_localSpeechStartRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("localSpeechStartRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "kLocalSpeechModelPathLimit" not in body or "kLocalSpeechLanguageLimit" not in body:
    raise SystemExit("localSpeechStartRaw must reject oversized paths before JNI copies")
if "kLocalSpeechHotwordsLimit" not in body:
    raise SystemExit("localSpeechStartRaw must reject oversized hotwords before JNI copies")

print("Android local speech start JNI inputs are bounded before copies")
