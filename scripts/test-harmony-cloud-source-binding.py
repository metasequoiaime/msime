#!/usr/bin/env python3
"""Harmony snapshot restore must copy the picker selection through one verified handle."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ACCOUNT = ROOT / "platforms/harmony/entry/src/main/ets/account/HarmonyAccountCloudBridge.ets"


def main() -> int:
    source = ACCOUNT.read_text(encoding="utf-8")
    start = source.index("  private async restorePreview(")
    body = source[start : source.index("\n  private async restoreNative(", start)]
    required = {
        "picker source is opened once": "fs.openSync(source, fs.OpenMode.READ_ONLY)" in body,
        "picker source path is bound to descriptor": (
            "const sourcePathStat: fs.Stat = fs.lstatSync(source);" in body
            and "const sourceFdStat: fs.Stat = fs.statSync(sourceFile.fd);" in body
            and "sourceFdStat.ino !== sourcePathStat.ino" in body
        ),
        "copy uses the verified descriptor": "await fs.copyFile(sourceFile.fd, this.restoreFile);" in body,
        "picker source descriptor is closed": "fs.closeSync(sourceFile);" in body,
    }
    missing = [name for name, present in required.items() if not present]
    if missing:
        print("harmony cloud source binding: missing " + ", ".join(missing))
        return 1
    print("harmony snapshot restore binds the picker source before copying")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
