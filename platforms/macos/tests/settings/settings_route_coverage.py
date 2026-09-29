#!/usr/bin/env python3
"""Every page of the shared settings surface has to be reachable by a route, or say why not.

The page list lives in TypeScript and the route vocabulary lives in Rust, so nothing connects them. When a
page was added without a category, no host could open it: the account page had none, and macOS opened its
own bundled window instead of the page every other platform shows - which is the opposite of keeping the
shared UI in one place.

A page here is one entry of the settings navigation. A category is one `settings:<id>` a launcher can emit.
The two lists have to agree, except for the entries named below with the reason they differ, and for
former page ids the shared UI resolves through `settingsPageAliases` to a page that still exists: hosts
keep sending those, and the UI opens the page that now holds their contents.

Usage: settings_route_coverage.py <repository root>
"""

import re
import sys
from pathlib import Path

# Pages a host has no reason to route to. Each says why, because "no category" is what the defect looked
# like from the outside.
NOT_ROUTED = {
    "home": "where the settings window already opens; a route to it would name the default",
    "more": "a mobile-only index reached from the keyboard home page; desktop hosts hide it",
}


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: settings_route_coverage.py <repository root>", file=sys.stderr)
        return 2
    root = Path(sys.argv[1])

    ui = (root / "packages/ui/src/index.tsx").read_text(encoding="utf-8")
    # The navigation entries, which are what the user actually sees down the side of the window.
    pages = re.findall(r'\{\s*id:\s*"([a-z-]+)",\s*title:\s*"[^"]+",\s*icon:', ui)
    if not pages:
        print("could not read the settings navigation from packages/ui/src/index.tsx", file=sys.stderr)
        return 1

    # Former page ids the shared UI still accepts, each mapped to the page that now holds its contents.
    # Without the table there are no aliases, and a category that relied on one fails below by name.
    alias_table = re.search(r"const settingsPageAliases:[^=]*=\s*\{([^}]*)\}", ui)
    alias_body = alias_table.group(1) if alias_table else ""
    aliases = dict(re.findall(r'"?([a-z-]+)"?\s*:\s*"([a-z-]+)"', alias_body))
    if alias_body.strip() and not aliases:
        print("settingsPageAliases parsed to no entries", file=sys.stderr)
        return 1

    surface = (root / "crates/client-core/src/host_surface.rs").read_text(encoding="utf-8")
    # There is more than one as_str in the file, so match the arms themselves rather than a method body.
    categories = set(re.findall(r'SettingsCategory::\w+ => "([a-z-]+)"', surface))
    if not categories:
        print("SettingsCategory::as_str parsed to no identifiers", file=sys.stderr)
        return 1

    failures = []
    for page in pages:
        if page in categories or page in NOT_ROUTED:
            continue
        failures.append(
            f"the settings page {page!r} has no route: add a SettingsCategory for it, or say here why a "
            f"host would never open it"
        )
    routed_aliases = {}
    for alias, target in sorted(aliases.items()):
        if alias in pages:
            failures.append(
                f"settingsPageAliases maps {alias!r}, which is itself a settings page; the alias hides it"
            )
        elif target not in pages:
            failures.append(
                f"settingsPageAliases maps {alias!r} to {target!r}, which is not a settings page; a host "
                f"routing to settings:{alias} would open the default page instead"
            )
        elif alias in categories:
            routed_aliases[alias] = target
    for category in sorted(categories):
        if category not in pages and category not in routed_aliases:
            failures.append(
                f"settings:{category} names a page the shared UI does not have; a host routing to it "
                f"would open the default page instead"
            )
    for page in sorted(NOT_ROUTED):
        if page not in pages:
            failures.append(f"{page!r} is listed here but is no longer a settings page; drop the entry")

    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    summary = f"{len(pages)} settings pages, {len(categories)} routable; the rest are accounted for."
    if routed_aliases:
        summary += " Aliases: " + ", ".join(
            f"settings:{alias} -> {target}" for alias, target in sorted(routed_aliases.items())
        ) + "."
    print(summary)
    return 0


if __name__ == "__main__":
    sys.exit(main())
