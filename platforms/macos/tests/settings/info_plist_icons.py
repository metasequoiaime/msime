#!/usr/bin/env python3
"""Info.plist 里写到的每个图标都必须存在，而菜单图标还得是菜单能用的形状。

输入菜单经 HIToolbox 绘制图标，它读的是文件里的页，而不是某一页的 DPI。只有一个 2x 页的文件会被当成 32 点图，再被 16 点的菜单槽裁去四周——对一条笔画来说，交到菜单上的就是一个实心方块。这个毛病看起来像是设计如此，而且只有在真机上打开输入菜单才看得见。苹果自家的输入法都是一个文件里放 16x16 @72dpi 与 32x32 @144dpi 两页。

另一半是文件缺失：plist 里这些键是字符串，资源改名之后不会报错，菜单直接回落到一个通用图标。

菜单栏的图标取自当前输入模式，永远不取 bundle 那一级，所以模式图标是产品标志进入菜单栏的唯一通路：中文模式用标志，英文模式用「英」。两个模式共用同一个文件、或者同一个模式给菜单和面板写了不同文件，都会让菜单栏分不出模式——这也是标志不能干脆两个模式都用的原因。
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
        failures.append("bundle 声明的输入模式少于两个，菜单栏图标就无法在标志与「英」之间切换")

    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print(f"{sum(len(v) for v in named.values())} icon references resolve; the menu icons carry both pages and each of the {len(mode_icons)} input modes has its own.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
