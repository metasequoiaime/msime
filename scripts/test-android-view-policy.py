#!/usr/bin/env python3
"""检查 Android 递归控件启用策略是否集中在共享 ViewPolicy。"""
from pathlib import Path
import re


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
        "public static Drawable ripple(Context context, int fill, float radiusPx)",
        "public static Drawable pillRipple(Context context, int fill)",
        "public static void style(TextView view, int sizeSp, int weight, int color)",
        "public static TextView singleLineLabel(Context context, CharSequence text, int sizeSp,",
        "public static TextView centeredLabel(Context context, CharSequence text, float sizeSp,",
        "public static TextView headingLabel(Context context, CharSequence text, float sizeSp,",
        "public static TextView centeredSingleLineLabel(Context context, CharSequence text, int sizeSp,",
        "public static TextView liveStatus(Context context, int sizeSp, int color)",
        "public static EditText styledInput(Context context, int sizeSp, int weight, int color)",
        "public static TextView styledLabel(Context context, CharSequence text, int sizeSp,",
        "public static TextView textButton(Context context, CharSequence label, int sizeSp, int weight,",
        "public static TextView pillButton(Context context, CharSequence label, float sizeSp, int weight,",
        "public static TextView trailingValue(Context context, CharSequence text, int sizeSp, int color)",
        "public static void hideFromAccessibility(View view)",
        "public static void setSymmetricPaddingDp(Context context, View view, float horizontalDp,",
        "public static void setHorizontalPaddingDp(Context context, View view, float horizontalDp)",
        "public static void setPaddingDp(Context context, View view, float leftDp, float topDp,",
        "public static TextView label(Context context, CharSequence text, float sizeSp, int color)",
        "public static void afterTextChanged(TextView view, Consumer<Editable> listener)",
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
    if "ViewPolicy.setTextMinHeight(view, DimensionPolicy.pixels(context, heightDp));" not in ui:
        raise AssertionError("Ui 没有调用共享文本最小高度策略")
    if "public static void setTextMinWidthDp(" in ui:
        raise AssertionError("Ui 仍保留文本最小宽度转发方法")
    if "setTextMinWidth(button, DimensionPolicy.pixels(context, minWidthDp));" not in view_policy:
        raise AssertionError("ViewPolicy 胶囊按钮没有调用共享文本最小宽度策略")
    if "public static void setEnabledLook(" in ui:
        raise AssertionError("Ui 仍保留无调用方的启用状态转发方法")
    if "public static void afterTextChanged(TextView view, Consumer<Editable> listener)" in ui:
        raise AssertionError("Ui 仍保留 afterTextChanged 转发方法")
    if "public static TextView label(Context context, CharSequence text, float sizeSp, int color)" in ui:
        raise AssertionError("Ui 仍保留 label 转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.label(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享 label 工厂")
    if "public static void style(TextView view, int sizeSp, int weight, int color)" in ui:
        raise AssertionError("Ui 仍保留文本样式转发方法")
    if re.search(r"(?<![.\w])style\(", ui):
        raise AssertionError("Ui 仍调用已移除的文本样式方法")
    if ui.count("ViewPolicy.style(") != 0:
        raise AssertionError("Ui 文本工厂没有直接调用共享样式方法")
    if "public static TextView styledLabel(" in ui:
        raise AssertionError("Ui 仍保留 styledLabel 转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.styledLabel(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享样式标签工厂")
    if "public static TextView textButton(" in ui:
        raise AssertionError("Ui 仍保留 textButton 转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.textButton(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享文本按钮工厂")
    if "public static TextView pillButton(" in ui:
        raise AssertionError("Ui 仍保留 pillButton 转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.pillButton(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享胶囊按钮工厂")
    if "public static TextView trailingValue(" in ui:
        raise AssertionError("Ui 仍保留 trailingValue 转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.trailingValue(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享尾值工厂")
    if "public static void hideFromAccessibility(" in ui:
        raise AssertionError("Ui 仍保留无障碍隐藏转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.hideFromAccessibility(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享无障碍隐藏策略")
    if "public static void setSymmetricPaddingDp(" in ui:
        raise AssertionError("Ui 仍保留对称 dp 内边距转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.setSymmetricPaddingDp(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享对称 dp 内边距策略")
    if "public static void setSymmetricPaddingPx(" in ui:
        raise AssertionError("Ui 仍保留对称像素内边距转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.setSymmetricPaddingPx(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享对称像素内边距策略")
    if "public static void setHorizontalPaddingDp(" in ui:
        raise AssertionError("Ui 仍保留水平 dp 内边距转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.setHorizontalPaddingDp(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享水平 dp 内边距策略")
    if "public static void setPaddingDp(" in ui:
        raise AssertionError("Ui 仍保留四边 dp 内边距转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.setPaddingDp(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享四边 dp 内边距策略")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.style(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享文本样式策略")
    if "public static TextView singleLineLabel(" in ui:
        raise AssertionError("Ui 仍保留 singleLineLabel 转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.singleLineLabel(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享单行标签工厂")
    if "public static TextView centeredLabel(" in ui:
        raise AssertionError("Ui 仍保留 centeredLabel 转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.centeredLabel(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享居中标签工厂")
    if "public static TextView headingLabel(" in ui:
        raise AssertionError("Ui 仍保留 headingLabel 转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.headingLabel(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享标题标签工厂")
    if "public static TextView centeredSingleLineLabel(" in ui:
        raise AssertionError("Ui 仍保留 centeredSingleLineLabel 转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.centeredSingleLineLabel(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享居中单行标签工厂")
    if "public static TextView liveStatus(" in ui:
        raise AssertionError("Ui 仍保留 liveStatus 转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.liveStatus(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享 live status 工厂")
    if "public static EditText styledInput(" in ui:
        raise AssertionError("Ui 仍保留 styledInput 转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.styledInput(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享编辑框工厂")
    if "public static TextView groupHeading(" in ui:
        raise AssertionError("Ui 仍保留 groupHeading 转发方法")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.groupHeading(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享标题工厂")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.afterTextChanged(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享文字变更策略")
    if "public static void makeClickable(" in ui:
        raise AssertionError("Ui 仍保留 makeClickable 转发方法")
    for path in HOME.glob("*.java"):
        if "Ui.makeClickable(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享点击策略")
    if "public static void setBottomPadding(" in ui:
        raise AssertionError("Ui 仍保留底部内边距转发方法")
    for path in HOME.glob("*.java"):
        if "Ui.setBottomPadding(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享底部内边距策略")
    if "public static void makeClickable(View view, Context context, Runnable action)" not in view_policy:
        raise AssertionError("ViewPolicy 缺少共享点击策略")
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
    if "public static Drawable pillRipple(" in ui or "public static Drawable rippleOn(" in ui:
        raise AssertionError("Ui 仍保留主题 ripple 转发方法")
    if re.search(r"(?<![.\w])pillRipple\(", ui):
        raise AssertionError("Ui 仍调用已移除的胶囊 ripple 方法")
    if "setBackground(button, pillRipple(context, fill));" not in view_policy:
        raise AssertionError("ViewPolicy 胶囊按钮没有调用共享 ripple 工厂")
    for path in ANDROID_JAVA.rglob("*.java"):
        if "Ui.pillRipple(" in path.read_text(encoding="utf-8") or "Ui.rippleOn(" in path.read_text(encoding="utf-8"):
            raise AssertionError(f"{path} 没有直接调用共享主题 ripple 工厂")
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
    if "public static LinearLayout row(Context context)" not in layout_policy:
        raise AssertionError("LayoutPolicy 缺少横向容器工厂")
    if "public static LinearLayout column(Context context)" not in layout_policy:
        raise AssertionError("LayoutPolicy 缺少纵向容器工厂")
    horizontal_factory = "LinearLayout view = new LinearLayout(context);\n        view.setOrientation(LinearLayout.HORIZONTAL);"
    if horizontal_factory in ui or horizontal_factory in keyboard_geometry:
        raise AssertionError("页面工具类仍保留重复的横向容器实现")
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
    image_policy = (ANDROID_JAVA / "app/msime/android/ImageViewPolicy.java").read_text(encoding="utf-8")
    if "ViewPolicy.newSquareParamsPx(" not in image_policy:
        raise AssertionError("ImageViewPolicy 没有调用共享正方形布局参数工厂")
    # isGone(null) 为假，`!isGone(x)` 不能代替判空；同一行接着改 x 的可见性却没判空，视图还没建好时就会空指针（6862a33265 在 ImeNineKeyPanel.dismiss 引入过）。
    unguarded = re.compile(
        r"!\s*ViewPolicy\.isGone\(([\w.]+)\)\)\s*\{?\s*ViewPolicy\."
        r"(?:hide|show|setVisible|setVisibleIfChanged|setInvisible|setVisibilityForText)\(\1[,)]")
    for path in ANDROID_JAVA.rglob("*.java"):
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            match = unguarded.search(line)
            if match and f"{match.group(1)} != null" not in line:
                raise AssertionError(f"{path}:{number} 用 !ViewPolicy.isGone 代替了判空：{line.strip()}")
    nine_key_panel = (ANDROID_JAVA / "app/msime/android/core/ImeNineKeyPanel.java").read_text(encoding="utf-8")
    if "if (root != null && !ViewPolicy.isGone(root)) ViewPolicy.hide(root);" not in nine_key_panel:
        raise AssertionError("ImeNineKeyPanel.dismiss 没有在 root 建好前判空")
    print("android view policy: recursive enabled state is shared")


if __name__ == "__main__":
    main()
