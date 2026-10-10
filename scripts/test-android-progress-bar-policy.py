#!/usr/bin/env python3
"""Android 不确定进度条通过共享策略应用单色着色。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
ANDROID = ROOT / "platforms/android/java/app/msime/android"
POLICY = ANDROID / "ProgressBarPolicy.java"
IME_PANELS = ANDROID / "core/ImePanels.java"
AI_SKIN = ANDROID / "home/AiSkinPage.java"


def main() -> int:
    errors = []
    if not POLICY.is_file():
        errors.append(f"{POLICY}: 缺少共享进度条策略")
        policy = ""
    else:
        policy = POLICY.read_text(encoding="utf-8")
    for snippet in (
        "public static void setIndeterminateTint(ProgressBar view, int color)",
        "view.setIndeterminateTintList(ColorStateList.valueOf(color));",
    ):
        if snippet not in policy:
            errors.append(f"{POLICY}: 缺少 {snippet}")

    for path, call in (
        (IME_PANELS, "ProgressBarPolicy.setIndeterminateTint(s.replyProgress, accent);"),
        (AI_SKIN, "ProgressBarPolicy.setIndeterminateTint(spinner, ThemeColorPolicy.accent(context));"),
    ):
        source = path.read_text(encoding="utf-8")
        if call not in source:
            errors.append(f"{path}: 未直接复用 ProgressBarPolicy")

    for path in ANDROID.rglob("*.java"):
        source = path.read_text(encoding="utf-8")
        if path != POLICY and "setIndeterminateTintList(ColorStateList.valueOf" in source:
            errors.append(f"{path}: 应复用 ProgressBarPolicy.setIndeterminateTint")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android indeterminate progress bars use the shared tint policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
