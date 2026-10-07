#!/usr/bin/env python3
"""Android snapshot activation versions must be bounded before JNI copies their byte arrays."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

marker = "Java_app_msime_android_NativeClient_snapshotActivateRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("snapshotActivateRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "env->GetArrayLength(expected)" not in body or "length != 64" not in body:
    raise SystemExit("snapshotActivateRaw must reject non-64-byte versions before JNI copies")

print("Android snapshot activation versions are bounded before JNI copies")
