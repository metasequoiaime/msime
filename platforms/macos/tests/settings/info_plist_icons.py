#!/usr/bin/env python3
"""Every icon the Info.plist names has to exist, and the menu icon has to be shaped for the menu.

The input menu draws its icon through HIToolbox, which reads the file's pages rather than the DPI of a
single one. A file with only a 2x page is taken for a 32-point image and cropped to its middle by the
16-point menu slot, which for a stroke arrives as a filled square - a bug that looks like a design choice
and is invisible until someone opens the input menu on a real install. Apple's own input methods ship
16x16 at 72 dpi and 32x32 at 144 dpi in one file.

A missing file is the other half: the plist keys are strings, so a renamed resource fails silently and the
menu falls back to a generic icon.

The menu bar icon is also the mode indicator: the Chinese, English and Japanese input modes each name their own icon, 中, 英 and 日, and the menu bar shows the active one. Two modes sharing a file, or a mode naming a different file for the menu and the palette, would leave the menu bar unable to tell the modes apart.
"""

import plistlib
import re
import struct
import sys
from pathlib import Path

ICON_KEYS = (
    "CFBundleIconFile",
    "tsInputMethodIconFileKey",
    "tsInputModeMenuIconFileKey",
    "tsInputModePaletteIconFileKey",
)
# The menu slot is 16 points; these are the pixel sizes that cover it at 1x and 2x.
REQUIRED_MENU_PAGES = {(16, 16), (32, 32)}


def tiff_pages(path: Path) -> list[tuple[int, int]]:
    """Width and height of every page, read from the IFD chain rather than through an image library."""
    data = path.read_bytes()
    if len(data) < 8 or data[:2] not in (b"II", b"MM"):
        raise ValueError(f"{path.name} is not a TIFF")
    endian = "<" if data[:2] == b"II" else ">"
    (magic,) = struct.unpack_from(endian + "H", data, 2)
    if magic != 42:
        raise ValueError(f"{path.name} is not a classic TIFF")
    (offset,) = struct.unpack_from(endian + "I", data, 4)
    pages: list[tuple[int, int]] = []
    seen: set[int] = set()
    while offset and offset not in seen:
        seen.add(offset)
        (count,) = struct.unpack_from(endian + "H", data, offset)
        size = {}
        for index in range(count):
            entry = offset + 2 + index * 12
            tag, kind = struct.unpack_from(endian + "HH", data, entry)
            if tag in (256, 257):  # ImageWidth, ImageLength
                # Both are SHORT or LONG, and either fits inline in the value field.
                fmt = "H" if kind == 3 else "I"
                (size[tag],) = struct.unpack_from(endian + fmt, data, entry + 8)
        if 256 in size and 257 in size:
            pages.append((size[256], size[257]))
        (offset,) = struct.unpack_from(endian + "I", data, offset + 2 + count * 12)
    return pages


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: info_plist_icons.py <Info.plist> <resources directory>", file=sys.stderr)
        return 2
    plist_path, resources = Path(sys.argv[1]), Path(sys.argv[2])
    text = plist_path.read_text(encoding="utf-8")

    failures = []
    named: dict[str, set[str]] = {}
    for key in ICON_KEYS:
        for value in re.findall(rf"<key>{key}</key>\s*<string>([^<]+)</string>", text):
            named.setdefault(key, set()).add(value)

    for key in ICON_KEYS:
        if key not in named:
            failures.append(f"{key} is not declared; the menu falls back to a generic icon")

    for key, values in sorted(named.items()):
        for value in sorted(values):
            if not (resources / value).is_file():
                failures.append(f"{key} names {value}, which is not in {resources}")

    for key in ("tsInputMethodIconFileKey", "tsInputModeMenuIconFileKey", "tsInputModePaletteIconFileKey"):
        for value in sorted(named.get(key, ())):
            icon = resources / value
            if not icon.is_file():
                continue
            if icon.suffix != ".tiff":
                failures.append(f"{key} names {value}; the menu needs a multi-page template TIFF")
                continue
            pages = set(tiff_pages(icon))
            if not REQUIRED_MENU_PAGES <= pages:
                missing = ", ".join(f"{w}x{h}" for w, h in sorted(REQUIRED_MENU_PAGES - pages))
                failures.append(
                    f"{value} is missing the {missing} page; the menu slot crops what it is given"
                )

    with plist_path.open("rb") as handle:
        modes = (plistlib.load(handle).get("ComponentInputModeDict", {}).get("tsInputModeListKey", {}) or {})
    mode_icons: dict[str, str] = {}
    for mode, body in sorted(modes.items()):
        menu, palette = body.get("tsInputModeMenuIconFileKey"), body.get("tsInputModePaletteIconFileKey")
        if not menu:
            failures.append(f"input mode {mode} names no menu icon; the menu bar cannot show which mode is active")
            continue
        if palette != menu:
            failures.append(f"input mode {mode} names {menu} for the menu but {palette} for the palette")
        mode_icons[mode] = menu
    for icon in sorted(set(mode_icons.values())):
        sharing = sorted(mode for mode, value in mode_icons.items() if value == icon)
        if len(sharing) > 1:
            failures.append(f"{', '.join(sharing)} share {icon}; the menu bar icon would not change with the mode")
    if len(modes) < 2:
        failures.append("the bundle declares fewer than two input modes; the menu bar icon cannot show 中 and 英")

    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print(f"{sum(len(v) for v in named.values())} icon references resolve; the menu icons carry both pages and each of the {len(mode_icons)} input modes has its own.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
