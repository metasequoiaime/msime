#!/usr/bin/env python3
"""HarmonyOS native code splits strings with `TextPolicy.characters`, not `Array.from` or `[...text]`.

On the HarmonyOS ArkTS runtime both `Array.from(text)` and `[...text]` split a string by UTF-16 code
unit: "a😊b" comes out as four pieces, the emoji as two lone surrogates. Only `for...of` gives three.
Measured on the HarmonyOS 6.0 emulator; Node gives three for all of them, so the unit tests under
platforms/harmony/tests cannot see the difference.

It is not a cosmetic difference. A check that splits a text and validates each piece reads every
lone surrogate as malformed: the AI chat refused every reply that contained an emoji and showed
「聊天服务暂不可用」, and feedback, prompts and reply templates with an emoji were refused the same
way. Counting characters that way counts an emoji twice.

`TextPolicy.characters` uses `for...of`. This check keeps the two code-unit forms out of the
ArkTS sources: any `Array.from(` call, and a spread that is the whole array literal (`[...text]`).
An array that genuinely needs copying can use `.slice()`. Comment lines are not code and are skipped.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCES = ROOT / "platforms" / "harmony" / "entry" / "src" / "main" / "ets"
FORBIDDEN = (
    (re.compile(r"\bArray\.from\("), "Array.from(...)"),
    (re.compile(r"\[\.\.\.[A-Za-z_$][\w$.]*(?:\(\))?\]"), "[...value]"),
)


def main() -> int:
    if not SOURCES.is_dir():
        print("harmony string code points: no HarmonyOS sources, skipped")
        return 0
    findings = []
    files = sorted(p for p in SOURCES.rglob("*") if p.suffix in (".ts", ".ets"))
    for path in files:
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            stripped = line.strip()
            if stripped.startswith(("//", "*", "/*")):
                continue
            for pattern, form in FORBIDDEN:
                if pattern.search(line):
                    findings.append(f"{path.relative_to(ROOT)}:{number}: {form} splits by UTF-16 code unit on ArkTS; use TextPolicy.characters")
    if findings:
        print("\n".join(findings))
        print(f"harmony string code points: {len(findings)} finding(s)")
        return 1
    print(f"harmony string code points: {len(files)} host sources split strings by code point")
    return 0


if __name__ == "__main__":
    sys.exit(main())
