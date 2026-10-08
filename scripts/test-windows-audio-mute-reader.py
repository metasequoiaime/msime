#!/usr/bin/env python3
"""The POSIX test fallback for Windows audio state must reject special files."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/windows/src/voice/AudioMuteState.h"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("inline bool read_audio_mute_state(")
    end = source.index("\n}\n\ninline bool write_audio_mute_state", start)
    region = source[start:end]
    required = ("O_NOFOLLOW", "O_NONBLOCK", "fstat", "S_ISREG", "::open(", "::read(")
    missing = [token for token in required if token not in region]
    if missing:
        print(f"{SOURCE}: audio state reader missing {', '.join(missing)}", file=sys.stderr)
        return 1
    print("Windows POSIX audio state reader rejects special files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
