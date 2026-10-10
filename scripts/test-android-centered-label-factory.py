#!/usr/bin/env python3
"""检查 Android 居中文本是否复用共享标签工厂。"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
UI = HOME / "Ui.java"


def main() -> int:
    errors = []
    ui = UI.read_text(encoding="utf-8")
    required = (
        "public static TextView centeredLabel(Context context, CharSequence text, float sizeSp,",
        "TextView centered = label(context, text, sizeSp, color);",
        "public static TextView centeredLabel(Context context, CharSequence text, int sizeSp, int weight,",
        "TextView centered = styledLabel(context, text, sizeSp, weight, color);",
        "TextView button = centeredSingleLineLabel(context, label, Math.round(sizeSp), weight, ink);",
        "TextView button = centeredLabel(context, label, sizeSp, weight, ink);",
    )
    for snippet in required:
        if snippet not in ui:
            errors.append(f"{UI}: 居中标签工厂缺少：{snippet}")

    consumers = {
        "CloudClipboardPage.java": 1,
        "InputDialog.java": 1,
        "LexiconPage.java": 1,
        "MsToast.java": 1,
        "OnboardingActivity.java": 2,
        "ProfilePage.java": 1,
        "SegmentedControl.java": 1,
        "SheetHeaderView.java": 1,
        "SheetOptionView.java": 1,
        "SkinsPage.java": 4,
    }
    duplicated = re.compile(
        r"Ui\.(?:styledLabel|label)\([^;]{0,300};\s*ViewPolicy\.setCentered\(", re.DOTALL
    )
    for name, minimum in consumers.items():
        path = HOME / name
        source = path.read_text(encoding="utf-8")
        centered = source.count("Ui.centeredLabel(") + source.count("Ui.centeredSingleLineLabel(")
        if centered < minimum:
            errors.append(f"{path}: 至少 {minimum} 处文本未复用 Ui 居中标签工厂")
        if duplicated.search(source):
            errors.append(f"{path}: 仍在标签构造后重复设置居中")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android centered labels use the shared factory")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
