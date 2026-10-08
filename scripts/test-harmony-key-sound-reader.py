#!/usr/bin/env python3
"""Harmony key sound input must reject special files without blocking."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/harmony/native/key_sound_render.cpp"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("bool readBounded(")
    end = source.index("\n}\n\nma_decoder_config", start)
    region = source[start:end]
    required = ("O_NOFOLLOW", "O_CLOEXEC", "O_NONBLOCK", "fstat", "S_ISREG", "::open(", "::read(")
    missing = [token for token in required if token not in region]
    if missing:
        print(f"{SOURCE}: key sound reader missing {', '.join(missing)}", file=sys.stderr)
        return 1
    print("Harmony key sound reader rejects special files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
