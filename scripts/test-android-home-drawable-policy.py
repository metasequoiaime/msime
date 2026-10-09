#!/usr/bin/env python3
"""Android 设置组件直接复用共享描边 drawable 工厂。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
UI = HOME / "Ui.java"
EXPECTED_CALLERS = (
    "AiSkinPage.java",
    "InputDialog.java",
    "KeyboardTryoutActivity.java",
    "LoginSheet.java",
    "OnboardingActivity.java",
    "SegmentedControl.java",
    "SkinsPage.java",
)
FORWARDERS = (
    "public static GradientDrawable circleOutlined(",
    "public static GradientDrawable outlined(",
    "public static GradientDrawable outlinedDashed(",
)
CALLS = ("Ui.circleOutlined(", "Ui.outlined(", "Ui.outlinedDashed(")


def main() -> int:
    errors = []
    ui = UI.read_text(encoding="utf-8")
    for signature in FORWARDERS:
        if signature in ui:
            errors.append(f"{UI}: 不应保留描边 drawable 转发方法 {signature}")

    for path in HOME.glob("*.java"):
        source = path.read_text(encoding="utf-8")
        for call in CALLS:
            if call in source:
                errors.append(f"{path}: 应直接调用 DrawablePolicy")

    for name in EXPECTED_CALLERS:
        path = HOME / name
        if "DrawablePolicy." not in path.read_text(encoding="utf-8"):
            errors.append(f"{path}: 未直接复用 DrawablePolicy")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android home components use shared outlined drawable factories directly")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
