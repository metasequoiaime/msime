#!/usr/bin/env python3
"""Android 文本调用点直接复用共享的小写去空白策略。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCES = (
    ROOT / "platforms/android/java/app/msime/android/candidate/CandidateTranslationPolicy.java",
    ROOT / "platforms/android/java/app/msime/android/home/PageId.java",
    ROOT / "platforms/android/java/app/msime/android/voice/WebSocketFrames.java",
)


def main() -> int:
    texts = {source: source.read_text(encoding="utf-8") for source in SOURCES}
    combined = "\n".join(texts.values())
    errors = []
    if "TextPolicy.lowercase(TextPolicy.trimmed(" in combined:
        errors.append("Android 文本调用点不应重复组合 lowercase 与 trimmed")
    candidate = texts[SOURCES[0]]
    if "private static String normalize(" in candidate:
        errors.append(f"{SOURCES[0]}: 不应保留 normalize 转发方法")
    if combined.count("TextPolicy.lowercaseTrimmed(") < 7:
        errors.append("Android 文本调用点应直接复用 TextPolicy.lowercaseTrimmed")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android text call sites use the shared lowercase-trimmed policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
