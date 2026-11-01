#!/usr/bin/env python3
"""Render the launcher icons from the brand artwork (brew install librsvg).

Everywhere else the mark is the standalone tile, `packages/ui/src/assets/msime.svg`: a dark rounded square whose green edge is its border, with the white stroke on it. A launcher icon cannot carry that edge. The launcher cuts the icon to a shape of its own choosing - a circle on Pixel, a rounded square on most others - and any border drawn into the icon fights whichever shape it did not anticipate: on a circle the tile's corners are cut off and the edge colour shows in crescents. So on Android the mask itself is the container: the field fills the whole icon and the white stroke sits on it at the size it has inside the tile, with nothing drawn around it.

The themes differ from the default by exactly one value. On iOS that value is the colour of the tile's edge; without an edge here it is the colour of the field, the theme's own colour darkened so the white stroke stays legible. The same theme table is used on iOS.
"""
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TILE = ROOT.parents[1] / "packages/ui/src/assets/msime.svg"

FIELD = "#252525"
DARKEN_TO = "#1A1A1A"
# The theme colour's share of each themed field; the rest is DARKEN_TO.
THEME_SHARE = 45
# Shared with platforms/ios/scripts/generate_app_icons.py (there they colour the tile's edge). `classic` has no theme colour: its field is the tile's own.
VARIANTS = {"classic": None, "forest": "#287B61", "sky": "#70DCF0",
            "dusk": "#A896EF", "vermilion": "#B74332"}
DENSITIES = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}

# The launcher shows the middle 72dp of the 108dp adaptive canvas. The stroke is scaled as it would be if the tile, whose outer extent is 105 of its 110 units, filled exactly that 72dp.
VISIBLE = 72 / 108
TILE_EXTENT = 105
FOREGROUND_DENSITIES = {"mdpi": 108, "hdpi": 162, "xhdpi": 216,
                        "xxhdpi": 324, "xxxhdpi": 432}
MARK = re.compile(r'<path d="M[^"]*" stroke="#FFFFFF"[^>]*/>')


def field(theme: str | None) -> str:
    """The icon's field: the tile's own for `classic`, otherwise the theme colour darkened."""
    if theme is None:
        return FIELD
    a = [int(theme[i:i + 2], 16) for i in (1, 3, 5)]
    b = [int(DARKEN_TO[i:i + 2], 16) for i in (1, 3, 5)]
    return "#" + "".join(f"{round(x * THEME_SHARE / 100 + y * (100 - THEME_SHARE) / 100):02X}"
                         for x, y in zip(a, b))


def mark() -> str:
    found = MARK.search(TILE.read_text())
    if not found:
        raise SystemExit(f"{TILE} no longer has the white stroke this script draws")
    return found.group(0)


def artwork(ground: str | None) -> str:
    """The stroke centred at its in-tile size, on `ground` or on nothing."""
    scale = VISIBLE * 110 / TILE_EXTENT
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" width="110" height="110" viewBox="0 0 110 110"'
        ' fill="none">\n'
        + (f'<rect width="110" height="110" fill="{ground}"/>\n' if ground else "")
        + f'<g transform="translate({55 - 55 * scale:.4f} {55 - 55 * scale:.4f}) scale({scale:.6f})">'
        f"{mark()}</g>\n</svg>\n"
    )


def render(source: Path, pixels: int, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["rsvg-convert", "-w", str(pixels), "-h", str(pixels), str(source), "-o", str(destination)],
        check=True,
    )


def main() -> None:
    with tempfile.TemporaryDirectory() as directory:
        for name, theme in VARIANTS.items():
            print(f"{name}: field {field(theme)} (res/values/colors.xml app_icon_field_{name})")
            # The plain bitmap, for anything that draws the icon without the adaptive layers: the field and the stroke, square.
            source = Path(directory) / f"{name}.svg"
            source.write_text(artwork(field(theme)))
            for density, pixels in DENSITIES.items():
                render(source, pixels, ROOT / f"res/drawable-{density}/app_icon_{name}.png")
            # The adaptive foreground is the stroke alone; the background layer is the field colour, so the mask cuts through that.
            layer = Path(directory) / f"{name}-foreground.svg"
            layer.write_text(artwork(None))
            for density, pixels in FOREGROUND_DENSITIES.items():
                render(layer, pixels,
                       ROOT / f"res/drawable-{density}/app_icon_{name}_foreground.png")


if __name__ == "__main__":
    main()
