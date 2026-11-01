#!/usr/bin/env python3
"""Render every iOS icon from the brand artwork with librsvg (brew install librsvg).

The icons are the standalone mark, `packages/ui/src/assets/msime.svg` - a dark rounded square whose green edge is its border, with the white stroke on it - the same picture Windows, macOS, Linux and HarmonyOS show. The alternate icons differ from the default by exactly one value - the colour of that edge - so they are derived rather than drawn, and only the rendered PNGs are kept. Changing the logo means changing that one file; the keyboard's brand key takes its stroke from the master, `apps/desktop/app-icon.svg`.
"""
import json
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1] / "App/Resources"
ASSETS = ROOT / "Assets.xcassets"
MASTER = Path(__file__).resolve().parents[3] / "apps/desktop/app-icon.svg"
# The standalone mark - a rounded square whose green edge is its border - shared with every other platform.
TILE = Path(__file__).resolve().parents[3] / "packages/ui/src/assets/msime.svg"

FRAME = "#A8DF8E"
# One colour each, taken from the theme's own palette and far enough from the default's light
# green to be told apart at home-screen size.
VARIANTS = {"Forest": "#287B61", "Sky": "#70DCF0", "Dusk": "#A896EF", "Vermilion": "#B74332"}

# The tile's outer extent in its 110-unit canvas: the rounded square runs from 5 to 105 and its 5-unit edge straddles that path.
TILE_EXTENT = 105
# The mark's own extent, measured the same way as BBOX. Only the keyboard's brand key needs it -
# it shows the stroke without the panel around it.
MARK_BBOX = (22.0, 9.9786, 85.9571, 99.0)

MARK = re.compile(r'<path d="M74\.[^"]*" stroke="white"[^>]*/>')
TILE_BODY = re.compile(r"<svg[^>]*>(.*)</svg>", re.S)


def mark() -> str:
    """The master's white stroke, for the keyboard's brand key."""
    found = MARK.search(MASTER.read_text())
    if not found:
        raise SystemExit(f"{MASTER} no longer has the mark this script composes")
    return found.group(0)


def tile(edge: str) -> str:
    """The standalone mark's drawing with its edge in `edge`."""
    body = TILE_BODY.search(TILE.read_text())
    if not body or FRAME not in body.group(1):
        raise SystemExit(f"{TILE} no longer has the {FRAME} edge this script recolours")
    return body.group(1).replace(FRAME, edge)


def artwork(edge: str) -> str:
    """The home-screen icon: the tile scaled until its edge meets the canvas, on a ground of the edge colour.

    iOS cuts every icon with its own rounded mask, so a transparent tile would sit on black and a tile with margins would show a second square around it. Scaled to the canvas, the mask lands on the green edge and the home screen shows the same rounded square as everywhere else; the ground only shows where the mask's corners are rounder than the tile's.
    """
    scale = 110 / TILE_EXTENT
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" width="110" height="110" viewBox="0 0 110 110"'
        ' fill="none">\n'
        f'<rect width="110" height="110" fill="{edge}"/>\n'
        f'<g transform="translate({55 - 55 * scale:.4f} {55 - 55 * scale:.4f}) scale({scale:.6f})">'
        f"{tile(edge)}</g>\n</svg>\n"
    )


def render(source: Path, destination: Path, pixels: int) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["rsvg-convert", "-w", str(pixels), "-h", str(pixels), str(source), "-o", str(destination)],
        check=True,
    )


def preview(name: str, source: Path) -> None:
    path = ASSETS / f"AppIconPreview{name}.imageset"
    path.mkdir(parents=True, exist_ok=True)
    (path / "Contents.json").write_text(
        json.dumps(
            {"images": [{"filename": "Preview.png", "idiom": "universal"}],
             "info": {"author": "xcode", "version": 1}},
            indent=2,
        ) + "\n"
    )
    render(source, path / "Preview.png", 256)


def keyboard_brand(staging: Path) -> None:
    """The mark's stroke alone, black on white: `SharedResources/KeyboardBrand.png`.

    Nothing reads this image any more. The keyboard's brand key and the candidate panel used to tint it with the skin's accent as a single-colour template; both now draw the full-colour mark with `MSIMELogo.draw(in:)`, like every other place. The file is still listed as a resource in platforms/ios/project.yml and the Tauri iOS project (and checked by scripts/test-ios-project-config.py), so it is still rendered here until those references are removed together.
    """
    left, top, right, bottom = MARK_BBOX
    scale = 0.9 * 110 / max(right - left, bottom - top)
    transform = (
        f"translate({55 - scale * (left + right) / 2:.4f} {55 - scale * (top + bottom) / 2:.4f})"
        f" scale({scale:.6f})"
    )
    source = staging / "KeyboardBrand.svg"
    source.write_text(
        '<svg xmlns="http://www.w3.org/2000/svg" width="110" height="110" viewBox="0 0 110 110"'
        ' fill="none">\n<rect width="110" height="110" fill="white"/>\n'
        f'<g transform="{transform}">{mark().replace("white", "black")}</g>\n</svg>\n'
    )
    render(source, ROOT.parent.parent / "SharedResources/KeyboardBrand.png", 1024)


def alternate(name: str, source: Path) -> None:
    """Explicit iPhone and iPad renditions, including the legacy-size slots."""
    path = ASSETS / f"AppIcon{name}.appiconset"
    path.mkdir(parents=True, exist_ok=True)
    images = []
    for idiom, sizes, scales in [
        ("iphone", [20, 29, 40, 60], [2, 3]),
        ("ipad", [20, 29, 40, 76], [1, 2]),
        ("ipad", [83.5], [2]),
    ]:
        for size in sizes:
            for scale in scales:
                pixels = int(size * scale)
                filename = f"Icon-{pixels}.png"
                render(source, path / filename, pixels)
                images.append(
                    {"idiom": idiom, "size": f"{size}x{size}", "scale": f"{scale}x",
                     "filename": filename}
                )
    images.append({"idiom": "ios-marketing", "size": "1024x1024", "scale": "1x",
                   "filename": "Icon.png"})
    render(source, path / "Icon.png", 1024)
    (path / "Contents.json").write_text(
        json.dumps({"images": images, "info": {"author": "xcode", "version": 1}}, indent=2) + "\n"
    )


def main() -> None:
    with tempfile.TemporaryDirectory() as directory:
        staging = Path(directory)

        default = staging / "AppIcon.svg"
        default.write_text(artwork(FRAME))
        render(default, ASSETS / "AppIcon.appiconset/AppIcon.png", 1024)
        preview("Classic", default)
        # The launch screen shows the mark on its own. Nothing masks it there, so it is the rounded square every other platform shows wherever the mark stands alone, rather than the full-bleed square the home screen masks.
        logo_set = ASSETS / "MSIMELogo.imageset"
        render(TILE, logo_set / "logo.png", 1024)
        # The rendering intent is part of this asset's contract with the artwork, so it is written
        # here rather than left beside it. It used to say `template`, which keeps only the alpha
        # channel and paints the rest in the tint colour - fine for the single-colour mark that was
        # here before, but this artwork is a dark panel, a coloured frame and a white stroke, and
        # all three collapsed into one black silhouette on the launch screen.
        (logo_set / "Contents.json").write_text(
            json.dumps(
                {"images": [{"filename": "logo.png", "idiom": "universal"}],
                 "info": {"author": "xcode", "version": 1},
                 "properties": {"template-rendering-intent": "original"}},
                indent=2,
            ) + "\n"
        )
        keyboard_brand(staging)

        for name, frame in VARIANTS.items():
            source = staging / f"{name}.svg"
            source.write_text(artwork(frame))
            alternate(name, source)
            preview(name, source)


if __name__ == "__main__":
    main()
