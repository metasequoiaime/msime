#!/usr/bin/env python3
"""Android Context 沿包装链查找 Activity 时复用共享策略。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
ANDROID = ROOT / "platforms/android/java/app/msime/android"
POLICY = ANDROID / "ContextPolicy.java"
HOME = ANDROID / "home"
UI = HOME / "Ui.java"
CALLERS = {
    HOME / "MsToast.java": "ContextPolicy.activity(context)",
    HOME / "SettingsNavigator.java": "ContextPolicy.activity(context)",
}


def main() -> int:
    errors = []
    if not POLICY.is_file():
        errors.append(f"{POLICY}: 缺少共享 Context 策略")
        policy = ""
    else:
        policy = POLICY.read_text(encoding="utf-8")
    for snippet in (
        "public static Activity activity(Context context)",
        "while (current instanceof ContextWrapper wrapper)",
        "current = wrapper.getBaseContext();",
    ):
        if snippet not in policy:
            errors.append(f"{POLICY}: 缺少 {snippet}")

    ui = UI.read_text(encoding="utf-8")
    if "activityOf(" in ui:
        errors.append(f"{UI}: 不应保留 Activity 查找转发方法")

    for path, call in CALLERS.items():
        source = path.read_text(encoding="utf-8")
        if call not in source:
            errors.append(f"{path}: 未直接复用 ContextPolicy")

    for path in ANDROID.rglob("*.java"):
        source = path.read_text(encoding="utf-8")
        if path != POLICY and "getBaseContext()" in source:
            errors.append(f"{path}: Context 包装链应由 ContextPolicy 解包")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android activity lookup uses the shared context policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
