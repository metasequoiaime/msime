#!/usr/bin/env python3
"""Android personal-dictionary sync requests must be bounded before JNI copies."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kPersonalDictionaryRequestLimit = 2248576;" not in text:
    raise SystemExit("Android personal-dictionary request limit is missing")

marker = "Java_app_msime_android_NativeClient_personalDictionarySyncRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("personalDictionarySyncRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "bounded_request(env, options, kPersonalDictionaryRequestLimit" not in body:
    raise SystemExit("personalDictionarySyncRaw must reject oversized arrays before JNI copies")

print("Android personal-dictionary sync requests are bounded before JNI copies")
