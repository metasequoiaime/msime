#!/usr/bin/env python3
"""Guard Harmony feedback writes against leaking handles on write errors."""

from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    source = (root / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets").read_text()
    required = {
        "private feedback writes use an atomic sibling":
            "private static writePrivateText(path: string, text: string): boolean" in source
            and "util.generateRandomUUID(false)" in source
            and "fs.renameSync(staging, path)" in source,
        "feedback callers use the atomic writer":
            source.count("KeyboardSession.writePrivateText(") >= 2,
    }
    problems = [name for name, present in required.items() if not present]
    if problems:
        for problem in problems:
            print(f"missing Harmony feedback lifecycle guard: {problem}")
        return 1
    print("harmony feedback lifecycle: write failures still close file handles")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
