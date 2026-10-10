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
    JAVA / "home/HomeActivity.java": ("msime-home-theme",),
    JAVA / "home/HostTask.java": ("msime-settings-host", "msime-settings-network"),
    JAVA / "home/OnboardingChoices.java": ("msime-onboarding-choices",),
    JAVA / "voice/AiPolishClient.java": ("msime-ai-polish",),
    JAVA / "voice/LocalAsrRecognizer.java": ("msime-local-asr-release",),
}
DIRECT_SITES = {
    JAVA / "home/AiSkinPage.java": ("msime-ai-skin-generate",),
}
STARTED_SITES = {
    JAVA / "account/AccountIdentity.java": ("msime-anonymous-account",),
    JAVA / "account/BackendAccount.java": ("msime-chat-cancel",),
    JAVA / "home/AuthRedirectActivity.java": ("msime-apple-sign-in",),
    JAVA / "home/LoginSheet.java": ("msime-login-sheet", "msime-email-code", "msime-email-login"),
    JAVA / "home/OnboardingActivity.java": ("msime-onboarding",),
    JAVA / "home/SignIn.java": ("msime-sign-in",),
}
STARTED_EXPRESSIONS = {
    JAVA / "core/ResourcePackService.java": (
        'ThreadPolicy.startNamedThread("msime-resource-pack-" + pack,',
    ),
}
UNSTARTED_SITES = {
    JAVA / "home/UpdateJobService.java": ("msime-update-job",),
    JAVA / "voice/LocalAsrRecognizer.java": ("msime-local-asr-capture",),
}
NON_DAEMON_SITES = {
    JAVA / "core/FirstRunPreparation.java": ("msime-first-run",),
    JAVA / "home/KeyboardTryoutActivity.java": ("msime-keyboard-tryout",),
}


def main() -> int:
    errors = []
    policy = (JAVA / "ThreadPolicy.java").read_text(encoding="utf-8")
    if "ThreadFactory namedDaemonFactory(String name)" not in policy:
        errors.append("ThreadPolicy 缺少命名守护线程工厂")
    if "ThreadFactory namedFactory(String name)" not in policy:
        errors.append("ThreadPolicy 缺少命名非守护线程工厂")
    if "Thread namedThread(String name, Runnable runnable)" not in policy:
        errors.append("ThreadPolicy 缺少命名线程创建方法")

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

    for path, expressions in STARTED_EXPRESSIONS.items():
        source = path.read_text(encoding="utf-8")
        for expression in expressions:
            if expression not in source:
                errors.append(f"{path}: 动态线程名未复用 ThreadPolicy")
        if "new Thread(" in source:
            errors.append(f"{path}: 仍在重复创建并启动命名线程")

    for path, names in UNSTARTED_SITES.items():
        source = path.read_text(encoding="utf-8")
        for name in names:
            expected = f'ThreadPolicy.namedThread("{name}",'
            if expected not in source:
                errors.append(f"{path}: {name} 未复用 ThreadPolicy")
        if "new Thread(" in source:
            errors.append(f"{path}: 仍在重复创建命名线程")

    for path, names in NON_DAEMON_SITES.items():
        source = path.read_text(encoding="utf-8")
        for name in names:
            expected = f'ThreadPolicy.namedFactory("{name}")'
            if expected not in source:
                errors.append(f"{path}: {name} 未复用 ThreadPolicy")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android named background threads use the shared thread policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
