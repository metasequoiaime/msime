#!/usr/bin/env python3
"""Info.plist 里写到的每个图标都必须存在，而菜单图标还得是菜单能用的形状。

输入菜单经 HIToolbox 绘制图标，它读的是文件里的页，而不是某一页的 DPI。只有一个 2x 页的文件会被当成 32 点图，再被 16 点的菜单槽裁去四周——对一条笔画来说，交到菜单上的就是一个实心方块。这个毛病看起来像是设计如此，而且只有在真机上打开输入菜单才看得见。苹果自家的输入法都是一个文件里放 16x16 @72dpi 与 32x32 @144dpi 两页。

另一半是文件缺失：plist 里这些键是字符串，资源改名之后不会报错，菜单直接回落到一个通用图标。

每个输入模式的菜单图标和面板图标都是 bundle 自己的水杉标志，与其他输入法一样在输入菜单里显示品牌；系统菜单栏和 Ctrl+空格 切换条因此分不出模式，模式由输入法自己的菜单栏状态项显示。状态项画的是每个模式各一张的大字（中 / 双 / 五 / 粤 / 注 / 日 / 한 / 越 / ཀ / 笔 / 英，scripts/render_menu_icon.swift 生成），plist 不引用它们，所以这里另外检查声明的每个模式都有自己的那张、带齐两页。
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
PRODUCT_LOGO = "MSIMEClientInputMethodMenuIcon.tiff"
# 模式标识后缀 → 状态项大字文件名里的那一段（MSIMEClientInputMethodMenuIcon<名>.tiff）。
GLYPH_NAMES = {
    "Hans": "Chinese", "Shuangpin": "Shuangpin", "Wubi": "Wubi", "Cantonese": "Cantonese", "Zhuyin": "Zhuyin",
    "Japanese": "Japanese", "Korean": "Korean", "Vietnamese": "Vietnamese", "Tibetan": "Tibetan", "Stroke": "Stroke",
    "Roman": "English",
}
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

    # 输入法 bundle 自身使用这张产品 logo，菜单栏状态项在模式图标缺失时也退回它；它必须继续由 plist 声明为输入法图标。
    if "MSIMEClientInputMethodMenuIcon.tiff" not in named.get("tsInputMethodIconFileKey", set()):
        failures.append("tsInputMethodIconFileKey no longer names MSIMEClientInputMethodMenuIcon.tiff, the product logo is missing")

    with plist_path.open("rb") as handle:
        modes = (plistlib.load(handle).get("ComponentInputModeDict", {}).get("tsInputModeListKey", {}) or {})
    for mode, body in sorted(modes.items()):
        menu, palette = body.get("tsInputModeMenuIconFileKey"), body.get("tsInputModePaletteIconFileKey")
        if menu != PRODUCT_LOGO or palette != PRODUCT_LOGO:
            failures.append(f"input mode {mode} names {menu} / {palette}; every mode shows the product logo {PRODUCT_LOGO}")
        # 状态项按模式标识的后缀取大字，与 InputModeIdentifiers.h 的 MSIMEInputModeGlyphFile 同一张表。
        suffix = mode.rsplit(".", 1)[-1]
        glyph_name = GLYPH_NAMES.get(suffix)
        if glyph_name is None:
            failures.append(f"input mode {mode} has no status item glyph; add it to MSIMEInputModeGlyphFile and here")
            continue
        glyph = resources / f"MSIMEClientInputMethodMenuIcon{glyph_name}.tiff"
        if not glyph.is_file():
            failures.append(f"{glyph.name} is missing; the menu bar status item draws it for {mode}")
        elif not REQUIRED_MENU_PAGES <= set(tiff_pages(glyph)):
            failures.append(f"{glyph.name} lacks the 16x16 and 32x32 pages the status item draws from")

    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print(f"{sum(len(v) for v in named.values())} icon references resolve; the menu icons carry both pages, all {len(modes)} input modes show the logo and each has its status item glyph.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
