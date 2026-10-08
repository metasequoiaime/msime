#!/usr/bin/env python3
"""Keep dictionary exports bounded before they cross the Harmony bridge."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
UI_EXPORT = ROOT / "packages/ui/src/dictionary/dictionary-export.ts"
UI_MANAGER = ROOT / "packages/ui/src/settings/use-dictionary-manager.ts"
HARMONY_SETTINGS = ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets"


def main() -> int:
    export = UI_EXPORT.read_text(encoding="utf-8")
    manager = UI_MANAGER.read_text(encoding="utf-8")
    settings = HARMONY_SETTINGS.read_text(encoding="utf-8")
    required = {
        "shared export byte ceiling":
            "MAX_DICTIONARY_EXPORT_BYTES = 32 * 1024 * 1024" in export
            and "utf8ByteLength(body)" in export,
        "complete export checks the ceiling while reading":
            "MAX_DICTIONARY_EXPORT_BYTES" in export
            and "dictionary_export_limit" in export,
        "settings page checks the bridge payload":
            "utf8Length(request.contents)" in settings
            and "MAX_DICTIONARY_EXPORT_BYTES: number = 32 * 1024 * 1024" in settings,
        "settings page refuses oversized export before writing":
            "error: 'export_too_large'" in settings,
        "shared manager reports oversized exports":
            "utf8ByteLength(body)" in manager
            and "dictionary_export_limit" in manager,
        "paged export stops accumulating oversized text":
            "utf8ByteLength(text)" in manager
            and "dictionary_export_limit" in manager,
    }
    missing = [name for name, present in required.items() if not present]
    if missing:
        print("harmony dictionary export bounds: missing " + ", ".join(missing))
        return 1
    print("harmony dictionary export bounds: bridge payload is capped")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
