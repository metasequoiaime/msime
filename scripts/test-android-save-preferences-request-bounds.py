#!/usr/bin/env python3
"""Android preference saves must be bounded before JNI copies either byte array."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

for declaration in (
    "constexpr jsize kSavePreferencesDirectoryLimit = 16384;",
    "constexpr jsize kSavePreferencesSnapshotLimit = 1 * 1024 * 1024;",
):
    if declaration not in text:
        raise SystemExit(f"missing Android preference-save bound: {declaration}")

marker = "Java_app_msime_android_NativeClient_savePreferencesRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("savePreferencesRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "kSavePreferencesDirectoryLimit" not in body or "kSavePreferencesSnapshotLimit" not in body:
    raise SystemExit("savePreferencesRaw must reject oversized arrays before JNI copies")

print("Android preference saves are bounded before JNI copies")
