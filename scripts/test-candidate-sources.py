#!/usr/bin/env python3
"""Candidate source identifiers agree with the engine enum whose values they copy.

`CandidateActionAvailability.h` decides whether the candidate right-click actions - 置顶, 固定排位, 取消固定, 删除 - are offered, and it decides on the engine's `CandidateSource` value. That value arrives as a number: the Windows Server talks to the engine through the shared host API, and the enum is Rust (`crates/engine/src/types.rs`), so C++ cannot name it.

Renumbering a source in that enum moves the number on the wire. Nothing would fail to compile, and 删除 would quietly start being offered for cloud suggestions - an action the engine then refuses, leaving a menu item that does nothing. That is the failure this holds each name to its value against.

`shared/input/JapaneseConversion.h` keeps its own copies for the Linux and Windows Japanese Space rule: Space counts only the rows the runtime's highlight can stop on, which leaves out `CloudPending`, and keeps its commit meaning on a lone `Fallback` row. A renumbered source there puts Space one row off from the highlight Enter commits, so those copies are held to the enum too.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ENUM = ROOT / "crates/engine/src/types.rs"
# 来源取值移到了 common/，Server 的候选窗与 TIP 的选词共用一份。
POLICY = ROOT / "platforms/windows/common/CandidateSources.h"

# The C++ constant, and the enumerator whose value it copies.
NAMES = {
    "candidate_source_database": "Database",
    "candidate_source_user_database": "UserDatabase",
    "candidate_source_cloud_suggestion": "CloudSuggestion",
    "candidate_source_ai_suggestion": "AiSuggestion",
    "candidate_source_english_dictionary": "EnglishDictionary",
    "candidate_source_ai_pending": "AiPending",
    "candidate_source_cloud_pending": "CloudPending",
}

# 日文空格规则在 Linux 与 Windows 共用的头文件里另抄了两个来源取值，同样钉在枚举上。
JAPANESE = ROOT / "shared/input/JapaneseConversion.h"
JAPANESE_NAMES = {
    "kCandidateSourceFallback": "Fallback",
    "kCandidateSourceCloudPending": "CloudPending",
}


def enum_values() -> dict[str, int] | None:
    text = ENUM.read_text(encoding="utf-8")
    match = re.search(r"pub enum CandidateSource\s*\{(.*?)\n\}", text, re.S)
    if not match:
        return None
    values: dict[str, int] = {}
    for line in match.group(1).splitlines():
        entry = re.match(r"\s*(\w+)\s*=\s*(\d+)\s*,?\s*$", line)
        if entry:
            values[entry.group(1)] = int(entry.group(2))
    return values or None


def main() -> int:
    values = enum_values()
    if values is None:
        print(
            f"FAIL {ENUM.relative_to(ROOT)} no longer declares CandidateSource with explicit values",
            file=sys.stderr,
        )
        return 1
    policy = POLICY.read_text(encoding="utf-8")
    japanese = JAPANESE.read_text(encoding="utf-8")
    copies = [(rf"constexpr unsigned {constant} = (\d+);", policy, constant, enumerator) for constant, enumerator in NAMES.items()]
    copies += [(rf"inline constexpr int {constant} = (\d+);", japanese, constant, enumerator) for constant, enumerator in JAPANESE_NAMES.items()]

    findings = []
    checked = []
    for pattern, text, constant, enumerator in copies:
        match = re.search(pattern, text)
        if not match:
            findings.append(f"{constant} is no longer a plain constant")
            continue
        declared = int(match.group(1))
        expected = values.get(enumerator)
        if expected is None:
            findings.append(f"the engine enum no longer has {enumerator}")
            continue
        if declared != expected:
            findings.append(
                f"{constant} is {declared}, but CandidateSource::{enumerator} is {expected}"
            )
            continue
        checked.append(f"{constant}={expected}")

    for finding in findings:
        print(f"FAIL {finding}", file=sys.stderr)
    if findings:
        print(
            "\nThe engine enum moved. Update the constants together, and check what the "
            "right-click actions are now offered for.",
            file=sys.stderr,
        )
        return 1
    print("candidate sources: " + ", ".join(checked))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
