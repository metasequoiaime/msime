#!/usr/bin/env python3
"""The handwriting panel never offers more than the shared contract accepts.

The shared handwriting helpers and Harmony native keyboard cap strokes, points per stroke and
candidates; `client-core`'s panel contract caps the same three and rejects a
request that exceeds them. The three are written in different languages and
live in different files, so this check keeps both producers within the contract.

Exceeding the contract is silent in the worst way: the user draws a stroke a producer accepts
and recognition simply returns nothing, because the request was refused before
it reached a recogniser. The invariant is one-directional - the panel may be
stricter, never looser - so that is what this checks.

Equality is fine and is the case for candidates today; what must not happen is
the panel exceeding the contract.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CONTRACT = ROOT / "crates/client-core/src/panels.rs"
HANDWRITING_INPUT = ROOT / "packages/ui/src/keyboard/handwriting-input.ts"
HANDWRITING = ROOT / "packages/ui/src/keyboard/handwriting.ts"
HARMONY = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/input/HandwritingStrokePolicy.ts"

# (what it bounds, Rust constant, TypeScript constant)
LIMITS = [
    ("strokes in one request", "MAX_STROKES", "MAX_HANDWRITING_STROKES"),
    ("points in one stroke", "MAX_POINTS_PER_STROKE", "MAX_CAPTURED_POINTS"),
    ("candidates offered", "MAX_CANDIDATES", "MAX_HANDWRITING_CANDIDATES"),
]

HARMONY_LIMITS = [
    ("strokes in one request", "MAX_STROKES", "HANDWRITING_MAX_STROKES"),
    ("points in one stroke", "MAX_POINTS_PER_STROKE", "HANDWRITING_MAX_POINTS"),
    ("candidates offered", "MAX_CANDIDATES", "HANDWRITING_MAX_CANDIDATES"),
]


def rust_value(text: str, name: str) -> int | None:
    match = re.search(rf"const {name}: usize = (\d+);", text)
    return int(match.group(1)) if match else None


def typescript_value(text: str, name: str) -> int | None:
    match = re.search(rf"const {name} = (\d+);", text)
    return int(match.group(1)) if match else None


def arkts_value(text: str, name: str) -> int | None:
    match = re.search(rf"export const {name}(?:: number)? = (\d+);", text)
    return int(match.group(1)) if match else None


def main() -> int:
    if (
        not CONTRACT.exists()
        or not HANDWRITING_INPUT.exists()
        or not HANDWRITING.exists()
        or not HARMONY.exists()
    ):
        print("skipped: the handwriting contract, shared helpers or Harmony policy is not present")
        return 0
    contract = CONTRACT.read_text(encoding="utf-8")
    handwriting_input = HANDWRITING_INPUT.read_text(encoding="utf-8")
    handwriting = HANDWRITING.read_text(encoding="utf-8")
    harmony = HARMONY.read_text(encoding="utf-8")

    findings = []
    checked = []
    for description, rust_name, ts_name in LIMITS:
        allowed = rust_value(contract, rust_name)
        source = handwriting if ts_name == "MAX_HANDWRITING_CANDIDATES" else handwriting_input
        offered = typescript_value(source, ts_name)
        if allowed is None:
            findings.append(f"{rust_name} is no longer a plain constant in panels.rs")
            continue
        if offered is None:
            findings.append(
                f"{ts_name} is no longer a plain constant in the shared handwriting helpers"
            )
            continue
        if offered > allowed:
            findings.append(
                f"{description}: the panel offers {offered} and the contract accepts "
                f"{allowed}, so the excess is drawn and then refused with nothing shown"
            )
            continue
        checked.append(f"{description} {offered}/{allowed}")

    for description, rust_name, arkts_name in HARMONY_LIMITS:
        allowed = rust_value(contract, rust_name)
        offered = arkts_value(harmony, arkts_name)
        if allowed is None:
            findings.append(f"{rust_name} is no longer a plain constant in panels.rs")
            continue
        if offered is None:
            findings.append(
                f"{arkts_name} is no longer a plain exported constant in the Harmony policy"
            )
            continue
        if offered > allowed:
            findings.append(
                f"Harmony {description}: the keyboard offers {offered} and the contract accepts "
                f"{allowed}, so the excess can be refused downstream"
            )
            continue
        checked.append(f"Harmony {description} {offered}/{allowed}")

    for finding in findings:
        print(f"FAIL {finding}", file=sys.stderr)
    if findings:
        return 1
    print("handwriting limits: " + ", ".join(checked))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
