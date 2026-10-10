#!/usr/bin/env python3
"""Every page of the shared settings surface has to be reachable by a route, or say why not.

The page list lives in TypeScript and the route vocabulary lives in Rust, so nothing connects them. When a
page was added without a category, no host could open it: the account page had none, and macOS opened its
own bundled window instead of the page every other platform shows - which is the opposite of keeping the
shared UI in one place.

A page here is one entry of the settings navigation. A category is one `settings:<id>` a launcher can emit.
The two lists have to agree, except for the entries named below with the reason they differ.

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
    "try-keyboard": "a touch-only subpage opened from the status card on the 设置 root page; desktop hosts hide it",
    "usage-reporting": "a subpage opened from the row under the usage reporting switch on 关于; nothing outside the settings window points at it",
}


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: settings_route_coverage.py <repository root>", file=sys.stderr)
        return 2
    root = Path(sys.argv[1])

    registry = "packages/ui/src/settings/settings-page-registry.ts"
    ui = (root / registry).read_text(encoding="utf-8")
    # The navigation entries, which are what the user actually sees down the side of the window.
    pages = re.findall(r'\{\s*id:\s*"([a-z-]+)",\s*title:\s*"[^"]+",\s*icon:', ui)
    if not pages:
        print(f"could not read the settings navigation from {registry}", file=sys.stderr)
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
    for category in sorted(categories):
        if category not in pages:
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
    print(f"{len(pages)} settings pages, {len(categories)} routable; the rest are accounted for.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
