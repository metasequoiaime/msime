#!/usr/bin/env python3
"""Harmony snapshot restore must copy the picker selection through one verified handle."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ACCOUNT = ROOT / "platforms/harmony/entry/src/main/ets/account/HarmonyAccountCloudBridge.ets"


def main() -> int:
    source = ACCOUNT.read_text(encoding="utf-8")
    start = source.index("  private async copyIn(")
    body = source[start : source.index("\n  private async restoreNative(", start)]
    private_start = source.index("  private privateFile(")
    private_body = source[private_start : source.index("\n  private static removeFile(", private_start)]
    preview_start = source.index("  private async restorePreview(")
    preview = source[preview_start : source.index("\n  private async copyIn(", preview_start)]
    required = {
        "picker source is opened once": "fs.openSync(source, fs.OpenMode.READ_ONLY)" in body,
        "picker source path is bound to descriptor": (
            "const sourcePathStat: fs.Stat = fs.lstatSync(source);" in body
            and "const sourceFdStat: fs.Stat = fs.statSync(sourceFile.fd);" in body
            and "sourceFdStat.ino !== sourcePathStat.ino" in body
        ),
        "copy uses the verified descriptor": "await fs.copyFile(sourceFile.fd, destination);" in body,
        "picker source descriptor is closed": "fs.closeSync(sourceFile);" in body,
        # 私有中转目录（快照导出、词库导出、恢复副本）与旧的词库导出一样拒绝符号链接。
        "private transfer directories reject symlinks": (
            "if (!fs.accessSync(directory)) fs.mkdirSync(directory, true);" in private_body
            and "if (!this.safeDirectory(directory)) return null;" in private_body
        ),
        "restore copies before handing Rust a path": (
            "await this.copyIn(selected[0], copy)" in preview
            and preview.index("await this.copyIn(selected[0], copy)")
            < preview.index("snapshot_restore_preview_file")
        ),
    }
    missing = [name for name, present in required.items() if not present]
    if missing:
        print("harmony cloud source binding: missing " + ", ".join(missing))
        return 1
    print("harmony snapshot restore binds the picker source before copying")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
