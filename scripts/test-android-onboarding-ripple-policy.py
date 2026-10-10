#!/usr/bin/env python3
"""Android 引导页复用设置界面的主题按压反馈。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
UI = HOME / "Ui.java"
ONBOARDING = HOME / "OnboardingActivity.java"


def main() -> int:
    errors = []
    ui = UI.read_text(encoding="utf-8")
    if "public static Drawable ripple(Context context)" in ui:
        errors.append(f"{UI}: 不应保留主题按压反馈转发")

    onboarding = ONBOARDING.read_text(encoding="utf-8")
    if "ViewPolicy.setBackground(button, DrawablePolicy.ripple(this));" not in onboarding:
        errors.append(f"{ONBOARDING}: 操作按钮未复用 Ui.ripple")

    for path in HOME.glob("*.java"):
        source = path.read_text(encoding="utf-8")
        if path != UI and "selectableItemBackground" in source:
            errors.append(f"{path}: 不应重复解析主题按压反馈")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android onboarding actions use the shared theme ripple")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
