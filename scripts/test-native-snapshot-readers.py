#!/usr/bin/env python3
"""Native snapshot readers must accept only regular private files."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCES = (
    ROOT / "platforms/android/native/client_jni.cpp",
    ROOT / "platforms/harmony/native/client_napi.cpp",
)


def main() -> int:
    ok = True
    for source in SOURCES:
        text = source.read_text(encoding="utf-8")
        start = text.index("struct SnapshotReader {")
        end = text.index("static intptr_t snapshotNext", start)
        region = text[start:end]
        if "O_NOFOLLOW" not in region:
            print(f"{source}: SnapshotReader 没有拒绝符号链接", file=sys.stderr)
            ok = False
        if "O_NONBLOCK" not in region:
            print(f"{source}: SnapshotReader 打开特殊文件时可能阻塞", file=sys.stderr)
            ok = False
        if "fstat" not in region or "S_ISREG" not in region:
            print(f"{source}: SnapshotReader 没有拒绝非普通文件", file=sys.stderr)
            ok = False
        if "std::ifstream" in region:
            print(f"{source}: SnapshotReader 仍使用路径 ifstream", file=sys.stderr)
            ok = False
    if ok:
        print("Native snapshot readers open files without following symlinks")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
