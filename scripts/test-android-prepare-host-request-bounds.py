#!/usr/bin/env python3
"""Android host preparation requests must be bounded before JNI copies their byte arrays."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kPrepareHostRequestLimit = 16384;" not in text:
    raise SystemExit("Android prepare-host request limit is missing")

marker = "Java_app_msime_android_NativeClient_prepareHostRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("prepareHostRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "bounded_request(env, options, kPrepareHostRequestLimit" not in body:
    raise SystemExit("prepareHostRaw must reject oversized requests before JNI copies")

print("Android prepare-host requests are bounded before JNI copies")
