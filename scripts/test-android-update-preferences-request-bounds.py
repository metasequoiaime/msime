#!/usr/bin/env python3
"""Android live preference updates must be bounded before JNI copies their byte arrays."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kUpdatePreferencesSnapshotLimit = 1 * 1024 * 1024;" not in text:
    raise SystemExit("Android update-preferences snapshot limit is missing")

marker = "Java_app_msime_android_NativeClient_updatePreferencesRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("updatePreferencesRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "length > kUpdatePreferencesSnapshotLimit" not in body:
    raise SystemExit("updatePreferencesRaw must reject oversized snapshots before JNI copies")

print("Android live preference updates are bounded before JNI copies")
