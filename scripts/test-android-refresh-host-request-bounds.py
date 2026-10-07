#!/usr/bin/env python3
"""Android refresh-host paths must be bounded before JNI copies their byte arrays."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kRefreshHostPathLimit = 4096;" not in text:
    raise SystemExit("Android refresh-host path limit is missing")

marker = "Java_app_msime_android_NativeClient_refreshHostRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("refreshHostRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "bounded_request(env, path, kRefreshHostPathLimit" not in body:
    raise SystemExit("refreshHostRaw must reject oversized paths before JNI copies")

print("Android refresh-host paths are bounded before JNI copies")
