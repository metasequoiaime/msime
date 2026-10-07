#!/usr/bin/env python3
"""Android voice host requests must be bounded before JNI copies their byte arrays."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kVoiceRequestLimit = 1 * 1024 * 1024;" not in text:
    raise SystemExit("Android voice request limit is missing")

for method in ("voiceHotwordsRaw", "voiceHotwordCorrectRaw"):
    marker = f"Java_app_msime_android_NativeClient_{method}"
    start = text.find(marker)
    if start < 0:
        raise SystemExit(f"{method} JNI export is missing")
    end = text.find("\n}", start)
    body = text[start:end]
    if "bounded_request(env, request, kVoiceRequestLimit" not in body:
        raise SystemExit(f"{method} must reject oversized requests before JNI copies")

if "host_request(env, request" in text:
    raise SystemExit("unbounded Android host_request helper is still used")

print("Android voice host requests are bounded before JNI copies")
