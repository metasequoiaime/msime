#!/usr/bin/env python3
"""Android 波纹 drawable 通过共享工厂包装颜色状态列表。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
ANDROID = ROOT / "platforms/android/java/app/msime/android"
POLICY = ANDROID / "DrawablePolicy.java"
UI = ANDROID / "home/Ui.java"
LOGIN = ANDROID / "home/LoginSheet.java"


def main() -> int:
    errors = []
    policy = POLICY.read_text(encoding="utf-8")
    for snippet in (
        "public static RippleDrawable ripple(int color, Drawable content, Drawable mask)",
        "return new RippleDrawable(ColorStateList.valueOf(color), content, mask);",
    ):
        if snippet not in policy:
            errors.append(f"{POLICY}: 缺少 {snippet}")

    for path, call in (
        (UI, "DrawablePolicy.ripple(pressed, DrawablePolicy.rounded(fill, radiusPx),"),
        (LOGIN, "DrawablePolicy.ripple(pressed, face, mask)"),
    ):
        source = path.read_text(encoding="utf-8")
        if call not in source:
            errors.append(f"{path}: 未直接复用 DrawablePolicy.ripple")

    for path in ANDROID.rglob("*.java"):
        source = path.read_text(encoding="utf-8")
        if path != POLICY and "new RippleDrawable(" in source:
            errors.append(f"{path}: 应复用 DrawablePolicy.ripple")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android ripple drawables use the shared factory")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
