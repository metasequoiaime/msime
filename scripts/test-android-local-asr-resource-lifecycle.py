#!/usr/bin/env python3
"""确认本地识别器的录音资源与原生句柄始终在取消锁和清理范围内。"""
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/voice/LocalAsrRecognizer.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    required = [
        re.compile(r"AudioRecord recorder;\s+synchronized \(captureStartLock\) \{\s+checkCancelled\(\);\s+if \(stopped\.get\(\)\) throw new Refused\(Failure\.EMPTY\);\s+recorder = openRecorder\(\);\s+\}\s+Thread capture ="),
        re.compile(r"long created = 0;\s+boolean captureStarted = false;\s+try \{\s+(?://[^\n]*\n\s+)*(?:diagnostic\(DiagnosticEventKind\.VOICE_LOCAL_NATIVE_START\);\s+)?synchronized \(handleLock\) \{\s+checkCancelled\(\);\s+created = NativeClient\.localSpeechCreate\(\);\s+handle = created;"),
        re.compile(r"finally \{\s+stop\(\);\s+joinQuietly\(capture\);\s+if \(!captureStarted\) recorder\.release\(\);\s+synchronized \(handleLock\) \{\s+handle = 0;\s+if \(created != 0\) NativeClient\.localSpeechDestroy\(created\);\s+\}"),
    ]
    if any(pattern.search(source) is None for pattern in required):
        print(f"{SOURCE}: native session creation must not leak AudioRecord on failure", file=sys.stderr)
        return 1
    print("Android local ASR creation has recorder cleanup coverage")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
