#!/usr/bin/env python3
"""Android 固定尺寸圆角 drawable 复用公共工厂。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT / "platforms/android/java/app/msime/android"
POLICY = JAVA / "DrawablePolicy.java"
SLIDER = JAVA / "home/MsSlider.java"


def main() -> int:
    errors = []
    policy = POLICY.read_text(encoding="utf-8")
    slider = SLIDER.read_text(encoding="utf-8")

    if "rounded(int color, float radiusPx, int width, int height)" not in policy:
        errors.append("DrawablePolicy 缺少固定尺寸圆角工厂")
    if "GradientDrawable thumb = DrawablePolicy.rounded(" not in slider:
        errors.append(f"{SLIDER}: 滑块未复用固定尺寸圆角工厂")
    if "thumb.setSize(" in slider:
        errors.append(f"{SLIDER}: 滑块仍在调用点重复设置 drawable 尺寸")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android sized rounded drawables use the shared factory")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
