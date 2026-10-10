#!/usr/bin/env python3
"""Android home 页面直接使用共享圆角 drawable 策略。"""
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "platforms/android/java/app/msime/android/home/Ui.java"
JAVA_ROOT = ROOT / "platforms/android/java"


def main() -> int:
    ok = True
    ui = UI.read_text(encoding="utf-8")
    if "GradientDrawable rounded(" in ui:
        print(f"{UI}: 仍保留 Ui.rounded 转发方法", file=sys.stderr)
        ok = False
    if re.search(r"(?<![.\w])rounded\(", ui):
        print(f"{UI}: 仍调用已删除的 rounded 方法", file=sys.stderr)
        ok = False
    for path in JAVA_ROOT.rglob("*.java"):
        source = path.read_text(encoding="utf-8")
        if "Ui.rounded(" in source:
            print(f"{path}: 仍调用已删除的 Ui.rounded", file=sys.stderr)
            ok = False
    if ok:
        print("Android home drawables use the shared rounded policy directly")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
