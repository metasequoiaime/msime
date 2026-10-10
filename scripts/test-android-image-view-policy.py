#!/usr/bin/env python3
"""Android 图片控件通过共享策略应用单色着色。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
ANDROID = ROOT / "platforms/android/java/app/msime/android"
POLICY = ANDROID / "ImageViewPolicy.java"
HOME = ANDROID / "home"
UI = HOME / "Ui.java"
FEEDBACK = HOME / "FeedbackPage.java"


def main() -> int:
    errors = []
    if not POLICY.is_file():
        errors.append(f"{POLICY}: 缺少共享图片控件策略")
        policy = ""
    else:
        policy = POLICY.read_text(encoding="utf-8")
    for snippet in (
        "public static void setTint(ImageView view, @ColorInt int color)",
        "view.setImageTintList(ColorStateList.valueOf(color));",
    ):
        if snippet not in policy:
            errors.append(f"{POLICY}: 缺少 {snippet}")

    ui = UI.read_text(encoding="utf-8")
    if "public static void setImageTint(" in ui:
        errors.append(f"{UI}: 不应保留图片着色转发方法")
    if ui.count("ImageViewPolicy.setTint(") < 3:
        errors.append(f"{UI}: 图片组件未直接复用 ImageViewPolicy")

    feedback = FEEDBACK.read_text(encoding="utf-8")
    if "Ui.setImageTint(" in feedback:
        errors.append(f"{FEEDBACK}: 不应经过 Ui 转发图片着色")
    if "ImageViewPolicy.setTint(remove," not in feedback:
        errors.append(f"{FEEDBACK}: 删除按钮未直接复用 ImageViewPolicy")

    for path in ANDROID.rglob("*.java"):
        if path != POLICY and "setImageTintList(ColorStateList.valueOf" in path.read_text(encoding="utf-8"):
            errors.append(f"{path}: 应复用 ImageViewPolicy.setTint")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android image views use the shared tint policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
