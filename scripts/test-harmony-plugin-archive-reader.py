#!/usr/bin/env python3
"""Harmony archive imports must reject special paths and bind the picked file handle."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SETTINGS = ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets"


def main() -> int:
    source = SETTINGS.read_text(encoding="utf-8")
    start = source.index("      } else {\n        source = `${staging}/${PluginImportPolicy.ARCHIVE_NAME}`;")
    body = source[start : source.index("\n      }\n      const action: PluginImportAction", start)]
    required = {
        "archive path rejects special files": "const pathStat: fs.Stat = fs.lstatSync(picked[0]);" in body
        and "pathStat.isSymbolicLink() || !pathStat.isFile()" in body,
        "archive descriptor is bound to the checked path": "const opened: fs.Stat = fs.statSync(file.fd);" in body
        and "opened.ino !== pathStat.ino" in body,
        "archive copy uses the descriptor": "await fs.copyFile(file.fd, source);" in body,
    }
    missing = [name for name, present in required.items() if not present]
    if missing:
        print("harmony plugin archive reader: missing " + ", ".join(missing))
        return 1
    print("harmony plugin archive imports bind regular picker files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
