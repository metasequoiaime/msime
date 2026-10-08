#!/usr/bin/env python3
"""Harmony plugin folder imports must bind each picked file to its opened handle."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SETTINGS = ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets"


def main() -> int:
    source = SETTINGS.read_text(encoding="utf-8")
    start = source.index("  private async copyPluginFolder(")
    body = source[start : source.index("\n  /** The shared settings page", start)]
    required = {
        "source files are opened by descriptor":
            "fs.openSync(sourcePath, fs.OpenMode.READ_ONLY)" in body,
        "source path is checked against descriptor":
            "const pathStat: fs.Stat = fs.lstatSync(sourcePath);" in body
            and "const opened: fs.Stat = fs.statSync(file.fd);" in body
            and "opened.ino !== pathStat.ino" in body,
        "copy uses the verified descriptor": "await fs.copyFile(file.fd, destinationPath);" in body,
        "source descriptors are always closed": "fs.closeSync(file);" in body,
    }
    missing = [name for name, present in required.items() if not present]
    if missing:
        print("harmony plugin import reader: missing " + ", ".join(missing))
        return 1
    print("harmony plugin import reader binds files before copying")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
