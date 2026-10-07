#!/usr/bin/env python3
"""Windows temporary-file failure paths must delete through trusted handles."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCES = (
    ROOT / "platforms/windows/src/input/PrepareHost.h",
    ROOT / "platforms/windows/src/voice/AudioMuteState.h",
    ROOT / "platforms/windows/src/entrypoints/server_main.cpp",
    ROOT / "platforms/windows/src/clipboard/ClipboardHistory.cpp",
)


def main() -> int:
    for path in SOURCES:
        source = path.read_text(encoding="utf-8")
        if "DeleteFileW" in source:
            print(f"{path}: temporary cleanup still uses an unbound path")
            return 1
        if path.name in {"server_main.cpp", "ClipboardHistory.cpp"} \
                and "std::filesystem::remove(temporary" in source:
            print(f"{path}: temporary cleanup still uses an unbound path")
            return 1
        if "remove_private_file" not in source:
            print(f"{path}: temporary cleanup is not bound to a trusted handle")
            return 1
    clipboard = (ROOT / "platforms/windows/src/clipboard/ClipboardHistory.cpp").read_text(encoding="utf-8")
    clear = clipboard.split("bool ClipboardHistory::clear()", 1)[1].split("\n}", 1)[0]
    if "remove_private_file(store_)" not in clear:
        print(f"{ROOT / 'platforms/windows/src/clipboard/ClipboardHistory.cpp'}: clipboard clear is not bound to a trusted handle")
        return 1
    print("Windows temporary failure cleanup uses trusted handles")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
