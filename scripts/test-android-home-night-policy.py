#!/usr/bin/env python3
"""Android 设置组件直接复用共享夜间模式策略。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
UI = HOME / "Ui.java"
EXPECTED_CALLERS = (
    "AppMode.java",
    "BadgeGridView.java",
    "DistributionView.java",
    "LoginSheet.java",
)


def main() -> int:
    errors = []
    ui = UI.read_text(encoding="utf-8")
    if "public static boolean isNight(Context context)" in ui:
        errors.append(f"{UI}: 不应保留 isNight 转发方法")

    for path in HOME.glob("*.java"):
        source = path.read_text(encoding="utf-8")
        if "Ui.isNight(" in source:
            errors.append(f"{path}: 应直接调用 DimensionPolicy.isNight")

    for name in EXPECTED_CALLERS:
        path = HOME / name
        if "DimensionPolicy.isNight(" not in path.read_text(encoding="utf-8"):
            errors.append(f"{path}: 未直接复用 DimensionPolicy.isNight")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android home components use the shared night-mode policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
