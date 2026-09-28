#!/usr/bin/env python3
"""Cloud query values must be encoded as URL query items, not interpolated strings."""

from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    clipboard = (ROOT / "platforms/macos/src/cloud/CloudClipboardClient.mm").read_text()
    dictionary = (ROOT / "platforms/macos/src/cloud/CloudDictionaryClient.mm").read_text()
    if "URLQueryAllowedCharacterSet" in clipboard:
        raise SystemExit("cloud clipboard search must use NSURLQueryItem")
    if "URLQueryAllowedCharacterSet" in dictionary:
        raise SystemExit("fixed-position context must use NSURLQueryItem")
    if clipboard.count("URLQueryItem") < 1 or dictionary.count("URLQueryItem") < 1:
        raise SystemExit("cloud clients must build query values with NSURLQueryItem")
    print("macOS cloud query values are encoded structurally")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
