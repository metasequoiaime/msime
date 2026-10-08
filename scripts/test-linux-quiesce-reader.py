#!/usr/bin/env python3
"""Quiesce lease readers must not block on special files or follow links."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/common/DictionaryQuiesceLease.h"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    regions = []
    for name, end in (
        ("dictionary_quiesced", "inline bool write_staged_dictionary_lease("),
        ("lower_dictionary_quiesce_lease", "}  // namespace msime::dictionary_lease"),
    ):
        start = source.index(f"inline", source.index(name) - 20)
        regions.append((name, source[start : source.index(end, start)]))
    required = ("O_NOFOLLOW", "O_NONBLOCK", "fstat", "S_ISREG")
    missing = []
    for name, region in regions:
        missing.extend(f"{name}: {token}" for token in required if token not in region)
        if "std::ifstream" in region:
            missing.append(f"{name}: path ifstream")
    if missing:
        print(f"{SOURCE}: lease reader missing {', '.join(missing)}", file=sys.stderr)
        return 1
    print("Linux quiesce lease readers reject links and special files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
