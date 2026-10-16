#!/usr/bin/env python3
"""The quick-phrase length limit agrees with the pipe field it is a limit on.

A quick phrase is finally written into the Windows candidate pipe's text field, whose size `shared/contracts/ipc_protocol_limits.h` declares. Everything that accepts a phrase has to refuse one longer than that field, or the phrase is taken, stored, and then truncated on its way to the editor - the kind of failure that appears once, at use, far from where it was entered.

The value lived as the bare literal `199` in four places across three crates, none of them attached to the header that decides it. It is now one constant, and this compares that constant with the header. Resizing the field in the header is what would change the limit; that is exactly when nothing else would notice.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CONTRACT = ROOT / "shared/contracts/ipc_protocol_limits.h"
CONSTANT = ROOT / "crates/client-services/src/dictionary/import.rs"
# Crates that accept a quick phrase. A bare literal here is the thing this guard exists to stop.
SEARCH = ["crates/client-core/src", "crates/client-providers/src", "crates/client-services/src", "crates/host-api/src"]


def contract_limit() -> int | None:
    if not CONTRACT.exists():
        return None
    text = CONTRACT.read_text(encoding="utf-8")
    capacity = re.search(r"CandidateTextCapacity\s*=\s*(\d+)", text)
    if not capacity:
        return None
    explicit = re.search(r"CandidateTextMaxLength\s*=\s*(\d+)", text)
    if explicit:
        return int(explicit.group(1))
    # The header spells it as capacity minus the terminator.
    if re.search(r"CandidateTextMaxLength\s*=\s*CandidateTextCapacity\s*-\s*1", text):
        return int(capacity.group(1)) - 1
    return None


def declared_limit() -> int | None:
    match = re.search(
        r"pub const MAX_QUICK_PHRASE_UTF16: usize = (\d+);",
        CONSTANT.read_text(encoding="utf-8"),
    )
    return int(match.group(1)) if match else None


def stray_literals(limit: int) -> list[str]:
    findings = []
    pattern = re.compile(rf"encode_utf16\(\)\s*\.\s*count\(\)\s*[<>]=?\s*{limit}\b")
    for area in SEARCH:
        for path in sorted((ROOT / area).rglob("*.rs")):
            relative = path.relative_to(ROOT).as_posix()
            for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
                if pattern.search(line):
                    findings.append(f"{relative}:{number}: {line.strip()}")
    return findings


def main() -> int:
    contract = contract_limit()
    if contract is None:
        print(
            f"FAIL {CONTRACT.relative_to(ROOT)} no longer declares CandidateTextMaxLength in a form this reads",
            file=sys.stderr,
        )
        return 1
    declared = declared_limit()
    if declared is None:
        print("FAIL MAX_QUICK_PHRASE_UTF16 is no longer a plain constant", file=sys.stderr)
        return 1
    if declared != contract:
        print(
            f"FAIL MAX_QUICK_PHRASE_UTF16 is {declared}, but the pipe field holds {contract}",
            file=sys.stderr,
        )
        print(
            "\nA phrase within the constant but past the field is accepted here and truncated "
            "on delivery. Follow the contract.",
            file=sys.stderr,
        )
        return 1
    stray = stray_literals(contract)
    for finding in stray:
        print(f"FAIL {finding}", file=sys.stderr)
    if stray:
        print(
            "\nCompare against dictionary::import::MAX_QUICK_PHRASE_UTF16 instead of the number; "
            "a copy of it drifts silently when the contract moves.",
            file=sys.stderr,
        )
        return 1
    print(f"quick phrase limit: {declared} UTF-16 units, matching the pipe field")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
