#!/usr/bin/env python3
"""Linux clipboard history reads must reject special files without blocking."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/linux/src/clipboard/ClipboardAtomicWrite.h"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("inline std::optional<std::string> read_clipboard_file(")
    end = source.index("\n}\n\ninline bool write_clipboard_file_atomically", start)
    region = source[start:end]
    required = ("O_NOFOLLOW", "O_NONBLOCK", "fstat", "S_ISREG", "::open(", "::read(")
    missing = [token for token in required if token not in region]
    if missing:
        print(f"{SOURCE}: clipboard reader missing {', '.join(missing)}", file=sys.stderr)
        return 1
    print("Linux clipboard reader rejects special files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
