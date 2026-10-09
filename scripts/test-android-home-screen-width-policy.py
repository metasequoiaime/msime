#!/usr/bin/env python3
"""Android 设置组件直接复用共享屏幕宽度策略。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
UI = HOME / "Ui.java"
EXPECTED_CALLERS = (
    "InputDialog.java",
    "KeyboardTryoutActivity.java",
)


def main() -> int:
    errors = []
    ui = UI.read_text(encoding="utf-8")
    if "public static int screenWidthPixels(Context context)" in ui:
        errors.append(f"{UI}: 不应保留 screenWidthPixels 转发方法")

    for path in HOME.glob("*.java"):
        source = path.read_text(encoding="utf-8")
        if "Ui.screenWidthPixels(" in source:
            errors.append(f"{path}: 应直接调用 KeyboardGeometry.screenWidthPixels")

    for name in EXPECTED_CALLERS:
        path = HOME / name
        if "KeyboardGeometry.screenWidthPixels(" not in path.read_text(encoding="utf-8"):
            errors.append(f"{path}: 未直接复用 KeyboardGeometry.screenWidthPixels")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android home components use the shared screen-width policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
