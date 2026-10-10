#!/usr/bin/env python3
"""验证键盘预览复用公共的有界透明度策略。"""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
COLOR = ROOT / "platforms/android/java/app/msime/android/ColorPolicy.java"
SKIN_PREVIEW = ROOT / "platforms/android/java/app/msime/android/keyboard/KeyboardSkinPreview.java"
KEYBOARD_PREVIEW = ROOT / "platforms/android/java/app/msime/android/home/KeyboardPreview.java"


def main() -> None:
    color = COLOR.read_text(encoding="utf-8")
    skin_preview = SKIN_PREVIEW.read_text(encoding="utf-8")
    keyboard_preview = KEYBOARD_PREVIEW.read_text(encoding="utf-8")
    errors = []
    if "withAlpha(int color, double alpha)" not in color:
        errors.append(f"{COLOR}: 缺少有界 double 透明度策略")
    if "private static int withOpacity(" in skin_preview:
        errors.append(f"{SKIN_PREVIEW}: 仍保留透明度转发方法")
    if skin_preview.count("ColorPolicy.withAlpha(") < 5:
        errors.append(f"{SKIN_PREVIEW}: 缩略图没有直接复用公共透明度策略")
    if "ColorPolicy.withAlpha(colour, skin.keyOpacity())" not in keyboard_preview:
        errors.append(f"{KEYBOARD_PREVIEW}: 应用预览没有复用有界透明度策略")
    if errors:
        for error in errors:
            print(error)
        raise SystemExit(1)
    print("Android keyboard previews use the shared bounded alpha policy")


if __name__ == "__main__":
    main()
