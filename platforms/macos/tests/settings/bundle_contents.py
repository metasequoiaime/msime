#!/usr/bin/env python3
"""What the built input method actually ships, checked against the bundle rather than the sources.

The template plist, the resources directory and the CMake source lists are each checked on their own
elsewhere. None of that says the bundle came out right: a resource can be declared and not staged, a
localisation can exist in the tree and not be copied, and a provider can be compiled into a library that
the executable was never linked against. Every one of those is invisible until someone installs the thing.

Usage: bundle_contents.py <path/to/App.app> [--languages zh-Hans,en]
"""

import argparse
import os
import plistlib
import re
import struct
import sys
from pathlib import Path

REQUIRED_MENU_PAGES = {(16, 16), (32, 32)}
ICON_KEYS = (
    "CFBundleIconFile",
    "tsInputMethodIconFileKey",
    "tsInputModeMenuIconFileKey",
    "tsInputModePaletteIconFileKey",
)


def tiff_pages(path: Path) -> set[tuple[int, int]]:
    data = path.read_bytes()
    endian = "<" if data[:2] == b"II" else ">"
    (offset,) = struct.unpack_from(endian + "I", data, 4)
    pages: set[tuple[int, int]] = set()
    seen: set[int] = set()
    while offset and offset not in seen:
        seen.add(offset)
        (count,) = struct.unpack_from(endian + "H", data, offset)
        size = {}
        for index in range(count):
            entry = offset + 2 + index * 12
            tag, kind = struct.unpack_from(endian + "HH", data, entry)
            if tag in (256, 257):
                (size[tag],) = struct.unpack_from(endian + ("H" if kind == 3 else "I"), data, entry + 8)
        if 256 in size and 257 in size:
            pages.add((size[256], size[257]))
        (offset,) = struct.unpack_from(endian + "I", data, offset + 2 + count * 12)
    return pages


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    # The languages the build stages. Without this a bundle that shipped none of them would pass by
    # having nothing left to check.
    parser.add_argument("--languages", default="")
    arguments = parser.parse_args()
    bundle = arguments.bundle
    contents = bundle / "Contents"
    resources = contents / "Resources"
    failures: list[str] = []

    if not (contents / "Info.plist").is_file():
        print(f"{bundle} has no Contents/Info.plist", file=sys.stderr)
        return 1
    with (contents / "Info.plist").open("rb") as handle:
        plist = plistlib.load(handle)

    executable = resources.parent / "MacOS" / plist["CFBundleExecutable"]
    if not executable.is_file():
        failures.append(f"CFBundleExecutable names {executable.name}, which is not in Contents/MacOS")

    # The per-mode icons live inside ComponentInputModeDict, which is where the input menu reads them from;
    # the bundle-level one sits at the top. Collect both rather than assuming a shape.
    declared_icons: dict[str, str] = {key: plist[key] for key in ICON_KEYS if key in plist}
    for mode in (plist.get("ComponentInputModeDict", {}).get("tsInputModeListKey", {}) or {}).values():
        for key in ICON_KEYS:
            if key in mode:
                declared_icons[key] = mode[key]

    # Declared and staged are different things; the menu draws whatever is actually in the bundle.
    for key in ICON_KEYS:
        name = declared_icons.get(key)
        if not name:
            failures.append(f"{key} is not declared")
            continue
        icon = resources / name
        if not icon.is_file():
            failures.append(f"{key} names {name}, which was not staged into Resources")
        elif key != "CFBundleIconFile":
            missing = REQUIRED_MENU_PAGES - tiff_pages(icon)
            if missing:
                failures.append(
                    f"{name} ships without the {', '.join(f'{w}x{h}' for w, h in sorted(missing))} page"
                )

    # A localisation that exists in the tree but was not copied leaves the user reading the plist's language.
    usage = {key: value for key, value in plist.items() if key.endswith("UsageDescription")}
    # The input menu names a source by looking its identifier up in the staged InfoPlist.strings, and prints the identifier itself when the lookup misses. info_plist_names.py pairs the two in the tree; this says the pairing survived into the bundle.
    identifiers = {value for value in (plist.get("CFBundleIdentifier"), plist.get("TISInputSourceID")) if value}
    identifiers |= set(plist.get("ComponentInputModeDict", {}).get("tsInputModeListKey", {}) or {})
    lprojs = sorted(resources.glob("*.lproj"))
    expected = [name for name in arguments.languages.split(",") if name]
    for name in expected:
        if not (resources / f"{name}.lproj").is_dir():
            failures.append(f"{name}.lproj was not staged; that language falls back to the plist")
    if not lprojs:
        failures.append("no .lproj directories were staged")
    for lproj in lprojs:
        strings = lproj / "InfoPlist.strings"
        if not strings.is_file():
            failures.append(f"{lproj.name} has no InfoPlist.strings")
            continue
        # Staged .strings are binary plists, not the UTF-8 source.
        try:
            with strings.open("rb") as handle:
                localised = plistlib.load(handle)
        except Exception:
            localised = dict(re.findall(r'"([^"]+)"\s*=\s*"([^"]*)"', strings.read_text(encoding="utf-8")))
        for key in usage:
            if not localised.get(key, "").strip():
                failures.append(f"{key} is not localised in {lproj.name}")
        for identifier in sorted(identifiers):
            if not localised.get(identifier, "").strip():
                failures.append(f"{identifier} has no name in {lproj.name}; the input menu shows the identifier there")

    # The voice cues are the product's start.mp3/end.mp3. Missing from the bundle, VoiceCuePlayer quietly falls back to the system Glass/Pop sounds, so only the bundle can say the product sound actually shipped.
    for cue in ("start.mp3", "end.mp3"):
        staged = resources / "audios" / cue
        if not staged.is_file() or staged.stat().st_size == 0:
            failures.append(f"audios/{cue} was not staged; the voice cue falls back to a system sound")

    # The built-in sound packs are what a fresh profile selects; without them key sounds have nothing to play. Every folder in the repository's resources/sound-packs has to arrive, since client-core lists each one as built in.
    packs = Path(__file__).resolve().parents[4] / "resources/sound-packs"
    for pack in sorted(path.name for path in packs.iterdir() if path.is_dir()):
        if not (resources / "sound-packs" / pack / "plugin.toml").is_file():
            failures.append(f"sound-packs/{pack} was not staged; the built-in sound pack is missing")

    # A macOS framework is mostly symlinks - Headers, Resources and the binary all point into
    # Versions/Current. A copy that follows them produces a directory codesign calls ambiguous and refuses
    # to seal, and an input method that cannot be signed cannot be registered as an input source at all.
    for framework in sorted((contents / "Frameworks").glob("*.framework")):
        versions = framework / "Versions"
        if not versions.is_dir():
            failures.append(f"{framework.name} has no Versions directory; it was flattened on the way in")
            continue
        for entry in ("Resources", framework.stem):
            staged = framework / entry
            if staged.exists() and not staged.is_symlink():
                failures.append(
                    f"{framework.name}/{entry} is a copy rather than a link into Versions; "
                    f"codesign will call the bundle format ambiguous"
                )

    # Installed on-device models run in the msime-voice-local helper, which loads the sherpa-onnx runtime from ../Frameworks. The input method spawns it from beside its own executable, so a bundle missing either one accepts a model directory in settings and then fails every recognition.
    helper = contents / "MacOS" / "msime-voice-local"
    if not helper.is_file() or not os.access(helper, os.X_OK):
        failures.append("Contents/MacOS/msime-voice-local is missing or not executable; installed voice models cannot run")
    runtime = contents / "Frameworks" / "libsherpa-onnx-c-api.dylib"
    if not runtime.is_file() or runtime.stat().st_size == 0:
        failures.append("Contents/Frameworks/libsherpa-onnx-c-api.dylib was not staged; the voice helper has no runtime to load")
    # The runtime is redistributed third-party code, so its licences travel with it.
    for notice in ("sherpa-onnx-Apache-2.0.txt", "onnxruntime-MIT.txt", "onnxruntime-ThirdPartyNotices.txt"):
        if not (contents / "Resources" / "Licenses" / notice).is_file():
            failures.append(f"Contents/Resources/Licenses/{notice} is missing; the bundled speech runtime ships without its licence")
    # The sound player links MPL-2.0 crates (symphonia, triple_buffer) into the input method, and the engine links rink-core.
    if not (contents / "Resources" / "Licenses" / "MPL-2.0.txt").is_file():
        failures.append("Contents/Resources/Licenses/MPL-2.0.txt is missing; the MPL-2.0 crates of the sound player and unit conversion ship without their licence")
    # The engine embeds the place names `@` mode offers from modood/Administrative-divisions-of-China.
    if not (contents / "Resources" / "Licenses" / "Administrative-divisions-of-China-WTFPL.txt").is_file():
        failures.append("Contents/Resources/Licenses/Administrative-divisions-of-China-WTFPL.txt is missing; the built-in place names ship without their licence")
    # The engine embeds the Korean Hanja table from libhangul, whose BSD-3-Clause licence requires the notice in binary distributions.
    if not (contents / "Resources" / "Licenses" / "libhangul-hanja-BSD-3-Clause.txt").is_file():
        failures.append("Contents/Resources/Licenses/libhangul-hanja-BSD-3-Clause.txt is missing; the built-in Korean Hanja table ships without its licence")
    # Cantonese and Zhuyin read dictionaries built from rime-cantonese (CC BY 4.0) and libchewing-data (LGPL-2.1-or-later). The notices ship unconditionally, so a build that later gains the dictionaries is never without them.
    for notice in ("rime-cantonese-CC-BY-4.0.txt", "libchewing-data-LGPL-2.1.txt"):
        if not (contents / "Resources" / "Licenses" / notice).is_file():
            failures.append(f"Contents/Resources/Licenses/{notice} is missing; Cantonese and Zhuyin ship without the licence of their data")
    # Vietnamese links the MIT-licensed vi crate, whose copyright and permission notice has to travel with the binary.
    if not (contents / "Resources" / "Licenses" / "vi-MIT.txt").is_file():
        failures.append("Contents/Resources/Licenses/vi-MIT.txt is missing; the vi crate Vietnamese mode links ships without its licence")
    # 藏文方案链接 MIT 许可的 ewts crate，它的版权和许可声明必须随二进制一起发布。
    if not (contents / "Resources" / "Licenses" / "ewts-MIT.txt").is_file():
        failures.append("Contents/Resources/Licenses/ewts-MIT.txt is missing; the ewts crate Tibetan mode links ships without its licence")

    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print(f"{bundle.name}: icons staged, {len(usage)} usage descriptions and {len(identifiers)} input source names "
          f"localised in {len(lprojs)} languages, voice cues and sound packs staged, local voice helper, runtime and its licences staged.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
