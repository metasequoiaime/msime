#!/usr/bin/env python3
"""Android 设置绘图组件直接复用共享浮点像素换算。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
UI = HOME / "Ui.java"
EXPECTED_CALLERS = (
    "CommunityAdapter.java",
    "KeyboardPreview.java",
    "SkinSwatchView.java",
)


def main() -> int:
    errors = []
    ui = UI.read_text(encoding="utf-8")
    if "public static float dpFloat(Context context, float value)" in ui:
        errors.append(f"{UI}: 不应保留 dpFloat 转发方法")

    for path in HOME.glob("*.java"):
        source = path.read_text(encoding="utf-8")
        if "Ui.dpFloat(" in source:
            errors.append(f"{path}: 应直接调用 DimensionPolicy.floatPixels")

    for name in EXPECTED_CALLERS:
        path = HOME / name
        if "DimensionPolicy.floatPixels(" not in path.read_text(encoding="utf-8"):
            errors.append(f"{path}: 未直接复用 DimensionPolicy.floatPixels")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android home drawing components use the shared float-pixel policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
