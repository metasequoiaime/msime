#!/usr/bin/env python3
"""鸿蒙私有文本读取必须通过同一个描述符完成校验与读取。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SESSION = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets"
STAGED = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/StagedResources.ets"


def main() -> int:
    session = SESSION.read_text(encoding="utf-8")
    staged = STAGED.read_text(encoding="utf-8")
    staged_region = staged[staged.index("  private static readMarker(") : staged.index("\n  /** Remove a staged tree", staged.index("  private static readMarker("))]
    helper_marker = "  private static readPrivateText("
    helper_start = session.find(helper_marker)
    helper_end = session.find("\n  /** Generates bounded reply candidates", helper_start)
    helper = session[helper_start:helper_end] if helper_start >= 0 and helper_end >= 0 else ""
    required_helper = (
        "safePrivateFile(path, maximumBytes)",
        "fs.openSync",
        "fs.statSync(handle.fd)",
        "stat.ino !== expected.ino",
        "fs.readSync",
        "NativeCleanup.close(handle)",
        "isFile",
    )
    missing = [token for token in required_helper if token not in helper]
    if not helper:
        missing.append("readPrivateText helper")
    for label, region in (("staged marker", staged_region),):
        for token in ("fs.openSync", "fs.statSync(handle.fd)", "fs.readSync", "NativeCleanup.close(handle)"):
            if token not in region:
                missing.append(f"{label}: {token}")
        for token in ("fs.lstatSync", "stat.ino !== expected.ino"):
            if token not in region:
                missing.append(f"{label}: {token}")
    # 账号会话和它的锁文件改由 Rust 读写（#8201 第 2 步），ArkTS 不再读会话文件，这里不再守它。
    if "fs.readTextSync(marker)" in staged_region:
        missing.append("staged marker path read")
    for call in (
        "return KeyboardFeedback.parse(fs.readTextSync(path));",
        "return KeyboardFeedback.parse(fs.readTextSync(this.feedbackFile()));",
        "return EmojiCatalogModel.parseRecents(fs.readTextSync(file));",
        "return CommunityReplyLibraryPolicy.parse(fs.readTextSync(file));",
    ):
        if call in session:
            missing.append(call)
    if missing:
        print("Harmony private text readers missing " + ", ".join(missing), file=sys.stderr)
        return 1
    print("Harmony private text readers use descriptor validation")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
