#!/usr/bin/env python3
"""Keep the HarmonyOS hardware-keyboard Tab and PageUp/PageDown paging keys fixed on.

While something is being composed, `HardwareKeyRouter` turns a disabled navigation binding into `HardwareKeyAction.IGNORED`, which consumes the key: switching Tab or PageUp/PageDown off never gave the key back to the application, it only made the key do nothing. So the session no longer reads `navigation.tab` and `navigation.page_up_down`, and `HostCapabilities.tab_page_paging_switches` is false for HarmonyOS so the settings page stops offering the two check boxes. The node suite cannot see what `KeyboardSession` passes to the router, so this pins the wiring on both sides: a session that reads the stored value again, or a capability flag that offers the switches again, would bring back a switch that only kills the key.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SESSION = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets"
ROUTER = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/HardwareKeyRouter.ts"
SURFACE = ROOT / "crates/client-core/src/host_surface.rs"


def main() -> int:
    session = SESSION.read_text(encoding="utf-8")
    router = ROUTER.read_text(encoding="utf-8")
    surface = SURFACE.read_text(encoding="utf-8")

    # The navigation block the session builds from the stored preferences (not the built-in defaults above it), up to its closing brace.
    start = session.find("minusEqual: preferences.navigation?.minus_equal")
    end = session.find("}", start) if start >= 0 else -1
    body = session[start:end] if end >= 0 else ""
    flag = re.search(r"tab_page_paging_switches: !matches!\((?P<hosts>[^)]*)\)", surface)
    hosts = flag.group("hosts") if flag else ""

    required = {
        "session navigation block": bool(body),
        "Tab fixed on": re.search(r"^\s*tab: true,$", body, re.M) is not None,
        "PageUp/PageDown fixed on": re.search(r"^\s*pageUpDown: true,$", body, re.M) is not None,
        "session ignores the stored Tab": "navigation?.tab" not in session,
        "session ignores the stored PageUp/PageDown": "navigation?.page_up_down" not in session,
        # The reason the switch was cut: a disabled binding is consumed as IGNORED, not handed back.
        "router consumes a disabled binding": ": HardwareKeyAction.IGNORED," in router,
        "capability flag fixes HarmonyOS": "HostPlatform::Harmony" in hosts,
    }
    problems = [name for name, present in required.items() if not present]
    if problems:
        print("harmony fixed paging keys: missing " + ", ".join(problems), file=sys.stderr)
        return 1
    print("harmony fixed paging keys: Tab and PageUp/PageDown stay on and the settings page offers no switch for them")
    return 0


if __name__ == "__main__":
    sys.exit(main())
