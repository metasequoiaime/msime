#!/usr/bin/env python3
"""Android 设置组件直接复用共享颜色透明度策略。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
UI = HOME / "Ui.java"
EXPECTED_CALLERS = (
    "BadgeGridView.java",
    "DistributionView.java",
    "KeyHeatmapView.java",
    "KeyboardPreview.java",
    "KeyboardTryoutActivity.java",
    "LoginSheet.java",
    "PageDots.java",
)


def main() -> int:
    errors = []
    ui = UI.read_text(encoding="utf-8")
    if "public static int withAlpha(@ColorInt int color, float alpha)" in ui:
        errors.append(f"{UI}: 不应保留 withAlpha 转发方法")

    for path in HOME.glob("*.java"):
        source = path.read_text(encoding="utf-8")
        if "Ui.withAlpha(" in source:
            errors.append(f"{path}: 应直接调用 ColorPolicy.withAlpha")

    for name in EXPECTED_CALLERS:
        path = HOME / name
        if "ColorPolicy.withAlpha(" not in path.read_text(encoding="utf-8"):
            errors.append(f"{path}: 未直接复用 ColorPolicy.withAlpha")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android home components use the shared alpha policy directly")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
