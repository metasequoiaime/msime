#!/usr/bin/env python3
"""Android 的云候选查询必须在 JNI 复制前遵守共享查询上限。"""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kOnlineQueryLimit = MSIME_ONLINE_QUERY_MAX_BYTES;" not in text:
    raise SystemExit("Android online-query limit is missing")

marker = "Java_app_msime_android_NativeClient_cloudRequestUrlRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("cloudRequestUrlRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "bounded_request(env, query, kOnlineQueryLimit" not in body:
    raise SystemExit("cloudRequestUrlRaw must reject oversized arrays before JNI copies")

print("Android cloud query URL requests are bounded before JNI copies")
