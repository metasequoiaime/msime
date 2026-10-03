#!/usr/bin/env python3
"""The input menu names an input source by looking its identifier up in InfoPlist.strings.

Nothing fails when the key is missing. TISRegisterInputSource still returns noErr, the mode still appears, the menu icon still draws - and where the name belongs the menu prints the identifier itself, so the picker reads `app.msime.inputmethod.MetasequoiaIME.Hans` in a list beside 日文 and ABC.

A rename is what produces that. The identifiers live in Info.plist.in and the names live in one .strings per language, with nothing connecting the two files, so changing the identifiers in the plist leaves the old keys behind as valid syntax attached to an input source that no longer exists.

bundle 声明了中文、双拼、五笔、粤拼、注音、日语、韩语、越南语、藏语和英文十个模式，每个在输入菜单和系统设置里各占一条。它们在每种语言下都必须读起来不同，否则列表里会出现两个同名条目，用户分不清哪一条打中文。

Hence both directions: every identifier the plist declares must be named in every language, and every identifier-shaped key in a .strings must name something the plist still declares. The plist's mode identifier and TISInputSourceID are checked against each other for the same reason - a mode whose identifier disagrees, or that is absent from the visible order, is registered and then never offered.

The names are not the only thing keyed on the identifier, so the last pass reads the sources: the settings app validates the packaged bundle by looking the identifier up inside it, launches the input method by it, and reads the preferences domain NSUserDefaults derives from it, and the uninstaller deletes that domain. Each of those is a string literal in another language in another directory, and each fails silently in its own way - an install that rejects the correct bundle, a restart that finds no application, a settings page that saves into a plist nobody reads.
"""

import plistlib
import re
import sys
from pathlib import Path

# Where the identifier is consumed, and the file types that can hold it.
CONSUMER_ROOTS = ("crates", "apps/desktop/src-tauri/src", "platforms/macos/src", "platforms/macos/tests")
CONSUMER_SUFFIXES = {".rs", ".m", ".mm", ".h", ".cpp", ".swift"}
# Identifier-shaped literals belonging to this input method. Narrow enough to leave test defaults
# domains alone; the settings bundle and its runtime state now share app.msime.macos.
IDENTIFIER = re.compile(r"app\.msime\.[A-Za-z0-9._-]*inputmethod[A-Za-z0-9._-]*")


def localized(path: Path) -> dict[str, str]:
    """The .strings sources are UTF-8 key/value pairs; the staged copies are binary plists, which are checked in bundle_contents.py instead."""
    return dict(re.findall(r'"([^"]+)"\s*=\s*"([^"]*)"', path.read_text(encoding="utf-8")))


def main() -> int:
    if len(sys.argv) not in (3, 4):
        print("usage: info_plist_names.py <Info.plist> <resources directory> [repository root]", file=sys.stderr)
        return 2
    plist_path, resources = Path(sys.argv[1]), Path(sys.argv[2])
    repository = Path(sys.argv[3]) if len(sys.argv) == 4 else None
    with plist_path.open("rb") as handle:
        plist = plistlib.load(handle)

    failures: list[str] = []
    bundle = plist.get("CFBundleIdentifier", "")
    source = plist.get("TISInputSourceID", "")
    modes = plist.get("ComponentInputModeDict", {}).get("tsInputModeListKey", {}) or {}
    visible = plist.get("ComponentInputModeDict", {}).get("tsVisibleInputModeOrderedArrayKey", []) or []

    if source:
        failures.append(
            "top-level TISInputSourceID creates a second menu entry; the visible ComponentInputModeDict mode must be the only selectable source"
        )
    for mode, body in sorted(modes.items()):
        if body.get("TISInputSourceID") != mode:
            failures.append(f"input mode {mode} carries TISInputSourceID {body.get('TISInputSourceID')}; the menu and the registration would name different sources")
        if bundle and not mode.startswith(bundle + "."):
            failures.append(f"input mode {mode} does not extend {bundle}; that is how the system tells one bundle's modes apart")
        if body.get("tsInputModeIsVisibleKey") and mode not in visible:
            failures.append(f"input mode {mode} is visible but absent from tsVisibleInputModeOrderedArrayKey; it never reaches the picker")
    if len(visible) < 2:
        failures.append("tsVisibleInputModeOrderedArrayKey offers fewer than two modes; the menu bar needs the Chinese and the English mode to show 中 and 英")
    for mode in visible:
        if mode not in modes:
            failures.append(f"tsVisibleInputModeOrderedArrayKey lists {mode}, which tsInputModeListKey does not declare")
    # 其他形式会被 imklaunchagent 拒绝，输入法不再被按需拉起，它的模式在输入菜单里就变灰。
    connection = plist.get("InputMethodConnectionName")
    if bundle and connection != f"{bundle}_Connection":
        failures.append(f"InputMethodConnectionName is {connection!r}; imklaunchagent only launches the input method under {bundle}_Connection")

    # The bundle's own identifier names the input method in System Settings; the mode identifiers name the entries in the input menu.
    identifiers = {value for value in (bundle, source) if value} | set(modes)
    # Everything else a .strings may key on is a plist key it localises, CFBundleDisplayName and the usage strings among them.
    keys = set(plist)

    lprojs = sorted(resources.glob("*.lproj"))
    if not lprojs:
        failures.append(f"no .lproj directories in {resources}; every input source would show its identifier")
    for lproj in lprojs:
        strings = lproj / "InfoPlist.strings"
        if not strings.is_file():
            failures.append(f"{lproj.name} has no InfoPlist.strings")
            continue
        names = localized(strings)
        for identifier in sorted(identifiers):
            value = names.get(identifier)
            if value is None:
                failures.append(f"{identifier} has no name in {lproj.name}; the menu prints the identifier instead")
            elif not value.strip():
                failures.append(f"{identifier} is named with an empty string in {lproj.name}")
        mode_names: dict[str, list[str]] = {}
        for mode in sorted(modes):
            if names.get(mode, "").strip():
                mode_names.setdefault(names[mode].strip(), []).append(mode)
        for name, sharing in sorted(mode_names.items()):
            if len(sharing) > 1:
                failures.append(f"{', '.join(sharing)} are all named {name!r} in {lproj.name}; the input menu would list the same name twice")
        for key in sorted(names):
            if key not in identifiers and key not in keys:
                failures.append(f"{lproj.name} names {key}, which {plist_path.name} does not declare; a renamed identifier leaves its old key behind")

    # The settings application is its own bundle, named after the input method it installs and launches.
    consumable = (identifiers | {f"{bundle}.settings"}) if bundle else identifiers
    consumers = 0
    for root in (repository / name for name in CONSUMER_ROOTS) if repository else ():
        for path in sorted(root.rglob("*")):
            if not path.is_file() or path.suffix not in CONSUMER_SUFFIXES:
                continue
            for number, line in enumerate(path.read_text(encoding="utf-8", errors="ignore").splitlines(), 1):
                for literal in IDENTIFIER.findall(line):
                    consumers += 1
                    if literal not in consumable:
                        failures.append(
                            f"{path.relative_to(repository)}:{number} names {literal}, which {plist_path.name} no longer declares; "
                            f"the bundle now ships as {bundle}"
                        )

    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print(f"{len(identifiers)} input source identifiers are named in {len(lprojs)} languages"
          + (f" and reached by {consumers} literals in the sources." if repository else "."))
    return 0


if __name__ == "__main__":
    sys.exit(main())
