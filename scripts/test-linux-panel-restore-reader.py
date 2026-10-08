#!/usr/bin/env python3
"""Panel restore records must be read from regular, non-symlinked files."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/linux/src/candidates/PanelRestoreRecord.h"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("inline std::optional<nlohmann::json> read_panel_restore(")
    end = source.index("// A relative XDG value", start)
    region = source[start:end]
    required = ("O_NOFOLLOW", "O_NONBLOCK", "fstat", "S_ISREG", "::open(", "::read(")
    missing = [token for token in required if token not in region]
    if "std::ifstream" in region:
        missing.append("path ifstream")
    if missing:
        print(f"{SOURCE}: restore reader missing {', '.join(missing)}", file=sys.stderr)
        return 1
    print("Linux panel restore reader rejects special files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
