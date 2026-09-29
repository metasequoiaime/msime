#!/usr/bin/env python3
"""Keep HarmonyOS Traditional output on the shared OpenCC s2t converter.

Every other host converts through `msime_client_simplified_to_traditional`, the phrase-level OpenCC tables compiled into the Rust library, so 头发 becomes 頭髮 and 发展 becomes 發展. ICU's `Simplified-Traditional` transliterator converts one character at a time and cannot tell those apart, so a HarmonyOS keyboard on it commits 頭發 where every other platform commits 頭髮. The node suite cannot see which converter the session passes to `ChineseOutputPolicy`, and the native call cannot run off a device, so this pins the wiring from the C header to the call site.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
HEADER = ROOT / "crates/host-api/include/msime_client.h"
NATIVE = ROOT / "platforms/harmony/native/client_napi.cpp"
TYPES = ROOT / "platforms/harmony/entry/src/main/cpp/types/libmsimeclient/index.d.ts"
SESSION = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets"
VIEW = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets"
SOURCES = ROOT / "platforms/harmony/entry/src/main/ets"


def body(text: str, opening: str, closing: str) -> str:
    """The definition that starts at `opening`, or nothing when it is gone."""
    start = text.find(opening)
    if start < 0:
        return ""
    end = text.find(closing, start)
    return text[start:] if end < 0 else text[start:end]


def main() -> int:
    header = HEADER.read_text(encoding="utf-8")
    native = NATIVE.read_text(encoding="utf-8")
    types = TYPES.read_text(encoding="utf-8")
    session = SESSION.read_text(encoding="utf-8")
    view = VIEW.read_text(encoding="utf-8")

    binding = body(native, "static napi_value SimplifiedToTraditional(", "\n}\n")
    convert = body(session, "  private asTraditional(text: string): string {", "\n  }\n")

    required = {
        "C ABI declaration": "char *msime_client_simplified_to_traditional(const uint8_t *text, size_t length);"
        in header,
        "NAPI call": "msime_client_simplified_to_traditional(" in binding,
        # `response` frees the Rust string after copying it into an ArkTS string.
        "NAPI result released": "return response(env, converted);" in binding,
        "NAPI null for a refused input": "napi_get_null(" in binding,
        "NAPI export": 'ENTRY("simplifiedToTraditional", SimplifiedToTraditional)' in native,
        "ArkTS declaration": "export const simplifiedToTraditional: (text: string) => string | null;" in types,
        "session converts natively": "client.simplifiedToTraditional(value)" in convert,
        "session keeps the output policy": "ChineseOutputPolicy.output(text, this.traditional," in convert
        and "ChineseOutputPolicy.applies(this.englishMode, this.scheme, this.localMode)" in convert,
        # The candidate bar, the flat list it is built from and the expanded panel all show converted text.
        "candidate display converts": session.count("text: this.asTraditional(candidate.text)") == 3,
        # The switch flips only once the store has taken the new value, then republishes at once, so a refused write leaves the output as it was.
        "switch republishes the candidates": re.search(
            r"toggleCharacterSet\(\): boolean \{\s*"
            r"if \(!this\.writePreference\('traditional_chinese_output', !this\.traditional\)\) \{\s*return this\.traditional;\s*\}\s*"
            r"this\.traditional = !this\.traditional;\s*"
            r"(?://[^\n]*\s*)?this\.publishCurrentCandidates\(\);",
            session,
        )
        is not None,
        # The keyboard's 繁 tile goes through the same switch rather than writing the preference behind the session's back.
        "keyboard tile uses the session switch": "this.traditional = KeyboardSession.shared.toggleCharacterSet();" in view,
    }
    problems = [name for name, present in required.items() if not present]

    for path in sorted(SOURCES.rglob("*")):
        if path.suffix not in (".ets", ".ts") or not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        if re.search(r"Transliterator|Simplified-Traditional", text):
            problems.append(f"ICU transliteration in {path.relative_to(ROOT)}")

    if problems:
        print("harmony traditional output: missing " + ", ".join(problems), file=sys.stderr)
        return 1
    print("harmony traditional output: candidates and commits convert through the shared OpenCC s2t tables")
    return 0


if __name__ == "__main__":
    sys.exit(main())
