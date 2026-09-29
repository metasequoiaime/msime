#!/usr/bin/env python3
"""The settings palette must stay equal to the source's, value for value.

`packages/ui/src/upstream/settings-variables.css` is the source's own
`styles/variables.css`, carried here the way the candidate-window themes under
the same directory are. The shared settings page defines the same names again in
`packages/ui/src/styles.css`, once for each theme, because it builds them with
Tailwind rather than importing the sheet.

Both copies currently agree on all 64 variables in both themes. That agreement
was produced by porting them carefully and nothing was checking it, so a single
edited hex would have drifted the window away from the source silently -- and
the palette is most of what makes one window look like another.

Whitespace differs legitimately: a long shadow is wrapped across lines here and
written on one line there. Values are compared with their whitespace collapsed.

Only the two base theme rules are guarded. The redesign's platform layer, which follows them in the same sheet and re-points the accent variables per platform, is checked against `packages/ui/src/theme/platform-tokens.ts` by a vitest instead; each base rule is therefore read from its own braces rather than to the end of the file.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
UPSTREAM = ROOT / "packages/ui/src/upstream/settings-variables.css"
OURS = ROOT / "packages/ui/src/styles.css"

VARIABLE = re.compile(r"(--[a-z0-9-]+):\s*([^;]+);")


def collapse(value: str) -> str:
    return re.sub(r"\s+", " ", value).strip().lower()


def block(text: str, start: int, end: int) -> dict[str, str]:
    return {name: collapse(value) for name, value in VARIABLE.findall(text[start:end])}


def themes_of_upstream(text: str) -> tuple[dict[str, str], dict[str, str]]:
    light = find(text, 'html[data-theme="light"]', "the source sheet's light theme block")
    return block(text, 0, light), block(text, light, len(text))


def themes_of_ours(text: str) -> tuple[dict[str, str], dict[str, str]]:
    # Anchored on `color-scheme`, which each theme block declares and which is not itself a palette
    # value. Anchoring on the accent instead, as the first version did, meant that editing the
    # accent -- the single most likely palette edit -- removed the landmark and crashed the check
    # rather than reporting the difference it exists to report.
    dark = find(text, "color-scheme: dark;", "our dark theme block")
    light = find(text, "color-scheme: light;", "our light theme block")
    return block(text, *rule_around(text, dark)), block(text, *rule_around(text, light))


def rule_around(text: str, at: int) -> tuple[int, int]:
    # The declaration block holding `at`: palette values carry no braces, so the nearest brace on each side bounds the rule.
    return text.rindex("{", 0, at), text.index("}", at)


def find(text: str, marker: str, what: str) -> int:
    try:
        return text.index(marker)
    except ValueError:
        raise SystemExit(f"could not locate {what} by `{marker}`; the stylesheet's shape changed")


def main() -> int:
    upstream_dark, upstream_light = themes_of_upstream(UPSTREAM.read_text(encoding="utf-8"))
    ours_dark, ours_light = themes_of_ours(OURS.read_text(encoding="utf-8"))
    failures = 0
    for theme, source, mine in (("dark", upstream_dark, ours_dark), ("light", upstream_light, ours_light)):
        for name, value in sorted(source.items()):
            if name not in mine:
                print(f"{theme}: {name} is in the source palette and not here", file=sys.stderr)
                failures += 1
            elif mine[name] != value:
                print(
                    f"{theme}: {name} is {mine[name]} here and {value} in the source palette",
                    file=sys.stderr,
                )
                failures += 1
    if failures:
        print(
            f"{failures} palette difference(s); {UPSTREAM.relative_to(ROOT)} is the source's own sheet",
            file=sys.stderr,
        )
        return 1
    print(
        f"settings palette parity: {len(upstream_dark)} dark and {len(upstream_light)} light "
        "variables match the source"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
