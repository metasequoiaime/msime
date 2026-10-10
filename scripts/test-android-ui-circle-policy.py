#!/usr/bin/env python3
"""Android 头像和徽章直接使用共享圆形 drawable 策略。"""
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "platforms/android/java/app/msime/android/home/Ui.java"
PROFILE = ROOT / "platforms/android/java/app/msime/android/home/ProfilePage.java"


def main() -> int:
    ui = UI.read_text(encoding="utf-8")
    profile = PROFILE.read_text(encoding="utf-8")
    ok = True
    if "GradientDrawable circle(" in ui:
        print(f"{UI}: 仍保留 Ui.circle 转发方法", file=sys.stderr)
        ok = False
    if re.search(r"(?<![\w.])circle\(", ui):
        print(f"{UI}: 仍调用已删除的 Ui.circle", file=sys.stderr)
        ok = False
    if profile.count("DrawablePolicy.circle(") != 2:
        print(f"{PROFILE}: 圆形 drawable 没有全部直接调用 DrawablePolicy.circle", file=sys.stderr)
        ok = False
    if "Ui.circle(" in profile:
        print(f"{PROFILE}: 仍调用已删除的 Ui.circle", file=sys.stderr)
        ok = False
    if ok:
        print("Android profile drawables use the shared circle policy directly")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
