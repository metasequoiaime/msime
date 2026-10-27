#!/usr/bin/env python3
"""Check the HarmonyOS module manifest for the two mistakes JSON will not report.

`module.json5` is read by hvigor against a schema, and it passed that schema while carrying both of
the faults below, because neither is a schema violation a parser can see.

A duplicate key. `"abilities"` was declared twice, verbatim, from `bdb801b63` — the commit that added
the settings application — until this check was written. JSON says the last one wins, so the first
block was dead text that looked exactly like the live one; editing the wrong copy would have changed
nothing and given no clue why. Python's `json` silently keeps the last value too, which is why this
counts the keys in the text rather than parsing and looking at the result.

An element that draws no icon. The schema says `icon` on an extension "must be configured" when the
extension is the `mainElement`, but says it as prose in a description rather than as `required`, so
nothing enforced it. This module's `mainElement` is `KeyboardExtensionAbility`, and without an icon
the keyboard sat in the system's input-method list with a name and a blank space. The same goes for
an ability carrying `entity.system.home`: it is what the launcher draws.

The packaged manifest is where the answer really lives — a `$media:` reference that does not resolve
is dropped rather than failing the build — so this also checks the built `module.json` when one is
there, which is where `iconId` appears.

An icon with transparent pixels. AppGallery review rejects a package whose configured icons carry any transparency (the published rule covers rounded corners cut into the image and transparent padding alike), and it sent the keyboard back once for pointing `KeyboardExtensionAbility` at the transparent `app_icon.png` the keyboard draws as an image. Every icon the manifests name must be a PNG without an alpha channel or `tRNS` chunk, or a layered icon whose background layer is one; the foreground layer of a layered icon is meant to be transparent around the mark.
"""
import json
import pathlib
import re
import struct
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE = ROOT / "platforms/harmony/entry/src/main/module.json5"
PACKAGED = ROOT / "platforms/harmony/entry/build/default/outputs/default"
APP = ROOT / "platforms/harmony/AppScope/app.json5"
MEDIA = (
    ROOT / "platforms/harmony/AppScope/resources/base/media",
    ROOT / "platforms/harmony/entry/src/main/resources/base/media",
)


def png_transparency(path: pathlib.Path) -> str | None:
    """Why the PNG at `path` can hold transparent pixels, or None when it cannot: colour types 4 and 6 carry alpha, and a `tRNS` chunk makes chosen colours transparent."""
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        return "is not a PNG"
    if data[25] in (4, 6):
        return "has an alpha channel"
    offset = 8
    while offset + 8 <= len(data):
        length, kind = struct.unpack(">I4s", data[offset : offset + 8])
        if kind == b"tRNS":
            return "has a tRNS chunk"
        if kind == b"IDAT":
            return None
        offset += 12 + length
    return None


def icon_transparency(reference: str) -> str | None:
    """Check the image a `$media:` icon reference names; a layered icon is judged by its background layer."""
    name = reference.removeprefix("$media:")
    for directory in MEDIA:
        layered = directory / f"{name}.json"
        if layered.is_file():
            background = json.loads(layered.read_text(encoding="utf-8"))["layered-image"]["background"]
            problem = icon_transparency(background)
            return None if problem is None else f"background layer {background} {problem}"
        image = directory / f"{name}.png"
        if image.is_file():
            return png_transparency(image)
    return "names no PNG or layered icon in the base media folders"


def without_comments(text: str) -> str:
    """JSON5 minus the two things that stop `json` reading it: comments and trailing commas."""
    stripped = re.sub(r"^\s*//.*$", "", text, flags=re.M)
    return re.sub(r",(\s*[\]}])", r"\1", stripped)


def main() -> int:
    if not SOURCE.is_file():
        print(f"no HarmonyOS manifest at {SOURCE}", file=sys.stderr)
        return 1
    text = SOURCE.read_text(encoding="utf-8")
    failures: list[str] = []

    # Module-level keys, counted in the text: a duplicate is invisible once parsed.
    for key in ("abilities", "extensionAbilities", "mainElement", "requestPermissions"):
        found = len(re.findall(rf'^    "{key}":', text, flags=re.M))
        if found > 1:
            failures.append(
                f'"{key}" is declared {found} times at module level; JSON keeps the last, '
                f"so the earlier one is dead text that reads like live configuration"
            )

    module = json.loads(without_comments(text))["module"]
    main_element = module.get("mainElement")
    elements = module.get("abilities", []) + module.get("extensionAbilities", [])
    by_name = {element["name"]: element for element in elements}

    if main_element and main_element in by_name and not by_name[main_element].get("icon"):
        failures.append(
            f"{main_element} is the mainElement and declares no icon, so the system lists it "
            f"with a name and a blank space"
        )
    for element in elements:
        home = any(
            "entity.system.home" in skill.get("entities", [])
            for skill in element.get("skills", [])
        )
        if home and not element.get("icon"):
            failures.append(f"{element['name']} is a launcher entry and declares no icon")

    icons = [(element["name"], element["icon"]) for element in elements if element.get("icon")]
    if APP.is_file():
        app_icon = json.loads(without_comments(APP.read_text(encoding="utf-8")))["app"].get("icon")
        if app_icon:
            icons.append(("app.json5", app_icon))
    for owner, reference in icons:
        problem = icon_transparency(reference)
        if problem is not None:
            failures.append(
                f"{owner} uses {reference}, which {problem}; AppGallery review rejects icons with transparent pixels"
            )

    # The build is where a $media: reference is resolved, and an unresolved one is simply dropped.
    built = PACKAGED / "module.json"
    if not built.is_file():
        candidates = sorted(PACKAGED.glob("*.hap"))
        built = None if not candidates else built
    if built is not None and built.is_file():
        packaged = json.loads(built.read_text(encoding="utf-8"))["module"]
        for element in packaged.get("abilities", []) + packaged.get("extensionAbilities", []):
            if element.get("icon") and not element.get("iconId"):
                failures.append(
                    f"{element['name']} names an icon the packaged manifest could not resolve"
                )

    if failures:
        print("HarmonyOS manifest:", file=sys.stderr)
        for failure in failures:
            print(f"  {failure}", file=sys.stderr)
        return 1

    named = ", ".join(sorted(by_name))
    print(f"harmony manifest: {len(by_name)} elements, all launcher-facing ones named ({named})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
