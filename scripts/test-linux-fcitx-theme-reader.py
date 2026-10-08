#!/usr/bin/env python3
"""Fcitx theme replacement must not block on or follow a special existing file."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/linux/src/candidates/CandidateFcitxTheme.h"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("inline bool write_fcitx_theme(")
    end = source.index("// Copies of decoration images", start)
    region = source[start:end]
    required = ("O_NOFOLLOW", "O_NONBLOCK", "fstat", "S_ISREG", "::open(", "::read(")
    missing = [token for token in required if token not in region]
    if "std::ifstream current" in region:
        missing.append("path ifstream")
    if missing:
        print(f"{SOURCE}: theme reader missing {', '.join(missing)}", file=sys.stderr)
        return 1
    print("Linux Fcitx theme reader rejects special files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
