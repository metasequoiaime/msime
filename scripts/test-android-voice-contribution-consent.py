#!/usr/bin/env python3
"""Queued Android voice contributions must honor consent revocation before upload."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/core/ImeVoiceEntry.java"


def main() -> None:
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("    private void contribute(")
    end = source.index("    /** 把键区的内容换成聆听面板", start)
    body = source[start:end]
    worker = body[body.index("worker.execute") :]
    if "AndroidLocalSettings.load(s).bool(AndroidLocalSettings.VOICE_CONTRIBUTE_AUDIO)" not in worker:
        raise SystemExit("queued voice contribution must re-read the local consent switch before upload")
    print("Android queued voice contributions honor consent revocation")


if __name__ == "__main__":
    main()
