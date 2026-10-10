#!/usr/bin/env python3
"""检查 Android 选择面板是否复用共享标题组件。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
COMPONENT = HOME / "SheetHeaderView.java"
UI = HOME / "Ui.java"


def main() -> int:
    errors = []
    component = COMPONENT.read_text(encoding="utf-8") if COMPONENT.exists() else ""
    required = (
        "public final class SheetHeaderView",
        "public static LinearLayout create(Context context, CharSequence title,",
        "LinearLayout header = Ui.column(context);",
        "ViewPolicy.setCenteredHorizontally(header);",
        "ViewPolicy.setPadding(header, horizontal, 0, horizontal, Ui.dp(context, 12));",
        "TextView heading = Ui.headingLabel(context, title, Ui.TEXT_SHEET_HEADER, 600,",
        "if (subtitle != null && subtitle.length() > 0)",
        "TextView note = Ui.centeredLabel(context, subtitle, Ui.TEXT_SHEET_HEADER, 400,",
        "params.topMargin = Ui.dp(context, 2);",
    )
    for snippet in required:
        if snippet not in component:
            errors.append(f"{COMPONENT}: 缺少共享面板标题语义：{snippet}")
    if "new TextView(context)" in component:
        errors.append(f"{COMPONENT}: 标题文本应复用 Ui.styledLabel")

    ui = UI.read_text(encoding="utf-8")
    signatures = {
        "setSheetHeaderPadding": "public static void setSheetHeaderPadding(",
        "sheetHeading": "public static TextView sheetHeading(",
        "sheetSubtitle": "public static TextView sheetSubtitle(",
    }
    for method, signature in signatures.items():
        if signature in ui:
            errors.append(f"{UI}: 不应保留 {method} 方法")
        for path in HOME.glob("*.java"):
            if f"Ui.{method}(" in path.read_text(encoding="utf-8"):
                errors.append(f"{path}: 不应调用已移除的 Ui.{method}")

    option_sheet = (HOME / "OptionSheet.java").read_text(encoding="utf-8")
    if "LinearLayout header = SheetHeaderView.create(context, title, subtitle);" not in option_sheet:
        errors.append("OptionSheet 未复用共享面板标题组件")
    app_theme = (HOME / "AppThemeSheet.java").read_text(encoding="utf-8")
    expected = 'SheetHeaderView.create(context, "应用主题", "四季会随季节自动更换配色")'
    if expected not in app_theme:
        errors.append("AppThemeSheet 未复用共享面板标题组件")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android option sheets use the shared header component")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
