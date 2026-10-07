#!/usr/bin/env python3
"""Android double-pinyin profile requests must be bounded before JNI copies."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kShuangpinProfileLimit = 64;" not in text:
    raise SystemExit("Android double-pinyin profile limit is missing")

marker = "Java_app_msime_android_NativeClient_shuangpinKeyHintsRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("shuangpinKeyHintsRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "bounded_request(env, profile, kShuangpinProfileLimit" not in body:
    raise SystemExit("shuangpinKeyHintsRaw must reject oversized arrays before JNI copies")

print("Android double-pinyin profile requests are bounded before JNI copies")
