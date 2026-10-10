#!/usr/bin/env python3
"""检查 Android 文本控件工厂是否复用基础标签构造。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "platforms/android/java/app/msime/android/home/Ui.java"


def main() -> int:
    errors = []
    source = UI.read_text(encoding="utf-8")
    required = (
        "return headingLabel(context, text, TEXT_GROUP_TITLE, 500, ThemeColorPolicy.accent(context));",
        "TextView button = centeredSingleLineLabel(context, label, Math.round(sizeSp), weight, ink);",
        "TextView button = centeredLabel(context, label, sizeSp, weight, ink);",
    )
    for snippet in required:
        if snippet not in source:
            errors.append(f"{UI}: 文本控件工厂未复用 styledLabel：{snippet}")

    duplicated = (
        "TextView heading = new TextView(context);\n        heading.setText(text);",
        "TextView button = new TextView(context);\n        button.setText(label);",
    )
    for snippet in duplicated:
        if snippet in source:
            errors.append(f"{UI}: 仍在重复基础标签构造：{snippet.splitlines()[0]}")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android text control factories use the shared label constructors")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
