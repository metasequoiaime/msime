#!/usr/bin/env python3
"""Render the launcher icons from the brand artwork (brew install librsvg).

The icon is the standalone mark, `packages/ui/src/assets/msime.svg` - a dark rounded square whose green edge is its border, with the white stroke on it - the same picture Windows, macOS, Linux, iOS and HarmonyOS show. The launcher cuts the icon to a shape of its own choosing, so the tile is scaled until its edge meets the 72dp the mask shows and the background layer is the edge colour: on a rounded-square launcher the mask lands on the green edge and the home screen shows the same rounded square as the PC icon, the ground only filling in where the mask's corners are rounder than the tile's. This is what iOS does with its own mask.

A circular mask cannot show that tile: its corners are cut off and the edge shows only in four crescents at the sides. No single adaptive icon can draw an edge that follows whichever shape the launcher cuts, so each style also has a round icon, `app_icon_<style>_round`, which the manifest names as `android:roundIcon`. The framework uses it in place of `android:icon` when the system sets `config_useRoundIcon` - Pixel's configuration overlay does, together with its circular mask, as do the icon-shape overlays that pick a circle - so the choice is made when the package is parsed, not by the app at runtime. The round icon is the same picture drawn for a circle: a dark disc whose edge is a ring in the style's edge colour, as wide relative to the circle as the tile's edge is relative to the tile, with the white stroke a little smaller than on the tile (see ROUND_MARK).

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
MARK = re.compile(r'<path d="M[^"]*" stroke="#FFFFFF"[^>]*/>')
FIELD = "#252525"
# The tile's green edge is 5 of its 105 outer units; the round icon's ring takes the same share of the circle's diameter.
EDGE_SHARE = 5 / TILE_EXTENT
# On the tile the stroke's two ends point into the corners, where the field reaches furthest; a disc has no corners, and at the tile's size the ends would come within 15% of the radius of the ring. At 88% the stroke's farthest point (40.5 of the tile's 110 units from the centre) sits at three quarters of the disc's radius.
ROUND_MARK = 0.88


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


def mark() -> str:
    found = MARK.search(TILE.read_text())
    if not found:
        raise SystemExit(f"{TILE} no longer has the white stroke this script draws")
    return found.group(0)


def round_icon(edge: str, diameter: float) -> str:
    """The round icon on a transparent 110-unit canvas: a circle `diameter` units across in the edge colour, the dark disc inside its ring, and the stroke at ROUND_MARK of the size it has on a tile spanning the same `diameter`."""
    ring = diameter * EDGE_SHARE
    scale = diameter / TILE_EXTENT * ROUND_MARK
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" width="110" height="110" viewBox="0 0 110 110"'
        ' fill="none">\n'
        f'<circle cx="55" cy="55" r="{diameter / 2:.4f}" fill="{edge}"/>'
        f'<circle cx="55" cy="55" r="{diameter / 2 - ring:.4f}" fill="{FIELD}"/>'
        f'<g transform="translate({55 - 55 * scale:.4f} {55 - 55 * scale:.4f}) scale({scale:.6f})">'
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
            # The round icon: the plain bitmap is the circle filling the bitmap; the adaptive foreground is the circle on the border of the 72dp the mask shows, over the same edge-coloured background, so a circular mask lands exactly on the ring.
            source = Path(directory) / f"{name}-round.svg"
            source.write_text(round_icon(edge, 110))
            for density, pixels in DENSITIES.items():
                render(source, pixels, ROOT / f"res/drawable-{density}/app_icon_{name}_round.png")
            layer = Path(directory) / f"{name}-round-foreground.svg"
            layer.write_text(round_icon(edge, 110 * VISIBLE))
            for density, pixels in FOREGROUND_DENSITIES.items():
                render(layer, pixels,
                       ROOT / f"res/drawable-{density}/app_icon_{name}_round_foreground.png")


if __name__ == "__main__":
    main()
