#!/usr/bin/env python3
"""Keep Harmony AI-skin model ids aligned with the shared 200-byte bound."""

from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    source = (root / "platforms/harmony/entry/src/main/ets/account/HarmonyAiSkins.ets").read_text(
        encoding="utf-8"
    )
    checks = {
        "uses the UTF-8 model bound": "utf8Length(model) > 200" in source,
        "does not use a UTF-16 model bound": "model.length > 200" not in source,
    }
    missing = [name for name, present in checks.items() if not present]
    if missing:
        print("harmony AI-skin model bounds: missing " + ", ".join(missing))
        return 1
    print("harmony AI-skin model bounds: shared 200-byte model ids")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
