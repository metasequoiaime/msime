#!/usr/bin/env python3
"""No host decides whether a clipboard entry is storable; the shared store does.

`crates/client-core/src/clipboard.rs` owns the rule: trimmed non-blank, at most forty thousand UTF-8
bytes, at most ten thousand **graphemes**, and no control characters other than newline, carriage
return and tab. Every mobile host writes into that store through
`msime_client_mobile_clipboard_history`, which already answers with `captured` and a `reason`.

Android kept a second copy of the rule and the two had drifted three ways at once. It counted
`text.length()` - UTF-16 code units - where the shared store counts graphemes, so six thousand emoji
were refused on the keyboard and accepted by the store the keyboard writes to. It had no
control-character check, so text the store refuses passed the local one and failed remotely. And the
remote refusal was collapsed to a boolean, so both of those reached the user as 「50 条历史均已固定」.

None of that was visible to the smoke beside it, which asserted that the *numbers* matched the
shared crate - and they did. Matching constants is not matching semantics, which is why this check
looks for the arithmetic rather than for the values.

A host may still say "the clipboard held no text": that is about the system clipboard, not about the
entry, and the store cannot answer it.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OWNER = ROOT / "crates/client-core/src/clipboard.rs"

# The two bounds as code spells them. Deliberately not the comma form: 「单条最多保存 10,000 字」 is a
# sentence telling the user the limit, which every host is entitled to say, not a second rule.
BOUND = re.compile(r"\b(?:10_?000|40_?000)\b")
# Measuring a candidate entry. `.length` alone is far too common to flag on its own.
MEASURE = re.compile(r"\.length\b|\.count\b|getBytes\(|utf8\.count|lengthOfBytes|byteLength")

# Second copies that exist today, each with what is actually wrong with it. This is a ratchet, not
# an exemption: a host is listed here only with a diagnosis, and a new copy anywhere else fails.
PENDING = {}

HOSTS = [
    "platforms/android/java/app/msime/client/clipboard",
    "platforms/ios/SharedUI/clipboard",
    "platforms/harmony/entry/src/main/ets/keyboard/clipboard",
]
SUFFIXES = {".java", ".swift", ".ts", ".ets", ".kt"}

findings = []
pending_seen = set()
searched = 0
for host in HOSTS:
    directory = ROOT / host
    if not directory.exists():
        continue
    for source in sorted(directory.rglob("*")):
        if source.suffix not in SUFFIXES:
            continue
        searched += 1
        relative = str(source.relative_to(ROOT))
        lines = source.read_text().split("\n")
        # File-level, not line-level. The realistic shape is a named constant on one line and the
        # comparison on another - which is exactly what Android had - so a same-line rule would
        # have missed the very code this check exists for.
        bound_at = [i for i, line in enumerate(lines, start=1) if BOUND.search(line)]
        measure_at = [i for i, line in enumerate(lines, start=1) if MEASURE.search(line)]
        if not (bound_at and measure_at):
            continue
        if relative in PENDING:
            pending_seen.add(relative)
            continue
        findings.append((source.relative_to(ROOT), bound_at[0], lines[bound_at[0] - 1].strip()))

if not OWNER.exists():
    sys.exit(f"{OWNER.relative_to(ROOT)} is gone; this check is pointed at nothing")
if searched == 0:
    sys.exit("found no host clipboard sources to check")

if findings:
    for path, number, text in findings:
        print(f"{path}:{number}: host measures a clipboard entry against a shared bound: {text}")
    sys.exit(
        "clipboard size and character rules belong to crates/client-core/src/clipboard.rs; "
        "capture through the shared entry and read its `reason`"
    )

# A PENDING entry that no longer matches is debt that was paid; drop it rather than let the list
# describe a tree it has stopped describing.
stale = sorted(set(PENDING) - pending_seen)
if stale:
    for relative in stale:
        print(f"{relative}: listed as a pending second copy, but none was found")
    sys.exit("remove the stale PENDING entries from scripts/test-clipboard-capture-bounds.py")

for relative in sorted(pending_seen):
    print(f"pending: {relative} - {PENDING[relative]}")
print(
    f"clipboard capture bounds: {searched} host sources, "
    f"{len(pending_seen)} known second copies, no new ones"
)
