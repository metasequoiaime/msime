#!/usr/bin/env python3
"""The Windows Doubao handshake must not replay credentials across redirects."""

from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    source = (ROOT / "platforms/windows/src/voice/DoubaoAsrClient.cpp").read_text()
    required = (
        "WINHTTP_OPTION_REDIRECT_POLICY_NEVER",
        "WINHTTP_OPTION_REDIRECT_POLICY",
        "WinHttpSetOption(request.value, WINHTTP_OPTION_REDIRECT_POLICY",
    )
    missing = [value for value in required if value not in source]
    if missing:
        raise SystemExit("Windows Doubao transport is missing redirect protection: " + ", ".join(missing))
    print("Windows Doubao handshake rejects redirects")


if __name__ == "__main__":
    main()
