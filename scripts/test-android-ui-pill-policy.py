#!/usr/bin/env python3
"""Android home surfaces use the shared pill drawable factory directly."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
ANDROID = ROOT / "platforms/android/java/app/msime/android"
UI = ANDROID / "home/Ui.java"
EXPECTED_CALLERS = (
    "AboutPage.java",
    "FeedbackPage.java",
    "MsToast.java",
    "ProfilePage.java",
    "SearchPill.java",
    "StatisticsFragment.java",
)


def main() -> int:
    errors = []
    ui = UI.read_text(encoding="utf-8")
    if "GradientDrawable pill(" in ui:
        errors.append(f"{UI}: 仍保留 Ui.pill 转发方法")
    for path in ANDROID.rglob("*.java"):
        source = path.read_text(encoding="utf-8")
        if "Ui.pill(" in source:
            errors.append(f"{path}: 仍调用已删除的 Ui.pill")
    policy = (ANDROID / "DrawablePolicy.java").read_text(encoding="utf-8")
    if "public static GradientDrawable pill(int color)" not in policy:
        errors.append(f"{ANDROID / 'DrawablePolicy.java'}: 缺少 pill 工厂")
    for name in EXPECTED_CALLERS:
        path = ANDROID / "home" / name
        if "DrawablePolicy.pill(" not in path.read_text(encoding="utf-8"):
            errors.append(f"{path}: 未直接复用 DrawablePolicy.pill")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android home surfaces use the shared pill drawable factory")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
