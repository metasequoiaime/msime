#!/usr/bin/env python3
"""Windows audio mute cleanup must delete through a trusted handle."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/windows/src/voice/SystemAudioMuter.cpp"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("void clear_state()")
    end = source.index("\n}\n", start) + 2
    region = source[start:end]
    if "remove_private_file" not in region or "DeleteFileW" in region:
        print("windows audio mute cleanup is not bound to a trusted handle")
        return 1
    print("windows audio mute cleanup uses a trusted handle")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
