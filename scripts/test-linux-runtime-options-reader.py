#!/usr/bin/env python3
"""Linux runtime options must reject special files without blocking."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/linux/src/core/RuntimeOptionsFile.h"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("inline std::string read_runtime_options(")
    end = source.index("\n}\n\n} // namespace", start)
    region = source[start:end]
    required = ("O_NOFOLLOW", "O_CLOEXEC", "O_NONBLOCK", "fstat", "S_ISREG", "::open(", "::read(")
    missing = [token for token in required if token not in region]
    if missing:
        print(f"{SOURCE}: runtime options reader missing {', '.join(missing)}", file=sys.stderr)
        return 1
    print("Linux runtime options reader rejects special files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
