#!/usr/bin/env python3
"""Android 设置组件直接复用共享几何换算并移除死转发。"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
UI = HOME / "Ui.java"
EXPECTED_CALLERS = (
    "AiSkinPage.java",
    "FeedbackPage.java",
    "InputDialog.java",
    "LoginSheet.java",
    "OnboardingActivity.java",
    "SkinsPage.java",
)
HEIGHT_PX_CALLERS = (
    "FeedbackPage.java",
    "OnboardingActivity.java",
    "SkinsPage.java",
)
BOTTOM_INSET_CALLERS = (
    "KeyboardTryoutActivity.java",
)
PAGE_BOTTOM_INSET_CALLERS = (
    "DetailPage.java",
    "KeyboardFragment.java",
)


def main() -> int:
    errors = []
    ui = UI.read_text(encoding="utf-8")
    if "public static int atLeastOnePx(Context context, float value)" in ui:
        errors.append(f"{UI}: 不应保留 atLeastOnePx 转发方法")
    if "public static float sp(Context context, float value)" in ui:
        errors.append(f"{UI}: 不应保留无调用方的 sp 转发方法")
    if "public static int hairlinePx(Context context)" in ui:
        errors.append(f"{UI}: 不应保留 hairlinePx 转发方法")
    if "public static int bottomContentInset(" in ui:
        errors.append(f"{UI}: 不应保留 bottomContentInset 转发方法")
    if "public static void bindPageBottomInsets(View view)" not in ui:
        errors.append(f"{UI}: 缺少页面底部避让监听绑定方法")
    if "WindowInsetsPolicy.bottomContentInset(" not in ui:
        errors.append(f"{UI}: 页面底部避让监听未复用 WindowInsetsPolicy")
    if "public static LinearLayout.LayoutParams matchWidthHeightPx(" in ui:
        errors.append(f"{UI}: 不应保留 matchWidthHeightPx 转发方法")
    if re.search(r"(?<![.\w])matchWidthHeightPx\(", ui):
        errors.append(f"{UI}: 不应调用已移除的 matchWidthHeightPx")
    if "public static LinearLayout.LayoutParams weightedWidth(" in ui:
        errors.append(f"{UI}: 不应保留 weightedWidth 转发方法")
    if re.search(r"(?<![.\w])weightedWidth\(", ui):
        errors.append(f"{UI}: 不应调用已移除的 weightedWidth")
    if "public static LinearLayout.LayoutParams weightedMatchParent(" in ui:
        errors.append(f"{UI}: 不应保留 weightedMatchParent 转发方法")
    if re.search(r"(?<![.\w])weightedMatchParent\(", ui):
        errors.append(f"{UI}: 不应调用已移除的 weightedMatchParent")
    if "public static LinearLayout.LayoutParams weightedHeight(" in ui:
        errors.append(f"{UI}: 不应保留 weightedHeight 转发方法")
    if re.search(r"(?<![.\w])weightedHeight\(", ui):
        errors.append(f"{UI}: 不应调用已移除的 weightedHeight")
    if "public static FrameLayout.LayoutParams frameWrap(" in ui:
        errors.append(f"{UI}: 不应保留 frameWrap 转发方法")
    if re.search(r"(?<![.\w])frameWrap\(", ui):
        errors.append(f"{UI}: 不应调用已移除的 frameWrap")
    if "public static LinearLayout.LayoutParams wrapHeight(" in ui:
        errors.append(f"{UI}: 不应保留 wrapHeight 转发方法")
    if re.search(r"(?<![.\w])wrapHeight\(", ui):
        errors.append(f"{UI}: 不应调用已移除的 wrapHeight")
    if "public static FrameLayout.LayoutParams frameMatchWidthHeight(" in ui:
        errors.append(f"{UI}: 不应保留 frameMatchWidthHeight 转发方法")
    if re.search(r"(?<![.\w])frameMatchWidthHeight\(", ui):
        errors.append(f"{UI}: 不应调用已移除的 frameMatchWidthHeight")

    for path in HOME.glob("*.java"):
        source = path.read_text(encoding="utf-8")
        if "import app.msime.android.keyboard.KeyboardGeometry;" in source:
            errors.append(f"{path}: KeyboardGeometry 的包名应为 app.msime.android")
        if "Ui.atLeastOnePx(" in source:
            errors.append(f"{path}: 应直接调用 DimensionPolicy.atLeastOnePixel")
        if "Ui.sp(" in source:
            errors.append(f"{path}: 不应调用已移除的 Ui.sp")
        if "Ui.hairlinePx(" in source:
            errors.append(f"{path}: 应直接调用 DimensionPolicy.atLeastOnePixel")
        if "Ui.bottomContentInset(" in source:
            errors.append(f"{path}: 应直接调用 WindowInsetsPolicy.bottomContentInset")
        if "Ui.matchWidthHeightPx(" in source:
            errors.append(f"{path}: 应直接调用 LayoutPolicy.matchWidthHeightPx")
        if "Ui.weightedWidth(" in source:
            errors.append(f"{path}: 应直接调用 LayoutPolicy.weightedWidthParams")
        if "Ui.weightedMatchParent(" in source:
            errors.append(f"{path}: 应直接调用 LayoutPolicy.weightedMatchParentParams")
        if "Ui.weightedHeight(" in source:
            errors.append(f"{path}: 应直接调用 LayoutPolicy.weightedHeightPxParams")
        if "Ui.frameWrap(" in source:
            errors.append(f"{path}: 应直接调用 LayoutPolicy.frameParamsPx")
        if "Ui.wrapHeight(" in source:
            errors.append(f"{path}: 应直接调用 LayoutPolicy.linearParamsPx")
        if "Ui.frameMatchWidthHeight(" in source:
            errors.append(f"{path}: 应直接调用 LayoutPolicy.frameMatchWidthHeightPx")

    for name in EXPECTED_CALLERS:
        path = HOME / name
        if "DimensionPolicy.atLeastOnePixel(" not in path.read_text(encoding="utf-8"):
            errors.append(f"{path}: 未直接复用 DimensionPolicy.atLeastOnePixel")

    for name in HEIGHT_PX_CALLERS:
        path = HOME / name
        if "LayoutPolicy.matchWidthHeightPx(" not in path.read_text(encoding="utf-8"):
            errors.append(f"{path}: 未直接复用 LayoutPolicy.matchWidthHeightPx")

    for name in BOTTOM_INSET_CALLERS:
        path = HOME / name
        if "WindowInsetsPolicy.bottomContentInset(" not in path.read_text(encoding="utf-8"):
            errors.append(f"{path}: 未直接复用 WindowInsetsPolicy.bottomContentInset")

    for name in PAGE_BOTTOM_INSET_CALLERS:
        path = HOME / name
        source = path.read_text(encoding="utf-8")
        if "Ui.bindPageBottomInsets(scroll);" not in source:
            errors.append(f"{path}: 未复用页面底部避让监听绑定方法")
        if "ViewCompat.setOnApplyWindowInsetsListener(" in source:
            errors.append(f"{path}: 不应重复绑定页面底部避让监听")

    option_sheet = HOME / "OptionSheet.java"
    if "LayoutPolicy.weightedWidthParams(" not in option_sheet.read_text(encoding="utf-8"):
        errors.append(f"{option_sheet}: 未直接复用 LayoutPolicy.weightedWidthParams")

    statistics = HOME / "StatisticsFragment.java"
    if statistics.read_text(encoding="utf-8").count(
        "LayoutPolicy.weightedMatchParentParams("
    ) != 2:
        errors.append(f"{statistics}: 未直接复用 LayoutPolicy.weightedMatchParentParams")

    input_dialog = HOME / "InputDialog.java"
    if input_dialog.read_text(encoding="utf-8").count(
        "LayoutPolicy.weightedHeightPxParams("
    ) != 2:
        errors.append(f"{input_dialog}: 未直接复用 LayoutPolicy.weightedHeightPxParams")

    for name in ("MsToast.java", "SheetOptionView.java"):
        path = HOME / name
        if path.read_text(encoding="utf-8").count("LayoutPolicy.frameParamsPx(") != 1:
            errors.append(f"{path}: 未直接复用 LayoutPolicy.frameParamsPx")

    for name in ("OnboardingActivity.java", "StatisticsFragment.java"):
        path = HOME / name
        if path.read_text(encoding="utf-8").count("LayoutPolicy.linearParamsPx(") != 1:
            errors.append(f"{path}: 未直接复用 LayoutPolicy.linearParamsPx")

    for name in ("AiSkinPage.java", "SkinsPage.java"):
        path = HOME / name
        if path.read_text(encoding="utf-8").count(
            "LayoutPolicy.frameMatchWidthHeightPx("
        ) != 1:
            errors.append(f"{path}: 未直接复用 LayoutPolicy.frameMatchWidthHeightPx")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android home components use shared geometry without forwarding methods")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
