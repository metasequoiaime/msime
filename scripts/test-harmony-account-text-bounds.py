#!/usr/bin/env python3
"""Keep Harmony account text validation on the shared UTF-8 byte contract."""

from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    source = (root / "platforms/harmony/entry/src/main/ets/account/AccountCloudBridge.ts").read_text(
        encoding="utf-8"
    )
    start = source.index("function validString(")
    end = source.index("\n}\n\nfunction validToken", start) + 2
    function = source[start:end]
    checks = {
        "uses the UTF-8 byte bound": "utf8Length(value) <= maximum" in function,
        "does not use a UTF-16 bound": "value.length <= maximum" not in function,
    }
    missing = [name for name, present in checks.items() if not present]
    if missing:
        print("harmony account text bounds: missing " + ", ".join(missing))
        return 1
    print("harmony account text bounds: shared UTF-8 limits")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
