#!/usr/bin/env python3
"""Keep the local recognizer's recorder alive only inside a cleanup scope."""
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/voice/LocalAsrRecognizer.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    required = [
        re.compile(r"AudioRecord recorder = openRecorder\(\);\s+Thread capture ="),
        re.compile(r"long created = 0;\s+boolean captureStarted = false;\s+try \{\s+(?://[^\n]*\n\s+)*(?:diagnostic\(DiagnosticEventKind\.VOICE_LOCAL_NATIVE_START\);\s+)?created = NativeClient\.localSpeechCreate\(\);"),
        re.compile(r"if \(created != 0\) NativeClient\.localSpeechDestroy\(created\);"),
    ]
    if any(pattern.search(source) is None for pattern in required):
        print(f"{SOURCE}: native session creation must not leak AudioRecord on failure", file=sys.stderr)
        return 1
    print("Android local ASR creation has recorder cleanup coverage")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
