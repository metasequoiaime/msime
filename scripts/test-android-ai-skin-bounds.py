#!/usr/bin/env python3
"""Keep Android AI-skin response bounds aligned with client-core's byte limits."""

from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    source = (root / "platforms/android/java/app/msime/android/account/SkinJobsApi.java").read_text(
        encoding="utf-8"
    )
    checks = {
        "uses the UTF-8 model bound": "TextPolicy.utf8Length(model) > 200" in source,
        "does not use a UTF-16 model bound": "model.length() > 200" not in source,
        "uses the UTF-8 chat response bound": "TextPolicy.utf8Length(text) > 16 * 1024" in source,
        "does not use a UTF-16 chat response bound": "text.length() > 16 * 1024" not in source,
    }
    missing = [name for name, present in checks.items() if not present]
    if missing:
        print("android AI-skin bounds: missing " + ", ".join(missing))
        return 1
    print("android AI-skin bounds: shared UTF-8 model and response limits")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
