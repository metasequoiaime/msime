#!/usr/bin/env python3
"""Candidate panel status comparisons must not open special files or follow links."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/linux/src/candidates/CandidatePanelStatus.h"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("inline bool write_candidate_panel_status(")
    end = source.index("} // namespace msime::linux_host", start)
    region = source[start:end]
    required = ("O_NOFOLLOW", "O_NONBLOCK", "fstat", "S_ISREG", "::open(", "::read(")
    missing = [token for token in required if token not in region]
    if "std::ifstream current" in region:
        missing.append("path ifstream")
    if missing:
        print(f"{SOURCE}: status reader missing {', '.join(missing)}", file=sys.stderr)
        return 1
    print("Linux candidate panel status reader rejects special files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
