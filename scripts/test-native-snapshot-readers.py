#!/usr/bin/env python3
"""Native snapshot readers must accept only regular private files."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
# Android 的 JNI 胶水从 #8201 迁移计划 5f 起不再自己读快照文件（准备和激活整份快照都在 Rust 的 msime_client_snapshot_queue 里），只剩鸿蒙的 N-API 胶水。
SOURCES = (
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
