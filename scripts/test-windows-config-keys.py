#!/usr/bin/env python3
"""Every configuration key the reference ships has a counterpart here.

The Windows factory configuration is the most complete list of what the
reference product can be told to do: 180 keys across eighteen sections. A key it
has and this repository does not is a feature that was never migrated, and
nothing else notices - the settings page renders what it knows about, the
preferences struct parses what it declares, and neither has any idea the other
product offers more.

Finding the two `[statistics]` keys this way is what this check exists to repeat
without anybody remembering to do it by hand.

The comparison is on key names, not values: the two products deliberately differ
on some defaults, and those are checked by `test-default-config-parity.py`.
Keys this repository adds are fine and are not reported - the adaptation is
allowed to offer more, just not less.

The reference checkout is optional. Without it the check reports what it would
have needed and passes, the same as every other stage that depends on something
not every machine has.
"""

from __future__ import annotations

import pathlib
import re
import sys

from reference_source import reference_root, show_file

ROOT = pathlib.Path(__file__).resolve().parent.parent
OURS = ROOT / "platforms/windows/installer/config.default.toml"
REFERENCE_CONFIG = "installer/default_config/config.default.toml"

REFERENCE = reference_root(ROOT)

# Keys the reference has that this repository answers somewhere other than a configuration key of
# the same name. Each needs the reason, because "it is handled elsewhere" is exactly what someone
# would write to make this check quiet.
ANSWERED_ELSEWHERE: dict[str, str] = {
    "ai_assistant.prompt": (
        "the shared preferences keep the three AI prompt slots as prompt_custom_1..3 and the "
        "selected slot as prompt_id; the settings page and desktop host resolve that selection "
        "through the shared AI assistant settings"
    ),
    "appearance.cand_text_color": (
        "candidate text colours come from the shared preferences global_theme / custom_theme "
        "(custom_theme.candidate_colors), resolved through msime_client_resolve_theme and chosen "
        "in 设置 → 主题"
    ),
    "appearance.candidate_skin": (
        "the candidate window, toolbar, menu and screen keyboard skin comes from the shared "
        "preferences global_theme / custom_theme (custom_theme.candidate_skin), resolved through "
        "msime_client_resolve_theme and chosen in 设置 → 主题"
    ),
    "appearance.ui_backend": (
        "the Windows host has one supported candidate renderer here (Direct2D), so there is no "
        "backend choice to persist or expose"
    ),
    "general.candidate_window_diagnostic_log": (
        "candidate diagnostics are represented by the shared diagnostic_log.server preference; "
        "the legacy Windows alias is intentionally not duplicated"
    ),
    "input.session_backend": (
        "the current host always uses the Engine session through input-runtime, so the legacy "
        "backend selector has no product behavior to configure"
    ),
    "voice_input.polish_prompt": (
        "the old single prompt migrated to voice_input.polish_prompt_custom_1; the shared voice "
        "prompt selector resolves the three custom slots and keeps the old value as slot one"
    ),
}


def keys(text: str) -> set[str]:
    section = None
    found: set[str] = set()
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            section = stripped[1:-1]
            continue
        match = re.match(r"^([A-Za-z_][A-Za-z0-9_]*)\s*=", stripped)
        if match:
            found.add(f"{section}.{match.group(1)}")
    return found


def reference_config() -> tuple[str, str, str] | None:
    return show_file(ROOT, REFERENCE_CONFIG)


def main() -> int:
    resolved = reference_config()
    if resolved is None:
        print("skipped: no MSIME-Windows checkout beside this repository to compare against")
        print(f"  expected a git checkout at {REFERENCE}")
        return 0
    text, ref, sha = resolved
    theirs = keys(text)
    ours = keys(OURS.read_text(encoding="utf-8"))
    missing = sorted(key for key in theirs - ours if key not in ANSWERED_ELSEWHERE)

    for key in missing:
        print(f"FAIL {key}: the reference configures this and nothing here does", file=sys.stderr)
    if missing:
        print(
            "\nEither migrate the feature, or record in ANSWERED_ELSEWHERE where this repository "
            "answers it and why the key does not exist.",
            file=sys.stderr,
        )
        return 1
    extra = len(ours - theirs)
    print(
        f"windows config keys: all {len(theirs)} keys of {ref} ({sha[:8]}) are present"
        + (f", plus {extra} this repository adds" if extra else "")
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
