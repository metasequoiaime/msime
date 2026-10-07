#!/usr/bin/env python3
"""Android theme requests must be bounded before JNI copies their byte arrays."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kThemeRequestLimit = 1 * 1024 * 1024;" not in text:
    raise SystemExit("Android theme request limit is missing")

marker = "Java_app_msime_android_NativeClient_resolveThemeRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("resolveThemeRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "bounded_request(env, request, kThemeRequestLimit" not in body:
    raise SystemExit("resolveThemeRaw must reject oversized requests before JNI copies")

print("Android theme requests are bounded before JNI copies")
