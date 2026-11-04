#!/usr/bin/env python3
"""Render the launcher icons from the brand artwork (brew install librsvg).

The icon is the standalone mark, `packages/ui/src/assets/msime.svg` - a dark rounded square whose green edge is its border, with the white stroke on it - the same picture Windows, macOS, Linux, iOS and HarmonyOS show. The launcher cuts the icon to a shape of its own choosing, so the tile is scaled until its edge meets the 72dp the mask shows and the background layer is the edge colour: on a rounded-square launcher the mask lands on the green edge and the home screen shows the same rounded square as the PC icon, the ground only filling in where the mask's corners are rounder than the tile's. This is what iOS does with its own mask. On a circular mask (Pixel) the tile's corners are cut off and the edge shows at the four sides only; that is the price of the launcher showing the same picture as every other platform.

The themes differ from the default by exactly one value, the colour of the tile's edge (and so of the ground), with the same theme table as platforms/ios/scripts/generate_app_icons.py.
"""
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TILE = ROOT.parents[1] / "packages/ui/src/assets/msime.svg"

FRAME = "#A8DF8E"
# Shared with platforms/ios/scripts/generate_app_icons.py. `classic` keeps the tile's own edge.
VARIANTS = {"classic": FRAME, "forest": "#287B61", "sky": "#70DCF0",
            "dusk": "#A896EF", "vermilion": "#B74332"}
DENSITIES = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}

# The launcher shows the middle 72dp of the 108dp adaptive canvas; the tile, whose outer extent is 105 of its 110 units, is scaled to fill exactly that 72dp.
VISIBLE = 72 / 108
TILE_EXTENT = 105
FOREGROUND_DENSITIES = {"mdpi": 108, "hdpi": 162, "xhdpi": 216,
                        "xxhdpi": 324, "xxxhdpi": 432}
TILE_BODY = re.compile(r"<svg[^>]*>(.*)</svg>", re.S)


def tile(edge: str) -> str:
    """The standalone mark's drawing with its edge in `edge`."""
    body = TILE_BODY.search(TILE.read_text())
    if not body or FRAME not in body.group(1):
        raise SystemExit(f"{TILE} no longer has the {FRAME} edge this script recolours")
    return body.group(1).replace(FRAME, edge)


def scaled(edge: str, scale: float) -> str:
    """The tile scaled about the canvas centre, on a transparent 110-unit canvas."""
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" width="110" height="110" viewBox="0 0 110 110"'
        ' fill="none">\n'
        f'<g transform="translate({55 - 55 * scale:.4f} {55 - 55 * scale:.4f}) scale({scale:.6f})">'
        f"{tile(edge)}</g>\n</svg>\n"
    )


def foreground(edge: str) -> str:
    """The adaptive foreground: the tile with its outer edge on the border of the 72dp the mask shows."""
    return scaled(edge, VISIBLE * 110 / TILE_EXTENT)


def plain(edge: str) -> str:
    """The plain bitmap: the tile filling the bitmap, transparent outside its rounded corners.

    Nothing on a device draws this (with `minSdk` 28 the adaptive icons always win), but it is the file anyone browsing the repository sees, so it shows the icon as a rounded-square launcher does.
    """
    return scaled(edge, 110 / TILE_EXTENT)


def render(source: Path, pixels: int, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["rsvg-convert", "-w", str(pixels), "-h", str(pixels), str(source), "-o", str(destination)],
        check=True,
    )


def main() -> None:
    with tempfile.TemporaryDirectory() as directory:
        for name, edge in VARIANTS.items():
            print(f"{name}: ground {edge} (res/values/colors.xml app_icon_ground_{name})")
            # The plain bitmap, for anything that draws the icon without the adaptive layers.
            source = Path(directory) / f"{name}.svg"
            source.write_text(plain(edge))
            for density, pixels in DENSITIES.items():
                render(source, pixels, ROOT / f"res/drawable-{density}/app_icon_{name}.png")
            # The adaptive foreground is the whole tile; the background layer is the edge colour, which shows wherever the mask reaches past the tile's corners.
            layer = Path(directory) / f"{name}-foreground.svg"
            layer.write_text(foreground(edge))
            for density, pixels in FOREGROUND_DENSITIES.items():
                render(layer, pixels,
                       ROOT / f"res/drawable-{density}/app_icon_{name}_foreground.png")


if __name__ == "__main__":
    main()
