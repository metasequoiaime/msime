#!/usr/bin/env python3
"""键盘布局调整视图直接复用共享颜色与几何策略。"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/keyboard/KeyboardLayoutAdjustView.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    errors = []
    for name in ("color", "dpFromPixels"):
        if re.search(rf"(?<![.\w]){name}\(", source):
            errors.append(f"{SOURCE}: 不应保留或调用 {name} 转发方法")
    if source.count("ColorPolicy.parse(") < 4:
        errors.append(f"{SOURCE}: 皮肤颜色应直接复用 ColorPolicy.parse")
    if source.count("DimensionPolicy.fromPixels(") < 3:
        errors.append(f"{SOURCE}: 拖动距离应直接复用 DimensionPolicy.fromPixels")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android keyboard layout adjustment uses shared colour and geometry policies")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
