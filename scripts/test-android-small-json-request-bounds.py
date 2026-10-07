#!/usr/bin/env python3
"""Android telemetry and notice JSON requests must be bounded before JNI copies them."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kSmallJsonRequestLimit = 16 * 1024;" not in text:
    raise SystemExit("Android small JSON request limit is missing")

start = text.find("static jbyteArray json_call(")
end = text.find("\n}", start)
if start < 0 or end < 0:
    raise SystemExit("Android json_call helper is missing")
body = text[start:end]
if "length > kSmallJsonRequestLimit" not in body:
    raise SystemExit("Android small JSON requests must be bounded before JNI copies")

print("Android telemetry and notice requests are bounded before JNI copies")
