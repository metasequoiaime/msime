#!/usr/bin/env python3
"""键盘方案卡片直接复用共享 drawable 策略。"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/keyboard/KeyboardSchemeCard.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    errors = []
    for name in ("rounded", "outlined"):
        if re.search(rf"(?<![.\w]){name}\(", source):
            errors.append(f"{SOURCE}: 不应保留或调用 {name} 转发方法")
    if source.count("DrawablePolicy.rounded(") < 1:
        errors.append(f"{SOURCE}: 圆角背景应直接复用 DrawablePolicy.rounded")
    if source.count("DrawablePolicy.outlined(") < 2:
        errors.append(f"{SOURCE}: 描边背景应直接复用 DrawablePolicy.outlined")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android keyboard scheme cards use the shared drawable policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
