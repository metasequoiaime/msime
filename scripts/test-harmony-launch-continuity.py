#!/usr/bin/env python3
"""The HarmonyOS cold start is one picture, in the system's light or dark appearance, so its layers share their colours.

A cold start shows the system start window (`start_window_background` and the `start_window_icon.png` logo, with no disc around it), then the cover `Settings.ets` lays over the web view (`launchCoverView`) until the page reports that it has painted. On an ordinary start the cover lifts onto the settings page; on the first start it lifts onto the welcome flow's splash (`HarmonySplash`), whose first frame is the same picture before veils of those colours fade away to reveal the season's colours.

Each colour exists twice, once per appearance: the start window and the cover take theirs from the `base` (light) and `dark` resource directories, which the system picks between, and the web splash and its system bars pick by the document's theme. When the layers disagreed, the start looked like unrelated screens stitched together: a black start window in light mode that cut to a light settings page, a dark tile icon that read as a missing image. This check keeps each appearance's colours equal across the places that state them, and keeps both start window icons at the size and padding that put the logo where the splash draws it (measured on the HarmonyOS 6.0 phone emulator, density 3.25: the 585px square is 180vp, the logo is 150vp centred in it, and the web area's centre sits 18px below the screen's, so the icon carries 36px of transparent padding on top).
"""

import json
import pathlib
import re
import struct
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
HARMONY = ROOT / "platforms" / "harmony" / "entry" / "src" / "main"
SETTINGS = HARMONY / "ets" / "pages" / "Settings.ets"
LOGO = HARMONY / "resources" / "base" / "media" / "launch_logo.svg"
STYLE = ROOT / "packages" / "ui" / "src" / "account" / "onboarding-style.ts"
BARS = ROOT / "packages" / "ui" / "src" / "settings" / "use-settings-theme.ts"
ICON_SIZE = (585, 621)


def style_colour(source: str, name: str, prefix: str, light: bool) -> str | None:
    match = re.search(rf'export const {name} =\s*"([^"]*)"', source)
    if not match:
        return None
    variant = "light-theme:" if light else r"(?<![\w:-])"
    found = re.search(rf"{variant}{prefix}-\[(#[0-9A-Fa-f]{{6}})\]", match.group(1))
    return found.group(1).upper() if found else None


def resource_colour(qualifier: str, name: str) -> str | None:
    path = HARMONY / "resources" / qualifier / "element" / "color.json"
    if not path.is_file():
        return None
    for item in json.loads(path.read_text(encoding="utf-8")).get("color", []):
        if item.get("name") == name:
            return str(item.get("value", "")).upper()
    return None


def main() -> int:
    if not SETTINGS.is_file():
        print("harmony launch continuity: no HarmonyOS sources, skipped")
        return 0
    settings = SETTINGS.read_text(encoding="utf-8")
    style = STYLE.read_text(encoding="utf-8")
    bars = BARS.read_text(encoding="utf-8")
    splash_bars = re.search(r'const splashBackground = dark \? "(#[0-9A-Fa-f]{6})" : "(#[0-9A-Fa-f]{6})"', bars)
    logo = LOGO.read_text(encoding="utf-8") if LOGO.is_file() else ""
    # The frame is the one path with a colour fill; the stroke over it is fill="none".
    logo_frame = re.search(r'<path d="M[^"]*" fill="(#[0-9A-Fa-f]{6})"', logo)
    groups = {
        "light background": {
            "start_window_background (base)": resource_colour("base", "start_window_background"),
            "onboarding-style.ts hSplash light-theme": style_colour(style, "hSplash", "bg", True),
            "use-settings-theme.ts light splash bars": splash_bars.group(2).upper() if splash_bars else None,
        },
        "dark background": {
            "start_window_background (dark)": resource_colour("dark", "start_window_background"),
            "onboarding-style.ts hSplash": style_colour(style, "hSplash", "bg", False),
            "use-settings-theme.ts dark splash bars": splash_bars.group(1).upper() if splash_bars else None,
        },
        "logo frame": {
            "launch_logo.svg": logo_frame.group(1).upper() if logo_frame else None,
            "onboarding-style.ts hSplashFrame": style_colour(style, "hSplashFrame", "fill", False),
        },
    }
    findings = []
    for group, places in groups.items():
        missing = [place for place, value in places.items() if value is None]
        if missing:
            findings.append(f"{group}: not found in {', '.join(missing)}")
        elif len(set(places.values())) != 1:
            listed = ", ".join(f"{place} {value}" for place, value in places.items())
            findings.append(f"{group}: the launch layers disagree: {listed}")
    for qualifier in ("base", "dark"):
        icon = HARMONY / "resources" / qualifier / "media" / "start_window_icon.png"
        header = icon.read_bytes()[:24] if icon.is_file() else b""
        size = struct.unpack(">II", header[16:24]) if header[:8] == b"\x89PNG\r\n\x1a\n" else None
        if size != ICON_SIZE:
            findings.append(f"{icon.relative_to(ROOT)} is {size}, expected {ICON_SIZE}: the start window draws it at its pixel size, centred on the screen")
    for needle, why in (
        ("$r('app.color.start_window_background')", "the cover's background is not the start window's resource colour"),
        ("$r('app.media.launch_logo')", "the cover does not draw launch_logo.svg"),
        ("'launchPainted'", "Settings.ets does not expose launchPainted, so the cover only lifts on its time limit"),
    ):
        if needle not in settings:
            findings.append(f"Settings.ets: {why}")
    if findings:
        print("\n".join(findings))
        print(f"harmony launch continuity: {len(findings)} finding(s)")
        return 1
    print("harmony launch continuity: start window, cover and splash share their first frame in both appearances")
    return 0


if __name__ == "__main__":
    sys.exit(main())
