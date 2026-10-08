#!/usr/bin/env python3
"""The Windows TSF word dictionary reader must use a bounded private handle."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/windows/msimeui/src/tsf/TextEditor.cpp"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("constexpr size_t kMaxWordDictionaryBytes")
    end = source.index("\nbool IsAsciiWordChar", start)
    region = source[start:end]
    required = (
        "kMaxWordDictionaryBytes",
        "CreateFileW",
        "FILE_FLAG_OPEN_REPARSE_POINT",
        "GetFileType",
        "FILE_TYPE_DISK",
        "ReadFile",
    )
    missing = [token for token in required if token not in region]
    if missing:
        print(f"{SOURCE}: dictionary reader missing {', '.join(missing)}", file=sys.stderr)
        return 1
    if "std::ifstream" in region:
        print(f"{SOURCE}: dictionary reader still uses std::ifstream", file=sys.stderr)
        return 1
    print("Windows TSF dictionary reader uses a bounded private handle")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
