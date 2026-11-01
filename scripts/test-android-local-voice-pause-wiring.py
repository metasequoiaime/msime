#!/usr/bin/env python3
"""The saved local voice pause and VAD sensitivity must reach the keyboard's auto-stop timer and the native recognizer options."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ANDROID = ROOT / "platforms/android"
VOICE = ANDROID / "java/app/msime/android/voice"
SENSITIVITY = "AndroidLocalSettings.VOICE_VAD_SENSITIVITY"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def method(source: str, signature: str, label: str) -> str:
    start = source.find(signature)
    if start < 0:
        raise SystemExit(f"{label}: missing {signature!r}")
    end = source.find("\n    }\n", start)
    if end < 0:
        raise SystemExit(f"{label}: cannot find the end of {signature!r}")
    return source[start:end]


def sensitivity_before_recognize(body: str, receiver: str, label: str) -> None:
    call = f"{receiver}.vadSensitivity("
    index = body.find(call)
    if index < 0:
        raise SystemExit(f"{label} must pass the saved VAD sensitivity to its local recognizer")
    line = body[index : body.find("\n", index)]
    if SENSITIVITY not in line:
        raise SystemExit(f"{label} must read {SENSITIVITY} for {call}")
    recognize = body.find(f"{receiver}.recognize(")
    if recognize < 0 or recognize < index:
        raise SystemExit(f"{label} must set the VAD sensitivity before {receiver}.recognize(")


def main() -> None:
    entry = read(ANDROID / "java/app/msime/android/core/ImeVoiceEntry.java")
    start = entry.find("if (engine == Engine.LOCAL) {")
    end = entry.find("} else if (engine == Engine.UPLOAD)", start)
    if start < 0 or end < 0:
        raise SystemExit("ImeVoiceEntry local engine branch is missing")
    local = entry[start:end]
    if "silenceStopMillis = localPreferences.integer(AndroidLocalSettings.VOICE_PAUSE_MILLIS);" not in local:
        raise SystemExit("keyboard local voice must take its auto-stop pause from VOICE_PAUSE_MILLIS")
    if f"local.vadSensitivity(localPreferences.integer({SENSITIVITY}));" not in local:
        raise SystemExit("keyboard local voice must pass the saved VAD sensitivity to LocalAsrRecognizer")
    arm = method(entry, "private void armSilenceStop(", "ImeVoiceEntry")
    if arm.count("postDelayed(") < 2 or arm.count("silenceStopMillis") < 2:
        raise SystemExit("armSilenceStop must schedule and postpone with silenceStopMillis")

    sensitivity_before_recognize(
        method(read(VOICE / "VoiceRecognitionActivity.java"), "private void startLocalRecognition(", "VoiceRecognitionActivity"),
        "running",
        "VoiceRecognitionActivity",
    )
    sensitivity_before_recognize(
        method(read(VOICE / "OfflineVoiceRecognitionActivity.java"), "private void startWhenVisible(", "OfflineVoiceRecognitionActivity"),
        "recognizer",
        "OfflineVoiceRecognitionActivity",
    )

    recognizer = read(VOICE / "LocalAsrRecognizer.java")
    if "private int vadSensitivity;" not in recognizer:
        raise SystemExit("LocalAsrRecognizer must keep the VAD sensitivity in the vadSensitivity field passed to localSpeechStart")
    setter = method(recognizer, "public void vadSensitivity(int value) {", "LocalAsrRecognizer")
    if "vadSensitivity = value;" not in setter:
        raise SystemExit("LocalAsrRecognizer.vadSensitivity must store the value it is given in the field passed to localSpeechStart")
    call = recognizer.find("NativeClient.localSpeechStart(")
    if call < 0:
        raise SystemExit("LocalAsrRecognizer must start the native local speech handle")
    arguments = recognizer[call : recognizer.find(";", call)]
    if not arguments.rstrip().endswith(", vadSensitivity)"):
        raise SystemExit("LocalAsrRecognizer must pass its vadSensitivity as the last localSpeechStart argument")

    client = method(
        read(ANDROID / "java/app/msime/android/core/NativeClient.java"),
        "String hotwords, int threads, int vadSensitivity) {",
        "NativeClient",
    )
    if "threads, vadSensitivity);" not in client:
        raise SystemExit("NativeClient.localSpeechStart must forward vadSensitivity to localSpeechStartRaw")

    jni = read(ANDROID / "native/client_jni.cpp")
    marker = "Java_app_msime_android_NativeClient_localSpeechStartRaw("
    start = jni.find(marker)
    if start < 0:
        raise SystemExit("localSpeechStartRaw JNI export is missing")
    body = jni[start : jni.find("\n}", start)]
    signature = body[: body.find(")")]
    if "jint vad_sensitivity" not in signature:
        raise SystemExit("localSpeechStartRaw must take a jint vad_sensitivity parameter")
    if "options.vad_sensitivity_percent = vad_sensitivity;" not in body:
        raise SystemExit("localSpeechStartRaw must copy vad_sensitivity into LocalAsrOptions")

    asr = read(ROOT / "shared/voice/LocalAsr.cpp")
    if "local_asr_vad_threshold(options.vad_sensitivity_percent)" not in asr:
        raise SystemExit("LocalAsr must derive the Silero VAD threshold from vad_sensitivity_percent")

    print("Android local voice pause and VAD sensitivity reach the timer and native options")


if __name__ == "__main__":
    main()
