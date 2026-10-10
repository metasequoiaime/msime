#!/usr/bin/env python3
"""检查 Android 安装状态标记是否集中在独立策略中。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
STATUS_MARK = HOME / "StatusMarkPolicy.java"
UI = HOME / "Ui.java"
CALLERS = ("OnboardingActivity.java", "KeyboardFragment.java")


def main() -> int:
    errors = []
    policy = STATUS_MARK.read_text(encoding="utf-8") if STATUS_MARK.exists() else ""
    ui = UI.read_text(encoding="utf-8")
    required = (
        "public final class StatusMarkPolicy",
        "public static void apply(TextView mark, Context context, boolean done)",
        'mark.setText(done ? "✓" : "!");',
        "ViewPolicy.setTextColor(mark, done ? Ui.onAccent(context) : 0xFFFFFFFF);",
        "DrawablePolicy.circle(done ? Ui.accent(context) : Ui.color(context, R.attr.msWarn))",
        "ViewPolicy.hideFromAccessibility(mark);",
    )
    for snippet in required:
        if snippet not in policy:
            errors.append(f"{STATUS_MARK}: 缺少共享状态标记语义：{snippet}")
    if "public static void applyStatusMark(" in ui:
        errors.append(f"{UI}: 不应保留 applyStatusMark 方法")
    for name in CALLERS:
        path = HOME / name
        source = path.read_text(encoding="utf-8")
        if "Ui.applyStatusMark(" in source:
            errors.append(f"{path}: 不应调用已移除的 Ui.applyStatusMark")
        if "StatusMarkPolicy.apply(" not in source:
            errors.append(f"{path}: 未直接调用共享状态标记策略")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android setup status marks use the shared policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
