#!/usr/bin/env python3
"""Protocol status flags on macOS must accept JSON booleans only.

Objective-C NSNumber.boolValue treats every nonzero number as true, and also lets
string-like values participate through dynamic dispatch. The host API and the
voice decoder publish actual JSON booleans, so status fields must use the shared
strict helper instead of a coercing read.
"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
INPUT = ROOT / "platforms/macos/src/input/InputController.mm"
DICTIONARY = ROOT / "platforms/macos/src/dictionary/DictionaryWindowController.mm"
DOUBAO = ROOT / "platforms/macos/src/voice/DoubaoVoiceRequest.mm"


def main() -> int:
    sources = {
        INPUT: INPUT.read_text(encoding="utf-8"),
        DICTIONARY: DICTIONARY.read_text(encoding="utf-8"),
        DOUBAO: DOUBAO.read_text(encoding="utf-8"),
    }
    forbidden = {
        INPUT: (
            '[@"handled"] boolValue',
            '[@"applied"] boolValue',
            '[@"moved"] boolValue',
            '[@"word_character_converted"] boolValue',
            '[@"handled"] isEqual:@YES',
            '[@"applied"] isEqual:@YES',
            '[@"moved"] isEqual:@YES',
        ),
        DICTIONARY: (
            'result[@"has_more"] boolValue',
            'result[@"truncated"] boolValue',
        ),
        DOUBAO: ('value[@"last"] boolValue',),
    }
    failures = []
    if "static BOOL MSIMEStrictBoolean(id value)" not in sources[INPUT]:
        failures.append(f"{INPUT}: strict boolean helper is missing")
    for path, needles in forbidden.items():
        for needle in needles:
            if needle in sources[path]:
                failures.append(f"{path}: protocol flag still uses coercing read {needle}")
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("macOS host and voice protocol status flags use strict JSON booleans")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
