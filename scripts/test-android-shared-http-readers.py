#!/usr/bin/env python3
"""语音 HTTP 响应统一使用 Android 的共享有界读取策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
READERS = (
    ROOT / "platforms/android/java/app/msime/android/voice/VoicePolisher.java",
    ROOT / "platforms/android/java/app/msime/android/voice/HttpAsrRecognizer.java",
)


def main() -> int:
    ok = True
    for path in READERS:
        source = path.read_text(encoding="utf-8")
        if not any(name in source for name in (
                "HttpBodyPolicy.readBounded", "HttpBodyPolicy.readRequired")):
            print(f"{path}: 没有使用共享 HttpBodyPolicy 有界读取", file=sys.stderr)
            ok = False
        if "private static String read(" in source:
            print(f"{path}: 仍保留自定义 HTTP body 读取器", file=sys.stderr)
            ok = False
    if ok:
        print("Android voice HTTP readers use the shared bounded body policy")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
