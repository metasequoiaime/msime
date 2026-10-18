#!/usr/bin/env python3
"""Every shared preference is either honoured on macOS or listed here with the reason it is not.

The settings page is shared, so a preference added for another platform appears on macOS too. When the host
does not read it, the switch saves, the page says it saved, and nothing changes - the user is told the
opposite of the truth. That is not hypothetical: 本地 Whisper accepted a model file and recognised through
Apple's service instead, and the two smart-punctuation direct switches did nothing while the host behaved
as though both were on.

Nothing catches that by itself, because the host compiles and the tests pass either way. So the coverage is
asserted here: a field reachable from neither the macOS sources nor the shared runtime has to be named in
NOT_APPLICABLE with a reason, and a name that no longer needs to be there is a failure too, so the list
cannot quietly outlive its entries.

Usage: preference_coverage.py <repository root>
"""

import re
import sys
from pathlib import Path

# Preferences that no macOS input method can act on. Each entry says why, because "macOS does not read it"
# is exactly what a defect looks like from the outside.
NOT_APPLICABLE = {
    "settings_theme": "the settings application's own appearance; the input method has no window to theme",
    # macOS 的这些界面一律跟随颜色模式（HostCapabilities 的 surface_themes_follow），设置页不给这几行；固定明暗的主题由 InputController 钉住传给面板的 `theme`。
    "toolbar_theme": "the floating toolbar and the mode badge follow the colour mode on macOS (surface_themes_follow)",
    "menu_theme": "the input menu and the candidate context menu follow the colour mode on macOS (surface_themes_follow)",
    "screen_keyboard_theme": "the screen keyboard follows the colour mode on macOS (surface_themes_follow)",
    "emoji_theme": "the emoji panel follows the colour mode on macOS (surface_themes_follow)",
    "handwriting_theme": "the handwriting panel follows the colour mode on macOS (surface_themes_follow)",
    "voice_theme": "the voice overlay follows the colour mode on macOS (surface_themes_follow)",
    "touch_key_spacing_tenths": "the touch keyboard; the macOS screen keyboard panel draws the default spacing whatever is stored, and the page offers no slider (`touch_keyboard_spacing` is false)",
    "touch_row_spacing_tenths": "the touch keyboard; the macOS screen keyboard panel draws the default spacing whatever is stored, and the page offers no slider (`touch_keyboard_spacing` is false)",
    "touch_keyboard_height_adjustment": "the touch keyboard and the Windows panel window; the macOS screen keyboard window has a fixed height, and the page offers no slider (`touch_keyboard_height` is false)",
    "touch_voice_shortcut": "the touch keyboard",
    "touch_quick_punctuation_symbols": "仅 iOS 原生触屏键盘提供自定义快捷符号；macOS 屏幕键盘不读取此列表，共享设置页不提供此项",
    "touch_number_keypad_order": "the touch keyboard's nine-key digit layer",
    "touch_twenty_six_key_number_layout": "the touch keyboard's 26-key digit layer",
    "touch_shuangpin_key_hints": "the touch keyboard's 26-key shuangpin letter keys",
    "touch_toolbar": "the touch keyboard's row above the keys",
    "harmony_toolbar": "the HarmonyOS phone keyboard's toolbar tools and display mode; the page offers them only where the host reports `touch_toolbar_tools`",
    "number_row_selection": "releasing the number row back to the editor, offered only where the host advertises it - Linux and HarmonyOS; Windows and macOS both keep number selection",
    "english_suggestions": "the mobile suggestion strip; desktop hosts show English candidates through mixed_input instead",
    "game_compatibility": "Windows TSF only: whether the TIP skips ITfUIElementMgr for games that declare UI-less and draw nothing; macOS has no UI-less host contract",
    "smart_punctuation_migrated": "not a setting: the one-time marker client-core's PreferencesStore sets while reading the document, after turning the old macOS default (smart punctuation off) on; the host reads the migrated values through msime_client_load_preferences",
    "second_third_candidate": "';' and '\\'' picking the second and third candidate are routed only by the Windows TIP and Server so far (.agents/notes/implemented/feature/2026-10-09-second-third-candidate-keys.md); the shared page does not show the switch, so no macOS user can turn on something the host ignores",
}


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: preference_coverage.py <repository root>", file=sys.stderr)
        return 2
    root = Path(sys.argv[1])

    source = (root / "crates/client-core/src/preferences.rs").read_text(encoding="utf-8")
    body = re.search(r"pub struct Preferences \{(.*?)\n\}", source, re.S)
    if not body:
        print("could not find the Preferences struct", file=sys.stderr)
        return 1
    fields = re.findall(r"\n    pub (\w+):", body.group(1))
    if not fields:
        print("the Preferences struct parsed to no fields", file=sys.stderr)
        return 1

    def read(*directories: str) -> str:
        text = []
        for directory in directories:
            for path in (root / directory).rglob("*"):
                # A field named only in a test is not a field the host acts on. Counting those made a
                # test about the touch keyboard - written for another platform, in a crate this scan
                # reaches - look like macOS consuming a preference it has no surface for, and the entry
                # explaining why it cannot was then reported as stale.
                if path.name in {"tests.rs", "test.rs"} or "tests" in path.parts:
                    continue
                if path.is_file() and path.suffix in {".h", ".m", ".mm", ".cpp", ".swift", ".rs"}:
                    text.append(path.read_text(encoding="utf-8", errors="ignore"))
        return "\n".join(text)

    # The host may read a preference itself, or leave it to the shared runtime it drives.
    reachable = read("platforms/macos/src", "crates/input-runtime/src", "crates/host-api/src")
    unreachable = [field for field in fields if field not in reachable]

    failures = []
    for field in unreachable:
        if field not in NOT_APPLICABLE:
            failures.append(
                f"{field} is not read on macOS and has no entry here: either honour it in the host, "
                f"or say why the platform cannot"
            )
    for field in sorted(NOT_APPLICABLE):
        if field not in fields:
            failures.append(f"{field} is listed here but is no longer a preference; drop the entry")
        elif field not in unreachable:
            failures.append(f"{field} is listed as out of scope but the host reads it now; drop the entry")

    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print(f"{len(fields) - len(unreachable)} of {len(fields)} shared preferences reach the macOS host; "
          f"the other {len(unreachable)} are accounted for.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
