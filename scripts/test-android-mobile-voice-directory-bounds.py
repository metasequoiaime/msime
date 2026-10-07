#!/usr/bin/env python3
"""Android mobile voice configuration paths must be bounded before JNI copies them."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/native/client_jni.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr jsize kMobileVoiceDirectoryLimit = 16384;" not in text:
    raise SystemExit("Android mobile voice directory limit is missing")

marker = "Java_app_msime_android_NativeClient_mobileVoiceConfigurationRaw"
start = text.find(marker)
if start < 0:
    raise SystemExit("mobileVoiceConfigurationRaw JNI export is missing")
end = text.find("\n}", start)
body = text[start:end]
if "bounded_request(env, directory, kMobileVoiceDirectoryLimit" not in body:
    raise SystemExit("mobileVoiceConfigurationRaw must reject oversized paths before JNI copies")

print("Android mobile voice directories are bounded before JNI copies")
