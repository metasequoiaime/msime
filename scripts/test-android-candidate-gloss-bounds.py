#!/usr/bin/env python3
"""Android candidate gloss requests must be bounded before JNI copies either array."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

for declaration in (
    "constexpr jsize kCandidateGlossRequestLimit = 262144;",
    "constexpr jsize kCandidateGlossResourcesLimit = 4096;",
):
    if declaration not in text:
        raise SystemExit(f"missing Android candidate-gloss bound: {declaration}")

marker = "Java_app_msime_android_NativeClient_candidateGlossesRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("candidateGlossesRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "kCandidateGlossRequestLimit" not in body or "kCandidateGlossResourcesLimit" not in body:
    raise SystemExit("candidateGlossesRaw must reject oversized arrays before JNI copies")

print("Android candidate gloss requests are bounded before JNI copies")
