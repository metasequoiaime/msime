#!/usr/bin/env python3
"""Keep Harmony resource replacement recursive and symlink-safe."""
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
STAGED = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/StagedResources.ets"
SETTINGS = ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets"


def main() -> int:
    staged = STAGED.read_text(encoding="utf-8")
    settings = SETTINGS.read_text(encoding="utf-8")
    required = [
        "static removeDirectory(directory: string): void",
        "fs.listFileSync(directory)",
        "fs.lstatSync(directory)",
        "fs.lstatSync(child).isDirectory()",
        "stat.isSymbolicLink()",
        "StagedResources.removeDirectory(previous)",
        "util.generateRandomUUID(false)",
        "fs.renameSync(staging, destination)",
    ]
    missing = [item for item in required if item not in staged and item not in settings]
    if missing or "fs.rmdirSync(destination)" in staged or "fs.rmdirSync(destination)" in settings:
        print("harmony resource staging: recursive replacement guard failed")
        return 1
    print("harmony resource staging: replacements remove non-empty trees")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
