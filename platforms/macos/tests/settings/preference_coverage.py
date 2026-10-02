#!/usr/bin/env python3
"""Every shared preference is either honoured on macOS or listed here with the reason it is not.

The settings page is shared, so a preference added for another platform appears on macOS too. When the host
does not read it, the switch saves, the page says it saved, and nothing changes - the user is told the
opposite of the truth. That is not hypothetical: 本地 Whisper accepted a model file and recognised through
Apple's service instead, and the two smart-punctuation direct switches did nothing while the host behaved
as though both were on.

Nothing catches that by itself, because the host compiles and the tests pass either way. So the coverage is
asserted here: a field reachable from neither the macOS sources nor the shared runtime has to be named in
NOT_APPLICABLE with a reason, and a name that no longer needs to be there is a failure too, so the list
cannot quietly outlive its entries.

Usage: preference_coverage.py <repository root>
"""

import re
import sys
from pathlib import Path

# Preferences that no macOS input method can act on. Each entry says why, because "macOS does not read it"
# is exactly what a defect looks like from the outside.
NOT_APPLICABLE = {
    "settings_theme": "the settings application's own appearance; the input method has no window to theme",
    "ui_backend": "which settings surface the host opens, chosen by the host rather than read from itself",
    "touch_keyboard_schemes": "the touch keyboard, which macOS has no equivalent of; the screen keyboard is a separate surface with its own preferences",
    "touch_key_spacing_tenths": "the touch keyboard",
    "touch_row_spacing_tenths": "the touch keyboard",
    "touch_keyboard_height_adjustment": "the touch keyboard",
    "touch_voice_shortcut": "the touch keyboard",
    "touch_toolbar": "the touch keyboard's row above the keys",
    "number_row_selection": "releasing the number row back to the editor, offered only where the host advertises it - Linux and HarmonyOS; Windows and macOS both keep number selection",
    "english_suggestions": "the mobile suggestion strip; desktop hosts show English candidates through mixed_input instead",
    "show_candidate_page_number": "offered only where the host advertises candidate_page_number - Linux alone (7508f045e); the macOS candidate window always draws its page indicator and the shared page does not show the switch here",
}


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: preference_coverage.py <repository root>", file=sys.stderr)
        return 2
    root = Path(sys.argv[1])

    source = (root / "crates/client-core/src/preferences.rs").read_text(encoding="utf-8")
    body = re.search(r"pub struct Preferences \{(.*?)\n\}", source, re.S)
    if not body:
        print("could not find the Preferences struct", file=sys.stderr)
        return 1
    fields = re.findall(r"\n    pub (\w+):", body.group(1))
    if not fields:
        print("the Preferences struct parsed to no fields", file=sys.stderr)
        return 1

    def read(*directories: str) -> str:
        text = []
        for directory in directories:
            for path in (root / directory).rglob("*"):
                # A field named only in a test is not a field the host acts on. Counting those made a
                # test about the touch keyboard - written for another platform, in a crate this scan
                # reaches - look like macOS consuming a preference it has no surface for, and the entry
                # explaining why it cannot was then reported as stale.
                if path.name in {"tests.rs", "test.rs"} or "tests" in path.parts:
                    continue
                if path.is_file() and path.suffix in {".h", ".m", ".mm", ".cpp", ".swift", ".rs"}:
                    text.append(path.read_text(encoding="utf-8", errors="ignore"))
        return "\n".join(text)

    # The host may read a preference itself, or leave it to the shared runtime it drives.
    reachable = read("platforms/macos/src", "crates/input-runtime/src", "crates/host-api/src")
    unreachable = [field for field in fields if field not in reachable]

    failures = []
    for field in unreachable:
        if field not in NOT_APPLICABLE:
            failures.append(
                f"{field} is not read on macOS and has no entry here: either honour it in the host, "
                f"or say why the platform cannot"
            )
    for field in sorted(NOT_APPLICABLE):
        if field not in fields:
            failures.append(f"{field} is listed here but is no longer a preference; drop the entry")
        elif field not in unreachable:
            failures.append(f"{field} is listed as out of scope but the host reads it now; drop the entry")

    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print(f"{len(fields) - len(unreachable)} of {len(fields)} shared preferences reach the macOS host; "
          f"the other {len(unreachable)} are accounted for.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
