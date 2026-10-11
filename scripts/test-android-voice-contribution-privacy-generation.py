#!/usr/bin/env python3
"""Pending Android voice contributions must be fenced when privacy mode restarts the session."""

from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
VOICE = ROOT / "platforms/android/java/app/msime/android/core/ImeVoiceEntry.java"
SERVICE = ROOT / "platforms/android/java/app/msime/android/core/MSIMEInputService.java"


def method(source: str, signature: str, end_marker: str) -> str:
    start = source.index(signature)
    end = source.index(end_marker, start)
    return source[start:end]


def main() -> None:
    voice = VOICE.read_text(encoding="utf-8")
    service = SERVICE.read_text(encoding="utf-8")
    contribution = method(voice, "    private void contribute(", "    /** 把键区的内容换成聆听面板")
    restart = method(service, "    private void restartSessionForPrivacy()", "    /** 功能面板的手写" )

    if not re.search(r"long\s+lifecycleGeneration\s*=\s*s\.inputLifecycleGeneration\(\)", contribution):
        raise SystemExit("voice contribution must capture the input-service generation")
    worker_guard = contribution[contribution.index("worker.execute") : contribution.index("new VoiceContributionApi", contribution.index("worker.execute"))]
    if not re.search(r"if\s*\(", worker_guard) or any(
        required not in worker_guard
        for required in (
            "lifecycleGeneration != s.inputLifecycleGeneration()",
            "!s.imePrivacyGate.contributesVoice()",
        )
    ):
        raise SystemExit("queued voice contribution must re-check generation and privacy before upload")
    stop = restart.index("stop(false);")
    if "++engineStartGeneration" not in restart[:stop]:
        raise SystemExit("privacy restart must invalidate queued contributions before stopping the session")
    print("Android pending voice contributions are fenced by privacy restart generation")


if __name__ == "__main__":
    main()
