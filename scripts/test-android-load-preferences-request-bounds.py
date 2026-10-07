#!/usr/bin/env python3
"""Android preference-load directories must be bounded before JNI copies their byte arrays."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kLoadPreferencesDirectoryLimit = 16384;" not in text:
    raise SystemExit("Android preference-load directory limit is missing")

marker = "Java_app_msime_android_NativeClient_loadPreferencesRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("loadPreferencesRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "bounded_request(env, directory, kLoadPreferencesDirectoryLimit" not in body:
    raise SystemExit("loadPreferencesRaw must reject oversized directories before JNI copies")

print("Android preference-load directories are bounded before JNI copies")
