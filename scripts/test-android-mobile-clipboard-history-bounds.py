#!/usr/bin/env python3
"""Android mobile clipboard history requests must be bounded before JNI copies."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kMobileClipboardHistoryRequestLimit = 524288;" not in text:
    raise SystemExit("Android mobile clipboard request limit is missing")

marker = "Java_app_msime_android_NativeClient_mobileClipboardHistoryRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("mobileClipboardHistoryRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "bounded_request(env, request, kMobileClipboardHistoryRequestLimit" not in body:
    raise SystemExit("mobileClipboardHistoryRaw must reject oversized arrays before JNI copies")

print("Android mobile clipboard history requests are bounded before JNI copies")
