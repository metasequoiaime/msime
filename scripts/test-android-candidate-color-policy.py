#!/usr/bin/env python3
"""候选栏颜色直接复用共享解析策略。"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/candidate/CandidateAppearance.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    errors = []
    if "private static int parseColor(" in source:
        errors.append(f"{SOURCE}: 不应保留 parseColor 转发方法")
    if re.search(r"(?<![.\w])parseColor\(", source):
        errors.append(f"{SOURCE}: 不应调用未限定的 parseColor 方法")
    if source.count("ColorPolicy.parseHex(") < 8:
        errors.append(f"{SOURCE}: 候选栏颜色应直接复用 ColorPolicy.parseHex")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android candidate colours use the shared parsing policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
