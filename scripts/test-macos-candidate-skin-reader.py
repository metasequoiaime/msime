#!/usr/bin/env python3
"""The macOS candidate skin stylesheet reader must reject symlinked files."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/macos/src/candidate/CandidateSkin.cpp"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("void ApplyToolbarStylesheet(")
    end = source.index("SkinTokens ToolbarSkinTokens(", start)
    region = source[start:end]
    required = ("O_NOFOLLOW", "O_CLOEXEC", "O_NONBLOCK", "::open(", "::read(", "::close(")
    missing = [token for token in required if token not in region]
    if "std::ifstream" in region:
        missing.append("path ifstream")
    if missing:
        print(f"{SOURCE}: stylesheet reader missing {', '.join(missing)}", file=sys.stderr)
        return 1
    print("macOS candidate stylesheet reader rejects symlinks")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
