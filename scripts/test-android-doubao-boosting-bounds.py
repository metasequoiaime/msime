#!/usr/bin/env python3
"""Android Doubao boosting IDs must be bounded before JNI copies."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kDoubaoBoostingLimit = 4096;" not in text:
    raise SystemExit("Android Doubao boosting limit is missing")

marker = "Java_app_msime_android_NativeClient_doubaoStartFrameRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("doubaoStartFrameRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "GetArrayLength(boosting)" not in body or "kDoubaoBoostingLimit" not in body:
    raise SystemExit("doubaoStartFrameRaw must reject oversized IDs before JNI copies")

print("Android Doubao boosting IDs are bounded before JNI copies")
