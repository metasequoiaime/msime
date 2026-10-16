#!/usr/bin/env python3
"""Every settings button that calls an optional host callback must be gated on it.

The shared settings UI is one page rendered on six hosts, and the things a host
can actually do differ: HarmonyOS has no folder to open for external skins, a
keyboard extension cannot restart the input method, an Apple host installs its
own input source. Those capabilities arrive as optional callbacks on `client`,
so a button whose handler calls one without checking it is present renders live
on a host that cannot do it and does nothing when pressed. A dead button is
worse than an absent one: it says the feature is there.

Every such button in the file is currently guarded, by `disabled={!client.x}`,
by optional chaining, or by a surrounding render condition. That holds only
because each was written carefully, and nothing has been checking it. The rule
is cheap to state and cheap to enforce, so it is enforced here rather than
rediscovered by hand -- which is how this check came to be written, after a
sweep for dead buttons found none.

Static because the alternative is rendering the page once per host with every
capability withheld in turn, which the settings suite does not do and which
would not catch a button added for a seventh host.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "packages/ui/src/index.tsx"
CLIENT_SOURCE = ROOT / "packages/ui/src/core/settings-client.ts"


def main() -> int:
    text = SOURCE.read_text(encoding="utf-8")
    # 宿主接口迁出入口后仍纳入能力集合；入口本身的可选回调继续检查。
    contracts = CLIENT_SOURCE.read_text(encoding="utf-8")
    optional = set(re.findall(r"^\s{2}(\w+)\?:\s*\(", text + "\n" + contracts, re.M))
    if not optional:
        print("no optional client callbacks found; the interface shape changed", file=sys.stderr)
        return 1
    offenders = []
    # Avoid a backtracking-heavy expression: JSX button bodies may contain nested
    # expressions and long strings. Scan each button linearly up to its closing tag.
    button = re.compile(r"<button\b(?P<body>.*?)</button>", re.DOTALL)
    for match in button.finditer(text):
        block = match.group("body")
        for name in sorted(optional):
            # Both shapes count: calling the callback, and handing it to a helper such as
            # `openPanel(client.openScreenKeyboard)`, which is how the panel buttons are written.
            # An earlier version of this check only matched the call and so passed a button whose
            # guard had been deleted -- the check has to see what the buttons actually do.
            if not re.search(r"client\." + name + r"\b", block):
                continue
            if f"!client.{name}" in block or f"client.{name}?." in block:
                continue
            before = text[max(0, match.start() - 900):match.start()]
            if f"client.{name} &&" in before or f"client.{name} ?" in before:
                continue
            line = text[: match.start()].count("\n") + 1
            offenders.append((line, name))
    if offenders:
        for line, name in offenders:
            print(
                f"{SOURCE.relative_to(ROOT)}:{line}: button calls optional client.{name} "
                f"without disabled={{!client.{name}}}, optional chaining or a render guard",
                file=sys.stderr,
            )
        return 1
    print(f"settings action guard: {len(optional)} optional callbacks, every button gated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
