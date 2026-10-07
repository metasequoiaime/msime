#!/usr/bin/env python3
"""Android vocabulary review requests must be bounded before JNI copies."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kVocabularyReviewRequestLimit = 8 * 1024 * 1024;" not in text:
    raise SystemExit("Android vocabulary-review request limit is missing")

marker = "Java_app_msime_android_NativeClient_vocabularyReviewRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("vocabularyReviewRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "bounded_request(env, request, kVocabularyReviewRequestLimit" not in body:
    raise SystemExit("vocabularyReviewRaw must reject oversized arrays before JNI copies")

print("Android vocabulary-review requests are bounded before JNI copies")
