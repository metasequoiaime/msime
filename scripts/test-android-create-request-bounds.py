#!/usr/bin/env python3
"""Android session creation options must be bounded before JNI copies."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kCreateOptionsLimit = 1 * 1024 * 1024;" not in text:
    raise SystemExit("Android create options limit is missing")

marker = "Java_app_msime_android_NativeClient_createRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("createRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "bounded_request(env, options, kCreateOptionsLimit" not in body:
    raise SystemExit("createRaw must reject oversized arrays before JNI copies")

print("Android session creation options are bounded before JNI copies")
