#!/usr/bin/env python3
"""Android 命名后台线程复用统一创建策略。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT / "platforms/android/java/app/msime/android"
SITES = {
    JAVA / "AccountTaskExecutor.java": ("msime-account",),
    JAVA / "account/SkinJobsApi.java": ("msime-ai-skin",),
    JAVA / "candidate/OnlineCandidateTransport.java": ("msime-cloud-deadline",),
    JAVA / "core/ImeDebugOverlay.java": ("msime-input-events",),
    JAVA / "core/ImeKeyFeedback.java": ("msime-key-sound",),
    JAVA / "core/Telemetry.java": ("msime-telemetry",),
    JAVA / "home/AboutPage.java": ("msime-home-network",),
    JAVA / "home/HostTask.java": ("msime-settings-host", "msime-settings-network"),
    JAVA / "home/OnboardingChoices.java": ("msime-onboarding-choices",),
    JAVA / "voice/AiPolishClient.java": ("msime-ai-polish",),
    JAVA / "voice/LocalAsrRecognizer.java": ("msime-local-asr-release",),
}
DIRECT_SITES = {
    JAVA / "home/AiSkinPage.java": ("msime-ai-skin-generate",),
}
STARTED_SITES = {
    JAVA / "account/BackendAccount.java": ("msime-chat-cancel",),
    JAVA / "home/AuthRedirectActivity.java": ("msime-apple-sign-in",),
    JAVA / "home/LoginSheet.java": ("msime-login-sheet", "msime-email-code", "msime-email-login"),
    JAVA / "home/OnboardingActivity.java": ("msime-onboarding",),
}


def main() -> int:
    errors = []
    policy = (JAVA / "ThreadPolicy.java").read_text(encoding="utf-8")
    if "ThreadFactory namedDaemonFactory(String name)" not in policy:
        errors.append("ThreadPolicy 缺少命名守护线程工厂")

    for path, names in SITES.items():
        source = path.read_text(encoding="utf-8")
        for name in names:
            expected = f'ThreadPolicy.namedDaemonFactory("{name}")'
            if expected not in source:
                errors.append(f"{path}: {name} 未复用 ThreadPolicy")
        if "setDaemon(true)" in source:
            errors.append(f"{path}: 仍在重复配置守护线程")

    for path, names in DIRECT_SITES.items():
        source = path.read_text(encoding="utf-8")
        for name in names:
            expected = f'ThreadPolicy.namedDaemonThread("{name}",'
            if expected not in source:
                errors.append(f"{path}: {name} 未复用 ThreadPolicy")
        if "setDaemon(true)" in source:
            errors.append(f"{path}: 仍在重复配置守护线程")

    for path, names in STARTED_SITES.items():
        source = path.read_text(encoding="utf-8")
        for name in names:
            expected = f'ThreadPolicy.startNamedThread("{name}",'
            if expected not in source:
                errors.append(f"{path}: {name} 未复用 ThreadPolicy")
        if "new Thread(" in source:
            errors.append(f"{path}: 仍在重复创建并启动命名线程")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android named background threads use the shared thread policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
