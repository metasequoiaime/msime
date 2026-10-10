#!/usr/bin/env python3
"""Guard the Harmony keyboard's own switch for local clipboard history.

From API 12 the keyboard extension reads the preference document in its own sandbox, so the settings page's switch never reaches it (platforms/harmony/README.md, 输入法扩展的独立沙箱). The panel therefore must not blame the settings page, and must offer the switch itself; turning it on has to refill the list rather than leave the panel empty until the next open.
"""

from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    view = (root / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets").read_text()
    enable_start = view.index("  private enableClipboardHistory(): void")
    enable = view[enable_start : view.index("\n  }\n", enable_start)]
    changed_start = view.index("KeyboardSession.shared.onClipboardHistoryChanged = (enabled: boolean)")
    changed = view[changed_start : view.index("\n    };\n", changed_start)]
    required = {
        "no notice blames the settings page": "剪贴板历史已在设置中关闭" not in view,
        "the panel offers the switch": ".onClick(() => this.enableClipboardHistory())" in view,
        "the switch writes the keyboard's own preference":
            "changes.set('clipboard_history', true)" in enable
            and "KeyboardSession.shared.changePreferences(changes)" in enable,
        "turning it on refills the list": "this.clips = KeyboardSession.shared.clipboardHistory()" in changed
            and "this.clipboardNotice = ''" in changed,
    }
    problems = [name for name, present in required.items() if not present]
    if problems:
        for problem in problems:
            print(f"missing clipboard history switch guard: {problem}")
        return 1
    print("harmony clipboard history: the keyboard panel turns its own history on")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
