#!/usr/bin/env python3
"""Dictionary builder private inputs must reject special files without blocking."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "crates/dict-builder/src/sources.rs"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("fn open_private(path: &Path)")
    end = source.index("\n}\n\nfn sha256_reader", start)
    region = source[start:end]
    required = ("O_NOFOLLOW", "O_CLOEXEC", "O_NONBLOCK", "metadata", "is_file")
    missing = [token for token in required if token not in region]
    if missing:
        print(f"{SOURCE}: dictionary builder reader missing {', '.join(missing)}", file=sys.stderr)
        return 1
    print("Dictionary builder reader rejects special files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
