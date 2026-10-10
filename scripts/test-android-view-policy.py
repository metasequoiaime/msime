#!/usr/bin/env python3
"""检查 Android 递归控件启用策略是否集中在共享 ViewPolicy。"""
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VIEW_POLICY = ROOT / "platforms/android/java/app/msime/android/ViewPolicy.java"
LOGIN_SHEET = ROOT / "platforms/android/java/app/msime/android/home/LoginSheet.java"
INPUT_SERVICE = ROOT / "platforms/android/java/app/msime/android/core/MSIMEInputService.java"
BOTTOM_BAR = ROOT / "platforms/android/java/app/msime/android/core/ImeBottomBar.java"
UI = ROOT / "platforms/android/java/app/msime/android/home/Ui.java"
HOME = ROOT / "platforms/android/java/app/msime/android/home"
ANDROID_JAVA = ROOT / "platforms/android/java"
KEYBOARD_GEOMETRY = ROOT / "platforms/android/java/app/msime/android/keyboard/KeyboardGeometry.java"
LAYOUT_POLICY = ROOT / "platforms/android/java/app/msime/android/LayoutPolicy.java"


def main() -> None:
    view_policy = VIEW_POLICY.read_text(encoding="utf-8")
    login_sheet = LOGIN_SHEET.read_text(encoding="utf-8")
    input_service = INPUT_SERVICE.read_text(encoding="utf-8")
    bottom_bar = BOTTOM_BAR.read_text(encoding="utf-8")
    ui = UI.read_text(encoding="utf-8")
    keyboard_geometry = KEYBOARD_GEOMETRY.read_text(encoding="utf-8")
    layout_policy = LAYOUT_POLICY.read_text(encoding="utf-8")
    required = (
        "public static void setEnabledRecursively(ViewGroup group, boolean enabled, float inactiveAlpha)",
        "if (child instanceof ViewGroup nested && !child.isClickable())",
        "setEnabledRecursively(nested, enabled, inactiveAlpha);",
        "setEnabledWithAlpha(child, enabled, inactiveAlpha);",
        "public static void setFixedHeight(View view, int height)",
        "if (params == null || params.height == height) return;",
        "public static boolean isVisible(View view)",
        "return view != null && view.getVisibility() == View.VISIBLE;",
        "public static void setPaddingIfChanged(View view, int left, int top, int right, int bottom)",
        "if (view.getPaddingLeft() == left && view.getPaddingTop() == top",
        "public static void setBottomPadding(View view, int bottom)",
        "setPadding(view, view.getPaddingLeft(), view.getPaddingTop(), view.getPaddingRight(), bottom);",
        "public static void setVisibleIfChanged(View view, boolean visible)",
        "if (view.getVisibility() == visibility) return;",
        "public static LinearLayout newRow(Context context)",
        "view.setOrientation(LinearLayout.HORIZONTAL);",
        "public static LinearLayout newColumn(Context context)",
        "view.setOrientation(LinearLayout.VERTICAL);",
        "public static LinearLayout.LayoutParams newSquareParamsPx(int size)",
        "return new LinearLayout.LayoutParams(size, size);",
        "public static View newColorView(Context context, int color)",
        "setBackgroundColor(view, color);",
    )
    missing = [snippet for snippet in required if snippet not in view_policy]
    if missing:
        raise AssertionError("ViewPolicy 缺少递归启用策略：" + ", ".join(missing))
    if "private static void setEnabled(ViewGroup group, boolean enabled)" in login_sheet:
        raise AssertionError("LoginSheet 仍保留重复的递归启用实现")
    if "ViewPolicy.setEnabledRecursively(options, enabled, 0.6f);" not in login_sheet:
        raise AssertionError("LoginSheet 没有调用共享递归启用策略")
    if "private static void setFixedHeight(View view, int height)" in input_service:
        raise AssertionError("MSIMEInputService 仍保留重复的固定高度实现")
    if "ViewPolicy.setFixedHeight(candidateLine, pixels(line));" not in input_service:
        raise AssertionError("MSIMEInputService 没有调用共享固定高度策略")
    if "private static boolean shown(View view)" in input_service:
        raise AssertionError("MSIMEInputService 仍保留重复的可见性判断")
    if "ViewPolicy.isVisible(" not in input_service:
        raise AssertionError("MSIMEInputService 没有调用共享可见性策略")
    if "private static void setPadding(View view, int left, int top, int right, int bottom)" in bottom_bar:
        raise AssertionError("ImeBottomBar 仍保留重复的条件内边距实现")
    if "ViewPolicy.setPaddingIfChanged(keyboard, 0, 0, 0, 0);" not in bottom_bar:
        raise AssertionError("ImeBottomBar 没有调用共享条件内边距策略")
    if "if (bar != null && (bar.getVisibility() == View.VISIBLE) != shown) ViewPolicy.setVisible(bar, shown);" in bottom_bar:
        raise AssertionError("ImeBottomBar 仍保留重复的条件可见性实现")
    if "ViewPolicy.setVisibleIfChanged(bar, shown);" not in bottom_bar:
        raise AssertionError("ImeBottomBar 没有调用共享条件可见性策略")
    if "view.setMinHeight(dp(context, heightDp));" in ui:
        raise AssertionError("Ui 仍直接实现文本最小高度策略")
    if "ViewPolicy.setTextMinHeight(view, dp(context, heightDp));" not in ui:
        raise AssertionError("Ui 没有调用共享文本最小高度策略")
    if "public static void setTextMinWidthDp(" in ui:
        raise AssertionError("Ui 仍保留文本最小宽度转发方法")
    if "ViewPolicy.setTextMinWidth(button, dp(context, minWidthDp));" not in ui:
        raise AssertionError("Ui 按钮没有直接调用共享文本最小宽度策略")
    if "public static void setEnabledLook(" in ui:
        raise AssertionError("Ui 仍保留无调用方的启用状态转发方法")
    if "public static void setBottomPadding(" in ui:
        raise AssertionError("Ui 仍保留底部内边距转发方法")
    for path in HOME.glob("*.java"):
        if "Ui.setBottomPadding(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享底部内边距策略")
    if "ViewPolicy.setBottomPadding(target, bottom);" not in ui:
        raise AssertionError("Ui 页面避让监听没有调用共享底部内边距策略")
    for name in ("DetailPage.java", "KeyboardFragment.java"):
        source = (HOME / name).read_text(encoding="utf-8")
        if "Ui.bindPageBottomInsets(scroll);" not in source:
            raise AssertionError(f"{name} 没有复用页面底部避让监听")
        if "ViewPolicy.setBottomPadding(target, bottom);" in source:
            raise AssertionError(f"{name} 仍重复应用底部内边距")
    if "public static View hairlineView(" in ui:
        raise AssertionError("Ui 仍保留发丝线视图工厂")
    for path in HOME.glob("*.java"):
        if "Ui.hairlineView(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享着色视图工厂")
    if "ViewPolicy.newColorView(context, ThemeColorPolicy.hairline(context));" not in ui:
        raise AssertionError("Ui 分隔线没有调用共享着色视图工厂")
    feedback = (HOME / "FeedbackPage.java").read_text(encoding="utf-8")
    if "ViewPolicy.newColorView(context, ThemeColorPolicy.hairline(context));" not in feedback:
        raise AssertionError("FeedbackPage 没有调用共享着色视图工厂")
    if "public static void setHorizontalPaddingPx(" in ui:
        raise AssertionError("Ui 仍保留水平像素内边距转发方法")
    for path in HOME.glob("*.java"):
        if "Ui.setHorizontalPaddingPx(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享水平内边距策略")
    slider = (HOME / "MsSlider.java").read_text(encoding="utf-8")
    if "ViewPolicy.setHorizontalPadding(this, inset);" not in slider:
        raise AssertionError("MsSlider 没有直接调用共享水平内边距策略")
    if "return ViewPolicy.newRow(context);" not in ui:
        raise AssertionError("Ui 没有调用共享横向容器工厂")
    if "return ViewPolicy.newRow(context);" not in layout_policy:
        raise AssertionError("KeyboardGeometry 没有调用共享横向容器工厂")
    horizontal_factory = "LinearLayout view = new LinearLayout(context);\n        view.setOrientation(LinearLayout.HORIZONTAL);"
    if horizontal_factory in ui or horizontal_factory in keyboard_geometry:
        raise AssertionError("页面工具类仍保留重复的横向容器实现")
    if "return ViewPolicy.newColumn(context);" not in ui:
        raise AssertionError("Ui 没有调用共享纵向容器工厂")
    if "return ViewPolicy.newColumn(context);" not in layout_policy:
        raise AssertionError("KeyboardGeometry 没有调用共享纵向容器工厂")
    vertical_factory = "LinearLayout view = new LinearLayout(context);\n        view.setOrientation(LinearLayout.VERTICAL);"
    if vertical_factory in ui or vertical_factory in keyboard_geometry:
        raise AssertionError("页面工具类仍保留重复的纵向容器实现")
    square_forwarder = "public static LinearLayout.LayoutParams squareParamsPx(int size)"
    if square_forwarder in ui or square_forwarder in keyboard_geometry:
        raise AssertionError("页面工具类仍保留正方形布局参数转发方法")
    for path in HOME.glob("*.java"):
        source = path.read_text(encoding="utf-8")
        if "Ui.squareParamsPx(" in source:
            raise AssertionError(f"{path} 没有直接调用共享正方形布局参数工厂")
    for path in ANDROID_JAVA.rglob("*.java"):
        source = path.read_text(encoding="utf-8")
        if "KeyboardGeometry.squareParamsPx(" in source:
            raise AssertionError(f"{path} 没有直接调用共享正方形布局参数工厂")
    for name in ("AboutPage.java", "DownloadPage.java", "Ui.java"):
        source = (HOME / name).read_text(encoding="utf-8")
        if "ViewPolicy.newSquareParamsPx(" not in source:
            raise AssertionError(f"{name} 没有调用共享正方形布局参数工厂")
    print("android view policy: recursive enabled state is shared")


if __name__ == "__main__":
    main()
