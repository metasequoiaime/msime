#!/usr/bin/env python3
"""Android 宿主界面的设计动效曲线集中在共享组件。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
CURVES = HOME / "MotionCurves.java"
UI = HOME / "Ui.java"
HOME_ACTIVITY = HOME / "HomeActivity.java"
PAGE_DOTS = HOME / "PageDots.java"


def main() -> int:
    errors = []
    if not CURVES.is_file():
        errors.append(f"{CURVES}: 缺少共享动效曲线组件")
        curves = ""
    else:
        curves = CURVES.read_text(encoding="utf-8")
    for snippet in (
        "public static final PathInterpolator EMPHASIZED = new PathInterpolator(0.2f, 0f, 0f, 1f);",
        "public static final PathInterpolator POP = new PathInterpolator(0.16f, 1f, 0.3f, 1f);",
        "public static final PathInterpolator EASE = new PathInterpolator(0.25f, 0.1f, 0.25f, 1f);",
    ):
        if snippet not in curves:
            errors.append(f"{CURVES}: 缺少 {snippet}")

    ui = UI.read_text(encoding="utf-8")
    if "emphasized()" in ui or "class MotionCurves" in ui:
        errors.append(f"{UI}: 不应保留动效曲线转发或嵌套实现")

    home_activity = HOME_ACTIVITY.read_text(encoding="utf-8")
    if "MotionCurves.POP" not in home_activity or "MotionCurves.EASE" not in home_activity:
        errors.append(f"{HOME_ACTIVITY}: 开屏动效未直接复用共享曲线")
    page_dots = PAGE_DOTS.read_text(encoding="utf-8")
    if "MotionCurves.EMPHASIZED" not in page_dots:
        errors.append(f"{PAGE_DOTS}: 页码点动效未直接复用共享曲线")
    if "Ui.emphasized()" in page_dots:
        errors.append(f"{PAGE_DOTS}: 不应经过 Ui 转发动效曲线")

    for path in HOME.glob("*.java"):
        if path != CURVES and "new PathInterpolator(" in path.read_text(encoding="utf-8"):
            errors.append(f"{path}: 应复用 MotionCurves 中的设计曲线")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android host animations use the shared motion curves")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
