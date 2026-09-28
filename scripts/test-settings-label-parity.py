#!/usr/bin/env python3
"""One setting reads the same wherever the user can read it.

Every other reference-facing check here compares identifiers: config keys, client actions,
setting-to-field coverage, the source inventory. An identifier being right says nothing about the
words printed next to it, and that is where two defects had been sitting. Both Linux menus wrote
the 首右 helpcode schemes as 搜狗 - a different company's input method, named in a menu for
switching helpcode schemes - while the identifier `shouyou2_0` was correct in every copy. The macOS
backend page called the paging choices 「减号 / 等号」 and 「方括号」 where the settings window
showed the keys themselves, so one setting read two ways inside one application.

Two kinds of comparison, because the two settings have different owners:

- The helpcode schemes are named by the reference, and the labels are checked against it rather
  than against each other - every copy here agreeing on the wrong name is exactly the state this
  exists to catch. The identifiers pair the two sides up.
- The paging preset is this platform's own (the reference offers seven independent switches, not a
  three-way preset), so nothing upstream can settle its wording. What is checked is that the two
  macOS surfaces showing it agree.

The reference checkout is optional. Without it the check reports what it would have needed and
passes, the same as every other stage that depends on something not every machine has - but the
copies here are still compared with each other, which is the part that does not need a network.
"""

from __future__ import annotations

import pathlib
import re
import sys

from reference_source import reference_root, show_file

ROOT = pathlib.Path(__file__).resolve().parent.parent
PARTIAL = "ui-html/webview2/settings/ime-settings/src/partials/helpcode.html"


REFERENCE = reference_root(ROOT)


def reference_labels() -> tuple[dict[str, str], str, str] | None:
    """The reference's own dropdown, at the fixed migration source.

    None means only that there is no checkout. A checkout whose partial parses to nothing, or to two spellings of one scheme, returns an empty table, which the caller fails on: treating it as "no checkout" is how an unreadable reference once passed as skipped.
    """
    shown = show_file(ROOT, PARTIAL)
    if shown is None:
        return None
    text, ref, sha = shown
    found = re.findall(
        r'<div class="dropdown-item" data-value="([^"]+)">([^<]+)</div>', text
    )
    # The partial repeats the same list once per scheme it applies to; both copies are the same
    # dropdown, so a disagreement between them is the reference's own problem to report.
    labels: dict[str, str] = {}
    for value, label in found:
        if value in labels and labels[value] != label:
            print(
                f"FAIL the reference itself spells {value} both {labels[value]} and {label}",
                file=sys.stderr,
            )
            return {}, ref, sha
        labels[value] = label
    if not labels:
        print(
            f"FAIL no helpcode scheme labels could be read from the reference's {PARTIAL}; the "
            f"parser needs updating",
            file=sys.stderr,
        )
    return labels, ref, sha


def shared_ui() -> dict[str, str]:
    source = (ROOT / "packages/ui/src/settings/pages/helpcode-page.tsx").read_text(
        encoding="utf-8"
    )
    block = re.search(
        r"const helpcodeSchemas:.*?= \[(.*?)\];", source, re.S
    )
    assert block, "the shared helper-code settings page no longer declares helpcodeSchemas"
    return dict(re.findall(r'\["([^"]+)",\s*"([^"]+)"\]', block.group(1)))


def linux_menus() -> dict[str, str]:
    source = (ROOT / "platforms/linux/src/core/HelpcodeSchemaNames.h").read_text(encoding="utf-8")
    block = re.search(r"kHelpcodeSchemaNames\{\{(.*?)\}\};", source, re.S)
    assert block, "the Linux table no longer declares kHelpcodeSchemaNames"
    return dict(re.findall(r'\{"([^"]+)",\s*"([^"]+)"\}', block.group(1)))


def ordered(identifiers: list[str], labels: list[str]) -> dict[str, str]:
    """Pair an identifier order with a label order, for the surfaces that keep the two apart."""
    assert len(identifiers) == len(labels), (identifiers, labels)
    return dict(zip(identifiers, labels))


def macos_settings() -> dict[str, str]:
    source = (ROOT / "platforms/macos/src/settings/AppearancePreferences.mm").read_text(
        encoding="utf-8"
    )
    identifiers = re.search(
        r"static NSArray<NSString \*> \*HelpcodeSchemas\(\) \{ return @\[(.*?)\]; \}", source
    )
    assert identifiers, "the macOS settings window no longer declares HelpcodeSchemas()"
    labels = re.search(r"\[schemas addItemsWithTitles:@\[(.*?)\]\];", source)
    assert labels, "the macOS settings window no longer titles the helpcode menu"
    return ordered(
        re.findall(r'@"([^"]+)"', identifiers.group(1)),
        re.findall(r'@"([^"]+)"', labels.group(1)),
    )


def macos_backend() -> dict[str, str]:
    source = (ROOT / "platforms/macos/src/backend/settings/BackendSettingsView.swift").read_text(
        encoding="utf-8"
    )
    labels = re.search(r'helpcode_schema":\s*names = \[(.*)$', source, re.M)
    assert labels, "the macOS backend page no longer names the helpcode schemes"
    # The backend page indexes its names by the stored integer, and that order is the C++ one.
    identifiers = (ROOT / "platforms/macos/src/settings/HelpcodeSchemaPreference.h").read_text(
        encoding="utf-8"
    )
    cases = re.findall(r'case (\d+): return "([^"]+)"', identifiers)
    default = re.search(r'default: return "([^"]+)"', identifiers)
    assert cases and default, "HelpcodeSchemaPreference.h no longer maps indices to identifiers"
    by_index = {int(index): value for index, value in cases}
    by_index[0] = default.group(1)
    return ordered(
        [by_index[index] for index in sorted(by_index)],
        re.findall(r'"([^"]+)"', labels.group(1)),
    )


SURFACES = {
    "the shared settings page": shared_ui,
    "the macOS settings window": macos_settings,
    "the macOS backend page": macos_backend,
    "the Linux menus": linux_menus,
}


def paging_preset() -> int:
    """The macOS paging preset, named twice on one platform.

    `platform.macos.candidate_page_shortcut` is this platform's own three-way preset - it is what
    the shared seven-switch navigation setting falls back to for a profile that predates it - and
    the settings window and the backend page each spell its choices out. Nothing upstream can
    settle the wording, so what has to hold is that the two agree; they did not, and a reader
    comparing the two screens had to work out that 「方括号」 and 「[ / ]」 were one choice.
    """
    window_source = (ROOT / "platforms/macos/src/settings/AppearancePreferences.mm").read_text(
        encoding="utf-8"
    )
    window = re.search(r"\[_pageShortcutButton addItemsWithTitles:@\[(.*?)\]\];", window_source)
    assert window, "the macOS settings window no longer titles the paging preset"
    backend_source = (
        ROOT / "platforms/macos/src/backend/settings/BackendSettingsView.swift"
    ).read_text(encoding="utf-8")
    # The choices contain a bracket of their own (`[ / ]`), so the array cannot be matched by
    # stopping at the first `]`. Take the rest of the line and read the quoted strings out of it.
    backend = re.search(r'candidate_page_shortcut":\s*names = \[(.*)$', backend_source, re.M)
    assert backend, "the macOS backend page no longer names the paging preset"
    in_window = re.findall(r'@"([^"]+)"', window.group(1))
    in_backend = re.findall(r'"([^"]+)"', backend.group(1))
    if in_window == in_backend:
        return 0
    print(
        f"FAIL the macOS settings window names the paging choices {in_window} where the backend "
        f"page names them {in_backend}; they are one setting",
        file=sys.stderr,
    )
    return 1


def main() -> int:
    copies = {name: read() for name, read in SURFACES.items()}
    failures = 0

    # Every surface offers the same schemes. A surface missing one offers the user fewer schemes
    # than the product has; a surface with an extra one offers a scheme nothing else knows.
    identifiers = {name: set(labels) for name, labels in copies.items()}
    first = next(iter(identifiers))
    for name, found in identifiers.items():
        if found != identifiers[first]:
            missing = sorted(identifiers[first] - found)
            extra = sorted(found - identifiers[first])
            print(
                f"FAIL {name} offers a different set of schemes than {first}: "
                f"missing {missing}, extra {extra}",
                file=sys.stderr,
            )
            failures += 1

    resolved = reference_labels()
    if resolved is None:
        print("skipped the reference comparison: no MSIME-Windows checkout beside this repository")
        print(f"  expected a git checkout at {REFERENCE} carrying {PARTIAL}")
        truth, source = copies[first], first
    elif not resolved[0]:
        failures += 1
        truth, source = copies[first], first
    else:
        truth, ref, sha = resolved
        source = f"the reference ({ref} {sha[:8]})"
        for name, found in identifiers.items():
            unknown = sorted(found - set(truth))
            if unknown:
                print(
                    f"FAIL {name} offers schemes the reference does not have: {unknown}",
                    file=sys.stderr,
                )
                failures += 1

    for name, labels in copies.items():
        for value, label in sorted(labels.items()):
            expected = truth.get(value)
            if expected is not None and label != expected:
                print(
                    f"FAIL {name} calls {value} 「{label}」 where {source} calls it 「{expected}」",
                    file=sys.stderr,
                )
                failures += 1

    failures += paging_preset()

    if failures:
        return 1
    print(
        f"helpcode schema labels: {len(copies)} surfaces name the same "
        f"{len(copies[first])} schemes, matching {source}"
    )
    print("candidate paging preset: the two macOS surfaces name its choices the same way")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
