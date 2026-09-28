#!/usr/bin/env python3
"""Authenticated macOS cloud requests must reject HTTP redirects."""

from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    helper = (ROOT / "platforms/macos/src/cloud/CloudURLSession.mm").read_text()
    header = (ROOT / "platforms/macos/src/cloud/CloudURLSession.h").read_text()
    cmake = (ROOT / "platforms/macos/CMakeLists.txt").read_text()
    if "completionHandler(nil);" not in helper:
        raise SystemExit("the macOS cloud session must reject redirects")
    if "MSIMECloudURLSession" not in header or "CloudURLSession.mm" not in cmake:
        raise SystemExit("the redirect policy helper is not built and exposed")

    for relative in (
        "platforms/macos/src/cloud/CloudDictionaryClient.mm",
        "platforms/macos/src/cloud/CloudClipboardClient.mm",
        "platforms/macos/src/core/AccountAuthClient.mm",
    ):
        text = (ROOT / relative).read_text()
        if "MSIMECloudURLSession()" not in text:
            raise SystemExit(f"{relative} does not use the redirect-safe cloud session")
        if "sharedSession" in text:
            raise SystemExit(f"{relative} still uses NSURLSession.sharedSession")
    print("macOS authenticated cloud requests reject redirects")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
