#!/usr/bin/env python3
"""Android 设置页直接复用共享颜色解析策略。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
UI = HOME / "Ui.java"
EXPECTED_CALLERS = (
    "AiSkinPage.java",
    "KeyboardOptionsPage.java",
    "KeyboardPreview.java",
    "SkinSwatchView.java",
)


def main() -> int:
    errors = []
    ui = UI.read_text(encoding="utf-8")
    if "public static int parseColor(String value, int fallback)" in ui:
        errors.append(f"{UI}: 不应保留 parseColor 转发方法")

    for path in HOME.glob("*.java"):
        source = path.read_text(encoding="utf-8")
        if "Ui.parseColor(" in source:
            errors.append(f"{path}: 应直接调用 ColorPolicy.parse")

    for name in EXPECTED_CALLERS:
        path = HOME / name
        if "ColorPolicy.parse(" not in path.read_text(encoding="utf-8"):
            errors.append(f"{path}: 未直接复用 ColorPolicy.parse")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android home pages use the shared color parsing policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
