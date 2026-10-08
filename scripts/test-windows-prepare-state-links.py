#!/usr/bin/env python3
"""Windows preparation must reject reparse ancestors before creating or writing state."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/windows/src/input/PrepareHost.h"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    directory_start = source.index("inline std::filesystem::path prepare_host_state_in_directory(")
    directory_body = source[directory_start : source.index("\ninline std::filesystem::path prepare_host_state(", directory_start)]
    fresh_start = source.index("inline std::filesystem::path prepare_host_state(")
    fresh_body = source[fresh_start : source.index("\n} // namespace msime::windows", fresh_start)]
    required = {
        "existing state preparation checks ancestors before writing": (
            "reject_reparse_ancestors(state);" in directory_body
            and "#ifdef _WIN32" in directory_body
        ),
        "fresh state preparation checks ancestors before mkdir": (
            "reject_reparse_ancestors(state);" in fresh_body
            and "#ifdef _WIN32" in fresh_body
        ),
    }
    missing = [name for name, present in required.items() if not present]
    if missing:
        print("windows preparation link guard: missing " + ", ".join(missing))
        return 1
    print("windows preparation rejects reparse ancestors before state writes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
