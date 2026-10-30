#!/usr/bin/env python3
"""The HarmonyOS keyboard and the settings page agree on which settings pages the keyboard may open.

The keyboard asks for a page by putting its id in the Want that starts the settings ability. Two lists decide whether the request gets through: `SettingsPageLink.PAGES`, which the keyboard and `EntryAbility` share, and `LINKED_PAGES` in apps/harmony/src/settings-link.ts, which the page checks before it navigates. A page that is in the first and not the second is silently dropped by the page: the keyboard's 「更多皮肤」 tile closed its panel and then nothing happened, because `skin` had been added to one list only.

The 屏幕键盘 page also takes a sub-target, the shared preference name of a nine-key sidebar table, which opens that table's editor in 按键布局. `SettingsPageLink.SCREEN_KEYBOARD_TARGETS` and `LINKED_SCREEN_KEYBOARD_TARGETS` have to agree in the same way, and both have to be the preference names `nineKeySidebarPreference` in packages/ui maps back to a table; a target the page does not know only opens 屏幕键盘.

The lists are read here as literal string arrays and compared as sets, and every id has to be a page the shared settings registry knows.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
KEYBOARD = ROOT / "platforms" / "harmony" / "entry" / "src" / "main" / "ets" / "keyboard" / "SettingsPageLink.ts"
PAGE = ROOT / "apps" / "harmony" / "src" / "settings-link.ts"
STYLES = ROOT / "packages" / "ui" / "src" / "keyboard" / "touch-key-layout-styles.ts"
REGISTRY = ROOT / "packages" / "ui" / "src" / "settings" / "settings-page-registry.ts"


def string_array(path: pathlib.Path, declaration: str) -> list[str] | None:
    match = re.search(declaration + r"[^=]*=\s*\[([^\]]*)\]", path.read_text(encoding="utf-8"))
    if match is None:
        return None
    return re.findall(r"""["']([^"']+)["']""", match.group(1))


def main() -> int:
    if not KEYBOARD.is_file() or not PAGE.is_file():
        print("harmony settings links: no HarmonyOS sources, skipped")
        return 0
    keyboard = string_array(KEYBOARD, r"static readonly PAGES")
    page = string_array(PAGE, r"const LINKED_PAGES")
    if keyboard is None or page is None:
        print("harmony settings links: could not find SettingsPageLink.PAGES or LINKED_PAGES")
        return 1
    findings = []
    if set(keyboard) != set(page):
        findings.append(f"SettingsPageLink.PAGES {sorted(keyboard)} and LINKED_PAGES {sorted(page)} differ; a page in only one of them is dropped")
    keyboard_source = KEYBOARD.read_text(encoding="utf-8")
    constants = dict(re.findall(r'static readonly (\w+): string = "([^"]+)";', keyboard_source))
    match = re.search(r"static readonly SCREEN_KEYBOARD_TARGETS[^=]*=\s*\[([^\]]*)\]", keyboard_source)
    page_targets = string_array(PAGE, r"const LINKED_SCREEN_KEYBOARD_TARGETS")
    preference = re.search(r"export const nineKeySidebarPreference[\s\S]*?=\s*\{([^}]*)\}", STYLES.read_text(encoding="utf-8"))
    if match is None or page_targets is None or preference is None:
        print("harmony settings links: could not find SCREEN_KEYBOARD_TARGETS, LINKED_SCREEN_KEYBOARD_TARGETS or nineKeySidebarPreference")
        return 1
    names = re.findall(r"SettingsPageLink\.(\w+)", match.group(1))
    keyboard_targets = [constants.get(name, name) for name in names] + re.findall(r'"([^"]+)"', match.group(1))
    sidebar_preferences = re.findall(r'"([^"]+)"', preference.group(1))
    if set(keyboard_targets) != set(page_targets):
        findings.append(f"SettingsPageLink.SCREEN_KEYBOARD_TARGETS {sorted(keyboard_targets)} and LINKED_SCREEN_KEYBOARD_TARGETS {sorted(page_targets)} differ; a target in only one of them is dropped")
    if set(page_targets) != set(sidebar_preferences):
        findings.append(f"LINKED_SCREEN_KEYBOARD_TARGETS {sorted(page_targets)} and nineKeySidebarPreference {sorted(sidebar_preferences)} differ; the settings page cannot open a target it does not map to a table")
    known = set(re.findall(r'id: "([^"]+)"', REGISTRY.read_text(encoding="utf-8")))
    unknown = sorted(set(keyboard) - known)
    if unknown:
        findings.append(f"not settings pages: {unknown}")
    if findings:
        print("\n".join(findings))
        return 1
    print(f"harmony settings links: the keyboard and the page both link {sorted(keyboard)}, and 屏幕键盘 targets {sorted(page_targets)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
