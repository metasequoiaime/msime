#!/usr/bin/env python3
"""Android developer diagnostics must not display a previous account's cloud state during reload."""
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/DeveloperPage.java"


def method_body(source: str, signature: str) -> str:
    start = source.index(signature)
    opening = source.index("{", start)
    depth = 0
    for index in range(opening, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return source[opening:index + 1]
    raise AssertionError(f"unterminated method: {signature}")


def main() -> None:
    source = SOURCE.read_text(encoding="utf-8")
    reload = method_body(source, "private void reload()")
    required = (
        "cloud = DiagnosticsApi.State.EMPTY;",
        "cloudLoaded = false;",
        'cloudAccountId = "";',
        'cloudSessionId = "";',
    )
    missing = [snippet for snippet in required if snippet not in reload]
    if missing:
        raise AssertionError(
            "DeveloperPage.reload must clear the previous cloud account before starting a new load: "
            + ", ".join(missing)
        )
    print("Android developer diagnostics clear stale cloud state on reload")


if __name__ == "__main__":
    main()
