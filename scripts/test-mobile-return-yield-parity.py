#!/usr/bin/env python3
"""三个手机平台的回车让给中/英的宽度必须是同一个值（见 .agents/notes/implemented/feature/2026-10-09-mobile-bottom-row-symbols-and-return-yield.md）。"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
# 每个平台：文件、匹配常量值的正则。
CONSTANTS = (
    (
        "Android KeyboardActionRow.RETURN_YIELD_DP",
        ROOT / "platforms/android/java/app/msime/android/keyboard/KeyboardActionRow.java",
        re.compile(r"public static final float RETURN_YIELD_DP = ([0-9.]+)f;"),
    ),
    (
        "HarmonyOS KeyboardGeometry.RETURN_YIELD_VP",
        ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardGeometry.ts",
        re.compile(r"static readonly RETURN_YIELD_VP: number = ([0-9.]+);"),
    ),
    (
        "iOS KeyGapRouting.returnYield",
        ROOT / "platforms/ios/KeyboardExtension/Sources/keyboard/KeyGapRouting.swift",
        re.compile(r"static let returnYield: CGFloat = ([0-9.]+)"),
    ),
)


def main() -> int:
    values = {}
    errors = []
    for name, path, pattern in CONSTANTS:
        matches = pattern.findall(path.read_text(encoding="utf-8"))
        if len(matches) != 1:
            errors.append(f"{path.relative_to(ROOT)}: 找不到唯一的 {name}")
            continue
        values[name] = float(matches[0])
    if not errors and len(set(values.values())) != 1:
        errors.append("回车让给中/英的宽度在各平台不一致: " + ", ".join(f"{name}={value:g}" for name, value in values.items()))
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Mobile return-key yield matches on Android, HarmonyOS and iOS ({next(iter(values.values())):g})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
