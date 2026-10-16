#!/usr/bin/env python3
"""Guard that the HarmonyOS keyboard draws every surface in the colour mode and leaves the number and border colours to the theme.

HostCapabilities::surface_themes_follow is true on HarmonyOS, so the shared settings page no longer offers the per-surface light/dark rows, and candidate_number_color / candidate_border_color are false, so it offers no number or border picker. A value stored before (or written by another host) must not keep taking effect in the keyboard where the user can no longer see or change it. KeyboardSession.ets has no unit test of its own (it needs ArkUI and NAPI), so this reads the source: the session must not read the seven per-surface keys from the preferences, and the resolve request must strip the two colour slots through GlobalTheme.hostDrawnCustomTheme. The helper itself is exercised in platforms/harmony/tests/keyboard-logic.test.ts.
"""

import re
from pathlib import Path

SURFACE_KEYS = (
    "settings_theme",
    "screen_keyboard_theme",
    "toolbar_theme",
    "menu_theme",
    "emoji_theme",
    "handwriting_theme",
    "voice_theme",
)


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    session_path = root / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets"
    theme_path = root / "platforms/harmony/entry/src/main/ets/keyboard/skin/GlobalTheme.ts"
    if not session_path.is_file() or not theme_path.is_file():
        print("harmony surface themes: HarmonyOS keyboard sources not present, skipped")
        return 0
    session = session_path.read_text(encoding="utf-8")
    theme = theme_path.read_text(encoding="utf-8")
    problems = []
    for key in SURFACE_KEYS:
        # 字段声明（`toolbar_theme?: string;`）保留，偏好文档照旧解析；不许的是从偏好里取值。
        if re.search(rf"preferences\??\.{key}\b", session):
            problems.append(f"KeyboardSession.ets still reads preferences.{key}")
    request_start = session.find("const request: ResolveThemeRequest = {")
    request = session[request_start : session.find("};", request_start)] if request_start >= 0 else ""
    if "custom_theme: GlobalTheme.hostDrawnCustomTheme(preferences.custom_theme)" not in request:
        problems.append("the resolve request does not strip the number and border colours through GlobalTheme.hostDrawnCustomTheme")
    helper_start = theme.find("static hostDrawnCustomTheme(")
    helper = theme[helper_start : theme.find("\n  }\n", helper_start)] if helper_start >= 0 else ""
    if not helper:
        problems.append("GlobalTheme.hostDrawnCustomTheme is missing")
    elif re.search(r"\b(number|border)\s*:", helper):
        problems.append("GlobalTheme.hostDrawnCustomTheme still copies the number or border slot")
    if problems:
        for problem in problems:
            print(f"harmony surface themes: {problem}")
        return 1
    print("harmony surface themes: every surface follows the colour mode, number and border follow the theme")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
