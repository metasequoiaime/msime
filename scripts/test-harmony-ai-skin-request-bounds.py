#!/usr/bin/env python3
"""Keep Harmony AI-skin request ids aligned with the shared mobile contract."""

from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    source = (root / "platforms/harmony/entry/src/main/ets/account/HarmonyAiSkins.ets").read_text(
        encoding="utf-8"
    )
    checks = {
        "uses the shared UTF-8 length helper": "import { utf8Length } from '../keyboard/Utf8';" in source,
        "uses the shared 96-byte ceiling": "utf8Length(value) <= 96" in source,
        "allows only shared identifier characters": "/^[0-9a-zA-Z_-]+$/" in source,
        "does not allow dots": "/^[0-9a-zA-Z._-]+$/" not in source,
    }
    missing = [name for name, present in checks.items() if not present]
    if missing:
        print("harmony AI-skin request bounds: missing " + ", ".join(missing))
        return 1
    print("harmony AI-skin request bounds: shared 96-byte ASCII identifiers")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
