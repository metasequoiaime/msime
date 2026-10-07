#!/usr/bin/env python3
"""Android local speech must stop when JNI PCM extraction reports an exception."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

marker = "Java_app_msime_android_NativeClient_localSpeechAcceptRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("localSpeechAcceptRaw JNI export is missing")
end = text.find("\n}", start)
if end < 0:
    raise SystemExit("localSpeechAcceptRaw JNI body is missing")
body = text[start:end]
copy = "env->GetShortArrayRegion(pcm, 0, count, samples.data());"
check = "if (env->ExceptionCheck()) return nullptr;"
if copy not in body:
    raise SystemExit("localSpeechAcceptRaw PCM region copy is missing")
if check not in body:
    raise SystemExit("localSpeechAcceptRaw must check JNI exceptions after PCM copy")
if body.index(check) < body.index(copy):
    raise SystemExit("JNI exception check must follow the PCM region copy")

print("Android local speech checks JNI PCM extraction failures")
