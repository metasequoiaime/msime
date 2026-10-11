#!/usr/bin/env python3
"""The HarmonyOS cold start is one picture that starts moving, so its three layers share their colours.

A cold start shows, in turn, the system start window (`start_window_background` and the `start_window_icon.png` disc), the cover `Settings.ets` lays over the web view until the page reports its first frame (`launchCoverView`), and the web splash (`HarmonySplash`), whose first frame is the same picture: a light disc holding the blank logo frame on near black. The splash then fades veils of those colours away to reveal the season's colours.

When the layers disagreed, the start looked like two unrelated splashes: a dark tile icon on black, some frames of empty web view, then a tinted background with a disc that dropped in from nothing. This check keeps the three colours equal across the four places that state them, and keeps the start window icon at the size and padding that put it on the web splash's disc (measured on the HarmonyOS 6.0 phone emulator, density 3.25: 180vp is 585px, and the web area's centre sits 18px below the screen's, so the icon carries 36px of transparent padding on top).
"""

import json
import pathlib
import re
import struct
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
HARMONY = ROOT / "platforms" / "harmony" / "entry" / "src" / "main"
SETTINGS = HARMONY / "ets" / "pages" / "Settings.ets"
ICON = HARMONY / "resources" / "base" / "media" / "start_window_icon.png"
STYLE = ROOT / "packages" / "ui" / "src" / "account" / "onboarding-style.ts"
BARS = ROOT / "packages" / "ui" / "src" / "settings" / "use-settings-theme.ts"
ICON_SIZE = (585, 621)


def constant(source: str, name: str) -> str | None:
    match = re.search(rf"const {name}: string = '(#[0-9A-Fa-f]{{6}})';", source)
    return match.group(1).upper() if match else None


def style_colour(source: str, name: str, prefix: str) -> str | None:
    match = re.search(rf'export const {name} =\s*"[^"]*\b{prefix}-\[(#[0-9A-Fa-f]{{6}})\]', source)
    return match.group(1).upper() if match else None


def resource_colour(qualifier: str) -> str | None:
    path = HARMONY / "resources" / qualifier / "element" / "color.json"
    if not path.is_file():
        return None
    for item in json.loads(path.read_text(encoding="utf-8")).get("color", []):
        if item.get("name") == "start_window_background":
            return str(item.get("value", "")).upper()
    return None


def main() -> int:
    if not SETTINGS.is_file():
        print("harmony launch continuity: no HarmonyOS sources, skipped")
        return 0
    settings = SETTINGS.read_text(encoding="utf-8")
    style = STYLE.read_text(encoding="utf-8")
    bars = BARS.read_text(encoding="utf-8")
    splash_bars = re.search(r'splashing \? "(#[0-9A-Fa-f]{6})"', bars)
    groups = {
        "background": {
            "start_window_background (base)": resource_colour("base"),
            "start_window_background (dark)": resource_colour("dark"),
            "Settings.ets LAUNCH_BACKGROUND": constant(settings, "LAUNCH_BACKGROUND"),
            "onboarding-style.ts hSplash": style_colour(style, "hSplash", "bg"),
            "use-settings-theme.ts splash bars": splash_bars.group(1).upper() if splash_bars else None,
        },
        "disc": {
            "Settings.ets LAUNCH_DISC": constant(settings, "LAUNCH_DISC"),
            "onboarding-style.ts hSplashDiscVeil": style_colour(style, "hSplashDiscVeil", "bg"),
        },
        "frame": {
            "Settings.ets LAUNCH_FRAME": constant(settings, "LAUNCH_FRAME"),
            "onboarding-style.ts hSplashFrameVeil": style_colour(style, "hSplashFrameVeil", "fill"),
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
    header = ICON.read_bytes()[:24]
    size = struct.unpack(">II", header[16:24]) if header[:8] == b"\x89PNG\r\n\x1a\n" else None
    if size != ICON_SIZE:
        findings.append(f"{ICON.relative_to(ROOT)} is {size}, expected {ICON_SIZE}: the start window draws it at its pixel size, centred on the screen")
    if "'launchPainted'" not in settings:
        findings.append("Settings.ets does not expose launchPainted, so the cover only lifts on its time limit")
    if findings:
        print("\n".join(findings))
        print(f"harmony launch continuity: {len(findings)} finding(s)")
        return 1
    print("harmony launch continuity: start window, cover and splash share their first frame")
    return 0


if __name__ == "__main__":
    sys.exit(main())
