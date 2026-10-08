#!/usr/bin/env python3
"""Fcitx candidate decoration reads must reject symlinked source files."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/linux/src/candidates/CandidateFcitxTheme.h"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("inline std::optional<FcitxThemeOverlay> stage_fcitx_overlay(")
    end = source.index("inline void remove_stale_fcitx_files(", start)
    region = source[start:end]
    required = ("O_NOFOLLOW", "O_CLOEXEC", "O_NONBLOCK", "::open(", "::read(", "::close(")
    missing = [token for token in required if token not in region]
    if "std::ifstream in(source" in region:
        missing.append("path ifstream")
    if missing:
        print(f"{SOURCE}: overlay reader missing {', '.join(missing)}", file=sys.stderr)
        return 1
    print("Linux Fcitx overlay reader rejects symlinks")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
