#!/usr/bin/env python3
"""Windows the macOS input method opens for the user go through MSIMEPresentWindow, or presentBackendWindow on the Swift side.

The input method is LSBackgroundOnly. Its activation policy is Prohibited, so `[NSApp activateIgnoringOtherApps:YES]` does nothing, and since macOS 14 activation is only a request anyway. A window shown that way opens behind the app the user was typing in, and the button that opened it looks dead: the floating toolbar gear did exactly that (#1035), and #1036 then found five more windows with the same two lines. MSIMEPresentWindow (src/core/WindowPresentation.h) lifts the policy to Accessory and orders the window front regardless.

So a direct activation request anywhere else in platforms/macos/src is the old pattern coming back. The exceptions are the two helpers themselves and the update controller, which sets Accessory before activating and has no window of its own to present.
"""

import re
import sys
from pathlib import Path

ACTIVATION = re.compile(r"activateIgnoringOtherApps|activate\(ignoringOtherApps")
ALLOWED = {
    "src/core/WindowPresentation.h",
    # presentBackendWindow, the same helper for the Swift backend's windows.
    "src/backend/core/BackendWindowBridge.swift",
    "src/core/UpdateController.mm",
}


def main() -> int:
    root = Path(__file__).resolve().parent.parent / "platforms/macos"
    if not (root / "src").is_dir():
        print(f"skipped: no {root / 'src'}")
        return 0

    failures = []
    for path in sorted((root / "src").rglob("*")):
        if path.suffix not in (".mm", ".m", ".h", ".swift") or not path.is_file():
            continue
        relative = path.relative_to(root).as_posix()
        if relative in ALLOWED:
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if ACTIVATION.search(line):
                failures.append(f"{relative}:{number}: present the window with MSIMEPresentWindow (presentBackendWindow in Swift) instead of activating directly")

    if failures:
        print("\n".join(failures))
        return 1
    print("macos window presentation: no window activates the input method directly")
    return 0


if __name__ == "__main__":
    sys.exit(main())
