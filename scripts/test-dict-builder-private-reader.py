#!/usr/bin/env python3
"""词库构建器的私有输入必须拒绝特殊文件，且不能阻塞。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "crates/dict-builder/src/sources.rs"
READER = ROOT / "crates/dict-builder/src/private_file.rs"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("fn open_private(path: &Path)")
    end = source.index("\n}\n\nfn sha256_reader", start)
    region = source[start:end]
    if "crate::private_file::open_read_only(path," not in region:
        print(f"{SOURCE}: dictionary builder reader bypasses shared opener", file=sys.stderr)
        return 1
    reader = READER.read_text(encoding="utf-8")
    required = ("O_NOFOLLOW", "O_CLOEXEC", "O_NONBLOCK", "metadata", "is_file")
    missing = [token for token in required if token not in reader]
    if missing:
        print(f"{READER}: dictionary builder reader missing {', '.join(missing)}", file=sys.stderr)
        return 1
    print("Dictionary builder reader rejects special files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
