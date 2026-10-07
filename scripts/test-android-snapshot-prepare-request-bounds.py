#!/usr/bin/env python3
"""Android snapshot preparation inputs must be bounded before JNI copies their byte arrays."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

for declaration in (
    "constexpr jsize kSnapshotPrepareRequestLimit = 1 * 1024 * 1024;",
    "constexpr jsize kSnapshotPreparePathLimit = 16384;",
):
    if declaration not in text:
        raise SystemExit(f"missing {declaration}")

marker = "Java_app_msime_android_NativeClient_snapshotPrepareRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("snapshotPrepareRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "request_length > kSnapshotPrepareRequestLimit" not in body:
    raise SystemExit("snapshotPrepareRaw must bound requests before JNI copies")
if "file_length > kSnapshotPreparePathLimit" not in body:
    raise SystemExit("snapshotPrepareRaw must bound paths before JNI copies")

print("Android snapshot preparation inputs are bounded before JNI copies")
