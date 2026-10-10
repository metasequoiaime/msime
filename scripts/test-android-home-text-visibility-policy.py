#!/usr/bin/env python3
"""Android 设置组件直接复用共享文本可见性策略。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
UI = HOME / "Ui.java"
EXPECTED_CALLERS = (
    "CommunityAdapter.java",
    "CommunityFragment.java",
    "GroupCard.java",
    "HomeNavGroup.java",
    "ListRows.java",
    "LoginSheet.java",
)


def main() -> int:
    errors = []
    ui = UI.read_text(encoding="utf-8")
    if "public static void setVisibilityForText(View view, CharSequence text)" in ui:
        errors.append(f"{UI}: 不应保留 setVisibilityForText 转发方法")

    for path in HOME.glob("*.java"):
        source = path.read_text(encoding="utf-8")
        if "Ui.setVisibilityForText(" in source:
            errors.append(f"{path}: 应直接调用 ViewPolicy.setVisibilityForText")

    for name in EXPECTED_CALLERS:
        path = HOME / name
        if "ViewPolicy.setVisibilityForText(" not in path.read_text(encoding="utf-8"):
            errors.append(f"{path}: 未直接复用 ViewPolicy.setVisibilityForText")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android home components use the shared text-visibility policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
