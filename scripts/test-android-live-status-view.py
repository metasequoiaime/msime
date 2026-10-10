#!/usr/bin/env python3
"""检查 Android 动态状态文本是否复用共享工厂。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
UI = HOME / "Ui.java"


def main() -> int:
    errors = []
    ui = UI.read_text(encoding="utf-8")
    required = (
        "public static TextView liveStatus(Context context, int sizeSp)",
        'TextView status = styledLabel(context, "", sizeSp, 400, subText(context));',
        "ViewPolicy.setPoliteLiveRegion(status);",
        "return status;",
    )
    for snippet in required:
        if snippet not in ui:
            errors.append(f"{UI}: 动态状态文本工厂缺少：{snippet}")

    consumers = {
        "SettingsSheet.java": "TextView status = Ui.liveStatus(context, 12);",
        "LoginSheet.java": "status = Ui.liveStatus(activity, 13);",
    }
    for name, call in consumers.items():
        path = HOME / name
        source = path.read_text(encoding="utf-8")
        if call not in source:
            errors.append(f"{path}: 未复用 Ui.liveStatus")
        if "ViewPolicy.setPoliteLiveRegion(status);" in source:
            errors.append(f"{path}: 不应重复设置动态状态文本的 live region")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android live status labels use the shared factory")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
