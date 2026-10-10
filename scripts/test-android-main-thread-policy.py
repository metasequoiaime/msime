#!/usr/bin/env python3
"""Android 主线程 Handler 复用统一创建策略。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT / "platforms/android/java/app/msime/android"
SITES = (
    JAVA / "core/FirstRunPreparation.java",
    JAVA / "core/ResourcePackService.java",
    JAVA / "home/AiSkinPage.java",
    JAVA / "home/HostTask.java",
    JAVA / "home/KeyboardTryoutActivity.java",
    JAVA / "home/LexiconDetailPage.java",
    JAVA / "home/OnboardingChoices.java",
    JAVA / "home/OnboardingActivity.java",
    JAVA / "home/SignIn.java",
)


def main() -> int:
    errors = []
    policy_path = JAVA / "MainThreadPolicy.java"
    policy = policy_path.read_text(encoding="utf-8") if policy_path.exists() else ""
    if "Handler mainHandler()" not in policy:
        errors.append("MainThreadPolicy 缺少主线程 Handler 创建方法")
    for path in SITES:
        source = path.read_text(encoding="utf-8")
        if "MainThreadPolicy.mainHandler()" not in source:
            errors.append(f"{path}: 未复用 MainThreadPolicy")
        if "new Handler(Looper.getMainLooper())" in source:
            errors.append(f"{path}: 仍在重复创建主线程 Handler")
        if "new android.os.Handler(android.os.Looper.getMainLooper())" in source:
            errors.append(f"{path}: 仍在重复创建主线程 Handler")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android main-thread handlers use the shared thread policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
